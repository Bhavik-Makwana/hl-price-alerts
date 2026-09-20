use backend::{
    AlertBatcher, CallbackHandler, UserStateManager,
    alerts::AlertService,
    cron::CronService,
    db::Database,
    notification::{Command, NotificationService},
};
use cron_parser::parse;
use hyperliquid_rust_sdk::{BaseUrl, InfoClient, Subscription};
use log::{debug, error, info, warn};
use std::sync::Arc;
use teloxide::dispatching::{UpdateFilterExt, UpdateHandler};
use teloxide::dptree;
use teloxide::prelude::*;
use teloxide::types::Update;
use tokio::sync::Mutex;
use tokio::sync::mpsc::unbounded_channel;

fn schema() -> UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
    let command_handler = Update::filter_message()
        .filter_command::<Command>()
        .endpoint(
            |bot: Bot,
             msg: teloxide::types::Message,
             cmd: Command,
             notification_service: NotificationService| async move {
                notification_service.handle_command(bot, msg, cmd).await?;
                Ok(())
            },
        );

    let callback_handler = Update::filter_callback_query().endpoint(
        |bot: Bot, query: teloxide::types::CallbackQuery, callback_handler: CallbackHandler| async move {
            callback_handler.handle_callback(bot, query).await?;
            Ok(())
        },
    );

    let text_handler = Update::filter_message()
        .filter(|msg: teloxide::types::Message| msg.text().is_some() && !msg.text().unwrap_or("").starts_with('/'))
        .endpoint(
            |bot: Bot, msg: teloxide::types::Message, callback_handler: CallbackHandler| async move {
                #[allow(clippy::collapsible_if)]
                if let Some(text) = msg.text() {
                    if let Err(e) = callback_handler.handle_text_input(&bot, msg.chat.id, text).await {
                        log::error!("Error handling text input from chat {}: {}", msg.chat.id, e);
                    }
                }
                Ok(())
            },
        );

    dptree::entry()
        .branch(command_handler)
        .branch(callback_handler)
        .branch(text_handler)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();
    dotenv::dotenv().ok();

    info!("Starting Alert Price Bot...");

    // Initialize database
    let db = Database::new("alerts.db")
        .map_err(|e| anyhow::anyhow!("Failed to open database: {}", e))?;
    db.initialize()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to initialize database: {}", e))?;
    let tokens = db
        .get_all_unique_tokens()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get tokens: {}", e))?;

    // Initialize Hyperliquid client
    let info_client = Arc::new(Mutex::new(
        InfoClient::new(None, Some(BaseUrl::Mainnet))
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create InfoClient: {}", e))?,
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
            .lock()
            .await
            .subscribe(
                Subscription::ActiveAssetCtx {
                    coin: token.clone(),
                },
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

    // Create notification and callback handlers
    let state_manager = UserStateManager::new();
    let notification_service = NotificationService::new(
        alert_service.clone(),
        cron_service.clone(),
        state_manager.clone(),
    );
    let callback_handler = CallbackHandler::new(alert_service, cron_service.clone(), state_manager);

    // Build dispatcher with all handlers
    let mut dispatcher = Dispatcher::builder(bot.clone(), schema())
        .dependencies(dptree::deps![notification_service, callback_handler])
        .enable_ctrlc_handler()
        .build();

    tokio::select! {
        // Telegram dispatcher (commands + callbacks + text input)
        _ = dispatcher.dispatch() => {
            info!("Telegram bot stopped, unsubscribing from price updates");
            for subscription_id in subscription_ids {
                if let Err(e) = info_client.lock().await.unsubscribe(subscription_id).await {
                    error!("Failed to unsubscribe: {}", e);
                }
            }
        }

        // Price monitoring worker
        _ = async move {
            loop {
                let order_updates = match receiver.recv().await {
                    Some(hyperliquid_rust_sdk::Message::ActiveSpotAssetCtx(order_updates)) => order_updates,
                    Some(other) => {
                        debug!("Ignoring non-price message on price feed: {:?}", other);
                        continue;
                    }
                    None => {
                        warn!("Price update channel closed, stopping price monitoring");
                        break;
                    }
                };

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

        // Cron alert worker - batches alerts triggered within the same minute
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

                if cron_alerts.is_empty() {
                    continue;
                }

                info!("Processing {} cron alerts", cron_alerts.len());

                // Collect all alerts with their prices
                let mut alerts_with_prices: Vec<(backend::db::CronAlert, f64)> = Vec::new();
                for cron_alert in cron_alerts {
                    match cron_service_for_worker.get_price(&cron_alert.token).await {
                        Ok(price) => {
                            alerts_with_prices.push((cron_alert, price));
                        }
                        Err(e) => {
                            error!("Failed to get price for {}: {}", cron_alert.token, e);
                            // Still mark as triggered to avoid repeated failures
                            if let Ok(next) = parse(cron_alert.cron_schedule.trim(), &chrono::Utc::now()) {
                                let _ = cron_service_for_worker.mark_cron_alert_triggered(cron_alert.id, next).await;
                            }
                        }
                    }
                }

                // Batch alerts by chat_id and send combined messages
                let batched_alerts = AlertBatcher::batch_alerts(alerts_with_prices.clone());
                for batched in batched_alerts {
                    let message = AlertBatcher::format_message(&batched);
                    info!("Sending batched cron alert to chat {} ({} alerts)", batched.chat_id, batched.alerts.len());
                    if let Err(e) = bot_for_cron.send_message(batched.chat_id, message).await {
                        error!("Failed to send batched cron alert: {}", e);
                    }
                }

                // Mark all successfully fetched alerts as triggered
                for (cron_alert, _) in alerts_with_prices {
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
