use crate::db::{Database, AlertTable};
use hyperliquid_rust_sdk::InfoClient;
use teloxide::types::ChatId;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct AlertService {
    db: Database,
    info_client: Arc<Mutex<InfoClient>>,
}

impl AlertService {
    pub fn new(db: Database, info_client: Arc<Mutex<InfoClient>>) -> Self {
        Self { db, info_client }
    }

    pub async fn get_triggered_alerts(&self, mark_px: f64) -> crate::Result<Vec<AlertTable>> {
        let lower_alert_price = mark_px * 0.999;
        let upper_alert_price = mark_px * 1.001;
        self.db.get_triggered_alerts(lower_alert_price, upper_alert_price)
            .await
            .map_err(|e| e.into())
    }

    pub async fn set_alert_cooldowns(&self, alerts: &[AlertTable]) -> crate::Result<()> {
        for alert in alerts {
            self.db.set_alert_cooldown(alert.id).await?;
        }
        Ok(())
    }

    pub async fn reset_cooldowns(&self) -> crate::Result<usize> {
        let result = self.db.reset_cooldowns().await?;
        if result > 0 {
            log::debug!("Cooldown reset for {} alerts", result);
        }
        Ok(result)
    }

    pub async fn get_all_alerts(&self) -> crate::Result<Vec<AlertTable>> {
        self.db.get_all_alerts().await.map_err(|e| e.into())
    }

    pub async fn get_all_alerts_for_chat(&self, chat_id: ChatId) -> crate::Result<Vec<AlertTable>> {
        self.db.get_all_alerts_for_chat(chat_id).await.map_err(|e| e.into())
    }

    pub async fn create_alert(&self, public_key: &str, chat_id: ChatId, coin: &str, price: f64) -> crate::Result<()> {
        let token = self.get_token(coin).await?;
        self.db.insert_alert(public_key, chat_id, coin, &token, price).await.map_err(|e| e.into())
    }

    async fn get_token(&self, coin: &str) -> crate::Result<String> {
        let spot_meta = self.info_client.lock().await.spot_meta().await
            .map_err(|e| crate::AppError::HyperliquidSdk(e.to_string()))?;
        let universe = spot_meta.universe;
        let tokens = spot_meta.tokens;
        let token_index = tokens.iter()
            .find(|t| t.name == coin)
            .ok_or_else(|| crate::AppError::TokenNotFound(format!("Coin '{}' not found", coin)))?
            .index;
        let token = universe.iter()
            .find(|t| t.tokens[0] == token_index)
            .ok_or_else(|| crate::AppError::TokenNotFound(format!("Token for coin '{}' not found in universe", coin)))?
            .name
            .clone();
        Ok(token)
    }
}
