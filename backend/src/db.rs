use chrono::{DateTime, Utc};
use cron_parser::parse;
use rusqlite::{Connection, Result, params};
use std::sync::Arc;
use teloxide::types::ChatId;
use tokio::sync::Mutex;

#[derive(Debug)]
pub struct AlertTable {
    pub id: i64,
    pub public_key: String,
    pub chat_id: i64,
    pub coin: String,
    pub token: String,
    pub price: f64,
    pub alerted: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub cooldown_until: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CronAlert {
    pub id: i64,
    pub chat_id: i64,
    pub coin: String,
    pub token: String,
    pub cron_schedule: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_triggered: Option<DateTime<Utc>>,
    pub next_trigger: Option<DateTime<Utc>>,
}

impl std::fmt::Display for AlertTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "🔔 {} at ${:.2} (created {})",
            self.coin,
            self.price,
            self.created_at.format("%Y-%m-%d %H:%M:%S")
        )
    }
}

impl std::fmt::Display for CronAlert {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let next_trigger_str = self
            .next_trigger
            .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "Not scheduled".to_string());
        write!(
            f,
            "⏰ {} {} (schedule: {}) (created {}) (next trigger: {})",
            self.coin,
            self.token,
            self.cron_schedule,
            self.created_at.format("%Y-%m-%d %H:%M:%S"),
            next_trigger_str
        )
    }
}

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn new(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        Ok(Database {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub async fn initialize(&self) -> Result<()> {
        let conn_guard = self.conn.lock().await;
        conn_guard.execute(
            r#"
        CREATE TABLE IF NOT EXISTS alerts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            public_key TEXT,
            chat_id INTEGER,
            coin VARCHAR(10),
            token VARCHAR(10),
            price REAL,
            alerted BOOLEAN DEFAULT FALSE,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            cooldown_until TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )
        "#,
            (),
        )?;

        conn_guard.execute(
            r#"
        CREATE TABLE IF NOT EXISTS cron_alerts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            chat_id INTEGER,
            coin TEXT,
            token TEXT,
            cron_schedule TEXT,
            is_active BOOLEAN DEFAULT TRUE,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            last_triggered TIMESTAMP,
            next_trigger TIMESTAMP
        )
        "#,
            (),
        )?;
        Ok(())
    }

    pub async fn insert_alert(
        &self,
        public_key: &str,
        chat_id: ChatId,
        coin: &str,
        token: &str,
        price: f64,
    ) -> Result<()> {
        let conn_guard = self.conn.lock().await;
        conn_guard.execute(r#"
        INSERT INTO alerts (public_key, chat_id, coin, token, price, alerted, created_at, updated_at, cooldown_until) 
        VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        "#, (public_key, chat_id.0, coin, token, price, false))?;
        Ok(())
    }

    pub async fn get_all_unique_tokens(&self) -> Result<Vec<String>> {
        let conn_guard = self.conn.lock().await;
        let mut stmt = conn_guard.prepare("SELECT DISTINCT token FROM alerts")?;
        let tokens = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<String>>>()?;
        Ok(tokens)
    }

    pub async fn get_all_alerts(&self) -> Result<Vec<AlertTable>> {
        let conn_guard = self.conn.lock().await;
        let mut stmt = conn_guard.prepare("SELECT * FROM alerts")?;
        let alerts = stmt
            .query_map([], |row| {
                Ok(AlertTable {
                    id: row.get(0)?,
                    public_key: row.get(1)?,
                    chat_id: row.get(2)?,
                    coin: row.get(3)?,
                    token: row.get(4)?,
                    price: row.get(5)?,
                    alerted: row.get(6)?,
                    created_at: row.get::<_, DateTime<Utc>>(7)?,
                    updated_at: row.get::<_, DateTime<Utc>>(8)?,
                    cooldown_until: row
                        .get::<_, DateTime<Utc>>(9)
                        .unwrap_or(DateTime::UNIX_EPOCH),
                })
            })?
            .collect::<Result<Vec<AlertTable>>>()?;
        Ok(alerts)
    }

    pub async fn get_all_alerts_for_chat(&self, chat_id: ChatId) -> Result<Vec<AlertTable>> {
        let conn_guard = self.conn.lock().await;
        let mut stmt = conn_guard.prepare("SELECT * FROM alerts WHERE chat_id = ?")?;
        let alerts = stmt
            .query_map([chat_id.0], |row| {
                Ok(AlertTable {
                    id: row.get(0)?,
                    public_key: row.get(1)?,
                    chat_id: row.get(2)?,
                    coin: row.get(3)?,
                    token: row.get(4)?,
                    price: row.get(5)?,
                    alerted: row.get(6)?,
                    created_at: row.get::<_, DateTime<Utc>>(7)?,
                    updated_at: row.get::<_, DateTime<Utc>>(8)?,
                    cooldown_until: row
                        .get::<_, DateTime<Utc>>(9)
                        .unwrap_or(DateTime::UNIX_EPOCH),
                })
            })?
            .collect::<Result<Vec<AlertTable>>>()?;
        Ok(alerts)
    }

    pub async fn get_triggered_alerts(
        &self,
        lower_price: f64,
        upper_price: f64,
    ) -> Result<Vec<AlertTable>> {
        let conn_guard = self.conn.lock().await;
        let mut stmt = conn_guard
            .prepare("SELECT * FROM alerts WHERE alerted = false AND price BETWEEN ? AND ?")?;
        let alerts = stmt
            .query_map([lower_price, upper_price], |row| {
                Ok(AlertTable {
                    id: row.get(0)?,
                    public_key: row.get(1)?,
                    chat_id: row.get(2)?,
                    coin: row.get(3)?,
                    token: row.get(4)?,
                    price: row.get(5)?,
                    alerted: row.get(6)?,
                    created_at: row.get::<_, DateTime<Utc>>(7)?,
                    updated_at: row.get::<_, DateTime<Utc>>(8)?,
                    cooldown_until: row
                        .get::<_, DateTime<Utc>>(9)
                        .unwrap_or(DateTime::UNIX_EPOCH),
                })
            })?
            .collect::<Result<Vec<AlertTable>>>()?;
        Ok(alerts)
    }

    pub async fn set_alert_cooldown(&self, alert_id: i64) -> Result<()> {
        let conn_guard = self.conn.lock().await;
        let mut stmt = conn_guard.prepare("UPDATE alerts SET alerted = true, cooldown_until = datetime('now', '+1 minute') WHERE id = ?")?;
        stmt.execute([alert_id])?;
        Ok(())
    }

    pub async fn reset_cooldowns(&self) -> Result<usize> {
        let conn_guard = self.conn.lock().await;
        let mut stmt = conn_guard.prepare("UPDATE alerts SET alerted = false, cooldown_until = NULL WHERE cooldown_until < CURRENT_TIMESTAMP")?;
        let result = stmt.execute(())?;
        Ok(result)
    }

    /// Deletes an alert, scoped to the owning chat. Returns `true` if a row was deleted,
    /// `false` if no alert with that id exists for this chat.
    pub async fn delete_alert(&self, chat_id: ChatId, alert_id: i64) -> Result<bool> {
        let conn_guard = self.conn.lock().await;
        let mut stmt = conn_guard.prepare("DELETE FROM alerts WHERE id = ? AND chat_id = ?")?;
        let rows_affected = stmt.execute(params![alert_id, chat_id.0])?;
        Ok(rows_affected > 0)
    }

    pub fn get_connection(&self) -> Arc<Mutex<Connection>> {
        self.conn.clone()
    }

    // Cron alert methods
    pub async fn insert_cron_alert(
        &self,
        chat_id: ChatId,
        coin: &str,
        token: &str,
        cron_schedule: &str,
    ) -> crate::Result<()> {
        let conn_guard = self.conn.lock().await;
        let next_trigger = parse(cron_schedule, &chrono::Utc::now()).map_err(|e| {
            crate::AppError::CronParse(format!("Invalid cron schedule '{}': {}", cron_schedule, e))
        })?;
        conn_guard.execute(r#"
        INSERT INTO cron_alerts (chat_id, coin, token, cron_schedule, is_active, created_at, updated_at, next_trigger)
        VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, ?)
        "#, (chat_id.0, coin, token, cron_schedule, true, next_trigger))?;
        Ok(())
    }

    pub async fn get_all_cron_alerts(&self) -> Result<Vec<CronAlert>> {
        let conn_guard = self.conn.lock().await;
        let mut stmt = conn_guard.prepare("SELECT * FROM cron_alerts WHERE is_active = true")?;
        let alerts = stmt
            .query_map([], |row| {
                Ok(CronAlert {
                    id: row.get(0)?,
                    chat_id: row.get(1)?,
                    coin: row.get(2)?,
                    token: row.get(3)?,
                    cron_schedule: row.get(4)?,
                    is_active: row.get(5)?,
                    created_at: row.get::<_, DateTime<Utc>>(6)?,
                    updated_at: row.get::<_, DateTime<Utc>>(7)?,
                    last_triggered: row.get::<_, Option<DateTime<Utc>>>(8)?,
                    next_trigger: row.get::<_, Option<DateTime<Utc>>>(9)?,
                })
            })?
            .collect::<Result<Vec<CronAlert>>>()?;
        Ok(alerts)
    }

    pub async fn get_cron_alerts_for_chat(&self, chat_id: ChatId) -> Result<Vec<CronAlert>> {
        let conn_guard = self.conn.lock().await;
        let mut stmt = conn_guard
            .prepare("SELECT * FROM cron_alerts WHERE chat_id = ? AND is_active = true")?;
        let alerts = stmt
            .query_map([chat_id.0], |row| {
                Ok(CronAlert {
                    id: row.get(0)?,
                    chat_id: row.get(1)?,
                    coin: row.get(2)?,
                    token: row.get(3)?,
                    cron_schedule: row.get(4)?,
                    is_active: row.get(5)?,
                    created_at: row.get::<_, DateTime<Utc>>(6)?,
                    updated_at: row.get::<_, DateTime<Utc>>(7)?,
                    last_triggered: row.get::<_, Option<DateTime<Utc>>>(8)?,
                    next_trigger: row.get::<_, Option<DateTime<Utc>>>(9)?,
                })
            })?
            .collect::<Result<Vec<CronAlert>>>()?;
        Ok(alerts)
    }

    pub async fn get_next_trigger_cron_alerts(&self) -> Result<Vec<CronAlert>> {
        let conn_guard = self.conn.lock().await;
        let mut stmt = conn_guard
            .prepare("SELECT * FROM cron_alerts WHERE next_trigger <= CURRENT_TIMESTAMP")?;
        let alerts = stmt
            .query_map([], |row| {
                Ok(CronAlert {
                    id: row.get(0)?,
                    chat_id: row.get(1)?,
                    coin: row.get(2)?,
                    token: row.get(3)?,
                    cron_schedule: row.get(4)?,
                    is_active: row.get(5)?,
                    created_at: row.get::<_, DateTime<Utc>>(6)?,
                    updated_at: row.get::<_, DateTime<Utc>>(7)?,
                    last_triggered: row.get::<_, Option<DateTime<Utc>>>(8)?,
                    next_trigger: row.get::<_, Option<DateTime<Utc>>>(9)?,
                })
            })?
            .collect::<Result<Vec<CronAlert>>>()?;
        Ok(alerts)
    }

    pub async fn update_cron_alert_last_triggered(
        &self,
        alert_id: i64,
        next_trigger: DateTime<Utc>,
    ) -> Result<()> {
        let conn_guard = self.conn.lock().await;
        let mut stmt = conn_guard.prepare(
            r#"
            UPDATE cron_alerts SET 
            last_triggered = CURRENT_TIMESTAMP, 
            updated_at = CURRENT_TIMESTAMP, 
            next_trigger = ?
            WHERE id = ?
            "#,
        )?;
        stmt.execute(params![next_trigger, alert_id])?;
        Ok(())
    }

    /// Deactivates a cron alert, scoped to the owning chat. Returns `true` if a row was
    /// updated, `false` if no cron alert with that id exists for this chat.
    pub async fn deactivate_cron_alert(&self, chat_id: ChatId, alert_id: i64) -> Result<bool> {
        let conn_guard = self.conn.lock().await;
        let mut stmt = conn_guard.prepare(
            "UPDATE cron_alerts SET is_active = false, updated_at = CURRENT_TIMESTAMP WHERE id = ? AND chat_id = ?",
        )?;
        let rows_affected = stmt.execute(params![alert_id, chat_id.0])?;
        Ok(rows_affected > 0)
    }

    /// Deletes a cron alert, scoped to the owning chat. Returns `true` if a row was
    /// deleted, `false` if no cron alert with that id exists for this chat.
    pub async fn delete_cron_alert(&self, chat_id: ChatId, alert_id: i64) -> Result<bool> {
        let conn_guard = self.conn.lock().await;
        let mut stmt =
            conn_guard.prepare("DELETE FROM cron_alerts WHERE id = ? AND chat_id = ?")?;
        let rows_affected = stmt.execute(params![alert_id, chat_id.0])?;
        Ok(rows_affected > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    async fn setup_test_db() -> (Database, NamedTempFile) {
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let db = Database::new(temp_file.path().to_str().unwrap())
            .expect("Failed to create test database");
        db.initialize()
            .await
            .expect("Failed to initialize database");
        (db, temp_file)
    }

    #[tokio::test]
    async fn test_database_initialization() {
        let (db, _temp) = setup_test_db().await;

        // Verify tables exist by querying them
        let alerts = db.get_all_alerts().await;
        assert!(alerts.is_ok());
        assert_eq!(alerts.unwrap().len(), 0);

        let cron_alerts = db.get_all_cron_alerts().await;
        assert!(cron_alerts.is_ok());
        assert_eq!(cron_alerts.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_insert_and_retrieve_alert() {
        let (db, _temp) = setup_test_db().await;

        db.insert_alert("0x123", ChatId(12345), "HYPE", "@1", 25.0)
            .await
            .expect("Insert should succeed");

        let alerts = db
            .get_all_alerts_for_chat(ChatId(12345))
            .await
            .expect("Query should succeed");

        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].coin, "HYPE");
        assert_eq!(alerts[0].token, "@1");
        assert_eq!(alerts[0].price, 25.0);
        assert_eq!(alerts[0].chat_id, 12345);
        assert!(!alerts[0].alerted);
    }

    #[tokio::test]
    async fn test_get_unique_tokens() {
        let (db, _temp) = setup_test_db().await;

        db.insert_alert("0x123", ChatId(12345), "HYPE", "@1", 25.0)
            .await
            .unwrap();
        db.insert_alert("0x123", ChatId(12345), "SOL", "@2", 150.0)
            .await
            .unwrap();
        db.insert_alert("0x123", ChatId(12345), "HYPE", "@1", 30.0)
            .await
            .unwrap(); // Same token

        let tokens = db.get_all_unique_tokens().await.unwrap();

        assert_eq!(tokens.len(), 2);
        assert!(tokens.contains(&"@1".to_string()));
        assert!(tokens.contains(&"@2".to_string()));
    }

    #[tokio::test]
    async fn test_triggered_alerts_within_price_range() {
        let (db, _temp) = setup_test_db().await;

        // Insert alert at price 100.0
        db.insert_alert("0x123", ChatId(12345), "BTC", "@1", 100.0)
            .await
            .unwrap();

        // Test within range (should trigger)
        let triggered = db.get_triggered_alerts(99.9, 100.1).await.unwrap();
        assert_eq!(triggered.len(), 1);

        // Test outside range (should not trigger)
        let not_triggered = db.get_triggered_alerts(95.0, 99.0).await.unwrap();
        assert_eq!(not_triggered.len(), 0);
    }

    #[tokio::test]
    async fn test_cooldown_mechanism() {
        let (db, _temp) = setup_test_db().await;

        db.insert_alert("0x123", ChatId(12345), "ETH", "@1", 50.0)
            .await
            .unwrap();

        let alerts = db.get_all_alerts().await.unwrap();
        let alert_id = alerts[0].id;

        // Set cooldown
        db.set_alert_cooldown(alert_id).await.unwrap();

        // Alert should not trigger during cooldown (alerted = true)
        let triggered = db.get_triggered_alerts(49.9, 50.1).await.unwrap();
        assert_eq!(triggered.len(), 0);

        // Verify alert is marked as alerted
        let alerts_after = db.get_all_alerts().await.unwrap();
        assert!(alerts_after[0].alerted);
    }

    #[tokio::test]
    async fn test_insert_and_retrieve_cron_alert() {
        let (db, _temp) = setup_test_db().await;

        db.insert_cron_alert(ChatId(12345), "HYPE", "@1", "0 8 * * *")
            .await
            .expect("Insert should succeed");

        let cron_alerts = db
            .get_cron_alerts_for_chat(ChatId(12345))
            .await
            .expect("Query should succeed");

        assert_eq!(cron_alerts.len(), 1);
        assert_eq!(cron_alerts[0].coin, "HYPE");
        assert_eq!(cron_alerts[0].token, "@1");
        assert_eq!(cron_alerts[0].cron_schedule, "0 8 * * *");
        assert!(cron_alerts[0].is_active);
        assert!(cron_alerts[0].next_trigger.is_some());
    }

    #[tokio::test]
    async fn test_delete_cron_alert() {
        let (db, _temp) = setup_test_db().await;

        db.insert_cron_alert(ChatId(12345), "HYPE", "@1", "0 8 * * *")
            .await
            .unwrap();

        let alerts_before = db.get_all_cron_alerts().await.unwrap();
        assert_eq!(alerts_before.len(), 1);
        let alert_id = alerts_before[0].id;

        let deleted = db.delete_cron_alert(ChatId(12345), alert_id).await.unwrap();
        assert!(deleted);

        let alerts_after = db.get_all_cron_alerts().await.unwrap();
        assert_eq!(alerts_after.len(), 0);
    }

    #[tokio::test]
    async fn test_delete_cron_alert_wrong_chat_is_noop() {
        let (db, _temp) = setup_test_db().await;

        db.insert_cron_alert(ChatId(12345), "HYPE", "@1", "0 8 * * *")
            .await
            .unwrap();

        let alerts_before = db.get_all_cron_alerts().await.unwrap();
        let alert_id = alerts_before[0].id;

        // A different chat should not be able to delete this alert.
        let deleted = db.delete_cron_alert(ChatId(99999), alert_id).await.unwrap();
        assert!(!deleted);

        let alerts_after = db.get_all_cron_alerts().await.unwrap();
        assert_eq!(alerts_after.len(), 1);
    }

    #[tokio::test]
    async fn test_deactivate_cron_alert() {
        let (db, _temp) = setup_test_db().await;

        db.insert_cron_alert(ChatId(12345), "HYPE", "@1", "0 8 * * *")
            .await
            .unwrap();

        let alerts = db.get_all_cron_alerts().await.unwrap();
        let alert_id = alerts[0].id;

        let deactivated = db
            .deactivate_cron_alert(ChatId(12345), alert_id)
            .await
            .unwrap();
        assert!(deactivated);

        // get_all_cron_alerts only returns active alerts
        let active_alerts = db.get_all_cron_alerts().await.unwrap();
        assert_eq!(active_alerts.len(), 0);
    }

    #[tokio::test]
    async fn test_deactivate_cron_alert_wrong_chat_is_noop() {
        let (db, _temp) = setup_test_db().await;

        db.insert_cron_alert(ChatId(12345), "HYPE", "@1", "0 8 * * *")
            .await
            .unwrap();

        let alerts = db.get_all_cron_alerts().await.unwrap();
        let alert_id = alerts[0].id;

        let deactivated = db
            .deactivate_cron_alert(ChatId(99999), alert_id)
            .await
            .unwrap();
        assert!(!deactivated);

        let active_alerts = db.get_all_cron_alerts().await.unwrap();
        assert_eq!(active_alerts.len(), 1);
    }

    #[tokio::test]
    async fn test_alert_table_display() {
        let alert = AlertTable {
            id: 1,
            public_key: "0x123".to_string(),
            chat_id: 12345,
            coin: "HYPE".to_string(),
            token: "@1".to_string(),
            price: 25.50,
            alerted: false,
            created_at: DateTime::from_timestamp(1704067200, 0).unwrap(), // 2024-01-01 00:00:00
            updated_at: DateTime::from_timestamp(1704067200, 0).unwrap(),
            cooldown_until: DateTime::from_timestamp(0, 0).unwrap(),
        };

        let display = format!("{}", alert);
        assert!(display.contains("HYPE"));
        assert!(display.contains("$25.50"));
    }

    #[tokio::test]
    async fn test_multiple_alerts_for_different_chats() {
        let (db, _temp) = setup_test_db().await;

        db.insert_alert("0x123", ChatId(11111), "HYPE", "@1", 25.0)
            .await
            .unwrap();
        db.insert_alert("0x123", ChatId(22222), "SOL", "@2", 150.0)
            .await
            .unwrap();
        db.insert_alert("0x123", ChatId(11111), "BTC", "@3", 50000.0)
            .await
            .unwrap();

        let chat1_alerts = db.get_all_alerts_for_chat(ChatId(11111)).await.unwrap();
        let chat2_alerts = db.get_all_alerts_for_chat(ChatId(22222)).await.unwrap();

        assert_eq!(chat1_alerts.len(), 2);
        assert_eq!(chat2_alerts.len(), 1);
    }

    #[tokio::test]
    async fn test_delete_alert_scoped_to_chat() {
        let (db, _temp) = setup_test_db().await;

        db.insert_alert("0x123", ChatId(11111), "HYPE", "@1", 25.0)
            .await
            .unwrap();

        let alerts = db.get_all_alerts_for_chat(ChatId(11111)).await.unwrap();
        let alert_id = alerts[0].id;

        // A different chat cannot delete this alert.
        let deleted = db.delete_alert(ChatId(22222), alert_id).await.unwrap();
        assert!(!deleted);
        assert_eq!(
            db.get_all_alerts_for_chat(ChatId(11111))
                .await
                .unwrap()
                .len(),
            1
        );

        // The owning chat can.
        let deleted = db.delete_alert(ChatId(11111), alert_id).await.unwrap();
        assert!(deleted);
        assert_eq!(
            db.get_all_alerts_for_chat(ChatId(11111))
                .await
                .unwrap()
                .len(),
            0
        );
    }
}
