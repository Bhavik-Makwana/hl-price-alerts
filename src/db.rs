use rusqlite::{Connection, Result};
use chrono::{DateTime, Utc};
use std::sync::Arc;
use tokio::sync::Mutex;
use teloxide::types::ChatId;

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
        conn_guard.execute(r#"
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
        "#, ())?;
        Ok(())
    }

    pub async fn insert_alert(&self, public_key: &str, chat_id: ChatId, coin: &str, token: &str, price: f64) -> Result<()> {
        let conn_guard = self.conn.lock().await;
        conn_guard.execute(r#"
        INSERT INTO alerts (public_key, chat_id, coin, token, price, alerted, created_at, updated_at, cooldown_until) 
        VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        "#, (public_key, chat_id.0, coin, token, price, false))?;
        Ok(())
    }

    pub async fn get_all_alerts(&self) -> Result<Vec<AlertTable>> {
        let conn_guard = self.conn.lock().await;
        let mut stmt = conn_guard.prepare("SELECT * FROM alerts")?;
        let alerts = stmt.query_map([], |row| {
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
                cooldown_until: row.get::<_, DateTime<Utc>>(9).unwrap_or(DateTime::<Utc>::from_timestamp(0, 0).unwrap()),
            })
        })?.collect::<Result<Vec<AlertTable>>>()?;
        Ok(alerts)
    }

    pub async fn get_all_alerts_for_chat(&self, chat_id: ChatId) -> Result<Vec<AlertTable>> {
        let conn_guard = self.conn.lock().await;
        let mut stmt = conn_guard.prepare("SELECT * FROM alerts WHERE chat_id = ?")?;
        let alerts = stmt.query_map([chat_id.0], |row| {
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
                cooldown_until: row.get::<_, DateTime<Utc>>(9).unwrap_or(DateTime::<Utc>::from_timestamp(0, 0).unwrap()),
            })
        })?.collect::<Result<Vec<AlertTable>>>()?;
        Ok(alerts)
    }

    pub async fn get_triggered_alerts(&self, lower_price: f64, upper_price: f64) -> Result<Vec<AlertTable>> {
        let conn_guard = self.conn.lock().await;
        let mut stmt = conn_guard.prepare("SELECT * FROM alerts WHERE alerted = false AND price BETWEEN ? AND ?")?;
        let alerts = stmt.query_map([lower_price, upper_price], |row| {
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
                cooldown_until: row.get::<_, DateTime<Utc>>(9).unwrap_or(DateTime::<Utc>::from_timestamp(0, 0).unwrap()),
            })
        })?.collect::<Result<Vec<AlertTable>>>()?;
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

    pub fn get_connection(&self) -> Arc<Mutex<Connection>> {
        self.conn.clone()
    }
}
