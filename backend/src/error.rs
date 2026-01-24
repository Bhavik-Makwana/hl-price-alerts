use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Telegram error: {0}")]
    Telegram(#[from] teloxide::RequestError),

    #[error("Hyperliquid SDK error: {0}")]
    HyperliquidSdk(String),

    #[error("Token not found: {0}")]
    TokenNotFound(String),

    #[error("Price parsing error: {0}")]
    PriceParse(String),

    #[error("Invalid time format: {0}")]
    InvalidTimeFormat(String),

    #[error("Cron parse error: {0}")]
    CronParse(String),
}

pub type Result<T> = std::result::Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_not_found_error_display() {
        let err = AppError::TokenNotFound("UNKNOWN".to_string());
        assert_eq!(err.to_string(), "Token not found: UNKNOWN");
    }

    #[test]
    fn test_price_parse_error_display() {
        let err = AppError::PriceParse("invalid_price".to_string());
        assert_eq!(err.to_string(), "Price parsing error: invalid_price");
    }

    #[test]
    fn test_invalid_time_format_display() {
        let err = AppError::InvalidTimeFormat("25:99".to_string());
        assert_eq!(err.to_string(), "Invalid time format: 25:99");
    }

    #[test]
    fn test_cron_parse_error_display() {
        let err = AppError::CronParse("bad cron".to_string());
        assert_eq!(err.to_string(), "Cron parse error: bad cron");
    }

    #[test]
    fn test_database_error_from_rusqlite() {
        let rusqlite_err = rusqlite::Error::InvalidQuery;
        let app_err: AppError = rusqlite_err.into();
        assert!(matches!(app_err, AppError::Database(_)));
    }
}
