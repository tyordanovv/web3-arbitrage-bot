use std::{collections::HashMap, sync::Arc};

use tokio::sync::RwLock;
use tracing::{debug, info, error, warn};

use crate::{dex::manager::DexManager, sync::reader::{PoolSnapshot, StateReader}, types::{DexId, Network, Result, pool_state::{ PoolId, PoolState}}};

pub struct StateManager {
    dex_manager: Arc<RwLock<DexManager>>,
    state_reader: Arc<StateReader>,
}

impl StateManager {
    pub fn new(dex_manager: Arc<RwLock<DexManager>>) -> Self {
        let state_reader = Arc::new(StateReader::new());
        Self {
            dex_manager,
            state_reader,
        }
    }

    pub fn with_reader(dex_manager: Arc<RwLock<DexManager>>, state_reader: Arc<StateReader>) -> Self {
        Self {
            dex_manager,
            state_reader,
        }
    }

    /// Get a reference to the StateReader for lock-free reads
    pub fn get_reader(&self) -> Arc<StateReader> {
        Arc::clone(&self.state_reader)
    }

    /// Rebuild the snapshot from current DexManager state
    /// Called after updates to propagate changes to readers
    async fn rebuild_snapshot(&self) -> Result<()> {
        let rebuild_start = std::time::Instant::now();

        debug!("Starting snapshot rebuild...");

        // Read current state from DexManager
        let manager = self.dex_manager.read().await;
        let pools_by_dex = self.collect_pools_by_dex(&manager).await;

        let collect_time = rebuild_start.elapsed();

        // Build new snapshot
        let snapshot_build_start = std::time::Instant::now();
        let new_snapshot = PoolSnapshot::new(pools_by_dex);
        let snapshot_build_time = snapshot_build_start.elapsed();

        // Atomically update the reader
        self.state_reader.update_snapshot(new_snapshot);

        let total_time = rebuild_start.elapsed();

        info!(
            collect_time_us = collect_time.as_micros(),
            snapshot_build_time_us = snapshot_build_time.as_micros(),
            total_rebuild_time_us = total_time.as_micros(),
            "Snapshot rebuild completed"
        );

        Ok(())
    }

    /// Collect all pool states grouped by DEX
    async fn collect_pools_by_dex(&self, manager: &DexManager) -> HashMap<DexId, Vec<PoolState>> {
        let mut pools_by_dex = HashMap::new();

        // Get all pools from DexManager
        let all_pools = manager.get_all_pools().await;

        // Group by DEX
        for pool in all_pools {
            pools_by_dex
                .entry(pool.dex_id)
                .or_insert_with(Vec::new)
                .push(pool);
        }

        debug!(
            total_pools = pools_by_dex.values().map(|v| v.len()).sum::<usize>(),
            dex_count = pools_by_dex.len(),
            "Collected pools by DEX"
        );

        pools_by_dex
    }

    pub async fn update_pool(&self, pool_state: PoolState) -> Result<()> {
        let update_start = std::time::Instant::now();
        let pool_id = pool_state.pool_id.clone();

        debug!(pool_id = ?pool_id, "Updating single pool state");

        // Acquire write lock
        let lock_start = std::time::Instant::now();
        let mut manager = self.dex_manager.write().await;
        let lock_wait_time = lock_start.elapsed();

        // Perform update
        let update_op_start = std::time::Instant::now();
        manager.update_pool_state(pool_state).await?;
        let update_op_time = update_op_start.elapsed();

        // Release lock before rebuilding snapshot
        drop(manager);

        // Rebuild snapshot for readers
        let snapshot_start = std::time::Instant::now();
        self.rebuild_snapshot().await?;
        let snapshot_time = snapshot_start.elapsed();

        let total_time = update_start.elapsed();

        info!(
            pool_id = ?pool_id,
            lock_wait_us = lock_wait_time.as_micros(),
            update_op_us = update_op_time.as_micros(),
            snapshot_rebuild_us = snapshot_time.as_micros(),
            total_time_us = total_time.as_micros(),
            "Single pool update completed"
        );

        Ok(())
    }

