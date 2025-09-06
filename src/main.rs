use hyperliquid_rust_sdk::{BaseUrl, InfoClient, Subscription};
use log::info;
use tokio::{sync::mpsc::unbounded_channel};
use teloxide::prelude::*;
use std::sync::Arc;
use tokio::sync::Mutex;
use backend::{
    db::Database,
    notification::{NotificationService, Command},
    alerts::AlertService,
};

const ALERT_PRICE_COIN: &str = "@107"; // spot index for hype token

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    dotenv::dotenv().ok();

    log::info!("Starting Alert Price Bot...");

    let db = Database::new("alerts.db").unwrap();
    db.initialize().await.unwrap();
    let tokens = db.get_all_unique_tokens().await.unwrap();
    
    let info_client = Arc::new(Mutex::new(InfoClient::new(None, Some(BaseUrl::Mainnet)).await.unwrap()));

    let alert_service = AlertService::new(db, info_client.clone());
    
    let alerts = alert_service.get_all_alerts().await.unwrap();
    for alert in alerts {
        println!("Alert: {alert:?}");
    }
    
    
    let bot = teloxide::Bot::from_env();
    
    

    let (sender, mut receiver) = unbounded_channel();
    let mut subscription_ids = Vec::new();
    for token in tokens {
        let subscription_id = info_client
            .lock().await
            .subscribe(
                Subscription::ActiveAssetCtx {
                    coin: token,
                },
                sender.clone(),
            )
            .await
            .unwrap();
        subscription_ids.push(subscription_id);
    }


    let alert_service_for_price_updates = alert_service.clone();
    let alert_service_for_cooldowns = alert_service.clone();
    let notification_service = NotificationService::new(alert_service);
    tokio::select! {
        _ = Command::repl(bot.clone(), move |bot, msg, cmd| {
            let notification_service = notification_service.clone();
            async move {
                notification_service.handle_command(bot, msg, cmd).await
            }
        }) => {
            info!("Telegram bot stopped, unsubscribing from price updates");
            for subscription_id in subscription_ids {
                info_client.lock().await.unsubscribe(subscription_id).await.unwrap();
            }
        }
        _ = async move {
            while let Some(hyperliquid_rust_sdk::Message::ActiveSpotAssetCtx(order_updates)) = receiver.recv().await {
                info!("Received order update data: {order_updates:?}");
                let mark_px = order_updates.data.ctx.shared.mark_px.parse::<f64>().unwrap();
                let alerts = alert_service_for_price_updates.get_triggered_alerts(mark_px).await.unwrap();
                for alert in &alerts {
                    println!("Alert triggered: {alert:?}");
                    
                    bot.send_message(teloxide::types::ChatId(alert.chat_id), format!("🔔 Price Alert: {} is at {}", alert.coin, alert.price)).await.unwrap();
                }
                alert_service_for_price_updates.set_alert_cooldowns(&alerts).await.unwrap();
            }
        } => {
            info!("Price monitoring stopped, unsubscribing from price updates");
            for subscription_id in subscription_ids {
                info_client.lock().await.unsubscribe(subscription_id).await.unwrap();
            }
        }
        _ = async move {
            loop {
                alert_service_for_cooldowns.reset_cooldowns().await.unwrap();
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        } => {
        }
    }
}


