use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};

use crate::{
    client::sui_rpc::SuiRpcClient, 
    dex::manager::DexManager, 
    sync::{fetcher::PoolStateFetcher, state::StateManager}, 
    types::{BotError, ChainAddress, DexId, Network, Result, pool_state::PoolId}, 
    utils::config::SyncConfig
};

pub struct SyncOrchestrator {
    state_manager: Arc<StateManager>,
    pool_fetcher: Arc<PoolStateFetcher>,
    config: SyncConfig,
}

#[derive(Debug, Clone)]
pub enum SyncType {
    Initial,
    All,
    Stale,
}

impl SyncOrchestrator {
    pub fn new(
        state_manager: Arc<StateManager>,
        pool_fetcher: Arc<PoolStateFetcher>,
        config: SyncConfig,
    ) -> Self {
        Self {
            state_manager,
            pool_fetcher,
            config,
        }
    }

    pub async fn initialize(&self) -> Result<()> {
        info!("initialize 1");
        self.sync_pools(SyncType::Initial).await?;
        Ok(())
    }

    pub async fn sync_pools(&self, sync_type: SyncType) -> Result<usize> {
        info!("sync_pools 2");
        let pools_by_network_dex = match sync_type {
            SyncType::Initial | SyncType::All => {
                self.state_manager.get_monitored_pools_grouped().await
            }
            SyncType::Stale => {
                let stale_pools = self.state_manager.get_stale_pools().await;
                if stale_pools.is_empty() {
                    debug!("No stale pools found");
                    return Ok(0);
                }
                self.state_manager.group_pools_by_network_and_dex(&stale_pools).await
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
                        let updated = self.state_manager.update_multiple_pools(pool_states).await?;
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
    dex_manager: Option<Arc<RwLock<DexManager>>>,
    rpc_endpoint: Option<String>,
    config: Option<SyncConfig>,
}

impl SyncOrchestratorBuilder {
    pub fn new() -> Self {
        Self {
            dex_manager: None,
            rpc_endpoint: None,
            config: None,
        }
    }

    pub fn with_dex_manager(mut self, dex_manager: Arc<RwLock<DexManager>>) -> Self {
        self.dex_manager = Some(dex_manager);
        self
    }

    pub fn with_rpc_endpoint(mut self, endpoint: String) -> Self {
        self.rpc_endpoint = Some(endpoint);
        self
    }

    pub fn with_config(mut self, config: SyncConfig) -> Self {
        self.config = Some(config);
        self
    }

    pub async fn build(self) -> Result<SyncOrchestrator> {
        let dex_manager = self.dex_manager
            .ok_or_else(|| BotError::Config("DexManager is required".to_string()))?;
        
        let rpc_endpoint = self.rpc_endpoint
            .ok_or_else(|| BotError::Config("RPC endpoint is required".to_string()))?;
        
        let config = self.config
            .ok_or_else(|| BotError::Config("SyncConfig is required".to_string()))?;

        let rpc_client = Arc::new(SuiRpcClient::new().await?);
        let state_manager = Arc::new(StateManager::new(dex_manager));
        let pool_fetcher = Arc::new(PoolStateFetcher::new(rpc_client, config.clone()));

        Ok(SyncOrchestrator::new(state_manager, pool_fetcher, config))
    }
}

impl Default for SyncOrchestratorBuilder {
    fn default() -> Self {
        Self::new()
    }
}