use std::sync::Arc;

use tracing::{debug, info, warn, error};

use crate::{sync::rpc::RpcClient, types::{BotError, PoolId, PoolState, Result}, utils::config::SyncConfig};

pub struct PoolStateFetcher {
    rpc_client: Arc<dyn RpcClient>,
    config: SyncConfig,
}

impl PoolStateFetcher {
    pub fn new(rpc_client: Arc<dyn RpcClient>, config: SyncConfig) -> Self {
        Self { rpc_client, config }
    }

    pub async fn fetch_with_retry(&self, pool_id: &PoolId) -> Result<PoolState> {
        debug!("Starting fetch with retry for pool: {}", pool_id);
        let mut last_error = None;
        
        for attempt in 0..self.config.max_retries {
            debug!("Fetch attempt {}/{} for pool: {}", attempt + 1, self.config.max_retries, pool_id);
            
            match self.rpc_client.fetch_pool_state(pool_id).await {
                Ok(state) => {
                    info!("Successfully fetched pool state for: {} on attempt {}", pool_id, attempt + 1);
                    return Ok(state);
                }
                Err(e) => {
                    warn!("Fetch attempt {} failed for pool {}: {}", attempt + 1, pool_id, e);
                    last_error = Some(e);
                    
                    if attempt < self.config.max_retries - 1 {
                        let delay = self.config.retry_delay();
                        debug!("Retrying in {:?}...", delay);
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        error!("All {} retry attempts exhausted for pool: {}", self.config.max_retries, pool_id);
        Err(BotError::Sync(format!(
            "Failed to sync pool {} after {} attempts: {:?}", 
            pool_id, 
            self.config.max_retries, 
            last_error
        )))
    }

    pub async fn fetch_batch(&self, pool_ids: &[PoolId]) -> Result<Vec<PoolState>> {
        info!("Fetching batch of {} pools", pool_ids.len());
        
        // Try batch fetch first if RPC supports it
        match self.rpc_client.fetch_multiple_pools(pool_ids).await {
            Ok(states) => {
                info!("Successfully batch fetched {} pools", states.len());
                Ok(states)
            }
            Err(e) => {
                warn!("Batch fetch failed, falling back to individual fetches: {}", e);
                self.fetch_individually(pool_ids).await
            }
        }
    }

    async fn fetch_individually(&self, pool_ids: &[PoolId]) -> Result<Vec<PoolState>> {
        let mut results = Vec::with_capacity(pool_ids.len());
        
        for pool_id in pool_ids {
            match self.fetch_with_retry(pool_id).await {
                Ok(state) => results.push(state),
                Err(e) => {
                    error!("Failed to fetch pool {} individually: {}", pool_id, e);
                }
            }
        }
        
        Ok(results)
    }
}