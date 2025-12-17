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

pub struct SyncOrchestrator {
    update_producer: Arc<UpdateProducer>, 
    pool_fetcher: Arc<PoolStateFetcher>,
    config: SyncConfig,
    dex_registry: Arc<DexRegistry>
}

#[derive(Debug, Clone)]
pub enum SyncType {
    Initial,
    All,
    Stale,
}

impl SyncOrchestrator {
    pub fn new(
        update_producer: Arc<UpdateProducer>, 
        pool_fetcher: Arc<PoolStateFetcher>,
        config: SyncConfig,
        dex_registry: Arc<DexRegistry>
    ) -> Self {
        Self {
            update_producer, 
            pool_fetcher,
            config,
            dex_registry,
        }
    }

    pub async fn initialize(&self) -> Result<()> {
        self.sync_pools(SyncType::Initial).await?;
        Ok(())
    }

    pub async fn sync_pools(&self, sync_type: SyncType) -> Result<usize> {
        
        let pools_by_network_dex = match sync_type {
            SyncType::Initial | SyncType::All => {
                self.dex_registry.get_monitored_pools_grouped()
            }
            SyncType::Stale => {
                let stale_pools = self.dex_registry.get_stale_pools(Duration::from_secs(3600));
                if stale_pools.is_empty() {
                    debug!("No stale pools found");
                    return Ok(0);
                }
                self.dex_registry.group_pools_by_network_and_dex(&stale_pools)
            }
        };

        if pools_by_network_dex.is_empty() {
            debug!("No pools to sync for {:?}", sync_type);
            return Ok(0);
        }

        let total_pools: usize = pools_by_network_dex
            .values()
            .flat_map(|dex_map| dex_map.values())
            .map(|pools| pools.len())
            .sum();

        info!("Syncing {} pools for {:?}", total_pools, sync_type);
        self.sync_pools_grouped(pools_by_network_dex).await
    }

    async fn sync_pools_grouped(
        &self, 
        pools_by_network_dex: HashMap<Network, HashMap<DexId, Vec<PoolId>>>
    ) -> Result<usize> {
        let mut success_count = 0;
        let total_pools: usize = pools_by_network_dex
            .values()
            .flat_map(|dex_map| dex_map.values())
            .map(|pools| pools.len())
            .sum();

        for (network, dex_map) in pools_by_network_dex {
            for (dex_id, pools) in dex_map {
                if pools.is_empty() {
                    continue;
                }

                info!("Fetching {} {} pools on {}", pools.len(), dex_id, network);
                
                match self.pool_fetcher.fetch_batch(&network, &dex_id, &pools).await {
                    Ok(pool_states) => {
                        let updates = pool_states
                            .into_iter()
                            .map(PoolUpdate::FullState)
                            .collect::<Vec<_>>();

                        let updated = self
                            .update_producer
                            .update_multiple_pools(updates)
                            .await?;
                        // let updated = self.update_producer.update_multiple_pools(PoolUpdate::FullState(pool_states)).await?;
                        success_count += updated;
                        debug!("Updated {}/{} {} pools on {}", updated, pools.len(), dex_id, network);
                    }
                    Err(e) => {
                        error!("Failed to fetch {} pools on {}: {}", dex_id, network, e);
                    }
                }
            }
        }

        info!("Sync completed: {}/{} pools updated", success_count, total_pools);
        Ok(success_count)
    }
}

pub struct SyncOrchestratorBuilder {
    rpc_endpoint: Option<String>,
    update_producer: Option<Arc<UpdateProducer>>, 
    config: Option<SyncConfig>,
    dex_registry: Option<Arc<DexRegistry>>,
}

impl SyncOrchestratorBuilder {
    pub fn new() -> Self {
        Self {
            rpc_endpoint: None,
            config: None,
            update_producer: None,
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

    pub fn with_update_producer(mut self, update_producer: Arc<UpdateProducer>) -> Self {
        self.update_producer = Some(update_producer);
        self
    }

    pub fn with_dex_registry(mut self, dex_registry: Arc<DexRegistry>) -> Self {
        self.dex_registry = Some(dex_registry);
        self
    }
    

    pub async fn build(self) -> Result<SyncOrchestrator> {
        let rpc_endpoint = self.rpc_endpoint
            .ok_or_else(|| BotError::Config("RPC endpoint is required".to_string()))?;
        
        let config = self.config
            .ok_or_else(|| BotError::Config("SyncConfig is required".to_string()))?;

        let update_producer = self.update_producer
            .ok_or_else(|| BotError::Config("Update sender is required".to_string()))?;

        let dex_registry = self.dex_registry
            .ok_or_else(|| BotError::Config("DexRegistry is required".to_string()))?;

        let pool_fetcher = Arc::new(PoolStateFetcher::new(config.clone()).await?);

        Ok(SyncOrchestrator::new(update_producer, pool_fetcher, config, dex_registry))
    }
}

impl Default for SyncOrchestratorBuilder {
    fn default() -> Self {
        Self::new()
    }
}