use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use rust_decimal::prelude::ToPrimitive;

pub type Timestamp = u64;
pub type PoolId = String;  

pub fn now() -> Timestamp {
    chrono::Utc::now().timestamp_millis() as u64
}

/// Supported DEX identifiers
/// 
/// TODO Turbos
/// TODO Aftermath
/// TODO Kriya
/// TODO FlowX
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DexId {
    Cetus,
    Turbos,
    Kriya,
}

impl DexId {
    pub fn all() -> Vec<DexId> {
        vec![
            DexId::Cetus,
            DexId::Turbos,
            DexId::Kriya,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            DexId::Cetus => "Cetus",
            DexId::Turbos => "Turbos",
            DexId::Kriya => "Kriya",
        }
    }
}

impl fmt::Display for DexId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl FromStr for DexId {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "cetus" => Ok(DexId::Cetus),
            _ => Err(format!("Unknown DEX: {}", s)),
        }
    }
}

/// Blockchain network identifier
/// 
/// TODO SuiMainnet
/// TODO AptosMainnet
/// TODO AptosTestnet
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Network {
    SuiTestnet,
    SuiMainnet
}

impl Network {
    pub fn is_testnet(&self) -> bool {
        matches!(self, Network::SuiTestnet)
    }

    pub fn is_mainnet(&self) -> bool {
        matches!(self, Network::SuiMainnet)
    }
}

impl fmt::Display for Network {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Network::SuiTestnet => write!(f, "sui-testnet"),
            Network::SuiMainnet => write!(f, "sui-mainnet")
        }
    }
}

// ============================================================================
// Token Information
// ============================================================================

/// Token metadata
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TokenInfo {
    pub symbol: String,
    #[serde(default)]
    pub address: Option<String>,
    pub decimals: u8,
    #[serde(default)]
    pub name: Option<String>,
}

impl TokenInfo {
    pub fn new(symbol: impl Into<String>, address: impl Into<String>, decimals: u8) -> Self {
        Self {
            symbol: symbol.into(),
            address: Some(address.into()),
            decimals,
            name: None,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Convert raw amount to decimal
    pub fn to_decimal(&self, raw_amount: u64) -> Decimal {
        Decimal::from(raw_amount) / Decimal::from(10u64.pow(self.decimals as u32))
    }

    /// Convert decimal to raw amount
    pub fn to_raw(&self, decimal_amount: Decimal) -> u64 {
        (decimal_amount * Decimal::from(10u64.pow(self.decimals as u32)))
            .to_u64()
            .unwrap_or(0)
    }
}

impl fmt::Display for TokenInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.symbol)
    }
}

// ============================================================================
// Token Pair
// ============================================================================

/// Trading pair (eg SUI/USDC)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TokenPair {
    pub base: TokenInfo,   // The token being bought/sold
    pub quote: TokenInfo,  // The token used for pricing
}

impl TokenPair {
    pub fn new(base: TokenInfo, quote: TokenInfo) -> Self {
        Self { base, quote }
    }

    /// Get pair symbol (eg "SUI/USDC")
    pub fn symbol(&self) -> String {
        format!("{}/{}", self.base.symbol, self.quote.symbol)
    }

    /// Check if tokens match (order-independent)
    pub fn matches(&self, other: &TokenPair) -> bool {
        (self.base == other.base && self.quote == other.quote)
            || (self.base == other.quote && self.quote == other.base)
    }
}

impl fmt::Display for TokenPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.symbol())
    }
}

// ============================================================================
// Token Amount
// ============================================================================

/// Token amount with automatic decimal handling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenAmount {
    pub token: TokenInfo,
    pub raw_amount: u64,  // Amount without decimal adjustment
}

impl TokenAmount {
    pub fn new(token: TokenInfo, raw_amount: u64) -> Self {
        Self { token, raw_amount }
    }

    /// Create from decimal amount
    pub fn from_decimal(token: TokenInfo, decimal_amount: Decimal) -> Self {
        let raw_amount = token.to_raw(decimal_amount);
        Self { token, raw_amount }
    }

    /// Get decimal representation
    pub fn to_decimal(&self) -> Decimal {
        self.token.to_decimal(self.raw_amount)
    }

    /// Get human-readable string (eg "10.5 SUI")
    pub fn to_string(&self) -> String {
        format!("{} {}", self.to_decimal(), self.token.symbol)
    }
}

impl fmt::Display for TokenAmount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

/// Price with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Price {
    pub value: Decimal,
    pub timestamp: Timestamp,
    pub source: PriceSource,
}

impl Price {
    pub fn new(value: Decimal, source: PriceSource) -> Self {
        Self {
            value,
            timestamp: now(),
            source,
        }
    }

    /// Get age in milliseconds
    pub fn age_ms(&self) -> u64 {
        now() - self.timestamp
    }

    /// Calculate price difference percentage
    pub fn diff_percent(&self, other: &Price) -> Decimal {
        if self.value.is_zero() {
            return Decimal::ZERO;
        }
        ((other.value - self.value) / self.value) * Decimal::from(100)
    }
}

/// Source of price information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PriceSource {
    Event {
        block_height: u64,
        transaction_digest: String,
    },
    RpcPoll {
        synced: bool,
    },
    Calculated,
    External {
        source: String,
    },
}