use rusqlite::{Connection, Result};
use teloxide::{prelude::*, utils::command::BotCommands};
use std::sync::Arc;
use tokio::sync::Mutex;

use hyperliquid_rust_sdk::{BaseUrl, InfoClient, Subscription};
use log::info;
use tokio::{sync::mpsc::unbounded_channel};
use chrono::{DateTime, Utc};
const ALERT_PRICE_COIN: &str = "@107"; // spot index for hype token

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "These commands are supported:")]
enum Command {
    #[command(description = "display this text.")]
    Help,
    #[command(description = "Set alert price.")]
    Alert(f64),
    #[command(parse_with = "split", alias = "ua", hide_aliases)]
    SetAlert{coin: String, price: f64},
}

#[derive(Debug)]
struct AlertTable {
    id: i64,
    _public_key: String,
    chat_id: i64,
    coin: String,
    token: String,
    price: f64,
    alerted: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    cooldown_until: DateTime<Utc>,
}

async fn update_price(bot: Bot, msg: teloxide::types::Message, cmd: Command, alert_price: Arc<Mutex<f64>>) -> ResponseResult<()> {
    match cmd {
        Command::Help => bot.send_message(msg.chat.id, Command::descriptions().to_string()).await?,
        Command::Alert(price) => {
            let mut alert_price_guard = alert_price.lock().await;
            *alert_price_guard = price;
            bot.send_message(msg.chat.id, format!("Alert price set to {price}.")).await?
        }
        Command::SetAlert{coin, price} => {
            let mut alert_price_guard = alert_price.lock().await;
            *alert_price_guard = price;
            bot.send_message(msg.chat.id, format!("Alert price set to {price} for {coin}.")).await?
        }
    };

    Ok(())
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    dotenv::dotenv().ok();
    let alert_price_chat_id: i64 = std::env::var("ALERT_PRICE_CHAT_ID").unwrap().parse::<i64>().unwrap();

    log::info!("Starting Alert Price Bot...");

    let conn = Arc::new(Mutex::new(Connection::open("alerts.db").unwrap()));
    let conn_clone = conn.clone();
    {
        let conn_guard = conn.lock().await;
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
        "#, ()).unwrap();
        conn_guard.execute(r#"
        INSERT INTO alerts (public_key, chat_id, coin, token, price, alerted, created_at, updated_at, cooldown_until) VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        "#, ("0x00", alert_price_chat_id, ALERT_PRICE_COIN, ALERT_PRICE_COIN, 46.6, false)).unwrap();
        let mut stmt = conn_guard.prepare("SELECT * FROM alerts").unwrap();
        let alerts = stmt.query_map(
            [], |row| {
                Ok(AlertTable {
                    id: row.get(0)?,
                    _public_key: row.get(1)?,
                    chat_id: row.get(2)?,
                    coin: row.get(3)?,
                    token: row.get(4)?,
                    price: row.get(5)?,
                    alerted: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                    cooldown_until: row.get(9)?,
                })
            }
        ).unwrap().collect::<Result<Vec<AlertTable>>>();
        println!("Alerts: {alerts:?}");
    }


    let bot = Bot::from_env();

    let alert_price = Arc::new(Mutex::new(46.6));

    let mut info_client = InfoClient::new(None, Some(BaseUrl::Mainnet)).await.unwrap();

    let (sender, mut receiver) = unbounded_channel();
    let subscription_id = info_client
        .subscribe(
            Subscription::ActiveAssetCtx {
                coin: ALERT_PRICE_COIN.to_string(), //spot index for hype token
            },
            sender,
        )
        .await
        .unwrap();

    let alert_price_clone = alert_price.clone();

    tokio::select! {
        _ = Command::repl(bot.clone(), move |bot, msg, cmd| {
            let alert_price = alert_price_clone.clone();

            update_price(bot, msg, cmd, alert_price)
        }) => {
            info!("Telegram bot stopped, unsubscribing from price updates");
            info_client.unsubscribe(subscription_id).await.unwrap();
        }
        _ = async move {
            while let Some(hyperliquid_rust_sdk::Message::ActiveSpotAssetCtx(order_updates)) = receiver.recv().await {
                info!("Received order update data: {order_updates:?}");
                let mark_px = order_updates.data.ctx.shared.mark_px.parse::<f64>().unwrap();
                let alerts = alerts_triggered(mark_px, &conn).await.unwrap();
                for alert in &alerts {
                    println!("Alert triggered: {alert:?}");
                    
                    bot.send_message(teloxide::types::ChatId(alert.chat_id), format!("🔔 Price Alert: {} is at {}", alert.coin, alert.price)).await.unwrap();
                }
                cooldown_alerts(&alerts, &conn).await.unwrap();
            }
        } => {
            info!("Price monitoring stopped, unsubscribing from price updates");
            info_client.unsubscribe(subscription_id).await.unwrap();
        }
        _ = async move {
            loop {
                reset_cooldown(&conn_clone).await.unwrap();
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        } => {
        }
    }
}

async fn alerts_triggered(mark_px: f64, conn: &Arc<Mutex<Connection>>) -> Result<Vec<AlertTable>> {
    let lower_alert_price = mark_px * 0.95;
    let upper_alert_price = mark_px * 1.05;
    let conn_guard = conn.lock().await;
    let mut stmt = conn_guard.prepare("SELECT * FROM alerts WHERE alerted = false AND price BETWEEN ? AND ?").unwrap();
    let alerts = stmt.query_map([lower_alert_price, upper_alert_price], |row| {
        Ok(AlertTable {
            id: row.get(0)?,
            _public_key: row.get(1)?,
            chat_id: row.get(2)?,
            coin: row.get(3)?,
            token: row.get(4)?,
            price: row.get(5)?,
            alerted: row.get(6)?,
            created_at: row.get::<_, DateTime<Utc>>(7)?,
            updated_at: row.get::<_, DateTime<Utc>>(8)?,
            cooldown_until: row.get::<_, DateTime<Utc>>(9).unwrap_or(DateTime::<Utc>::from_timestamp(0, 0).unwrap()),
        })
    }).unwrap().collect::<Result<Vec<AlertTable>>>();
    
    alerts
}

async fn cooldown_alerts(alerts: &Vec<AlertTable>, conn: &Arc<Mutex<Connection>>) -> Result<()> {
    for alert in alerts {
        let conn_guard = conn.lock().await;
        let mut stmt = conn_guard.prepare("UPDATE alerts SET alerted = true, cooldown_until = datetime('now', '+1 minute') WHERE id = ?")?;
        stmt.execute([alert.id])?;
    }
    println!("Cooldown set for {} alerts", alerts.len());
    Ok(())
}

async fn reset_cooldown(conn: &Arc<Mutex<Connection>>) -> Result<()> {
    let conn_guard = conn.lock().await;
    let mut stmt = conn_guard.prepare("UPDATE alerts SET alerted = false, cooldown_until = NULL WHERE cooldown_until < CURRENT_TIMESTAMP")?;
    let result = stmt.execute(())?;
    if result > 0 {
        println!("Cooldown reset for {result} alerts");
    }
    Ok(())
}
// {
//     data: ActiveSpotAssetCtxData {
//         coin: "@107",
//         ctx: SpotAssetCtx {
//             shared: SharedAssetCtx {
//                 day_ntl_vlm: "174548678.1354800463",
//                 prev_day_px: "46.179",
//                 mark_px: "46.584",
//                 mid_px: Some("46.586")
//             },
//             circulating_supply: "336499483.8430535197"
//         }
//     }
// }
