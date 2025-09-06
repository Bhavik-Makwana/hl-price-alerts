use teloxide::{prelude::*, utils::command::BotCommands};
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::db::AlertTable;

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "These commands are supported:")]
pub enum Command {
    #[command(description = "display this text.")]
    Help,
    #[command(description = "Set alert price.")]
    Alert(f64),
    #[command(parse_with = "split", alias = "ua", hide_aliases)]
    SetAlert{coin: String, price: f64},
}

pub struct NotificationService {
    bot: Bot,
    alert_price: Arc<Mutex<f64>>,
}

impl NotificationService {
    pub fn new(bot: Bot, initial_price: f64) -> Self {
        Self {
            bot,
            alert_price: Arc::new(Mutex::new(initial_price)),
        }
    }

    pub async fn handle_command(&self, msg: teloxide::types::Message, cmd: Command) -> ResponseResult<()> {
        match cmd {
            Command::Help => {
                self.bot.send_message(msg.chat.id, Command::descriptions().to_string()).await?
            }
            Command::Alert(price) => {
                let mut alert_price_guard = self.alert_price.lock().await;
                *alert_price_guard = price;
                self.bot.send_message(msg.chat.id, format!("Alert price set to {price}.")).await?
            }
            Command::SetAlert{coin, price} => {
                let mut alert_price_guard = self.alert_price.lock().await;
                *alert_price_guard = price;
                self.bot.send_message(msg.chat.id, format!("Alert price set to {price} for {coin}.")).await?
            }
        };

        Ok(())
    }

    pub async fn send_alert(&self, alert: &AlertTable) -> ResponseResult<()> {
        self.bot.send_message(
            teloxide::types::ChatId(alert.chat_id), 
            format!("🔔 Price Alert: {} is at {}", alert.coin, alert.price)
        ).await?;
        Ok(())
    }

    pub fn get_alert_price(&self) -> Arc<Mutex<f64>> {
        self.alert_price.clone()
    }

    pub fn get_bot(&self) -> Bot {
        self.bot.clone()
    }
}
