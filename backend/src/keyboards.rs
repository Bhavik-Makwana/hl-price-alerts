use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

/// Callback data prefixes for parsing
pub const CB_COIN_SELECT: &str = "coin:";
pub const CB_CUSTOM_COIN: &str = "custom_coin";
pub const CB_DELETE_ALERT: &str = "del_alert:";
pub const CB_DELETE_CRON: &str = "del_cron:";
pub const CB_CONFIRM_DELETE_ALERT: &str = "confirm_del_alert:";
pub const CB_CONFIRM_DELETE_CRON: &str = "confirm_del_cron:";
pub const CB_CANCEL: &str = "cancel";
pub const CB_MAIN_MENU: &str = "main_menu";
pub const CB_MENU_ALERTS: &str = "menu:alerts";
pub const CB_MENU_CRON: &str = "menu:cron";
pub const CB_MENU_NEW_ALERT: &str = "menu:new_alert";
pub const CB_MENU_NEW_CRON: &str = "menu:new_cron";
pub const CB_MENU_HELP: &str = "menu:help";
pub const CB_SCHEDULE_PREFIX: &str = "schedule:";

/// Popular coins for quick selection
pub const POPULAR_COINS: &[&str] = &["HYPE", "BTC", "ETH", "SOL", "PURR"];

/// Build the main menu keyboard
pub fn main_menu_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("📊 Price Alerts", CB_MENU_ALERTS),
            InlineKeyboardButton::callback("⏰ Cron Alerts", CB_MENU_CRON),
        ],
        vec![
            InlineKeyboardButton::callback("➕ New Price Alert", CB_MENU_NEW_ALERT),
            InlineKeyboardButton::callback("➕ New Cron Alert", CB_MENU_NEW_CRON),
        ],
        vec![InlineKeyboardButton::callback("❓ Help", CB_MENU_HELP)],
    ])
}

/// Build coin selection keyboard for creating alerts
pub fn coin_selection_keyboard() -> InlineKeyboardMarkup {
    let coin_buttons: Vec<Vec<InlineKeyboardButton>> = POPULAR_COINS
        .chunks(3)
        .map(|chunk| {
            chunk
                .iter()
                .map(|coin| {
                    InlineKeyboardButton::callback(*coin, format!("{}{}", CB_COIN_SELECT, coin))
                })
                .collect()
        })
        .collect();

    let mut buttons = coin_buttons;
    buttons.push(vec![InlineKeyboardButton::callback(
        "✏️ Other",
        CB_CUSTOM_COIN,
    )]);
    buttons.push(vec![InlineKeyboardButton::callback("🔙 Cancel", CB_CANCEL)]);

    InlineKeyboardMarkup::new(buttons)
}

/// Build alert list with delete buttons
pub fn alerts_list_keyboard(alerts: &[(i64, String, f64)]) -> InlineKeyboardMarkup {
    let mut buttons: Vec<Vec<InlineKeyboardButton>> = alerts
        .iter()
        .map(|(id, coin, price)| {
            vec![
                InlineKeyboardButton::callback(
                    format!("{} @ ${:.2}", coin, price),
                    format!("view_alert:{}", id),
                ),
                InlineKeyboardButton::callback("🗑", format!("{}{}", CB_DELETE_ALERT, id)),
            ]
        })
        .collect();

    buttons.push(vec![InlineKeyboardButton::callback(
        "🔙 Back",
        CB_MAIN_MENU,
    )]);

    InlineKeyboardMarkup::new(buttons)
}

/// Build cron alert list with delete buttons
pub fn cron_alerts_list_keyboard(alerts: &[(i64, String, String)]) -> InlineKeyboardMarkup {
    let mut buttons: Vec<Vec<InlineKeyboardButton>> = alerts
        .iter()
        .map(|(id, coin, schedule)| {
            vec![
                InlineKeyboardButton::callback(
                    format!("{} ({})", coin, schedule),
                    format!("view_cron:{}", id),
                ),
                InlineKeyboardButton::callback("🗑", format!("{}{}", CB_DELETE_CRON, id)),
            ]
        })
        .collect();

    buttons.push(vec![InlineKeyboardButton::callback(
        "🔙 Back",
        CB_MAIN_MENU,
    )]);

    InlineKeyboardMarkup::new(buttons)
}

