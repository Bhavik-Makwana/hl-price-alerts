use crate::db::{AlertTable, Database};
use hyperliquid_rust_sdk::{InfoClient, Message, Subscription};
use std::collections::HashMap;
use std::sync::Arc;
use teloxide::types::ChatId;
use tokio::sync::Mutex;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Clone)]
pub struct AlertService {
    db: Database,
    info_client: Arc<Mutex<InfoClient>>,
    price_sender: UnboundedSender<Message>,
    /// Tokens with an active price-feed subscription, mapped to their subscription id.
    subscriptions: Arc<Mutex<HashMap<String, u32>>>,
}

impl AlertService {
    pub fn new(
        db: Database,
        info_client: Arc<Mutex<InfoClient>>,
        price_sender: UnboundedSender<Message>,
    ) -> Self {
        Self {
            db,
            info_client,
            price_sender,
            subscriptions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Ensures the given token has an active price-feed subscription, subscribing if
    /// this is the first alert for it. Without this, alerts for a coin that had no
    /// prior subscription would sit in the DB but never receive price updates until
    /// the process restarted and re-subscribed from scratch.
    pub async fn ensure_subscribed(&self, token: &str) {
        {
            let subscriptions = self.subscriptions.lock().await;
            if subscriptions.contains_key(token) {
                return;
            }
        }

        let result = self
            .info_client
            .lock()
            .await
            .subscribe(
                Subscription::ActiveAssetCtx {
                    coin: token.to_string(),
                },
                self.price_sender.clone(),
            )
            .await;

        match result {
            Ok(subscription_id) => {
                self.subscriptions
                    .lock()
                    .await
                    .insert(token.to_string(), subscription_id);
                log::debug!("Subscribed to price feed for {}", token);
            }
            Err(e) => {
                log::error!("Failed to subscribe to price feed for {}: {}", token, e);
            }
        }
    }

    /// Ids of every price-feed subscription currently held, for cleanup on shutdown.
    pub async fn subscription_ids(&self) -> Vec<u32> {
        self.subscriptions.lock().await.values().copied().collect()
    }

    pub async fn get_triggered_alerts(&self, mark_px: f64) -> crate::Result<Vec<AlertTable>> {
        let lower_alert_price = mark_px * 0.999;
        let upper_alert_price = mark_px * 1.001;
        self.db
            .get_triggered_alerts(lower_alert_price, upper_alert_price)
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
        self.db
            .get_all_alerts_for_chat(chat_id)
            .await
            .map_err(|e| e.into())
    }

    pub async fn create_alert(
        &self,
        public_key: &str,
        chat_id: ChatId,
        coin: &str,
        price: f64,
    ) -> crate::Result<()> {
        let token = self.get_token(coin).await?;
        self.db
            .insert_alert(public_key, chat_id, coin, &token, price)
            .await?;
        self.ensure_subscribed(&token).await;
        Ok(())
    }

    pub async fn delete_alert(&self, alert_id: i64) -> crate::Result<()> {
        self.db.delete_alert(alert_id).await.map_err(|e| e.into())
    }

    /// Validate that a coin exists in the Hyperliquid spot market
    /// Returns the token name if valid, or TokenNotFound error if not
    pub async fn validate_coin(&self, coin: &str) -> crate::Result<String> {
        self.get_token(coin).await
    }

    async fn get_token(&self, coin: &str) -> crate::Result<String> {
        let spot_meta = self
            .info_client
            .lock()
            .await
            .spot_meta()
            .await
            .map_err(|e| crate::AppError::HyperliquidSdk(e.to_string()))?;
        let universe = spot_meta.universe;
        let tokens = spot_meta.tokens;
        let token_index = tokens
            .iter()
            .find(|t| t.name == coin)
            .ok_or_else(|| crate::AppError::TokenNotFound(format!("Coin '{}' not found", coin)))?
            .index;
        let token = universe
            .iter()
            .find(|t| t.tokens[0] == token_index)
            .ok_or_else(|| {
                crate::AppError::TokenNotFound(format!(
                    "Token for coin '{}' not found in universe",
                    coin
                ))
            })?
            .name
            .clone();
        Ok(token)
    }
}
