use crate::db::{Database, AlertTable};
use rusqlite::Result;

#[derive(Clone)]
pub struct AlertService {
    db: Database,
}

impl AlertService {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get_triggered_alerts(&self, mark_px: f64) -> Result<Vec<AlertTable>> {
        let lower_alert_price = mark_px * 0.95;
        let upper_alert_price = mark_px * 1.05;
        self.db.get_triggered_alerts(lower_alert_price, upper_alert_price).await
    }

    pub async fn set_alert_cooldowns(&self, alerts: &[AlertTable]) -> Result<()> {
        for alert in alerts {
            self.db.set_alert_cooldown(alert.id).await?;
        }
        println!("Cooldown set for {} alerts", alerts.len());
        Ok(())
    }

    pub async fn reset_cooldowns(&self) -> Result<usize> {
        let result = self.db.reset_cooldowns().await?;
        if result > 0 {
            println!("Cooldown reset for {result} alerts");
        }
        Ok(result)
    }

    pub async fn get_all_alerts(&self) -> Result<Vec<AlertTable>> {
        self.db.get_all_alerts().await
    }
}

