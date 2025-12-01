use rust_decimal::Decimal;
use thiserror::Error;

use crate::types::DexId;

#[derive(Error, Debug)]
pub enum BotError {
    #[error("DEX error ({dex}): {message}")]
    Dex { dex: DexId, message: String },

    #[error("Event error: {0}")]
    Event(String),
    
    #[error("WebSocket error: {0}")]
    WebSocket(String),
    
    #[error("Sync error: {0}")]
    Sync(String),
    
    #[error("Execution error: {0}")]
    Execution(String),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Parse error: {0}")]
    Parse(String),
    
    #[error("Not found: {0}")]
    NotFound(String),
    
    #[error("Invalid state: {0}")]
    InvalidState(String),
    
    #[error("Insufficient liquidity in pool {pool_id}")]
    InsufficientLiquidity { pool_id: String },
    
    #[error("Slippage too high: expected {expected}, got {actual}")]
    SlippageTooHigh { expected: Decimal, actual: Decimal },
    
    #[error("Price stale: age {age_ms}ms exceeds max {max_age_ms}ms")]
    StalePrice { age_ms: u64, max_age_ms: u64 },
    
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("Decimal error: {0}")]
    Decimal(#[from] rust_decimal::Error),
    
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Sui rpc read error: {0}")]
    SuiReadRpc(String),
    
    #[error("Unknown error: {0}")]
    Unknown(String),
}

pub type Result<T> = std::result::Result<T, BotError>;

impl BotError {
    pub fn dex(dex: DexId, message: impl Into<String>) -> Self {
        Self::Dex {
            dex,
            message: message.into(),
        }
    }
}

impl From<sui_sdk::error::Error> for BotError {
    fn from(err: sui_sdk::error::Error) -> Self {
        BotError::SuiReadRpc(err.to_string())
    }
}