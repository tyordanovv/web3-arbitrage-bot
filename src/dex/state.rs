use std::{collections::HashMap, sync::Arc};

use chrono::Duration;
use tokio::sync::RwLock;

use crate::types::{DexId, FeeStructure, PoolId, PoolState, Price, Timestamp, TokenPair};

/// State for a single DEX instance
pub struct DexState {
    pub dex_id: DexId,
    pub pools: HashMap<PoolId, PoolState>,
    pub prices: Arc<RwLock<HashMap<TokenPair, Price>>>,
    pub stats: DexStatistics,
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