use crate::db::{Database, CronAlert};
use chrono::{DateTime, Utc};
use rusqlite::Result;
use teloxide::types::ChatId;
use std::sync::Arc;
use tokio::sync::Mutex;
use hyperliquid_rust_sdk::InfoClient;

#[derive(Clone)]
pub struct CronService {
    db: Database,
    info_client: Arc<Mutex<InfoClient>>,
}

impl CronService {
    pub fn new(db: Database, info_client: Arc<Mutex<InfoClient>>) -> Self {
        Self { db, info_client }
    }

    pub async fn create_cron_alert(&self, chat_id: ChatId, coin: &str, cron_schedule: &str) -> Result<()> {
        // Insert into database
        let token = self.get_token(coin).await.unwrap();
        self.db.insert_cron_alert(chat_id, coin, &token, cron_schedule).await?;
        Ok(())
    }

    pub async fn get_all_cron_alerts(&self) -> Result<Vec<CronAlert>> {
        self.db.get_all_cron_alerts().await
    }

    pub async fn get_cron_alerts_for_chat(&self, chat_id: ChatId) -> Result<Vec<CronAlert>> {
        self.db.get_cron_alerts_for_chat(chat_id).await
    }

    pub async fn deactivate_cron_alert(&self, alert_id: i64) -> Result<()> {
        self.db.deactivate_cron_alert(alert_id).await
    }

    pub async fn delete_cron_alert(&self, alert_id: i64) -> Result<()> {
        self.db.delete_cron_alert(alert_id).await
    }

    pub async fn get_triggered_cron_alerts(&self) -> Result<Vec<CronAlert>> {
        self.db.get_next_trigger_cron_alerts().await
    }

    pub async fn mark_cron_alert_triggered(&self, alert_id: i64, next_trigger: DateTime<Utc>) -> Result<()> {
        self.db.update_cron_alert_last_triggered(alert_id, next_trigger).await
    }

    pub async fn get_price(&self, token: &str) -> anyhow::Result<f64> {
        let all_mids = self.info_client.lock().await.all_mids().await?;
        let price = all_mids.get(token).unwrap().parse::<f64>().unwrap();

        Ok(price)
    }

    async fn get_token(&self, coin: &str) -> anyhow::Result<String> {
        let spot_meta = self.info_client.lock().await.spot_meta().await?;
        let universe = spot_meta.universe;
        let tokens = spot_meta.tokens;
        let token_index = tokens.iter().find(|t| t.name == coin).unwrap().index;
        let token = universe.iter().find(|t| t.tokens[0] == token_index).unwrap().name.clone();
        println!("Token: {token}");
        Ok(token)
    }
}
