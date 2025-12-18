use std::{collections::HashMap, sync::Arc, time::Duration};
use futures::channel::mpsc;
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};

use crate::{
    client::sui_rpc::SuiRpcClient, 
    dex::registry::DexRegistry, 
    sync::{fetcher::PoolStateFetcher, state::StateUpdater, update_producer::{self, PoolUpdate, UpdateProducer}}, 
    types::{BotError, ChainAddress, DexId, Network, Result, SwapDelta, pool_state::PoolId}, 
    utils::config::SyncConfig
};

/// Manages periodic and on-demand synchronization of pool states
pub struct StateSynchronizer {
    update_producer: Arc<UpdateProducer>,
    pool_fetcher: Arc<PoolStateFetcher>,
    dex_registry: Arc<DexRegistry>,
    config: SyncConfig,
}

#[derive(Debug, Clone, Copy)]
pub enum SyncType {
    /// Initial full sync on startup
    Initial,
    /// Sync all monitored pools
    All,
    /// Sync only stale pools (last updated > threshold)
    Stale,
}

impl StateSynchronizer {
    pub fn new(
        update_producer: Arc<UpdateProducer>,
        pool_fetcher: Arc<PoolStateFetcher>,
        dex_registry: Arc<DexRegistry>,
        config: SyncConfig,
    ) -> Self {
        Self {
            update_producer,
            pool_fetcher,
            dex_registry,
            config,
        }
    }

    /// Perform initial synchronization on startup
    pub async fn initialize(&self) -> Result<()> {
        info!("Initializing state synchronization...");
        let synced = self.sync_pools(SyncType::Initial).await?;
        info!("Initial sync completed: {} pools synchronized", synced);
        Ok(())
    }

    /// Run periodic background synchronization
    pub async fn run_periodic_sync(&self) -> Result<()> {
        let mut interval = tokio::time::interval(Duration::from_secs(3600)); // 1 hour

        loop {
            interval.tick().await;
            
            info!("Running periodic state synchronization...");
            match self.sync_pools(SyncType::Stale).await {
                Ok(count) => {
                    info!("Periodic sync completed: {} pools updated", count);
                }
                Err(e) => {
                    error!("Periodic sync failed: {}", e);
                }
            }
        }
    }

    /// Synchronize pools based on the specified type
    pub async fn sync_pools(&self, sync_type: SyncType) -> Result<usize> {
        let pools_to_sync = self.get_pools_for_sync(sync_type)?;

        if pools_to_sync.is_empty() {
            debug!("No pools to sync for {:?}", sync_type);
            return Ok(0);
        }

        let total: usize = pools_to_sync
            .values()
            .flat_map(|dex_map| dex_map.values())
            .map(|pools| pools.len())
            .sum();

        info!("Syncing {} pools ({:?})", total, sync_type);
        
        self.fetch_and_update_pools(pools_to_sync, total).await
    }

    fn get_pools_for_sync(
        &self,
        sync_type: SyncType,
    ) -> Result<HashMap<Network, HashMap<DexId, Vec<PoolId>>>> {
        match sync_type {
            SyncType::Initial | SyncType::All => {
                Ok(self.dex_registry.get_monitored_pools_grouped())
            }
            SyncType::Stale => {
                let stale_threshold = Duration::from_secs(self.config.state_ttl_seconds);
                let stale_pools = self.dex_registry.get_stale_pools(stale_threshold);
                
                if stale_pools.is_empty() {
                    return Ok(HashMap::new());
                }
                
                Ok(self.dex_registry.group_pools_by_network_and_dex(&stale_pools))
            }
        }
    }

    async fn fetch_and_update_pools(
        &self,
        pools_by_network_dex: HashMap<Network, HashMap<DexId, Vec<PoolId>>>,
        total_pools: usize,
    ) -> Result<usize> {
        let mut success_count = 0;

        for (network, dex_map) in pools_by_network_dex {
            for (dex_id, pools) in dex_map {
                if pools.is_empty() {
                    continue;
                }

                debug!(
                    "Fetching {} {} pools on {}",
                    pools.len(),
                    dex_id,
                    network
                );

                match self.fetch_and_publish_batch(&network, &dex_id, &pools).await {
                    Ok(updated) => {
                        success_count += updated;
                        debug!(
                            "Updated {}/{} {} pools on {}",
                            updated,
                            pools.len(),
                            dex_id,
                            network
                        );
                    }
                    Err(e) => {
                        error!("Failed to fetch {} pools on {}: {}", dex_id, network, e);
                    }
                }
            }
        }

        info!(
            "Sync completed: {}/{} pools updated successfully",
            success_count, total_pools
        );
        
        Ok(success_count)
    }

    async fn fetch_and_publish_batch(
        &self,
        network: &Network,
        dex_id: &DexId,
        pools: &[PoolId],
    ) -> Result<usize> {
        let pool_states = self
            .pool_fetcher
            .fetch_batch(network, dex_id, pools)
            .await?;

        let updates: Vec<PoolUpdate> = pool_states
            .into_iter()
            .map(PoolUpdate::FullState)
            .collect();

        self.update_producer.update_multiple_pools(updates).await
    }

    pub fn builder() -> StateSynchronizerBuilder {
        StateSynchronizerBuilder::new()
    }
}

pub struct StateSynchronizerBuilder {
    rpc_endpoint: Option<String>,
    update_producer: Option<Arc<UpdateProducer>>,
    config: Option<SyncConfig>,
    dex_registry: Option<Arc<DexRegistry>>,
}

impl StateSynchronizerBuilder {
    pub fn new() -> Self {
        Self {
            rpc_endpoint: None,
            update_producer: None,
            config: None,
            dex_registry: None,
        }
    }

    pub fn with_rpc_endpoint(mut self, endpoint: String) -> Self {
        self.rpc_endpoint = Some(endpoint);
        self
    }

    pub fn with_config(mut self, config: SyncConfig) -> Self {
        self.config = Some(config);
        self
    }

    pub fn with_update_producer(mut self, producer: Arc<UpdateProducer>) -> Self {
        self.update_producer = Some(producer);
        self
    }

    pub fn with_dex_registry(mut self, registry: Arc<DexRegistry>) -> Self {
        self.dex_registry = Some(registry);
        self
    }

    pub async fn build(self) -> Result<StateSynchronizer> {
        let rpc_endpoint = self
            .rpc_endpoint
            .ok_or_else(|| BotError::Config("RPC endpoint required".to_string()))?;

        let config = self
            .config
            .ok_or_else(|| BotError::Config("SyncConfig required".to_string()))?;

        let update_producer = self
            .update_producer
            .ok_or_else(|| BotError::Config("UpdateProducer required".to_string()))?;

        let dex_registry = self
            .dex_registry
            .ok_or_else(|| BotError::Config("DexRegistry required".to_string()))?;

        let pool_fetcher = Arc::new(PoolStateFetcher::new(config.clone()).await?);

        Ok(StateSynchronizer::new(
            update_producer,
            pool_fetcher,
            dex_registry,
            config,
        ))
    }
}

impl Default for StateSynchronizerBuilder {
    fn default() -> Self {
        Self::new()
    }
}