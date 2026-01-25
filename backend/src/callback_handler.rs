use log::{debug, error, info};
use std::collections::HashMap;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::{CallbackQuery, ChatId, MessageId};
use tokio::sync::RwLock;

use crate::AppError;
use crate::alerts::AlertService;
use crate::cron::CronService;
use crate::keyboards::{self, CallbackAction, parse_callback};

/// User state for multi-step interactions
#[derive(Debug, Clone, Default)]
pub enum UserState {
    /// Waiting for custom coin input for price alert
    WaitingForCustomCoin,
    /// Waiting for custom coin input for cron alert
    WaitingForCustomCronCoin,
    /// Waiting for price input after coin selection for price alert
    WaitingForPrice { coin: String },
    /// Waiting for time input after selecting schedule for cron alert
    WaitingForCronTime { coin: String, schedule: String },
    /// Idle state
    #[default]
    Idle,
}

/// Manages user states across the application
#[derive(Clone, Default)]
pub struct UserStateManager {
    states: Arc<RwLock<HashMap<i64, UserState>>>,
}

impl UserStateManager {
    pub fn new() -> Self {
        Self {
            states: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get_state(&self, chat_id: i64) -> UserState {
        self.states
            .read()
            .await
            .get(&chat_id)
            .cloned()
            .unwrap_or_default()
    }

    pub async fn set_state(&self, chat_id: i64, state: UserState) {
        self.states.write().await.insert(chat_id, state);
    }

    pub async fn clear_state(&self, chat_id: i64) {
        self.states.write().await.remove(&chat_id);
    }
}

/// Handles callback queries from inline keyboards
#[derive(Clone)]
pub struct CallbackHandler {
    alert_service: AlertService,
    cron_service: CronService,
    state_manager: UserStateManager,
}

impl CallbackHandler {
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

    pub fn state_manager(&self) -> &UserStateManager {
        &self.state_manager
    }

    /// Handle a callback query from an inline keyboard button press
    pub async fn handle_callback(&self, bot: Bot, query: CallbackQuery) -> ResponseResult<()> {
        let data = match query.data.as_ref() {
            Some(d) => d,
            None => {
                bot.answer_callback_query(query.id.clone()).await?;
                return Ok(());
            }
        };

        let chat_id = query.message.as_ref().map(|m| m.chat().id.0).unwrap_or(0);
        let message_id = query.message.as_ref().and_then(|m| m.id().into());

        debug!("Callback received: {} from chat {}", data, chat_id);

        let action = parse_callback(data);
        let result = match action {
            CallbackAction::MainMenu => self.handle_main_menu(&bot, chat_id, message_id).await,
            CallbackAction::MenuAlerts => self.handle_menu_alerts(&bot, chat_id, message_id).await,
            CallbackAction::MenuCron => self.handle_menu_cron(&bot, chat_id, message_id).await,
            CallbackAction::MenuNewAlert => {
                self.handle_menu_new_alert(&bot, chat_id, message_id).await
            }
            CallbackAction::MenuNewCron => {
                self.handle_menu_new_cron(&bot, chat_id, message_id).await
            }
            CallbackAction::MenuHelp => self.handle_menu_help(&bot, chat_id, message_id).await,
            CallbackAction::CoinSelect(coin) => {
                self.handle_coin_select(&bot, chat_id, message_id, &coin)
                    .await
            }
            CallbackAction::CustomCoin => self.handle_custom_coin(&bot, chat_id, message_id).await,
            CallbackAction::ScheduleSelect(schedule) => {
                self.handle_schedule_select(&bot, chat_id, message_id, &schedule)
                    .await
            }
            CallbackAction::DeleteAlert(id) => {
                self.handle_delete_alert(&bot, chat_id, message_id, id)
                    .await
            }
            CallbackAction::DeleteCron(id) => {
                self.handle_delete_cron(&bot, chat_id, message_id, id).await
            }
            CallbackAction::ConfirmDeleteAlert(id) => {
                self.handle_confirm_delete_alert(&bot, chat_id, message_id, id)
                    .await
            }
            CallbackAction::ConfirmDeleteCron(id) => {
                self.handle_confirm_delete_cron(&bot, chat_id, message_id, id)
                    .await
            }
            CallbackAction::Cancel => self.handle_cancel(&bot, chat_id, message_id).await,
            CallbackAction::Unknown(unknown) => {
                debug!("Unknown callback action: {}", unknown);
                Ok(())
            }
        };

        // Answer the callback query to remove the loading indicator
        bot.answer_callback_query(query.id.clone()).await?;

        if let Err(e) = result {
            error!("Error handling callback: {}", e);
        }

        Ok(())
    }

    async fn handle_main_menu(
        &self,
        bot: &Bot,
        chat_id: i64,
        message_id: Option<MessageId>,
    ) -> Result<(), AppError> {
        self.state_manager.clear_state(chat_id).await;
        let text = "Welcome! Choose an option:";
        let keyboard = keyboards::main_menu_keyboard();

        if let Some(msg_id) = message_id {
            bot.edit_message_text(ChatId(chat_id), msg_id, text)
                .reply_markup(keyboard)
                .await?;
        } else {
            bot.send_message(ChatId(chat_id), text)
                .reply_markup(keyboard)
                .await?;
        }
        Ok(())
    }

    async fn handle_menu_alerts(
        &self,
        bot: &Bot,
        chat_id: i64,
        message_id: Option<MessageId>,
    ) -> Result<(), AppError> {
        let alerts = self
            .alert_service
            .get_all_alerts_for_chat(ChatId(chat_id))
            .await?;

        let (text, keyboard) = if alerts.is_empty() {
            (
                "No price alerts set.\n\nUse the button below to create one.".to_string(),
                keyboards::main_menu_keyboard(),
            )
        } else {
            let alert_data: Vec<(i64, String, f64)> = alerts
                .iter()
                .map(|a| (a.id, a.coin.clone(), a.price))
                .collect();
            (
                "Your Price Alerts:".to_string(),
                keyboards::alerts_list_keyboard(&alert_data),
            )
        };

        if let Some(msg_id) = message_id {
            bot.edit_message_text(ChatId(chat_id), msg_id, text)
                .reply_markup(keyboard)
                .await?;
        } else {
            bot.send_message(ChatId(chat_id), text)
                .reply_markup(keyboard)
                .await?;
        }
        Ok(())
    }

    async fn handle_menu_cron(
        &self,
        bot: &Bot,
        chat_id: i64,
        message_id: Option<MessageId>,
    ) -> Result<(), AppError> {
        let cron_alerts = self
            .cron_service
            .get_cron_alerts_for_chat(ChatId(chat_id))
            .await?;

        let (text, keyboard) = if cron_alerts.is_empty() {
            (
                "No cron alerts set.\n\nUse the button below to create one.".to_string(),
                keyboards::main_menu_keyboard(),
            )
        } else {
            let alert_data: Vec<(i64, String, String)> = cron_alerts
                .iter()
                .map(|a| (a.id, a.token.clone(), a.cron_schedule.clone()))
                .collect();
            (
                "Your Cron Alerts:".to_string(),
                keyboards::cron_alerts_list_keyboard(&alert_data),
            )
        };

        if let Some(msg_id) = message_id {
            bot.edit_message_text(ChatId(chat_id), msg_id, text)
                .reply_markup(keyboard)
                .await?;
        } else {
            bot.send_message(ChatId(chat_id), text)
                .reply_markup(keyboard)
                .await?;
        }
        Ok(())
    }

    async fn handle_menu_new_alert(
        &self,
        bot: &Bot,
        chat_id: i64,
        message_id: Option<MessageId>,
    ) -> Result<(), AppError> {
        let text = "Select a coin for your price alert:";
        let keyboard = keyboards::coin_selection_keyboard();

        if let Some(msg_id) = message_id {
            bot.edit_message_text(ChatId(chat_id), msg_id, text)
                .reply_markup(keyboard)
                .await?;
        } else {
            bot.send_message(ChatId(chat_id), text)
                .reply_markup(keyboard)
                .await?;
        }
        Ok(())
    }

    async fn handle_menu_new_cron(
        &self,
        bot: &Bot,
        chat_id: i64,
        message_id: Option<MessageId>,
    ) -> Result<(), AppError> {
        let text = "Select a coin for your cron alert:";
        let keyboard = keyboards::coin_selection_keyboard();

        // Set state to indicate we're creating a cron alert (will be updated after coin selection)
        self.state_manager
            .set_state(
                chat_id,
                UserState::WaitingForCronTime {
                    coin: String::new(),
                    schedule: String::new(),
                },
            )
            .await;

        if let Some(msg_id) = message_id {
            bot.edit_message_text(ChatId(chat_id), msg_id, text)
                .reply_markup(keyboard)
                .await?;
        } else {
            bot.send_message(ChatId(chat_id), text)
                .reply_markup(keyboard)
                .await?;
        }
        Ok(())
    }

    async fn handle_menu_help(
        &self,
        bot: &Bot,
        chat_id: i64,
        message_id: Option<MessageId>,
    ) -> Result<(), AppError> {
        let text = r#"Hyperliquid Alerts Bot

Price Alerts:
Trigger when a coin reaches a specific price.

Cron Alerts:
Send price updates on a schedule (daily or specific days).

Commands:
/menu - Open this menu
/setalert <coin> <price> - Create price alert
/setcronalert <coin> <schedule> <time> - Create cron alert
/alert - List price alerts
/cronalerts - List cron alerts"#;

        let keyboard = keyboards::main_menu_keyboard();

        if let Some(msg_id) = message_id {
            bot.edit_message_text(ChatId(chat_id), msg_id, text)
                .reply_markup(keyboard)
                .await?;
        } else {
            bot.send_message(ChatId(chat_id), text)
                .reply_markup(keyboard)
                .await?;
        }
        Ok(())
    }

    async fn handle_coin_select(
        &self,
        bot: &Bot,
        chat_id: i64,
        message_id: Option<MessageId>,
        coin: &str,
    ) -> Result<(), AppError> {
        let state = self.state_manager.get_state(chat_id).await;

        match state {
            UserState::WaitingForCronTime { .. } => {
                // User is creating a cron alert, show schedule selection
                let text = format!("Selected: {}\n\nChoose a schedule:", coin);
                let keyboard = keyboards::schedule_selection_keyboard();

                self.state_manager
                    .set_state(
                        chat_id,
                        UserState::WaitingForCronTime {
                            coin: coin.to_string(),
                            schedule: String::new(),
                        },
                    )
                    .await;

                if let Some(msg_id) = message_id {
                    bot.edit_message_text(ChatId(chat_id), msg_id, text)
                        .reply_markup(keyboard)
                        .await?;
                }
            }
            _ => {
                // User is creating a price alert
                let text = format!(
                    "Selected: {}\n\nPlease reply with the target price (e.g., 25.50):",
                    coin
                );

                self.state_manager
                    .set_state(
                        chat_id,
                        UserState::WaitingForPrice {
                            coin: coin.to_string(),
                        },
                    )
                    .await;

                if let Some(msg_id) = message_id {
                    bot.edit_message_text(ChatId(chat_id), msg_id, text).await?;
                }
            }
        }
        Ok(())
    }

    async fn handle_custom_coin(
        &self,
        bot: &Bot,
        chat_id: i64,
        message_id: Option<MessageId>,
    ) -> Result<(), AppError> {
        let state = self.state_manager.get_state(chat_id).await;

        let (new_state, text) = match state {
            UserState::WaitingForCronTime { .. } => (
                UserState::WaitingForCustomCronCoin,
                "Enter the coin ticker (e.g., HYPE, BTC, ETH):".to_string(),
            ),
            _ => (
                UserState::WaitingForCustomCoin,
                "Enter the coin ticker (e.g., HYPE, BTC, ETH):".to_string(),
            ),
        };

        self.state_manager.set_state(chat_id, new_state).await;

        if let Some(msg_id) = message_id {
            bot.edit_message_text(ChatId(chat_id), msg_id, text).await?;
        } else {
            bot.send_message(ChatId(chat_id), text).await?;
        }
        Ok(())
    }

    async fn handle_schedule_select(
        &self,
        bot: &Bot,
        chat_id: i64,
        message_id: Option<MessageId>,
        schedule: &str,
    ) -> Result<(), AppError> {
        let state = self.state_manager.get_state(chat_id).await;

        if let UserState::WaitingForCronTime { coin, .. } = state {
            let text = format!(
                "Selected: {} on {}\n\nPlease reply with the time in HH:MM format (e.g., 09:00):",
                coin, schedule
            );

            self.state_manager
                .set_state(
                    chat_id,
                    UserState::WaitingForCronTime {
                        coin,
                        schedule: schedule.to_string(),
                    },
                )
                .await;

            if let Some(msg_id) = message_id {
                bot.edit_message_text(ChatId(chat_id), msg_id, text).await?;
            }
        }
        Ok(())
    }

    async fn handle_delete_alert(
        &self,
        bot: &Bot,
        chat_id: i64,
        message_id: Option<MessageId>,
        alert_id: i64,
    ) -> Result<(), AppError> {
        let text = format!("Are you sure you want to delete alert #{}?", alert_id);
        let keyboard = keyboards::confirm_delete_alert_keyboard(alert_id);

        if let Some(msg_id) = message_id {
            bot.edit_message_text(ChatId(chat_id), msg_id, text)
                .reply_markup(keyboard)
                .await?;
        }
        Ok(())
    }

    async fn handle_delete_cron(
        &self,
        bot: &Bot,
        chat_id: i64,
        message_id: Option<MessageId>,
        alert_id: i64,
    ) -> Result<(), AppError> {
        let text = format!("Are you sure you want to delete cron alert #{}?", alert_id);
        let keyboard = keyboards::confirm_delete_cron_keyboard(alert_id);

        if let Some(msg_id) = message_id {
            bot.edit_message_text(ChatId(chat_id), msg_id, text)
                .reply_markup(keyboard)
                .await?;
        }
        Ok(())
    }

    async fn handle_confirm_delete_alert(
        &self,
        bot: &Bot,
        chat_id: i64,
        message_id: Option<MessageId>,
        alert_id: i64,
    ) -> Result<(), AppError> {
        match self.alert_service.delete_alert(alert_id).await {
            Ok(_) => {
                info!("Deleted alert {} for chat {}", alert_id, chat_id);
                // Show updated alerts list
                self.handle_menu_alerts(bot, chat_id, message_id).await?;
            }
            Err(e) => {
                error!("Failed to delete alert {}: {}", alert_id, e);
                let text = format!("Failed to delete alert #{}. It may not exist.", alert_id);
                if let Some(msg_id) = message_id {
                    bot.edit_message_text(ChatId(chat_id), msg_id, text)
                        .reply_markup(keyboards::main_menu_keyboard())
                        .await?;
                }
            }
        }
        Ok(())
    }

    async fn handle_confirm_delete_cron(
        &self,
        bot: &Bot,
        chat_id: i64,
        message_id: Option<MessageId>,
        alert_id: i64,
    ) -> Result<(), AppError> {
        match self.cron_service.delete_cron_alert(alert_id).await {
            Ok(_) => {
                info!("Deleted cron alert {} for chat {}", alert_id, chat_id);
                // Show updated cron alerts list
                self.handle_menu_cron(bot, chat_id, message_id).await?;
            }
            Err(e) => {
                error!("Failed to delete cron alert {}: {}", alert_id, e);
                let text = format!(
                    "Failed to delete cron alert #{}. It may not exist.",
                    alert_id
                );
                if let Some(msg_id) = message_id {
                    bot.edit_message_text(ChatId(chat_id), msg_id, text)
                        .reply_markup(keyboards::main_menu_keyboard())
                        .await?;
                }
            }
        }
        Ok(())
    }

    async fn handle_cancel(
        &self,
        bot: &Bot,
        chat_id: i64,
        message_id: Option<MessageId>,
    ) -> Result<(), AppError> {
        self.state_manager.clear_state(chat_id).await;
        self.handle_main_menu(bot, chat_id, message_id).await
    }

    /// Handle text messages that might be responses to prompts (price input, time input)
    pub async fn handle_text_input(
        &self,
        bot: &Bot,
        chat_id: ChatId,
        text: &str,
    ) -> Result<bool, AppError> {
        let state = self.state_manager.get_state(chat_id.0).await;

        match state {
            UserState::WaitingForCustomCoin => {
                let coin = text.trim().to_uppercase();
                // Validate the coin exists
                match self.alert_service.validate_coin(&coin).await {
                    Ok(_) => {
                        // Coin is valid, proceed to price input
                        let text = format!(
                            "Selected: {}\n\nPlease reply with the target price (e.g., 25.50):",
                            coin
                        );
                        self.state_manager
                            .set_state(chat_id.0, UserState::WaitingForPrice { coin })
                            .await;
                        bot.send_message(chat_id, text).await?;
                    }
                    Err(AppError::TokenNotFound(_)) => {
                        bot.send_message(
                            chat_id,
                            format!(
                                "Coin '{}' not found on Hyperliquid.\n\nPlease enter a valid ticker:",
                                coin
                            ),
                        )
                        .await?;
                        return Ok(true); // Keep state, let user try again
                    }
                    Err(e) => {
                        error!("Failed to validate coin: {}", e);
                        bot.send_message(chat_id, "Failed to validate coin. Please try again:")
                            .await?;
                        return Ok(true);
                    }
                }
                Ok(true)
            }
            UserState::WaitingForCustomCronCoin => {
                let coin = text.trim().to_uppercase();
                // Validate the coin exists
                match self.alert_service.validate_coin(&coin).await {
                    Ok(_) => {
                        // Coin is valid, proceed to schedule selection
                        let text = format!("Selected: {}\n\nChoose a schedule:", coin);
                        let keyboard = keyboards::schedule_selection_keyboard();
                        self.state_manager
                            .set_state(
                                chat_id.0,
                                UserState::WaitingForCronTime {
                                    coin,
                                    schedule: String::new(),
                                },
                            )
                            .await;
                        bot.send_message(chat_id, text)
                            .reply_markup(keyboard)
                            .await?;
                    }
                    Err(AppError::TokenNotFound(_)) => {
                        bot.send_message(
                            chat_id,
                            format!(
                                "Coin '{}' not found on Hyperliquid.\n\nPlease enter a valid ticker:",
                                coin
                            ),
                        )
                        .await?;
                        return Ok(true); // Keep state, let user try again
                    }
                    Err(e) => {
                        error!("Failed to validate coin: {}", e);
                        bot.send_message(chat_id, "Failed to validate coin. Please try again:")
                            .await?;
                        return Ok(true);
                    }
                }
                Ok(true)
            }
            UserState::WaitingForPrice { coin } => {
                match text.trim().parse::<f64>() {
                    Ok(price) => {
                        match self
                            .alert_service
                            .create_alert("0x00", chat_id, &coin, price)
                            .await
                        {
                            Ok(_) => {
                                bot.send_message(
                                    chat_id,
                                    format!("Price alert set for {} at ${:.2}", coin, price),
                                )
                                .reply_markup(keyboards::main_menu_keyboard())
                                .await?;
                            }
                            Err(AppError::TokenNotFound(msg)) => {
                                bot.send_message(chat_id, format!("Error: {}", msg))
                                    .reply_markup(keyboards::main_menu_keyboard())
                                    .await?;
                            }
                            Err(e) => {
                                error!("Failed to create alert: {}", e);
                                bot.send_message(
                                    chat_id,
                                    "Failed to create alert. Please try again.",
                                )
                                .reply_markup(keyboards::main_menu_keyboard())
                                .await?;
                            }
                        }
                    }
                    Err(_) => {
                        bot.send_message(
                            chat_id,
                            "Invalid price. Please enter a number (e.g., 25.50):",
                        )
                        .await?;
                        return Ok(true); // Keep the state, let user try again
                    }
                }
                self.state_manager.clear_state(chat_id.0).await;
                Ok(true)
            }
            UserState::WaitingForCronTime { coin, schedule } => {
                // Validate time format HH:MM
                let time = text.trim();
                if !time.contains(':') || time.len() != 5 {
                    bot.send_message(
                        chat_id,
                        "Invalid time format. Please use HH:MM (e.g., 09:00):",
                    )
                    .await?;
                    return Ok(true);
                }

                match self.cron_service.create_schedule(&schedule, time).await {
                    Ok(cron_schedule) => {
                        match self
                            .cron_service
                            .create_cron_alert(chat_id, &coin, &cron_schedule)
                            .await
                        {
                            Ok(_) => {
                                bot.send_message(
                                    chat_id,
                                    format!(
                                        "Cron alert set for {} ({} at {})",
                                        coin, schedule, time
                                    ),
                                )
                                .reply_markup(keyboards::main_menu_keyboard())
                                .await?;
                            }
                            Err(AppError::TokenNotFound(msg)) => {
                                bot.send_message(chat_id, format!("Error: {}", msg))
                                    .reply_markup(keyboards::main_menu_keyboard())
                                    .await?;
                            }
                            Err(e) => {
                                error!("Failed to create cron alert: {}", e);
                                bot.send_message(
                                    chat_id,
                                    "Failed to create cron alert. Please try again.",
                                )
                                .reply_markup(keyboards::main_menu_keyboard())
                                .await?;
                            }
                        }
                    }
                    Err(AppError::InvalidTimeFormat(msg)) => {
                        bot.send_message(chat_id, format!("Error: {}", msg))
                            .reply_markup(keyboards::main_menu_keyboard())
                            .await?;
                    }
                    Err(e) => {
                        error!("Failed to create schedule: {}", e);
                        bot.send_message(chat_id, "Failed to create schedule. Please try again.")
                            .reply_markup(keyboards::main_menu_keyboard())
                            .await?;
                    }
                }
                self.state_manager.clear_state(chat_id.0).await;
                Ok(true)
            }
            UserState::Idle => Ok(false), // Not handled by callback handler
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_state_default() {
        let state = UserState::default();
        assert!(matches!(state, UserState::Idle));
    }

    #[tokio::test]
    async fn test_state_manager_set_get() {
        let manager = UserStateManager::new();

        // Initially idle
        let state = manager.get_state(123).await;
        assert!(matches!(state, UserState::Idle));

        // Set and retrieve
        manager
            .set_state(
                123,
                UserState::WaitingForPrice {
                    coin: "HYPE".to_string(),
                },
            )
            .await;
        let state = manager.get_state(123).await;
        assert!(matches!(state, UserState::WaitingForPrice { coin } if coin == "HYPE"));

        // Clear
        manager.clear_state(123).await;
        let state = manager.get_state(123).await;
        assert!(matches!(state, UserState::Idle));
    }

    #[tokio::test]
    async fn test_state_manager_multiple_users() {
        let manager = UserStateManager::new();

        manager
            .set_state(
                1,
                UserState::WaitingForPrice {
                    coin: "BTC".to_string(),
                },
            )
            .await;
        manager
            .set_state(
                2,
                UserState::WaitingForCronTime {
                    coin: "ETH".to_string(),
                    schedule: "daily".to_string(),
                },
            )
            .await;

        let state1 = manager.get_state(1).await;
        let state2 = manager.get_state(2).await;

        assert!(matches!(state1, UserState::WaitingForPrice { coin } if coin == "BTC"));
        assert!(
            matches!(state2, UserState::WaitingForCronTime { coin, schedule } if coin == "ETH" && schedule == "daily")
        );
    }
}
