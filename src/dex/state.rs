use std::{collections::HashMap, sync::Arc};

use chrono::Duration;
use tokio::sync::RwLock;

use crate::types::{DexId, FeeStructure, PoolId, PoolState, Price, Timestamp, TokenPair};

/// State for a single DEX instance
pub struct DexState {
    pub dex_id: DexId,
    pub config: DexConfig,
    pub pools: HashMap<PoolId, PoolState>,
    pub prices: Arc<RwLock<HashMap<TokenPair, Price>>>,
    pub health: DexHealthState,
    pub sync_state: SyncState,
    pub stats: DexStatistics,
}

/// Health monitoring for a DEX
pub struct DexHealthState {
    pub last_event: Option<Timestamp>,
    pub last_heartbeat: Timestamp,
    pub last_sync: Timestamp,
    pub consecutive_failures: u32,
    pub is_healthy: bool,
}

/// Synchronization state
pub struct SyncState {
    pub last_full_sync: Timestamp,
    pub next_sync_due: Timestamp,
    pub sync_interval: Duration,
    pub heartbeat_interval: Duration,
    pub heartbeat_timeout: Duration,
}

/// DEX configuration
pub struct DexConfig {
    pub name: String,
    pub package_id: String,
    pub monitored_pairs: Vec<TokenPair>,
    pub pool_addresses: HashMap<TokenPair, String>,
    pub fee_structure: FeeStructure,
    pub sync_settings: SyncSettings,
}

pub struct SyncSettings {
    pub heartbeat_interval_secs: u64,
    pub heartbeat_timeout_secs: u64,
    pub periodic_sync_interval_secs: u64,
    pub enable_fallback_polling: bool,
}

/// Statistics tracking
pub struct DexStatistics {
    pub events_received: u64,
    pub events_processed: u64,
    pub polls_executed: u64,
    pub syncs_completed: u64,
    pub errors_encountered: u64,
    pub last_error: Option<(Timestamp, String)>,
}