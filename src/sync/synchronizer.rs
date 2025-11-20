use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};

use crate::{dex::manager::DexManager, sync::{fetcher::PoolStateFetcher, rpc::{DefaultRpcClient, RpcClient}, state::StateManager}, types::{BotError, PoolId, PoolState, Result}, utils::config::SyncConfig};

pub struct SyncOrchestrator {
    state_manager: Arc<StateManager>,
    pool_fetcher: Arc<PoolStateFetcher>,
    config: SyncConfig,
    shared_state: Arc<SyncState>,
}

struct SyncState {
    is_running: AtomicBool,
    health_status: AtomicBool,
}

impl SyncOrchestrator {
    pub fn new(
        state_manager: Arc<StateManager>,
        pool_fetcher: Arc<PoolStateFetcher>,
        config: SyncConfig,
    ) -> Self {
        info!("Creating SyncOrchestrator");
        Self {
            state_manager,
            pool_fetcher,
            config,
            shared_state: Arc::new(SyncState {
                is_running: AtomicBool::new(false),
                health_status: AtomicBool::new(true),
            }),
        }
    }

    pub async fn initialize(&self) -> Result<()> {
        info!("Initializing SyncOrchestrator...");
        
        let pools_to_sync = self.state_manager.get_monitored_pools().await;

        if pools_to_sync.is_empty() {
            warn!("No pools to sync - make sure pools are registered first");
            return Ok(());
        }

        info!("Performing initial sync for {} pools", pools_to_sync.len());
        
        let success_count = self.sync_pools_batch(&pools_to_sync).await?;
        
        if success_count == pools_to_sync.len() {
            info!("Initial sync completed successfully for all {} pools", success_count);
            self.shared_state.health_status.store(true, Ordering::Relaxed);
        } else {
            warn!(
                "Initial sync completed with {} errors out of {} pools", 
                pools_to_sync.len() - success_count, 
                pools_to_sync.len()
            );
            self.shared_state.health_status.store(false, Ordering::Relaxed);
        }

        Ok(())
    }

    pub async fn sync_all_pools(&self) -> Result<usize> {
        info!("Starting sync for all monitored pools");
        let pools_to_sync = self.state_manager.get_monitored_pools().await;
        info!("Found {} pools to sync", pools_to_sync.len());
        self.sync_pools_batch(&pools_to_sync).await
    }

    pub async fn sync_pool(&self, pool_id: &PoolId) -> Result<PoolState> {
        info!("Syncing single pool: {}", pool_id);
        
        let state = self.pool_fetcher.fetch_with_retry(pool_id).await?;
        self.state_manager.update_pool(state.clone()).await?;
        
        info!("Successfully synced pool: {}", pool_id);
        Ok(state)
    }

    pub async fn sync_stale_pools(&self) -> Result<usize> {
        info!("Starting sync for stale pools");
        let stale_pools = self.state_manager.get_stale_pools().await;

        if stale_pools.is_empty() {
            debug!("No stale pools found");
            return Ok(0);
        }

        info!("Found {} stale pools to sync", stale_pools.len());
        self.sync_pools_batch(&stale_pools).await
    }

    pub fn start_periodic_sync(&self) {
        if self.shared_state.is_running.load(Ordering::Relaxed) {
            warn!("Periodic sync already running, ignoring start request");
            return;
        }

        info!("Starting periodic state synchronization");
        self.shared_state.is_running.store(true, Ordering::Relaxed);
        
        let state_manager = Arc::clone(&self.state_manager);
        let pool_fetcher = Arc::clone(&self.pool_fetcher);
        let shared_state = Arc::clone(&self.shared_state);
        let config = self.config.clone();
        
        tokio::spawn(async move {
            Self::run_periodic_sync_loop(
                state_manager,
                pool_fetcher,
                config,
                shared_state,
            ).await;
        });
    }

    pub fn stop_periodic_sync(&self) {
        info!("Stopping periodic state synchronization");
        self.shared_state.is_running.store(false, Ordering::Relaxed);
    }

    pub fn is_healthy(&self) -> bool {
        self.shared_state.health_status.load(Ordering::Relaxed)
    }

