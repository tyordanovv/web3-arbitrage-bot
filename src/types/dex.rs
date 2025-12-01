use std::collections::HashMap;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::types::{DexId, Price, Timestamp, TokenPair, now, pool_state::PoolId};

/// 0.3%
pub const DEX_SWAP_FEE_RATE: Decimal = Decimal::from_parts(3, 0, 0, false, 3);
pub const MIN_PROFIT_PERCENT: Decimal = Decimal::from_parts(5, 0, 0, false, 3);

/// Fee structure for a DEX
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeStructure {
    pub swap_fee_rate: Decimal,
    pub protocol_fee_rate: Option<Decimal>,
    pub fee_tiers: HashMap<String, Decimal>,
}

impl FeeStructure {
    /// Create a simple flat fee structure
    pub fn flat(fee_rate: Decimal) -> Self {
        Self {
            swap_fee_rate: fee_rate,
            protocol_fee_rate: None,
            fee_tiers: HashMap::new(),
        }
    }
    
    /// Create tiered fee structure
    pub fn tiered(default_fee: Decimal, tiers: HashMap<String, Decimal>) -> Self {
        Self {
            swap_fee_rate: default_fee,
            protocol_fee_rate: None,
            fee_tiers: tiers,
        }
    }
    
    /// Get fee for a specific pair
    pub fn get_fee(&self, pair: &TokenPair) -> Decimal {
        let pair_key = pair.symbol();
        self.fee_tiers
            .get(&pair_key)
            .copied()
            .unwrap_or(self.swap_fee_rate)
    }
    
    /// Get total fee including protocol fee
    pub fn total_fee(&self, pair: &TokenPair) -> Decimal {
        let base_fee = self.get_fee(pair);
        let protocol_fee = self.protocol_fee_rate.unwrap_or(Decimal::ZERO);
        base_fee + protocol_fee
    }
}

impl Default for FeeStructure {
    fn default() -> Self {
        Self::flat(DEX_SWAP_FEE_RATE) // 0.3% default
    }
}

/// Health status of a DEX
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub is_healthy: bool,
    pub last_event: Option<Timestamp>,
    pub last_heartbeat: Timestamp,
    pub consecutive_failures: u32,
    pub message: String,
    pub details: Option<HealthDetails>,
}

impl HealthStatus {
    /// Create a healthy status
    pub fn healthy(message: impl Into<String>) -> Self {
        Self {
            is_healthy: true,
            last_event: Some(now()),
            last_heartbeat: now(),
            consecutive_failures: 0,
            message: message.into(),
            details: None,
        }
    }
    
    /// Create an unhealthy status
    pub fn unhealthy(message: impl Into<String>, failures: u32) -> Self {
        Self {
            is_healthy: false,
            last_event: None,
            last_heartbeat: now(),
            consecutive_failures: failures,
            message: message.into(),
            details: None,
        }
    }
}

/// Additional health details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthDetails {
    pub last_error: Option<String>,
    pub time_since_last_success_ms: u64,
    pub pools_monitored: usize,
    pub stale_pools: usize,
    pub avg_response_time_ms: Option<u64>,
}

/// Raw event received from WebSocket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawEvent {
    pub data: serde_json::Value,
    pub timestamp: Timestamp,
    pub package_id: String,
    pub event_type: String,
    pub transaction_digest: Option<String>,
    pub sender: Option<String>,
}

impl RawEvent {
    pub fn new(
        data: serde_json::Value,
        package_id: String,
        event_type: String,
    ) -> Self {
        Self {
            data,
            timestamp: now(),
            package_id,
            event_type,
            transaction_digest: None,
            sender: None,
        }
    }
    
    /// Extract a string field
    pub fn get_string(&self, field: &str) -> Option<String> {
        self.data.get(field)?.as_str().map(|s| s.to_string())
    }
    
    /// Extract a u64 field
    pub fn get_u64(&self, field: &str) -> Option<u64> {
        self.data.get(field)?.as_u64()
    }
    
    /// Extract a bool field
    pub fn get_bool(&self, field: &str) -> Option<bool> {
        self.data.get(field)?.as_bool()
    }
}

/// Parsed swap event from a DEX
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapEvent {
    pub dex_id: DexId,
    pub pool_id: PoolId,
    pub amount_in: u64,
    pub amount_out: u64,
    pub base_to_quote: bool,
    pub timestamp: Timestamp,
    pub transaction_digest: String,
    pub sender: Option<String>,
    pub block_height: Option<u64>,
    pub sequence: Option<u64>,
}

impl SwapEvent {
    pub fn new() -> Self {
        Self {
            dex_id: DexId::Cetus,
            pool_id: PoolId::Sui(todo!()),
            amount_in: 0,
            amount_out: 0,
            base_to_quote: true,
            timestamp: now(),
            transaction_digest: String::new(),
            sender: None,
            block_height: None,
            sequence: None,
        }
    }

    pub fn direction_str(&self) -> &'static str {
        if self.base_to_quote {
            "BASE -> QUOTE"
        } else {
            "QUOTE -> BASE"
        }
    }
}

