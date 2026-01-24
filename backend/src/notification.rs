use teloxide::{prelude::*, utils::command::BotCommands};
use crate::db::AlertTable;
use crate::alerts::AlertService;
use crate::cron::CronService;

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "These commands are supported:")]
pub enum Command {
    #[command(description = "display this text.")]
    Help,
    #[command(description = "Display all alerts.")]
    Alert,
    #[command(parse_with = "split", alias = "ua", hide_aliases)]
    SetAlert{coin: String, price: f64},
    #[command(description = "Display all cron alerts.")]
    CronAlerts,
    #[command(parse_with = "split", description = "Create a cron alert at a specific time.")]
    SetCronAlert{coin: String, schedule: String, time: String},
    #[command(parse_with = "split", description = "Delete a cron alert by ID.")]
    DeleteCronAlert{id: i64},
}

#[derive(Clone)]
pub struct NotificationService {
    alert_service: AlertService,
    cron_service: CronService,
}

impl NotificationService {
    pub fn new(alert_service: AlertService, cron_service: CronService) -> Self {
        Self {
            alert_service,
            cron_service,
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
            Command::CronAlerts => {
                let cron_alerts = self.cron_service.get_cron_alerts_for_chat(msg.chat.id).await.unwrap();
                let alerts_buffer = cron_alerts.iter().map(|alert| alert.to_string()).collect::<Vec<String>>().join("\n");
                bot.send_message(msg.chat.id, format!("Cron Alerts:\n{alerts_buffer}")).await?
            }
            Command::SetCronAlert{coin, schedule, time} => {
                let cron_schedule = self.cron_service.create_schedule(&schedule, &time).await.unwrap();
                if let Ok(_) = cron_parser::parse(&cron_schedule, &chrono::Utc::now()) {
                    println!("Cron alert set with schedule {cron_schedule} for {coin}.");
                    self.cron_service.create_cron_alert(msg.chat.id, &coin, &cron_schedule).await.unwrap();
                    bot.send_message(msg.chat.id, format!("Cron alert set with schedule {cron_schedule} for {coin}.")).await?
                } else {
                    println!("Invalid schedule: {schedule}");
                    bot.send_message(msg.chat.id, format!("Invalid schedule: {schedule}")).await?;
                    return Ok(());
                }
                
            }
            Command::DeleteCronAlert{id} => {
                self.cron_service.delete_cron_alert(id).await.unwrap();
                bot.send_message(msg.chat.id, format!("Cron alert {id} deleted.")).await?
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
