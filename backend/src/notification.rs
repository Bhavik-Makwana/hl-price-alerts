use crate::AppError;
use crate::alerts::AlertService;
use crate::cron::CronService;
use crate::db::AlertTable;
use crate::keyboards;
use crate::callback_handler::UserStateManager;
use teloxide::{prelude::*, utils::command::BotCommands};

#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
pub enum Command {
    #[command(description = "display this text.")]
    Help,
    #[command(description = "Open the interactive menu.")]
    Menu,
    #[command(description = "Open the interactive menu.")]
    Start,
    #[command(description = "Display all alerts.")]
    Alert,
    #[command(parse_with = "split", alias = "ua", hide_aliases)]
    SetAlert { coin: String, price: f64 },
    #[command(description = "Display all cron alerts.")]
    CronAlerts,
    #[command(
        parse_with = "split",
        description = "Create a cron alert at a specific time."
    )]
    SetCronAlert {
        coin: String,
        schedule: String,
        time: String,
    },
    #[command(parse_with = "split", description = "Delete a cron alert by ID.")]
    DeleteCronAlert { id: i64 },
}

#[derive(Clone)]
pub struct NotificationService {
    alert_service: AlertService,
    cron_service: CronService,
    state_manager: UserStateManager,
}

impl NotificationService {
    pub fn new(
        alert_service: AlertService,
        cron_service: CronService,
        state_manager: UserStateManager,
    ) -> Self {
        Self {
            alert_service,
            cron_service,
            state_manager,
        }
    }

    pub async fn handle_command(
        &self,
        bot: Bot,
        msg: teloxide::types::Message,
        cmd: Command,
    ) -> ResponseResult<()> {
        match cmd {
            Command::Help => {
                bot.send_message(msg.chat.id, Command::descriptions().to_string())
                    .await?
            }
            Command::Menu | Command::Start => {
                self.state_manager.clear_state(msg.chat.id.0).await;
                bot.send_message(msg.chat.id, "Welcome! Choose an option:")
                    .reply_markup(keyboards::main_menu_keyboard())
                    .await?
            }
            Command::Alert => {
                match self
                    .alert_service
                    .get_all_alerts_for_chat(msg.chat.id)
                    .await
                {
                    Ok(alerts) => {
                        let alerts_buffer = if alerts.is_empty() {
                            "No alerts set.".to_string()
                        } else {
                            alerts
                                .iter()
                                .map(|alert| alert.to_string())
                                .collect::<Vec<String>>()
                                .join("\n")
                        };
                        bot.send_message(msg.chat.id, format!("📊 Alerts:\n{}", alerts_buffer))
                            .await?
                    }
                    Err(e) => {
                        log::error!("Failed to fetch alerts: {}", e);
                        bot.send_message(
                            msg.chat.id,
                            "❌ Failed to fetch alerts. Please try again.",
                        )
                        .await?
                    }
                }
            }
            Command::SetAlert { coin, price } => {
                let coin = coin.to_uppercase();
                match self
                    .alert_service
                    .create_alert("0x00", msg.chat.id, &coin, price)
                    .await
                {
                    Ok(_) => {
                        bot.send_message(
                            msg.chat.id,
                            format!("✅ Alert set for {} at ${:.2}", coin, price),
                        )
                        .await?
                    }
                    Err(AppError::TokenNotFound(msg_text)) => {
                        bot.send_message(msg.chat.id, format!("❌ {}", msg_text))
                            .await?
                    }
                    Err(e) => {
                        log::error!("Failed to create alert: {}", e);
                        bot.send_message(
                            msg.chat.id,
                            "❌ Failed to create alert. Please try again.",
                        )
                        .await?
                    }
                }
            }
            Command::CronAlerts => {
                match self
                    .cron_service
                    .get_cron_alerts_for_chat(msg.chat.id)
                    .await
                {
                    Ok(cron_alerts) => {
                        let alerts_buffer = if cron_alerts.is_empty() {
                            "No cron alerts set.".to_string()
                        } else {
                            cron_alerts
                                .iter()
                                .map(|alert| alert.to_string())
                                .collect::<Vec<String>>()
                                .join("\n")
                        };
                        bot.send_message(msg.chat.id, format!("⏰ Cron Alerts:\n{}", alerts_buffer))
                            .await?
                    }
                    Err(e) => {
                        log::error!("Failed to fetch cron alerts: {}", e);
                        bot.send_message(
                            msg.chat.id,
                            "❌ Failed to fetch cron alerts. Please try again.",
                        )
                        .await?
                    }
                }
            }
            Command::SetCronAlert {
                coin,
                schedule,
                time,
            } => match self.cron_service.create_schedule(&schedule, &time).await {
                Ok(cron_schedule) => {
                    let coin = coin.to_uppercase();
                    match self
                        .cron_service
                        .create_cron_alert(msg.chat.id, &coin, &cron_schedule)
                        .await
                    {
                        Ok(_) => {
                            log::info!(
                                "Cron alert set with schedule {} for {}",
                                cron_schedule,
                                coin
                            );
                            bot.send_message(
                                msg.chat.id,
                                format!(
                                    "✅ Cron alert set for {} (schedule: {})",
                                    coin, cron_schedule
                                ),
                            )
                            .await?
                        }
                        Err(AppError::TokenNotFound(msg_text)) => {
                            bot.send_message(msg.chat.id, format!("❌ {}", msg_text))
                                .await?
                        }
                        Err(AppError::CronParse(msg_text)) => {
                            bot.send_message(msg.chat.id, format!("❌ {}", msg_text))
                                .await?
                        }
                        Err(e) => {
                            log::error!("Failed to create cron alert: {}", e);
                            bot.send_message(
                                msg.chat.id,
                                "❌ Failed to create cron alert. Please try again.",
                            )
                            .await?
                        }
                    }
                }
                Err(AppError::InvalidTimeFormat(msg_text)) => {
                    bot.send_message(msg.chat.id, format!("❌ {}", msg_text))
                        .await?
                }
                Err(e) => {
                    log::error!("Failed to create schedule: {}", e);
                    bot.send_message(msg.chat.id, "❌ Invalid schedule. Use 'daily' or a day name (monday, tuesday, etc.) with time in HH:MM format.").await?
                }
            },
            Command::DeleteCronAlert { id } => {
                match self.cron_service.delete_cron_alert(id).await {
                    Ok(_) => {
                        bot.send_message(msg.chat.id, format!("✅ Cron alert {} deleted.", id))
                            .await?
                    }
                    Err(e) => {
                        log::error!("Failed to delete cron alert {}: {}", id, e);
                        bot.send_message(
                            msg.chat.id,
                            format!("❌ Failed to delete cron alert {}. It may not exist.", id),
                        )
                        .await?
                    }
                }
            }
        };

        Ok(())
    }

    pub async fn send_alert(&self, bot: Bot, alert: &AlertTable) -> ResponseResult<()> {
        bot.send_message(
            teloxide::types::ChatId(alert.chat_id),
            format!("🔔 Price Alert: {} is at {}", alert.coin, alert.price),
        )
        .await?;
        Ok(())
    }
}
