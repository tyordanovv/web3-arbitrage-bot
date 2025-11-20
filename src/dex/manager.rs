use tracing::{ info, debug, warn, error };

use crate::{dex::state::DexState, types::{BotError, DexId, HealthStatus, Network, PoolId, PoolState, Price, Result, StateSnapshot, TokenPair}};
use std::{collections::{HashMap, HashSet}, u64};
pub struct DexManager {
    dexes: HashMap<DexId, Box<dyn DexState>>,
    pool_to_dex: HashMap<PoolId, DexId>,
    monitored_pools: HashSet<PoolId>,
    max_pools_per_dex: usize,
    state_ttl: std::time::Duration,
    last_sync_time: std::time::Instant,
    sync_failures: u32,
}

impl DexManager {
    pub fn new() -> Self {
        Self {
            dexes: HashMap::new(),
            pool_to_dex: HashMap::new(),
            monitored_pools: HashSet::new(),
            max_pools_per_dex: u64::MAX as usize,
            state_ttl: std::time::Duration::from_secs(3600),
            last_sync_time: std::time::Instant::now(),
            sync_failures: 0,
        }
    }
    
    pub fn with_config(max_pools_per_dex: usize, state_ttl: std::time::Duration) -> Self {
        Self {
            max_pools_per_dex,
            state_ttl,
            ..Self::new()
        }
    }
        
    pub fn register_dex(&mut self, dex: Box<dyn DexState>) -> Result<()> {
        let dex_id = dex.dex_id();
        
        if self.dexes.contains_key(&dex_id) {
            return Err(BotError::Dex { dex: dex_id.clone(), message: format!("DEX {} already registered", dex_id) });
        }
        
        self.dexes.insert(dex_id.clone(), dex);
        info!("Registered DEX: {}", dex_id);
        Ok(())
    }
    
    pub fn register_pool(&mut self, dex_id: &DexId, pool_id: PoolId) -> Result<()> {
        if !self.dexes.contains_key(dex_id) {
            return Err(BotError::Dex { dex: dex_id.clone(), message: format!("DEX {} not registered", dex_id) });
        }
        
        if self.monitored_pools.len() >= self.max_pools_per_dex * self.dexes.len() {
            return Err(BotError::Dex { dex: dex_id.clone(), message: "Maximum monitored pools limit reached".to_string() });
        }
        
        self.pool_to_dex.insert(pool_id.clone(), dex_id.clone());
        self.monitored_pools.insert(pool_id.clone());
        
        debug!("Registered pool {} for DEX {}", pool_id, dex_id);
        Ok(())
    }

    pub fn register_pools(&mut self, dex_id: &DexId, pool_ids: Vec<PoolId>) -> Result<()> {
        for pool_id in pool_ids {
            self.register_pool(dex_id, pool_id)?;
        }
        Ok(())
    }
    
    pub async fn initialize_all(&mut self) -> Result<Vec<DexId>> {
        info!("Initializing all DEX adapters...");
        
        let mut successful_dexes = Vec::new();
        
        for (dex_id, dex) in self.dexes.iter_mut() {
            if dex.initialize().await.is_ok() {
                successful_dexes.push(dex_id.clone());
                info!("Successfully initialized DEX: {}", dex_id);
            } else {
                error!("Failed to initialize DEX: {}", dex_id);
            }
        }
        
        info!("DEX initialization completed: {}/{} successful", 
              successful_dexes.len(), self.dexes.len());
        
        Ok(successful_dexes)
    }
    
    // ========== PRICE METHODS ==========
    
    /// Get all current prices for a pair across all DEXs
    pub fn get_all_prices(&self, pair: &TokenPair) -> Vec<(DexId, Price)> {
        self.dexes.iter()
            .filter_map(|(dex_id, dex)| {
                dex.get_price(pair)
                    .map(|price| (dex_id.clone(), price))
            })
            .collect()
    }
    
    pub fn get_dex(&self, dex_id: &DexId) -> Option<&dyn DexState> {
        self.dexes.get(dex_id).map(|dex| &**dex)
    }
    
    pub fn get_dex_mut(&mut self, dex_id: &DexId) -> Option<&mut Box<dyn DexState>> {
        self.dexes.get_mut(dex_id)
    }
    
    pub fn healthy_dexes(&self) -> HashMap<Network, Vec<DexId>> {
        let mut result: HashMap<Network, Vec<DexId>> = HashMap::new();
        
        for (dex_id, dex) in &self.dexes {
            if dex.is_healthy() {
                todo!("Determine network for DEX");
            }
        }
        
        result
    }
    
    pub async fn heartbeat_all(&mut self) -> Result<HashMap<DexId, HealthStatus>> {
        let mut results = HashMap::new();
        
        for (dex_id, dex) in self.dexes.iter_mut() {
            match dex.heartbeat().await {
                Ok(status) => {
                    todo!("Update health status tracking");
                }
                Err(e) => {
                    error!("Heartbeat failed for DEX {}: {}", dex_id, e);
                    todo!("Decide how to handle heartbeat failure")
                }
            }
        }
        
        Ok(results)
    }
    
    pub fn get_state_snapshot(&self) -> Result<StateSnapshot> {
        todo!("Implement state snapshot aggregation across DEXs")
    }
    
    // ========== STATE SYNCHRONIZATION METHODS ==========
    
    pub fn get_monitored_pools(&self) -> Vec<PoolId> {
        self.monitored_pools.iter().cloned().collect()
    }
    
    /// Get all pool states from all DEXs
    pub async fn get_all_pools(&self) -> Vec<PoolState> {
        let mut all_pools = Vec::new();
        
        for dex in self.dexes.values() {
            if let Ok(pools) = dex.get_all_pool_states().await {
                all_pools.extend(pools);
            }
        }
        
        all_pools
    }
    
    pub fn get_stale_pools(&self) -> Vec<PoolId> {
        self.get_monitored_pools()
    }
    
    pub async fn update_pool_state(&mut self, pool_state: PoolState) -> Result<()> {
        let dex_id = self.pool_to_dex.get(&pool_state.pool_id)
            .ok_or_else(|| BotError::NotFound(format!("Pool {} not monitored", pool_state.pool_id)))?;
        
        let dex = self.dexes.get_mut(dex_id)
            .ok_or_else(|| BotError::NotFound(format!("DEX {} not found", dex_id)))?;
        
        dex.update_pool_state(pool_state).await?;
        self.last_sync_time = std::time::Instant::now();
        
        Ok(())
    }
    
    pub async fn update_pool_states(&mut self, pool_states: Vec<PoolState>) -> Result<usize> {
        let mut success_count = 0;
        
        for pool_state in pool_states {
            if self.update_pool_state(pool_state).await.is_ok() {
                success_count += 1;
            }
        }
        
        if success_count > 0 {
            self.last_sync_time = std::time::Instant::now();
        }
        
        Ok(success_count)
    }
    
    pub fn last_sync_time(&self) -> std::time::Instant {
        self.last_sync_time
    }
}

impl Default for DexManager {
    fn default() -> Self {
        Self::new()
    }
}