    pub async fn update_multiple_pools(&self, pool_states: Vec<PoolState>) -> Result<usize> {
        let batch_start = std::time::Instant::now();
        let pools_count = pool_states.len();

        info!(pools_count = pools_count, "Starting batch pool update");

        // Acquire write lock
        let lock_start = std::time::Instant::now();
        let mut manager = self.dex_manager.write().await;
        let lock_wait_time = lock_start.elapsed();

        // Perform batch updates
        let update_start = std::time::Instant::now();
        let mut success_count = 0;
        let mut failure_count = 0;

        for pool_state in pool_states {
            match manager.update_pool_state(pool_state.clone()).await {
                Ok(_) => {
                    success_count += 1;
                    debug!(pool_id = ?pool_state.pool_id, "Updated pool");
                }
                Err(e) => {
                    failure_count += 1;
                    error!(
                        pool_id = ?pool_state.pool_id,
                        error = %e,
                        "Failed to update pool"
                    );
                }
            }
        }

        let update_time = update_start.elapsed();

        // Release lock before rebuilding snapshot
        drop(manager);

        // Rebuild snapshot once for entire batch
        let snapshot_start = std::time::Instant::now();
        if success_count > 0 {
            if let Err(e) = self.rebuild_snapshot().await {
                warn!(error = %e, "Failed to rebuild snapshot after batch update");
            }
        }
        let snapshot_time = snapshot_start.elapsed();

        let total_time = batch_start.elapsed();

        info!(
            pools_count = pools_count,
            success_count = success_count,
            failure_count = failure_count,
            lock_wait_us = lock_wait_time.as_micros(),
            update_time_us = update_time.as_micros(),
            snapshot_rebuild_us = snapshot_time.as_micros(),
            total_time_us = total_time.as_micros(),
            avg_time_per_pool_us = if pools_count > 0 { total_time.as_micros() / pools_count as u128 } else { 0 },
            "Batch pool update completed"
        );

        Ok(success_count)
    }

    pub async fn get_monitored_pools_grouped(&self) -> HashMap<Network, HashMap<DexId, Vec<PoolId>>> {
        let mut result: HashMap<Network, HashMap<DexId, Vec<PoolId>>> = HashMap::new();
        let manager = self.dex_manager.read().await;        
        
        // Cache this to avoid repeated method calls
        let monitored_pools: Vec<PoolId> = manager.get_monitored_pools().iter().cloned().collect();
        info!("monitored pools {:?}", monitored_pools);
        
        for pool_id in &monitored_pools {
            if let Some(dex_id) = manager.get_monitored_dex_by_pool_id(pool_id) {
                result
                    .entry(pool_id.network().clone())
                    .or_default()
                    .entry(*dex_id)
                    .or_default()
                    .push(pool_id.clone());
            }
        }
        
        result
    }

    pub async fn group_pools_by_network_and_dex(&self, pool_ids: &[PoolId]) -> HashMap<Network, HashMap<DexId, Vec<PoolId>>> {
        let mut result: HashMap<Network, HashMap<DexId, Vec<PoolId>>> = HashMap::new();

        let manager = self.dex_manager.read().await;
                
        for pool_id in pool_ids {
            if let Some(dex_id) = manager.get_monitored_dex_by_pool_id(pool_id) {
                result
                    .entry(pool_id.network().clone())
                    .or_default()
                    .entry(*dex_id)
                    .or_default()
                    .push(pool_id.clone());
            }
        }
        
        result
    }

    pub async fn get_stale_pools(&self) -> Vec<PoolId> {
        debug!("Retrieving stale pools from state manager");
        let manager = self.dex_manager.read().await;
        manager.get_stale_pools()
    }
}