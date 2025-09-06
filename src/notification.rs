use teloxide::{prelude::*, utils::command::BotCommands};
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::db::AlertTable;
use crate::alerts::AlertService;

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "These commands are supported:")]
pub enum Command {
    #[command(description = "display this text.")]
    Help,
    #[command(description = "Display all alerts.")]
    Alert,
    #[command(parse_with = "split", alias = "ua", hide_aliases)]
    SetAlert{coin: String, price: f64},
}

#[derive(Clone)]
pub struct NotificationService {
    alert_service: AlertService,
}

impl NotificationService {
    pub fn new(alert_service: AlertService) -> Self {
        Self {
            alert_service,
        }
    }

    pub async fn handle_command(&self, bot: Bot, msg: teloxide::types::Message, cmd: Command) -> ResponseResult<()> {
        match cmd {
            Command::Help => {
                bot.send_message(msg.chat.id, Command::descriptions().to_string()).await?
            }
            Command::Alert => {
                // let mut alert_price_guard = self.alert_price.lock().await;
                // *alert_price_guard = price;
                let alerts = self.alert_service.get_all_alerts_for_chat(msg.chat.id).await.unwrap();
                let alerts_buffer = alerts.iter().map(|alert| alert.to_string()).collect::<Vec<String>>().join("\n");
                bot.send_message(msg.chat.id, format!("Alerts: {alerts_buffer}")).await?
            }
            Command::SetAlert{coin, price} => {
                self.alert_service.create_alert("0x00",msg.chat.id, &coin, price).await.unwrap();
                bot.send_message(msg.chat.id, format!("Alert price set to {price} for {coin}.")).await?
            }
        };

        Ok(())
    }

    pub async fn send_alert(&self, bot: Bot, alert: &AlertTable) -> ResponseResult<()> {
        bot.send_message(
            teloxide::types::ChatId(alert.chat_id), 
            format!("🔔 Price Alert: {} is at {}", alert.coin, alert.price)
        ).await?;
        Ok(())
    }

}
