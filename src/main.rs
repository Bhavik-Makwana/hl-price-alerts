use hyperliquid_rust_sdk::{BaseUrl, InfoClient, Subscription};
use log::info;
use tokio::{sync::mpsc::unbounded_channel};
use teloxide::prelude::*;

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
    
    let alert_service = AlertService::new(db);

    let alerts = alert_service.get_all_alerts().await.unwrap();
    for alert in alerts {
        println!("Alert: {alert:?}");
    }


    let bot = teloxide::Bot::from_env();

    let mut info_client = InfoClient::new(None, Some(BaseUrl::Mainnet)).await.unwrap();

    let (sender, mut receiver) = unbounded_channel();
    let subscription_id = info_client
        .subscribe(
            Subscription::ActiveAssetCtx {
                coin: ALERT_PRICE_COIN.to_string(), //spot index for hype token
            },
            sender,
        )
        .await
        .unwrap();


    let alert_service_clone = alert_service.clone();
    let alert_service_clone_2 = alert_service.clone();
    tokio::select! {
        _ = Command::repl(bot.clone(), move |bot, msg, cmd| {
            let alert_service = alert_service.clone();
            let notification_service = NotificationService::new(bot, alert_service);
            async move {
                notification_service.handle_command(msg, cmd).await
            }
        }) => {
            info!("Telegram bot stopped, unsubscribing from price updates");
            info_client.unsubscribe(subscription_id).await.unwrap();
        }
        _ = async move {
            let alert_service = alert_service_clone.clone();
            while let Some(hyperliquid_rust_sdk::Message::ActiveSpotAssetCtx(order_updates)) = receiver.recv().await {
                info!("Received order update data: {order_updates:?}");
                let mark_px = order_updates.data.ctx.shared.mark_px.parse::<f64>().unwrap();
                let alerts = alert_service.get_triggered_alerts(mark_px).await.unwrap();
                for alert in &alerts {
                    println!("Alert triggered: {alert:?}");
                    
                    bot.send_message(teloxide::types::ChatId(alert.chat_id), format!("🔔 Price Alert: {} is at {}", alert.coin, alert.price)).await.unwrap();
                }
                alert_service.set_alert_cooldowns(&alerts).await.unwrap();
            }
        } => {
            info!("Price monitoring stopped, unsubscribing from price updates");
            info_client.unsubscribe(subscription_id).await.unwrap();
        }
        _ = async move {
            let alert_service = alert_service_clone_2.clone();
            loop {
                alert_service.reset_cooldowns().await.unwrap();
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        } => {
        }
    }
}