/// Build confirmation dialog for deleting an alert
pub fn confirm_delete_alert_keyboard(alert_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(
            "✅ Yes, delete",
            format!("{}{}", CB_CONFIRM_DELETE_ALERT, alert_id),
        ),
        InlineKeyboardButton::callback("❌ Cancel", CB_CANCEL),
    ]])
}

/// Build confirmation dialog for deleting a cron alert
pub fn confirm_delete_cron_keyboard(alert_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(
            "✅ Yes, delete",
            format!("{}{}", CB_CONFIRM_DELETE_CRON, alert_id),
        ),
        InlineKeyboardButton::callback("❌ Cancel", CB_CANCEL),
    ]])
}

/// Schedule selection for cron alerts
pub fn schedule_selection_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            "Daily",
            format!("{}daily", CB_SCHEDULE_PREFIX),
        )],
        vec![
            InlineKeyboardButton::callback("Mon", format!("{}monday", CB_SCHEDULE_PREFIX)),
            InlineKeyboardButton::callback("Tue", format!("{}tuesday", CB_SCHEDULE_PREFIX)),
            InlineKeyboardButton::callback("Wed", format!("{}wednesday", CB_SCHEDULE_PREFIX)),
        ],
        vec![
            InlineKeyboardButton::callback("Thu", format!("{}thursday", CB_SCHEDULE_PREFIX)),
            InlineKeyboardButton::callback("Fri", format!("{}friday", CB_SCHEDULE_PREFIX)),
            InlineKeyboardButton::callback("Sat", format!("{}saturday", CB_SCHEDULE_PREFIX)),
        ],
        vec![InlineKeyboardButton::callback(
            "Sun",
            format!("{}sunday", CB_SCHEDULE_PREFIX),
        )],
        vec![InlineKeyboardButton::callback("🔙 Cancel", CB_CANCEL)],
    ])
}

/// Parsed callback action
#[derive(Debug, Clone, PartialEq)]
pub enum CallbackAction {
    CoinSelect(String),
    CustomCoin,
    DeleteAlert(i64),
    DeleteCron(i64),
    ConfirmDeleteAlert(i64),
    ConfirmDeleteCron(i64),
    Cancel,
    MainMenu,
    MenuAlerts,
    MenuCron,
    MenuNewAlert,
    MenuNewCron,
    MenuHelp,
    ScheduleSelect(String),
    Unknown(String),
}

