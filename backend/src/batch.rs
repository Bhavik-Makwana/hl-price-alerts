use crate::db::CronAlert;
use std::collections::HashMap;
use teloxide::types::ChatId;

/// Groups cron alerts by chat_id for batch sending within the same minute
pub struct AlertBatcher;

/// A batch of alerts to send to a single chat
#[derive(Debug, Clone)]
pub struct BatchedAlert {
    pub chat_id: ChatId,
    /// List of (coin, price) pairs to include in the message
    pub alerts: Vec<(String, f64)>,
}

impl AlertBatcher {
    /// Groups alerts by chat_id for batched sending
    ///
    /// All alerts triggered within the same minute should be passed to this function
    /// to be grouped together.
    pub fn batch_alerts(alerts: Vec<(CronAlert, f64)>) -> Vec<BatchedAlert> {
        let mut grouped: HashMap<i64, Vec<(String, f64)>> = HashMap::new();

        for (alert, price) in alerts {
            grouped
                .entry(alert.chat_id)
                .or_default()
                .push((alert.coin.clone(), price));
        }

        grouped
            .into_iter()
            .map(|(chat_id, alerts)| BatchedAlert {
                chat_id: ChatId(chat_id),
                alerts,
            })
            .collect()
    }

    /// Format a batched alert message for sending
    pub fn format_message(batched: &BatchedAlert) -> String {
        if batched.alerts.len() == 1 {
            let (coin, price) = &batched.alerts[0];
            format!("⏰ {}: ${:.2}", coin, price)
        } else {
            let mut msg = String::from("⏰ Scheduled Alert Update:\n\n");
            for (coin, price) in &batched.alerts {
                msg.push_str(&format!("• {}: ${:.2}\n", coin, price));
            }
            msg.trim_end().to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_cron_alert(chat_id: i64, coin: &str) -> CronAlert {
        CronAlert {
            id: 1,
            chat_id,
            coin: coin.to_string(),
            token: format!("@{}", coin),
            cron_schedule: "0 8 * * *".to_string(),
            is_active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_triggered: None,
            next_trigger: Some(Utc::now()),
        }
    }

    #[test]
    fn test_batch_single_alert() {
        let alerts = vec![(make_cron_alert(12345, "HYPE"), 25.0)];

        let batched = AlertBatcher::batch_alerts(alerts);

        assert_eq!(batched.len(), 1);
        assert_eq!(batched[0].chat_id, ChatId(12345));
        assert_eq!(batched[0].alerts.len(), 1);
        assert_eq!(batched[0].alerts[0].0, "HYPE");
        assert_eq!(batched[0].alerts[0].1, 25.0);
    }

    #[test]
    fn test_batch_multiple_alerts_same_user() {
        let alerts = vec![
            (make_cron_alert(12345, "HYPE"), 25.0),
            (make_cron_alert(12345, "SOL"), 150.0),
            (make_cron_alert(12345, "BTC"), 50000.0),
        ];

        let batched = AlertBatcher::batch_alerts(alerts);

        assert_eq!(batched.len(), 1);
        assert_eq!(batched[0].alerts.len(), 3);
    }

    #[test]
    fn test_batch_multiple_users() {
        let alerts = vec![
            (make_cron_alert(12345, "HYPE"), 25.0),
            (make_cron_alert(67890, "SOL"), 150.0),
            (make_cron_alert(12345, "BTC"), 50000.0),
        ];

        let batched = AlertBatcher::batch_alerts(alerts);

        assert_eq!(batched.len(), 2);

        // Find each user's batch
        let user1_batch = batched.iter().find(|b| b.chat_id == ChatId(12345)).unwrap();
        let user2_batch = batched.iter().find(|b| b.chat_id == ChatId(67890)).unwrap();

        assert_eq!(user1_batch.alerts.len(), 2);
        assert_eq!(user2_batch.alerts.len(), 1);
    }

    #[test]
    fn test_format_single_alert() {
        let batched = BatchedAlert {
            chat_id: ChatId(12345),
            alerts: vec![("HYPE".to_string(), 25.0)],
        };

        let msg = AlertBatcher::format_message(&batched);
        assert_eq!(msg, "⏰ HYPE: $25.00");
    }

    #[test]
    fn test_format_multiple_alerts() {
        let batched = BatchedAlert {
            chat_id: ChatId(12345),
            alerts: vec![("HYPE".to_string(), 25.0), ("SOL".to_string(), 150.0)],
        };

        let msg = AlertBatcher::format_message(&batched);
        assert!(msg.contains("Scheduled Alert Update"));
        assert!(msg.contains("• HYPE: $25.00"));
        assert!(msg.contains("• SOL: $150.00"));
    }

    #[test]
    fn test_format_three_alerts() {
        let batched = BatchedAlert {
            chat_id: ChatId(12345),
            alerts: vec![
                ("HYPE".to_string(), 25.0),
                ("SOL".to_string(), 150.0),
                ("BTC".to_string(), 50000.0),
            ],
        };

        let msg = AlertBatcher::format_message(&batched);
        assert!(msg.contains("Scheduled Alert Update"));
        assert!(msg.contains("• HYPE: $25.00"));
        assert!(msg.contains("• SOL: $150.00"));
        assert!(msg.contains("• BTC: $50000.00"));
    }

    #[test]
    fn test_batch_empty_list() {
        let alerts: Vec<(CronAlert, f64)> = vec![];
        let batched = AlertBatcher::batch_alerts(alerts);
        assert!(batched.is_empty());
    }

    #[test]
    fn test_format_preserves_precision() {
        let batched = BatchedAlert {
            chat_id: ChatId(12345),
            alerts: vec![("HYPE".to_string(), 25.123456)],
        };

        let msg = AlertBatcher::format_message(&batched);
        assert_eq!(msg, "⏰ HYPE: $25.12");
    }
}
