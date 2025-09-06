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
    let alert_price_chat_id: i64 = std::env::var("ALERT_PRICE_CHAT_ID").unwrap().parse::<i64>().unwrap();

    log::info!("Starting Alert Price Bot...");

    // Initialize database
    let db = Database::new("alerts.db").unwrap();
    db.initialize().await.unwrap();
    
    // Insert initial alert
    db.insert_alert("0x00", alert_price_chat_id, ALERT_PRICE_COIN, ALERT_PRICE_COIN, 46.6).await.unwrap();
    
    // Initialize alert service
    let alert_service = AlertService::new(db);
    
    // Get all alerts for debugging
    let alerts = alert_service.get_all_alerts().await.unwrap();
    println!("Alerts: {alerts:?}");

    // Initialize notification service
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

    tokio::select! {
        _ = Command::repl(bot.clone(), move |bot, msg, cmd| {
            let notification_service = NotificationService::new(bot, 46.6);
            async move {
                notification_service.handle_command(msg, cmd).await
            }
        }) => {
            info!("Telegram bot stopped, unsubscribing from price updates");
            info_client.unsubscribe(subscription_id).await.unwrap();
        }
        _ = async move {
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
            loop {
                alert_service_clone.reset_cooldowns().await.unwrap();
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        } => {
        }
    }
}
