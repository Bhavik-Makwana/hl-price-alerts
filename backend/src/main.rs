use hyperliquid_rust_sdk::{BaseUrl, InfoClient, Subscription};
use log::{info, error, debug};
use tokio::sync::mpsc::unbounded_channel;
use teloxide::prelude::*;
use std::sync::Arc;
use tokio::sync::Mutex;
use backend::{
    db::Database,
    notification::{NotificationService, Command},
    alerts::AlertService,
    cron::CronService,
};
use cron_parser::parse;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();
    dotenv::dotenv().ok();

    info!("Starting Alert Price Bot...");

    // Initialize database
    let db = Database::new("alerts.db")
        .map_err(|e| anyhow::anyhow!("Failed to open database: {}", e))?;
    db.initialize().await
        .map_err(|e| anyhow::anyhow!("Failed to initialize database: {}", e))?;
    let tokens = db.get_all_unique_tokens().await
        .map_err(|e| anyhow::anyhow!("Failed to get tokens: {}", e))?;

    // Initialize Hyperliquid client
    let info_client = Arc::new(Mutex::new(
        InfoClient::new(None, Some(BaseUrl::Mainnet)).await
            .map_err(|e| anyhow::anyhow!("Failed to create InfoClient: {}", e))?
    ));

    let alert_service = AlertService::new(db.clone(), info_client.clone());
    let cron_service = CronService::new(db.clone(), info_client.clone());

    // Log existing alerts at startup
    match alert_service.get_all_alerts().await {
        Ok(alerts) => {
            info!("Loaded {} price alerts", alerts.len());
            for alert in alerts {
                debug!("Alert: {:?}", alert);
            }
        }
        Err(e) => error!("Failed to load alerts: {}", e),
    }

    match cron_service.get_all_cron_alerts().await {
        Ok(cron_alerts) => {
            info!("Loaded {} cron alerts", cron_alerts.len());
            for cron_alert in cron_alerts {
                debug!("Cron Alert: {:?}", cron_alert);
            }
        }
        Err(e) => error!("Failed to load cron alerts: {}", e),
    }

    let bot = teloxide::Bot::from_env();

    // Subscribe to price updates for all tracked tokens
    let (sender, mut receiver) = unbounded_channel();
    let mut subscription_ids = Vec::new();
    for token in tokens {
        match info_client
            .lock().await
            .subscribe(
                Subscription::ActiveAssetCtx { coin: token.clone() },
                sender.clone(),
            )
            .await
        {
            Ok(subscription_id) => {
                subscription_ids.push(subscription_id);
                debug!("Subscribed to {}", token);
            }
            Err(e) => error!("Failed to subscribe to {}: {}", token, e),
        }
    }

    let alert_service_for_price_updates = alert_service.clone();
    let alert_service_for_cooldowns = alert_service.clone();
    let cron_service_for_worker = cron_service.clone();
    let bot_for_cron = bot.clone();
    let notification_service = NotificationService::new(alert_service, cron_service.clone());

    tokio::select! {
        // Telegram command handler
        _ = Command::repl(bot.clone(), move |bot, msg, cmd| {
            let notification_service = notification_service.clone();
            async move {
                notification_service.handle_command(bot, msg, cmd).await
            }
        }) => {
            info!("Telegram bot stopped, unsubscribing from price updates");
            for subscription_id in subscription_ids {
                if let Err(e) = info_client.lock().await.unsubscribe(subscription_id).await {
                    error!("Failed to unsubscribe: {}", e);
                }
            }
        }

        // Price monitoring worker
        _ = async move {
            while let Some(hyperliquid_rust_sdk::Message::ActiveSpotAssetCtx(order_updates)) = receiver.recv().await {
                debug!("Received order update: {:?}", order_updates);

                // Parse the mark price
                let mark_px = match order_updates.data.ctx.shared.mark_px.parse::<f64>() {
                    Ok(px) => px,
                    Err(e) => {
                        error!("Failed to parse price '{}': {}", order_updates.data.ctx.shared.mark_px, e);
                        continue;
                    }
                };

                // Get triggered alerts
                let alerts = match alert_service_for_price_updates.get_triggered_alerts(mark_px).await {
                    Ok(alerts) => alerts,
                    Err(e) => {
                        error!("Failed to get triggered alerts: {}", e);
                        continue;
                    }
                };

                // Send notifications for each triggered alert
                for alert in &alerts {
                    info!("Alert triggered: {} at {}", alert.coin, alert.price);
                    if let Err(e) = bot.send_message(
                        teloxide::types::ChatId(alert.chat_id),
                        format!("🔔 Price Alert: {} is at ${:.2}", alert.coin, mark_px)
                    ).await {
                        error!("Failed to send alert message: {}", e);
                    }
                }

                // Set cooldowns for triggered alerts
                if let Err(e) = alert_service_for_price_updates.set_alert_cooldowns(&alerts).await {
                    error!("Failed to set cooldowns: {}", e);
                }
            }
        } => {
            info!("Price monitoring stopped");
        }

        // Cooldown reset worker
        _ = async move {
            loop {
                if let Err(e) = alert_service_for_cooldowns.reset_cooldowns().await {
                    error!("Failed to reset cooldowns: {}", e);
                }
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        } => {
            info!("Cooldown worker stopped");
        }

        // Cron alert worker
        _ = async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                debug!("Checking for scheduled alerts");

                let cron_alerts = match cron_service_for_worker.get_triggered_cron_alerts().await {
                    Ok(alerts) => alerts,
                    Err(e) => {
                        error!("Failed to get triggered cron alerts: {}", e);
                        continue;
                    }
                };

                for cron_alert in cron_alerts {
                    info!("Sending cron alert for {}", cron_alert.coin);

                    // Get current price
                    let price = match cron_service_for_worker.get_price(&cron_alert.token).await {
                        Ok(p) => p,
                        Err(e) => {
                            error!("Failed to get price for {}: {}", cron_alert.token, e);
                            // Still mark as triggered to avoid repeated failures
                            if let Ok(next) = parse(cron_alert.cron_schedule.trim(), &chrono::Utc::now()) {
                                let _ = cron_service_for_worker.mark_cron_alert_triggered(cron_alert.id, next).await;
                            }
                            continue;
                        }
                    };

                    // Send notification
                    if let Err(e) = bot_for_cron.send_message(
                        teloxide::types::ChatId(cron_alert.chat_id),
                        format!("⏰ {}: ${:.2}", cron_alert.coin, price)
                    ).await {
                        error!("Failed to send cron alert: {}", e);
                    }

                    // Calculate next trigger time and update database
                    match parse(cron_alert.cron_schedule.trim(), &chrono::Utc::now()) {
                        Ok(next_trigger) => {
                            if let Err(e) = cron_service_for_worker.mark_cron_alert_triggered(cron_alert.id, next_trigger).await {
                                error!("Failed to update cron alert {}: {}", cron_alert.id, e);
                            }
                        }
                        Err(e) => {
                            error!("Failed to parse cron schedule '{}': {}", cron_alert.cron_schedule, e);
                        }
                    }
                }
            }
        } => {
            info!("Cron worker stopped");
        }
    }

    Ok(())
}