/// Price update notification sent to subscribers, broadcasted after processing event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceUpdate {
    pub dex_id: DexId,
    pub pair: TokenPair,
    pub old_price: Option<Price>,
    pub new_price: Price,
    pub price_change_percent: Decimal,
    pub trigger: UpdateTrigger,
    pub update_timestamp: Timestamp,
}

impl PriceUpdate {
    pub fn new(
        dex_id: DexId,
        pair: TokenPair,
        old_price: Option<Price>,
        new_price: Price,
        trigger: UpdateTrigger,
    ) -> Self {
        let price_change_percent = if let Some(ref old) = old_price {
            old.diff_percent(&new_price)
        } else {
            Decimal::ZERO
        };
        
        Self {
            dex_id,
            pair,
            old_price,
            new_price,
            price_change_percent,
            trigger,
            update_timestamp: now(),
        }
    }
    
    /// Check if price increased
    pub fn is_increase(&self) -> bool {
        self.price_change_percent > Decimal::ZERO
    }
    
    /// Check if price decreased
    pub fn is_decrease(&self) -> bool {
        self.price_change_percent < Decimal::ZERO
    }
    
    /// Check if price change is significant
    pub fn is_significant(&self, threshold_percent: Decimal) -> bool {
        self.price_change_percent.abs() >= threshold_percent
    }
    
    /// Get absolute price change
    pub fn absolute_change(&self) -> Decimal {
        if let Some(ref old) = self.old_price {
            self.new_price.value - old.value
        } else {
            Decimal::ZERO
        }
    }
}

/// What triggered a price update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UpdateTrigger {
    /// Triggered by a swap event
    SwapEvent {
        transaction_digest: String,
        block_height: Option<u64>,
    },
    /// Triggered by heartbeat poll
    HeartbeatPoll,
    /// Triggered by periodic sync
    PeriodicSync,
    /// Manually triggered
    Manual,
    /// Initial price fetch
    Initialization,
}

impl UpdateTrigger {
    /// Check if this was from a real-time event
    pub fn is_realtime(&self) -> bool {
        matches!(self, UpdateTrigger::SwapEvent { .. })
    }
    
    /// Get description
    pub fn description(&self) -> String {
        match self {
            UpdateTrigger::SwapEvent { transaction_digest, .. } => {
                format!("Swap event (tx: {})", &transaction_digest[..8])
            }
            UpdateTrigger::HeartbeatPoll => "Heartbeat poll".into(),
            UpdateTrigger::PeriodicSync => "Periodic sync".into(),
            UpdateTrigger::Manual => "Manual update".into(),
            UpdateTrigger::Initialization => "Initial fetch".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::types::{DexId, Price, PriceSource, TokenInfo};

    use super::*;
    
    #[test]
    fn test_fee_structure() {
        // Simple flat fee
        let fees = FeeStructure::flat(DEX_SWAP_FEE_RATE);
        assert_eq!(fees.swap_fee_rate, DEX_SWAP_FEE_RATE);
        
        // Tiered fees
        let mut tiers = HashMap::new();
        tiers.insert("SUI/USDC".to_string(), Decimal::from_parts(1, 0, 0, false, 3));
        tiers.insert("BTC/USDC".to_string(), Decimal::from_parts(5, 0, 0, false, 3));
        
        let fees = FeeStructure::tiered(
            DEX_SWAP_FEE_RATE,
            tiers
        );
        
        let sui_usdc = TokenPair {
            base: TokenInfo::new("SUI", "0x2::sui::SUI", 9),
            quote: TokenInfo::new("USDC", "0x2::usdc::USDC", 6),
        };
        
        assert_eq!(
            fees.get_fee(&sui_usdc),
            Decimal::from_parts(1, 0, 0, false, 3)
        );
    }
    
    #[test]
    fn test_health_status() {
        let healthy = HealthStatus::healthy("All systems operational");
        assert!(healthy.is_healthy);
        
        let unhealthy = HealthStatus::unhealthy("Connection lost", 3);
        assert!(!unhealthy.is_healthy);
    }
    
    #[test]
    fn test_price_update() {
        let dex_id = DexId::Cetus;
        let pair = TokenPair {
            base: TokenInfo::new("SUI", "0x2::sui::SUI", 9),
            quote: TokenInfo::new("USDC", "0x2::usdc::USDC", 6),
        };
        
        let old_price = Price::new(
            Decimal::from(2),
            PriceSource::Calculated,
        );
        
        let new_price = Price::new(
            Decimal::from_parts(21, 0, 0, false, 1),
            PriceSource::Event {
                block_height: 1000,
                transaction_digest: "0xabc".into(),
            },
        );
        
        let update = PriceUpdate::new(
            dex_id,
            pair,
            Some(old_price),
            new_price,
            UpdateTrigger::SwapEvent {
                transaction_digest: "0xabc".into(),
                block_height: Some(1000),
            },
        );
        
        assert!(update.is_increase());
        assert_eq!(update.price_change_percent, Decimal::from(5)); // 5% increase
    }
}