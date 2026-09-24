use crate::AppError;
use crate::alerts::AlertService;
use crate::callback_handler::UserStateManager;
use crate::cron::CronService;
use crate::db::AlertTable;
use crate::keyboards;
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

/// Whether `word` (a command as typed, lowercased, without the leading '/' or any
/// `@botname` suffix) is one of ours. Used to decide whether a slash-command that
/// failed to parse was meant for this bot (worth a helpful reply) or for some other
/// bot/command in the chat (stay silent).
pub fn is_known_command_word(word: &str) -> bool {
    let word = word.to_lowercase();
    // BotCommand::command keeps its leading '/' (it's built from `{prefix}{name}`),
    // so compare against a re-prefixed word rather than stripping it from bot_commands().
    word == "ua"
        || Command::bot_commands()
            .iter()
            .any(|c| c.command == format!("/{word}"))
}

/// Usage hint for a command whose arguments failed to parse, keyed by the command
/// word as typed. `filter_command` silently drops a message when `Command::parse`
/// errors - this is shown instead so a wrong argument count/format (e.g. from
/// `/setcronalert`'s strict single-space split) doesn't fail with zero feedback.
pub fn usage_hint(command_word: &str) -> &'static str {
    match command_word.to_lowercase().as_str() {
        "setalert" | "ua" => "Usage: /setalert <coin> <price>\nExample: /setalert HYPE 25.50",
        "setcronalert" => {
            "Usage: /setcronalert <coin> <schedule> <time>\nExample: /setcronalert HYPE daily 09:00\n(schedule: 'daily' or a weekday name like 'monday'; time is HH:MM, UTC)"
        }
        "deletecronalert" => "Usage: /deletecronalert <id>\nExample: /deletecronalert 3",
        _ => "Use /help to see all commands, or /menu for a guided, step-by-step flow.",
    }
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
                match self.cron_service.delete_cron_alert(msg.chat.id, id).await {
                    Ok(true) => {
                        bot.send_message(msg.chat.id, format!("✅ Cron alert {} deleted.", id))
                            .await?
                    }
                    Ok(false) => {
                        bot.send_message(
                            msg.chat.id,
                            format!("❌ Failed to delete cron alert {}. It may not exist.", id),
                        )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_known_command_word_matches_primary_names() {
        assert!(is_known_command_word("setcronalert"));
        assert!(is_known_command_word("setalert"));
        assert!(is_known_command_word("deletecronalert"));
        assert!(is_known_command_word("help"));
    }

    #[test]
    fn test_is_known_command_word_matches_alias() {
        assert!(is_known_command_word("ua"));
    }

    #[test]
    fn test_is_known_command_word_is_case_insensitive() {
        assert!(is_known_command_word("SetCronAlert"));
    }

    #[test]
    fn test_is_known_command_word_rejects_unrelated_words() {
        assert!(!is_known_command_word("banana"));
        assert!(!is_known_command_word("start_other_bot_command"));
    }

    #[test]
    fn test_usage_hint_for_setcronalert() {
        let hint = usage_hint("setcronalert");
        assert!(hint.contains("/setcronalert <coin> <schedule> <time>"));
    }

    #[test]
    fn test_usage_hint_for_setalert_and_alias() {
        assert!(usage_hint("setalert").contains("/setalert <coin> <price>"));
        assert!(usage_hint("ua").contains("/setalert <coin> <price>"));
    }

    #[test]
    fn test_usage_hint_falls_back_for_unknown_word() {
        assert!(usage_hint("banana").contains("/help"));
    }
}
