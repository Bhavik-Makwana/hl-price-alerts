use rusqlite::{Connection, Result, params};
use chrono::{DateTime, Utc};
use std::sync::Arc;
use tokio::sync::Mutex;
use teloxide::types::ChatId;
use cron_parser::parse;

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

#[derive(Debug)]
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
        write!(
            f,
            "⏰ {} {} (schedule: {}) (created {}) (next trigger: {})",
            self.coin,
            self.token,
            self.cron_schedule,
            self.created_at.format("%Y-%m-%d %H:%M:%S"),
            self.next_trigger.unwrap().format("%Y-%m-%d %H:%M:%S")
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
        
        conn_guard.execute(r#"
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

    pub async fn get_all_unique_tokens(&self) -> Result<Vec<String>> {
        let conn_guard = self.conn.lock().await;
        let mut stmt = conn_guard.prepare("SELECT DISTINCT token FROM alerts")?;
        let tokens = stmt.query_map([], |row| {
            Ok(row.get(0)?)
        })?.collect::<Result<Vec<String>>>()?;
        Ok(tokens)
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

    // Cron alert methods
    pub async fn insert_cron_alert(&self, chat_id: ChatId, coin: &str, token: &str, cron_schedule: &str) -> Result<()> {
        let conn_guard = self.conn.lock().await;
        let next_trigger = parse(cron_schedule, &chrono::Utc::now()).unwrap();
        conn_guard.execute(r#"
        INSERT INTO cron_alerts (chat_id, coin, token, cron_schedule, is_active, created_at, updated_at, next_trigger) 
        VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, ?)
        "#, (chat_id.0, coin, token, cron_schedule, true, next_trigger))?;
        Ok(())
    }

    pub async fn get_all_cron_alerts(&self) -> Result<Vec<CronAlert>> {
        let conn_guard = self.conn.lock().await;
        let mut stmt = conn_guard.prepare("SELECT * FROM cron_alerts WHERE is_active = true")?;
        let alerts = stmt.query_map([], |row| {
            Ok(CronAlert {
                id: row.get(0)?,
                chat_id: row.get(1)?,
                coin: row.get(2)?,
                token: row.get(3)?  ,
                cron_schedule: row.get(4)?,
                is_active: row.get(5)?,
                created_at: row.get::<_, DateTime<Utc>>(6)?,
                updated_at: row.get::<_, DateTime<Utc>>(7)?,
                last_triggered: row.get::<_, Option<DateTime<Utc>>>(8)?,
                next_trigger: row.get::<_, Option<DateTime<Utc>>>(9)?,
            })
        })?.collect::<Result<Vec<CronAlert>>>()?;
        Ok(alerts)
    }

    pub async fn get_cron_alerts_for_chat(&self, chat_id: ChatId) -> Result<Vec<CronAlert>> {
        let conn_guard = self.conn.lock().await;
        let mut stmt = conn_guard.prepare("SELECT * FROM cron_alerts WHERE chat_id = ? AND is_active = true")?;
        let alerts = stmt.query_map([chat_id.0], |row| {
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
        })?.collect::<Result<Vec<CronAlert>>>()?;
        Ok(alerts)
    }

    pub async fn get_next_trigger_cron_alerts(&self) -> Result<Vec<CronAlert>> {
        let conn_guard = self.conn.lock().await;
        let mut stmt = conn_guard.prepare("SELECT * FROM cron_alerts WHERE next_trigger <= CURRENT_TIMESTAMP")?;
        let alerts = stmt.query_map([], |row| {
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
        })?.collect::<Result<Vec<CronAlert>>>()?;
        Ok(alerts)
    }

    pub async fn update_cron_alert_last_triggered(&self, alert_id: i64, next_trigger: DateTime<Utc>) -> Result<()> {
        let conn_guard = self.conn.lock().await;
        let mut stmt = conn_guard.prepare(r#"
            UPDATE cron_alerts SET 
            last_triggered = CURRENT_TIMESTAMP, 
            updated_at = CURRENT_TIMESTAMP, 
            next_trigger = ?
            WHERE id = ?
            "#)?;
        stmt.execute(params![next_trigger, alert_id])?;
        Ok(())
    }

    pub async fn deactivate_cron_alert(&self, alert_id: i64) -> Result<()> {
        let conn_guard = self.conn.lock().await;
        let mut stmt = conn_guard.prepare("UPDATE cron_alerts SET is_active = false, updated_at = CURRENT_TIMESTAMP WHERE id = ?")?;
        stmt.execute([alert_id])?;
        Ok(())
    }

    pub async fn delete_cron_alert(&self, alert_id: i64) -> Result<()> {
        let conn_guard = self.conn.lock().await;
        let mut stmt = conn_guard.prepare("DELETE FROM cron_alerts WHERE id = ?")?;
        stmt.execute([alert_id])?;
        Ok(())
    }
}
