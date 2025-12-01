use std::result::Result::Ok;
use sui_sdk::types::base_types::ObjectID;
use tracing::{ info, debug, warn, error };

use crate::{dex::{cetus::CetusDexState, state::DexState}, types::{BotError, ChainAddress, DexId, HealthStatus, Network, Price, Result, StateSnapshot, SuiAddress, Timestamp, TokenPair, now, pool, pool_state::{ PoolId, PoolState}}, utils::config::DexConfig};
use std::{collections::{HashMap, HashSet}, u64};
pub struct DexManager {
    dexes: HashMap<DexId, Box<dyn DexState>>,
    pool_to_dex: HashMap<PoolId, DexId>,
    monitored_pools: HashSet<PoolId>,
    max_pools_per_dex: usize,
    state_ttl: std::time::Duration,
    last_sync_time: Timestamp,
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
            last_sync_time: now(),
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
        
        debug!("Registered pool {:?} for DEX {}", pool_id, dex_id);
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

    pub fn get_monitored_dex_by_pool_id(&self, pool_id: &PoolId) -> Option<&DexId> {
        self.pool_to_dex.get(pool_id)
    }
    
    pub async fn update_pool_state(&mut self, pool_state: PoolState) -> Result<()> {
        let dex_id = self.get_monitored_dex_by_pool_id(&pool_state.pool_id)
            .cloned()
            .ok_or_else(|| BotError::NotFound(format!("Pool {} is not monitored", &pool_state.pool_id)))?;
        
        let dex = self.dexes.get_mut(&dex_id)
            .ok_or_else(|| BotError::NotFound(format!("DEX {} not found", dex_id)))?;
        
        dex.update_pool_state(pool_state).await?;
        self.last_sync_time = now();
        
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
            self.last_sync_time = now();
        }
        
        Ok(success_count)
    }
    
    pub fn last_sync_time(&self) -> Timestamp {
        self.last_sync_time
    }
}

impl Default for DexManager {
    fn default() -> Self {
        Self::new()
    }
}

pub struct DexManagerBuilder {
    max_pools_per_dex: Option<usize>,
    state_ttl: Option<std::time::Duration>,
    dex_configs: Vec<DexConfig>,
}

impl DexManagerBuilder {
    pub fn new() -> Self {
        Self {
            max_pools_per_dex: None,
            state_ttl: None,
            dex_configs: Vec::new(),
        }
    }

    pub fn with_max_pools_per_dex(mut self, max_pools: usize) -> Self {
        self.max_pools_per_dex = Some(max_pools);
        self
    }

    pub fn with_state_ttl(mut self, ttl: std::time::Duration) -> Self {
        self.state_ttl = Some(ttl);
        self
    }

    pub fn with_dex_configs(mut self, dex_configs: Vec<DexConfig>) -> Self {
        self.dex_configs = dex_configs;
        self
    }

    pub fn add_dex_config(mut self, dex_config: DexConfig) -> Self {
        self.dex_configs.push(dex_config);
        self
    }

    pub fn build(self) -> Result<DexManager> {
        let max_pools_per_dex = self.max_pools_per_dex.unwrap_or(u64::MAX as usize);
        let state_ttl = self.state_ttl.unwrap_or_else(|| std::time::Duration::from_secs(3600));

        let mut dex_manager = DexManager::with_config(max_pools_per_dex, state_ttl);
        
        Self::register_dexes_and_pools(&mut dex_manager, &self.dex_configs)?;
        
        Ok(dex_manager)
    }

    fn register_dexes_and_pools(
        dex_manager: &mut DexManager, 
        dex_configs: &[DexConfig]
    ) -> Result<()> {
        info!("Registering DEXes and pools from config");
        
        for dex_config in dex_configs {
            if !dex_config.enabled {
                info!("Skipping disabled DEX: {}", dex_config.id);
                continue;
            }
            
            let dex_state: Box<dyn DexState> = Self::create_dex_state(dex_config)?;
            dex_manager.register_dex(dex_state)?;
            info!("Registered DEX: {}", dex_config.id);
        }
        
        // Then register all pools for each DEX
        for dex_config in dex_configs {
            if !dex_config.enabled {
                continue;
            }
                        
            let pool_ids: Vec<PoolId> = dex_config.pools.iter()
                .map(|pool_config| {
                    let object_id = ObjectID::from_hex(&pool_config.address)
                        .map_err(|e| BotError::Config(format!("Invalid pool address {}: {}", pool_config.address, e)))?;
                    
                    ChainAddress::Sui(SuiAddress::new(object_id))
                })
                .collect::<Result<Vec<_>>>()?;
            
            dex_manager.register_pools(&dex_config.id, pool_ids)?;
            info!("Registered {} pools for DEX: {}", dex_config.pools.len(), dex_config.id);
        }
        
        info!("Successfully registered {} DEXes with total {} pools", 
              dex_configs.iter().filter(|d| d.enabled).count(),
              dex_configs.iter().map(|d| d.pools.len()).sum::<usize>());
        
        Ok(())
    }

    fn create_dex_state(dex_config: &DexConfig) -> Result<Box<dyn DexState>> {
        match dex_config.id {
            DexId::Cetus => Ok(Box::new(CetusDexState::from_config(dex_config))),
            _ => Err(BotError::Config(format!("Unknown DEX ID: {}", dex_config.id))),
        }
    }
}

impl Default for DexManagerBuilder {
    fn default() -> Self {
        Self::new()
    }
}