    async fn sync_pools_batch(&self, pool_ids: &[PoolId]) -> Result<usize> {
        info!("Starting batch sync for {} pools", pool_ids.len());
        let mut success_count = 0;
        let total_pools = pool_ids.len();

        for (batch_idx, chunk) in pool_ids.chunks(self.config.batch_size).enumerate() {
            info!("Processing batch {}/{} with {} pools", 
                  batch_idx + 1, 
                  (total_pools + self.config.batch_size - 1) / self.config.batch_size,
                  chunk.len());
            
            let pool_fetcher = Arc::clone(&self.pool_fetcher);
            let state_manager = Arc::clone(&self.state_manager);
            
            // Fetch all pools in chunk concurrently
            let mut fetch_handles = Vec::new();
            
            for pool_id in chunk {
                let pool_id = pool_id.clone();
                let fetcher = Arc::clone(&pool_fetcher);
                
                let handle = tokio::spawn(async move {
                    fetcher.fetch_with_retry(&pool_id).await
                });
                
                fetch_handles.push(handle);
            }

            // Collect results
            let mut fetched_states = Vec::new();
            for handle in fetch_handles {
                match handle.await {
                    Ok(Ok(state)) => {
                        fetched_states.push(state);
                    }
                    Ok(Err(e)) => {
                        error!("Failed to fetch pool in batch: {}", e);
                    }
                    Err(join_err) => {
                        error!("Task join error in batch fetch: {}", join_err);
                    }
                }
            }

            // Update all successfully fetched states
            if !fetched_states.is_empty() {
                match state_manager.update_multiple_pools(fetched_states).await {
                    Ok(count) => success_count += count,
                    Err(e) => error!("Failed to update pools in batch: {}", e),
                }
            }

            // Small delay between batches to avoid rate limiting
            if batch_idx < (total_pools + self.config.batch_size - 1) / self.config.batch_size - 1 {
                debug!("Waiting before next batch...");
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }

        info!("Batch sync completed: {}/{} pools successful", success_count, total_pools);
        Ok(success_count)
    }

    async fn run_periodic_sync_loop(
        state_manager: Arc<StateManager>,
        pool_fetcher: Arc<PoolStateFetcher>,
        config: SyncConfig,
        shared_state: Arc<SyncState>,
    ) {
        info!("Periodic sync loop started");
        let mut interval = tokio::time::interval(config.sync_interval());

        while shared_state.is_running.load(Ordering::Relaxed) {
            interval.tick().await;
            
            info!("Running periodic state synchronization...");
            
            let stale_pools = state_manager.get_stale_pools().await;
            
            if stale_pools.is_empty() {
                debug!("No stale pools to sync in this cycle");
                continue;
            }

            info!("Found {} stale pools to sync", stale_pools.len());
            
            // Create temporary orchestrator for this sync cycle
            let temp_orchestrator = SyncOrchestrator {
                state_manager: Arc::clone(&state_manager),
                pool_fetcher: Arc::clone(&pool_fetcher),
                config: config.clone(),
                shared_state: Arc::clone(&shared_state),
            };
            
            match temp_orchestrator.sync_pools_batch(&stale_pools).await {
                Ok(success_count) => {
                    if success_count > 0 {
                        info!("Periodic sync completed: {} pools updated", success_count);
                        shared_state.health_status.store(true, Ordering::Relaxed);
                    } else {
                        warn!("Periodic sync completed but no pools were updated");
                    }
                }
                Err(e) => {
                    error!("Periodic sync failed: {}", e);
                    shared_state.health_status.store(false, Ordering::Relaxed);
                }
            }
        }
        
        info!("Periodic sync loop stopped");
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

    pub fn build(self) -> Result<SyncOrchestrator> {
        let dex_manager = self.dex_manager
            .ok_or_else(|| BotError::Config("DexManager is required".to_string()))?;
        
        let rpc_endpoint = self.rpc_endpoint
            .ok_or_else(|| BotError::Config("RPC endpoint is required".to_string()))?;
        
        let config = self.config
            .ok_or_else(|| BotError::Config("SyncConfig is required".to_string()))?;

        let rpc_client = Arc::new(DefaultRpcClient::new(rpc_endpoint)) as Arc<dyn RpcClient>;
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