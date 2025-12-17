use std::{collections::{HashMap, HashSet}, time::{Duration, Instant}};

use crate::{types::{AptosAddress, BotError, ChainAddress, DexId, Network, Result, SuiAddress, pool_state::PoolId}, utils::config::DexConfig};

pub struct DexRegistry {
    pool_to_dex: HashMap<PoolId, DexId>,
    monitored_pools: HashSet<PoolId>,
    pool_last_update: HashMap<PoolId, Instant>,
}

impl DexRegistry {
    /// Creates a new DexRegistry from configurations
    pub fn from_configs(configs: &[DexConfig]) -> Result<Self> {
        let builder = DexRegistryBuilder::new()
            .with_dex_configs(configs.to_vec())
            .build()?;
        Ok(builder)
    }

    /// Get all monitored pools grouped by network and DEX
    pub fn get_monitored_pools_grouped(&self) -> HashMap<Network, HashMap<DexId, Vec<PoolId>>> {
        let mut result: HashMap<Network, HashMap<DexId, Vec<PoolId>>> = HashMap::new();
        
        for pool_id in &self.monitored_pools {
            if let Some(dex_id) = self.pool_to_dex.get(pool_id) {
                result
                    .entry(pool_id.network().clone())
                    .or_default()
                    .entry(dex_id.clone())
                    .or_default()
                    .push(pool_id.clone());   
            }
        }
        result
    }
    
    /// Group specific pools by network and DEX
    pub fn group_pools_by_network_and_dex(
        &self, 
        pool_ids: &[PoolId]
    ) -> HashMap<Network, HashMap<DexId, Vec<PoolId>>> {
        let mut result: HashMap<Network, HashMap<DexId, Vec<PoolId>>> = HashMap::new();
        
        for pool_id in pool_ids {
            if let Some(dex_id) = self.pool_to_dex.get(pool_id) {
                result
                    .entry(pool_id.network().clone())
                    .or_default()
                    .entry(dex_id.clone())
                    .or_default()
                    .push(pool_id.clone());
            }
        }
        result
    }
    
    /// Get stale pools (haven't been updated recently)
    pub fn get_stale_pools(&self, max_age: Duration) -> Vec<PoolId> {
        let now = Instant::now();
        self.monitored_pools
            .iter()
            .filter(|pool_id| {
                self.pool_last_update
                    .get(pool_id)
                    .map(|last_update| now.duration_since(*last_update) > max_age)
                    .unwrap_or(true) // Never updated pools are considered stale
            })
            .cloned()
            .collect()
    }
    
    /// Get DEX for a specific pool
    pub fn get_dex_for_pool(&self, pool_id: &PoolId) -> Option<&DexId> {
        self.pool_to_dex.get(pool_id)
    }
    
    /// Check if a pool is being monitored
    pub fn is_monitored(&self, pool_id: &PoolId) -> bool {
        self.monitored_pools.contains(pool_id)
    }
}

/// Builder pattern for DexRegistry with sensible defaults
pub struct DexRegistryBuilder {
    max_pools_per_dex: usize,
    state_ttl: Duration,
    dex_configs: Vec<DexConfig>,
}

impl Default for DexRegistryBuilder {
    fn default() -> Self {
        Self {
            max_pools_per_dex: 1000, // Sensible default
            state_ttl: Duration::from_secs(3600), // 1 hour default
            dex_configs: Vec::new(),
        }
    }
}

impl DexRegistryBuilder {
    /// Creates a new builder with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum pools per DEX (for rate limiting/performance)
    pub fn with_max_pools_per_dex(mut self, max_pools: usize) -> Self {
        self.max_pools_per_dex = max_pools;
        self
    }

    /// Set state time-to-live (TTL) for pool updates
    pub fn with_state_ttl(mut self, ttl: Duration) -> Self {
        self.state_ttl = ttl;
        self
    }

    /// Add DEX configurations
    pub fn with_dex_configs(mut self, dex_configs: Vec<DexConfig>) -> Self {
        self.dex_configs = dex_configs;
        self
    }

    pub fn build(self) -> Result<DexRegistry> {
        let mut pool_to_dex = HashMap::new();
        let mut monitored_pools = HashSet::new();
        
        for config in &self.dex_configs {
            if !config.enabled {
                continue;
            }
            
            let pools_to_monitor = config.pools
                .iter()
                .filter(|pool_config| {
                    match config.network {
                        Network::SuiMainnet => {
                            SuiAddress::from_str(&pool_config.address).is_ok()
                        }
                        Network::AptosMainnet => {
                            !pool_config.address.is_empty()
                        }
                        _ => false,
                    }
                })
                .take(self.max_pools_per_dex)
                .collect::<Vec<_>>();
            
            for pool_config in pools_to_monitor {
                let chain_address = match config.network {
                    Network::SuiMainnet => {
                        let sui_addr = SuiAddress::from_str(&pool_config.address)
                            .map_err(|e| BotError::Parse(format!("Invalid Sui address '{}': {}", pool_config.address, e)))?;
                        ChainAddress::Sui(sui_addr)
                    }
                    Network::AptosMainnet => {
                        ChainAddress::Aptos(AptosAddress::new(pool_config.address.clone()))
                    }
                    _ => return Err(BotError::Parse(format!("Unsupported network for DEX: {:?}", config.network))),
                };
                
                monitored_pools.insert(chain_address.clone());
                pool_to_dex.insert(chain_address, config.id.clone());
            }
        }
        
        // Initialize last update times to now
        let pool_last_update = monitored_pools
            .iter()
            .map(|pool_id| (pool_id.clone(), Instant::now()))
            .collect();
        
        Ok(DexRegistry {
            pool_to_dex,
            monitored_pools,
            pool_last_update,
        })
    }
}