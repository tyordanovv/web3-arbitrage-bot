use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use crate::types::{DexId, pool_state::{ PoolId, PoolState}, Price, Timestamp, TokenInfo, TokenPair, now};

/// Atomic snapshot of all DEX states for consistent arbitrage calculations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    /// All prices across all DEXs and pairs
    pub prices: HashMap<PriceKey, Price>,
    
    /// Pool states for detailed calculations
    pub pools: HashMap<PoolId, PoolState>,
    
    /// Token information for decimal handling
    pub tokens: HashMap<String, TokenInfo>,
    
    /// Metadata
    pub timestamp: Timestamp,
    pub sequence: u64,
    pub dex_count: usize,
    pub pool_count: usize,
}

/// Key for price lookups (DEX + Pair)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PriceKey {
    pub dex_id: DexId,
    pub pair: TokenPair,
}

impl PriceKey {
    pub fn new(dex_id: DexId, pair: TokenPair) -> Self {
        Self { dex_id, pair }
    }
}

impl StateSnapshot {
    /// Create a new empty snapshot
    pub fn new() -> Self {
        Self {
            prices: HashMap::new(),
            pools: HashMap::new(),
            tokens: HashMap::new(),
            timestamp: now(),
            sequence: 0,
            dex_count: 0,
            pool_count: 0,
        }
    }
    /// Get statistics about the snapshot
    pub fn get_stats(&self) -> SnapshotStats {
        SnapshotStats {
            timestamp: self.timestamp,
            dex_count: self.dex_count,
            pool_count: self.pool_count,
            price_count: self.prices.len(),
            token_count: self.tokens.len(),
            age_ms: now() - self.timestamp,
        }
    }
}

/// Statistics about a state snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotStats {
    pub timestamp: Timestamp,
    pub dex_count: usize,
    pub pool_count: usize,
    pub price_count: usize,
    pub token_count: usize,
    pub age_ms: u64,
}

impl Default for StateSnapshot {
    fn default() -> Self {
        Self::new()
    }
}

// Helper implementations for PriceKey
impl std::fmt::Display for PriceKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.dex_id, self.pair)
    }
}