use std::{collections::HashMap, ops::Range, sync::Arc};

use arc_swap::ArcSwap;
use tracing::{debug, info};

use crate::types::{DexId, pool_state::{PoolId, PoolState}};

/// Read-optimized snapshot of all pool states
/// This is immutable and designed for fast, lock-free access
#[derive(Debug, Clone)]
pub struct PoolSnapshot {
    /// All pools in a contiguous Vec for cache-friendly iteration
    /// Pools are grouped by DEX for efficient slicing
    pools: Arc<Vec<PoolState>>,

    /// Fast lookup: PoolId -> index in pools Vec
    pool_index: Arc<HashMap<PoolId, usize>>,

    /// DEX-based slicing: DexId -> range in pools Vec
    /// Allows efficient "get all pools for DEX X" queries
    dex_ranges: Arc<HashMap<DexId, Range<usize>>>,

    /// Metadata
    pub snapshot_time: std::time::Instant,
    pub pool_count: usize,
    pub dex_count: usize,
}

impl PoolSnapshot {
    /// Create a new snapshot from raw pool states
    /// Pools will be grouped by DEX for efficient range-based access
    pub fn new(pools_by_dex: HashMap<DexId, Vec<PoolState>>) -> Self {
        let start = std::time::Instant::now();

        let mut all_pools = Vec::new();
        let mut pool_index = HashMap::new();
        let mut dex_ranges = HashMap::new();

        let dex_count = pools_by_dex.len();

        // Build contiguous pool vec with DEX grouping
        for (dex_id, mut pools) in pools_by_dex.into_iter() {
            let start_idx = all_pools.len();

            // Sort pools by ID for deterministic ordering
            pools.sort_by(|a, b| format!("{:?}", a.pool_id).cmp(&format!("{:?}", b.pool_id)));

            // Add to index
            for (i, pool) in pools.iter().enumerate() {
                pool_index.insert(pool.pool_id.clone(), start_idx + i);
            }

            // Extend main vec
            all_pools.extend(pools);

            let end_idx = all_pools.len();
            dex_ranges.insert(dex_id, start_idx..end_idx);
        }

        let pool_count = all_pools.len();
        let duration = start.elapsed();

        info!(
            pool_count = pool_count,
            dex_count = dex_count,
            build_time_us = duration.as_micros(),
            "Built new pool snapshot"
        );

        Self {
            pools: Arc::new(all_pools),
            pool_index: Arc::new(pool_index),
            dex_ranges: Arc::new(dex_ranges),
            snapshot_time: std::time::Instant::now(),
            pool_count,
            dex_count,
        }
    }

    /// Get a single pool by ID - O(1) lookup
    pub fn get_pool(&self, pool_id: &PoolId) -> Option<&PoolState> {
        self.pool_index
            .get(pool_id)
            .and_then(|&idx| self.pools.get(idx))
    }

    /// Get all pools for a specific DEX - O(1) slice access
    pub fn get_dex_pools(&self, dex_id: &DexId) -> &[PoolState] {
        self.dex_ranges
            .get(dex_id)
            .map(|range| &self.pools[range.clone()])
            .unwrap_or(&[])
    }

    /// Get all pools as a slice - zero-copy
    pub fn get_all_pools(&self) -> &[PoolState] {
        &self.pools
    }

    /// Get pool count for a specific DEX
    pub fn get_dex_pool_count(&self, dex_id: &DexId) -> usize {
        self.dex_ranges
            .get(dex_id)
            .map(|range| range.len())
            .unwrap_or(0)
    }

    /// Get all DEX IDs in this snapshot
    pub fn get_dex_ids(&self) -> Vec<DexId> {
        self.dex_ranges.keys().copied().collect()
    }

    /// Check if snapshot is stale (older than threshold)
    pub fn is_stale(&self, max_age: std::time::Duration) -> bool {
        self.snapshot_time.elapsed() > max_age
    }
}

impl Default for PoolSnapshot {
    fn default() -> Self {
        Self {
            pools: Arc::new(Vec::new()),
            pool_index: Arc::new(HashMap::new()),
            dex_ranges: Arc::new(HashMap::new()),
            snapshot_time: std::time::Instant::now(),
            pool_count: 0,
            dex_count: 0,
        }
    }
}

/// Lock-free state reader for high-frequency access
/// Provides fast, contention-free reads via atomic snapshot swapping
pub struct StateReader {
    snapshot: Arc<ArcSwap<PoolSnapshot>>,
}

impl StateReader {
    /// Create a new StateReader with an initial empty snapshot
    pub fn new() -> Self {
        Self {
            snapshot: Arc::new(ArcSwap::from_pointee(PoolSnapshot::default())),
        }
    }

    /// Create a new StateReader with an initial snapshot
    pub fn with_snapshot(snapshot: PoolSnapshot) -> Self {
        Self {
            snapshot: Arc::new(ArcSwap::from_pointee(snapshot)),
        }
    }

    /// Get a reference to the current snapshot - LOCK-FREE
    /// This is the primary read method, designed for 50Hz+ access
    /// Cost: ~500ns (just Arc clone + atomic load)
    pub fn get_snapshot(&self) -> Arc<PoolSnapshot> {
        let start = std::time::Instant::now();
        let snapshot = self.snapshot.load_full();
        let duration = start.elapsed();

        debug!(
            pool_count = snapshot.pool_count,
            read_time_ns = duration.as_nanos(),
            "Read pool snapshot"
        );

        snapshot
    }

    /// Update the snapshot atomically - called by StateManager after updates
    /// This is lock-free from the reader's perspective
    pub fn update_snapshot(&self, new_snapshot: PoolSnapshot) {
        let start = std::time::Instant::now();

        self.snapshot.store(Arc::new(new_snapshot));

        let duration = start.elapsed();
        info!(
            swap_time_us = duration.as_micros(),
            "Updated snapshot atomically"
        );
    }

    /// Get statistics about the current snapshot without full load
    pub fn get_stats(&self) -> SnapshotStats {
        let snapshot = self.snapshot.load_full();

        SnapshotStats {
            pool_count: snapshot.pool_count,
            dex_count: snapshot.dex_count,
            age_ms: snapshot.snapshot_time.elapsed().as_millis(),
            snapshot_time: snapshot.snapshot_time,
        }
    }

    /// Clone the underlying ArcSwap for sharing across components
    pub fn clone_handle(&self) -> Arc<ArcSwap<PoolSnapshot>> {
        Arc::clone(&self.snapshot)
    }
}

impl Clone for StateReader {
    fn clone(&self) -> Self {
        Self {
            snapshot: Arc::clone(&self.snapshot),
        }
    }
}

impl Default for StateReader {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about a snapshot
#[derive(Debug, Clone)]
pub struct SnapshotStats {
    pub pool_count: usize,
    pub dex_count: usize,
    pub age_ms: u128,
    pub snapshot_time: std::time::Instant,
}
