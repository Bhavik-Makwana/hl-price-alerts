use crate::db::{Database, AlertTable};
use hyperliquid_rust_sdk::{BaseUrl, InfoClient};
use rusqlite::Result;
use teloxide::types::ChatId;

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

    pub async fn get_all_alerts_for_chat(&self, chat_id: ChatId) -> Result<Vec<AlertTable>> {
        self.db.get_all_alerts_for_chat(chat_id).await
    }

    pub async fn create_alert(&self, public_key: &str, chat_id: ChatId, coin: &str, price: f64) -> Result<()> {
        let token = self.get_token(coin).await.unwrap();

        self.db.insert_alert(public_key, chat_id, coin, &token, price).await
    }

    async fn get_token(&self, coin: &str) -> anyhow::Result<String> {
        let info_client = InfoClient::new(None, Some(BaseUrl::Mainnet)).await.unwrap();
        let spot_meta = info_client.spot_meta().await?;
        let universe = spot_meta.universe;
        let tokens = spot_meta.tokens;
        let token_index = tokens.iter().find(|t| t.name == coin).unwrap().index;
        let token = universe.iter().find(|t| t.index == token_index).unwrap().name.clone();
        println!("Token: {token}");
        Ok(token)
    }
}
