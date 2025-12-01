use std::{collections::HashMap, sync::Arc};

use tokio::sync::RwLock;
use tracing::{debug, info, error};

use crate::{dex::manager::DexManager, types::{DexId, Network, Result, pool_state::{ PoolId, PoolState}}};

pub struct StateManager {
    dex_manager: Arc<RwLock<DexManager>>,
}

impl StateManager {
    pub fn new(dex_manager: Arc<RwLock<DexManager>>) -> Self {
        Self { dex_manager }
    }

    pub async fn update_pool(&self, pool_state: PoolState) -> Result<()> {
        debug!("Updating pool state for pool: {:?}", pool_state.pool_id);
        
        let mut manager = self.dex_manager.write().await;
        manager.update_pool_state(pool_state).await?;
        
        debug!("Successfully updated pool state");
        Ok(())
    }

    pub async fn update_multiple_pools(&self, pool_states: Vec<PoolState>) -> Result<usize> {
        info!("Updating {} pool states in state manager", pool_states.len());
        
        let mut success_count = 0;
        let mut manager = self.dex_manager.write().await;
        let pools_len = pool_states.len();
        
        for pool_state in pool_states {
            match manager.update_pool_state(pool_state.clone()).await {
                Ok(_) => {
                    success_count += 1;
                    debug!("Updated pool: {:?}", pool_state.pool_id);
                }
                Err(e) => {
                    error!("Failed to update pool {:?}: {}", pool_state.pool_id, e);
                }
            }
        }
        
        info!("Updated {}/{} pools successfully", success_count, pools_len);
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