/// Parse callback data into an action
pub fn parse_callback(data: &str) -> CallbackAction {
    if let Some(coin) = data.strip_prefix(CB_COIN_SELECT) {
        CallbackAction::CoinSelect(coin.to_string())
    } else if data == CB_CUSTOM_COIN {
        CallbackAction::CustomCoin
    } else if let Some(id_str) = data.strip_prefix(CB_DELETE_ALERT) {
        id_str
            .parse::<i64>()
            .map(CallbackAction::DeleteAlert)
            .unwrap_or_else(|_| CallbackAction::Unknown(data.to_string()))
    } else if let Some(id_str) = data.strip_prefix(CB_DELETE_CRON) {
        id_str
            .parse::<i64>()
            .map(CallbackAction::DeleteCron)
            .unwrap_or_else(|_| CallbackAction::Unknown(data.to_string()))
    } else if let Some(id_str) = data.strip_prefix(CB_CONFIRM_DELETE_ALERT) {
        id_str
            .parse::<i64>()
            .map(CallbackAction::ConfirmDeleteAlert)
            .unwrap_or_else(|_| CallbackAction::Unknown(data.to_string()))
    } else if let Some(id_str) = data.strip_prefix(CB_CONFIRM_DELETE_CRON) {
        id_str
            .parse::<i64>()
            .map(CallbackAction::ConfirmDeleteCron)
            .unwrap_or_else(|_| CallbackAction::Unknown(data.to_string()))
    } else if let Some(schedule) = data.strip_prefix(CB_SCHEDULE_PREFIX) {
        CallbackAction::ScheduleSelect(schedule.to_string())
    } else if data == CB_CANCEL {
        CallbackAction::Cancel
    } else if data == CB_MAIN_MENU {
        CallbackAction::MainMenu
    } else if data == CB_MENU_ALERTS {
        CallbackAction::MenuAlerts
    } else if data == CB_MENU_CRON {
        CallbackAction::MenuCron
    } else if data == CB_MENU_NEW_ALERT {
        CallbackAction::MenuNewAlert
    } else if data == CB_MENU_NEW_CRON {
        CallbackAction::MenuNewCron
    } else if data == CB_MENU_HELP {
        CallbackAction::MenuHelp
    } else {
        CallbackAction::Unknown(data.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_coin_select() {
        assert_eq!(
            parse_callback("coin:HYPE"),
            CallbackAction::CoinSelect("HYPE".to_string())
        );
    }

    #[test]
    fn test_parse_delete_alert() {
        assert_eq!(
            parse_callback("del_alert:123"),
            CallbackAction::DeleteAlert(123)
        );
    }

    #[test]
    fn test_parse_delete_cron() {
        assert_eq!(
            parse_callback("del_cron:456"),
            CallbackAction::DeleteCron(456)
        );
    }

    #[test]
    fn test_parse_confirm_delete_alert() {
        assert_eq!(
            parse_callback("confirm_del_alert:789"),
            CallbackAction::ConfirmDeleteAlert(789)
        );
    }

    #[test]
    fn test_parse_confirm_delete_cron() {
        assert_eq!(
            parse_callback("confirm_del_cron:101"),
            CallbackAction::ConfirmDeleteCron(101)
        );
    }

    #[test]
    fn test_parse_schedule_select() {
        assert_eq!(
            parse_callback("schedule:monday"),
            CallbackAction::ScheduleSelect("monday".to_string())
        );
    }

    #[test]
    fn test_parse_custom_coin() {
        assert_eq!(parse_callback("custom_coin"), CallbackAction::CustomCoin);
    }

    #[test]
    fn test_parse_cancel() {
        assert_eq!(parse_callback("cancel"), CallbackAction::Cancel);
    }

    #[test]
    fn test_parse_main_menu() {
        assert_eq!(parse_callback("main_menu"), CallbackAction::MainMenu);
    }

    #[test]
    fn test_parse_menu_actions() {
        assert_eq!(parse_callback("menu:alerts"), CallbackAction::MenuAlerts);
        assert_eq!(parse_callback("menu:cron"), CallbackAction::MenuCron);
        assert_eq!(
            parse_callback("menu:new_alert"),
            CallbackAction::MenuNewAlert
        );
        assert_eq!(parse_callback("menu:new_cron"), CallbackAction::MenuNewCron);
        assert_eq!(parse_callback("menu:help"), CallbackAction::MenuHelp);
    }

    #[test]
    fn test_parse_unknown() {
        assert_eq!(
            parse_callback("unknown_action"),
            CallbackAction::Unknown("unknown_action".to_string())
        );
    }

    #[test]
    fn test_parse_invalid_id() {
        assert_eq!(
            parse_callback("del_alert:not_a_number"),
            CallbackAction::Unknown("del_alert:not_a_number".to_string())
        );
    }

    #[test]
    fn test_main_menu_keyboard_has_buttons() {
        let kb = main_menu_keyboard();
        assert_eq!(kb.inline_keyboard.len(), 3); // 3 rows
    }

    #[test]
    fn test_coin_selection_keyboard_has_coins() {
        let kb = coin_selection_keyboard();
        // Should have ceil(5/3) = 2 rows of coins + 1 "Other" row + 1 cancel row = 4 rows
        assert_eq!(kb.inline_keyboard.len(), 4);
    }

    #[test]
    fn test_alerts_list_keyboard_empty() {
        let alerts: Vec<(i64, String, f64)> = vec![];
        let kb = alerts_list_keyboard(&alerts);
        // Should just have the back button
        assert_eq!(kb.inline_keyboard.len(), 1);
    }

    #[test]
    fn test_alerts_list_keyboard_with_alerts() {
        let alerts = vec![(1, "HYPE".to_string(), 25.0), (2, "SOL".to_string(), 150.0)];
        let kb = alerts_list_keyboard(&alerts);
        // 2 alert rows + 1 back row
        assert_eq!(kb.inline_keyboard.len(), 3);
    }

    #[test]
    fn test_schedule_selection_keyboard() {
        let kb = schedule_selection_keyboard();
        // Daily row + 2 weekday rows + Sunday row + Cancel row = 5 rows
        assert_eq!(kb.inline_keyboard.len(), 5);
    }
}
