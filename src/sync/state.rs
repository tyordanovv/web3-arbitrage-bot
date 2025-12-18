use std::{collections::HashMap, sync::Arc};

use tokio::sync::{RwLock, mpsc};
use tracing::{debug, info, error, warn};
use std::time::{Duration, Instant};

use crate::{dex::registry::DexRegistry, sync::{reader::{PoolSnapshot, StateReader}, update_producer::PoolUpdate}, types::{BotError, DexId, Network, Result, SwapDelta, pool_state::{ PoolId, PoolState}}};

pub struct StateUpdater {
    batch_rx: mpsc::Receiver<Vec<PoolUpdate>>,
    // TODO RwLock on teh pool id not the whole state
    // TODO maybe sharded write state
    current_state: RwLock<HashMap<PoolId, PoolState>>,
    reader: Arc<StateReader>,
    dex_registry: Arc<DexRegistry>,
    // config: StateUpdaterConfig,
    // metrics: StateUpdaterMetrics,
    last_snapshot_publish: Instant,
}

impl StateUpdater {
    pub fn new(
        batch_rx: mpsc::Receiver<Vec<PoolUpdate>>,
        reader: Arc<StateReader>,
        dex_registry: Arc<DexRegistry>,
        // config: Option<StateUpdaterConfig>,
    ) -> Self {
        Self {
            batch_rx,
            current_state: RwLock::new(HashMap::new()),
            reader,
            dex_registry,
            // config: config.unwrap_or_default(),
            // metrics: StateUpdaterMetrics::new(),
            last_snapshot_publish: Instant::now(),
        }
    }

    /// Main event loop that processes batches of updates
    pub async fn run(&mut self) -> Result<()> {
        info!("StateUpdater started");
        
        while let Some(batch) = self.batch_rx.recv().await {
            self.process_batch(batch).await?;
            
            // Check if we should rebuild snapshot
            if self.should_rebuild_snapshot() {
                self.rebuild_snapshot().await?;
                self.last_snapshot_publish = Instant::now();
            }
        }
        
        info!("StateUpdater shutdown");
        Ok(())
    }
    
    /// Process a batch of updates
    async fn process_batch(&self, batch: Vec<PoolUpdate>) -> Result<()> {
        let start = Instant::now();
        let batch_size = batch.len();
        
        for update in batch {
            match update {
                PoolUpdate::FullState(pool_state) => {
                    self.update_pool(pool_state).await?;
                }
                PoolUpdate::Delta(delta) => {
                    self.apply_delta(delta).await?;
                }
            }
        }
        debug!(batch_size = batch_size, process_time_ms = start.elapsed().as_millis(), "Processed batch");
        Ok(())
    }

    /// Update or insert a pool with full state
    pub async fn update_pool(&self, pool_state: PoolState) -> Result<()> {
        if !self.dex_registry.is_monitored(&pool_state.pool_id) {
            debug!(pool_id = ?pool_state.pool_id, dex_id = ?pool_state.dex_id, "Ignoring unmonitored pool" );
            return Ok(());
        }
        
        let mut current_state = self.current_state.write().await;
        let is_update = current_state.contains_key(&pool_state.pool_id);
        
        current_state.insert(pool_state.pool_id.clone(), pool_state.clone());
        
        // Record the update in registry for freshness tracking
        // Note: We need to handle the mutability issue - either:
        // 1. Use interior mutability (Arc<Mutex<DexRegistry>>)
        // 2. Make DexRegistry methods accept &self with interior mutability
        // 3. Skip this tracking if not critical
        
        debug!(pool_id = ?pool_state.pool_id, dex_id = ?pool_state.dex_id, is_update = is_update, "Updated pool state");
        
        Ok(())
    }    

    /// Apply a delta update to an existing pool
    async fn apply_delta(&self, delta: SwapDelta) -> Result<()> {
        // Quick check: is this pool monitored?
        if !self.dex_registry.is_monitored(&delta.pool_id) {
            debug!(
                pool_id = ?delta.pool_id,
                "Delta received for unmonitored pool, ignoring"
            );
            return Ok(());
        }
        
        let mut current_state = self.current_state.write().await;
        
        match current_state.get_mut(&delta.pool_id) {
            Some(pool_state) => {
                // Validate pool belongs to correct DEX using registry
                let expected_dex = self.dex_registry.get_dex_for_pool(&delta.pool_id);
                if let Some(expected_dex) = expected_dex {
                    if *expected_dex != delta.dex_id {
                        return Err(BotError::Event(format!(
                            "DEX mismatch for pool {}: expected {}, got {}", 
                            delta.pool_id, expected_dex, delta.dex_id
                        )));
                    }
                }
                
                // TODO: Implement proper delta application with fee handling
                // This should call into DEX-specific logic via registry
                warn!(pool_id = ?delta.pool_id, "Delta application not yet implemented");
                
                // Update timestamp
                // pool_state.block_timestamp = delta.timestamp;
                
                debug!(pool_id = ?delta.pool_id, dex_id = ?delta.dex_id, "Applied delta update");
            }
            None => {
                // Pool exists in registry but not in current state
                // This might happen if we just started monitoring this pool
                // We'll need to fetch the full state from RPC
                warn!(pool_id = ?delta.pool_id, "Delta received for monitored but uninitialized pool");
                // TODO: Trigger full state fetch for this pool
            }
        }
        
        Ok(())
    }

    /// Rebuild the snapshot from current state and publish to readers
    async fn rebuild_snapshot(&self) -> Result<()> {
        let start = Instant::now();
        
        // Read current state (minimize lock time)
        let pools_by_dex = {
            let current_state = self.current_state.read().await;
            self.group_pools_by_dex(&current_state)
        };
        
        let snapshot = PoolSnapshot::new(pools_by_dex);
        self.reader.update_snapshot(snapshot);
        
        info!(rebuild_time_us = start.elapsed().as_micros(), "Rebuilt snapshot");
        
        Ok(())
    }

    /// Group pools by DEX for snapshot creation
    /// Use DexRegistry to validate and potentially enrich the grouping
    fn group_pools_by_dex(&self, state: &HashMap<PoolId, PoolState>) -> HashMap<DexId, Vec<PoolState>> {
        let mut pools_by_dex: HashMap<DexId, Vec<PoolState>> = HashMap::new();
        
        for pool_state in state.values() {
            // Only include monitored pools
            if self.dex_registry.is_monitored(&pool_state.pool_id) {
                pools_by_dex
                    .entry(pool_state.dex_id)
                    .or_insert_with(Vec::new)
                    .push(pool_state.clone());
            }
        }
        
        pools_by_dex
    }

    /// Determine if we should rebuild the snapshot
    fn should_rebuild_snapshot(&self) -> bool {
        let elapsed = self.last_snapshot_publish.elapsed();
        elapsed > Duration::from_millis(100)
    }
}