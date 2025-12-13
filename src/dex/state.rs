use std::collections::HashMap;

use async_trait::async_trait;

use crate::{types::{DexId, HealthStatus, pool_state::{ PoolId, PoolState}, Price, PriceUpdate, RawEvent, Result, SwapEvent, TokenPair}};

#[async_trait]
pub trait DexState: Send + Sync {
    // ========== IDENTITY ==========
    fn dex_id(&self) -> DexId;
    
    // ========== INITIALIZATION ==========
    async fn initialize(&mut self) -> Result<()>;
    
    // ========== POOL STATE MANAGEMENT ==========
    
    /// Get current state of a specific pool
    async fn get_pool_state(&self, pool_id: &PoolId) -> Result<PoolState>;
    
    /// Get all pool states managed by this DEX
    async fn get_all_pool_states(&self) -> Result<Vec<PoolState>>;
    
    /// Update pool state (used by synchronizer)
    async fn update_pool_state(&mut self, pool_state: PoolState) -> Result<()>;
    
    /// Fetch fresh pool state from external source (RPC/API)
    async fn fetch_pool_state(&self, pool_id: &PoolId) -> Result<PoolState>;
    
    /// Fetch all pool states from external source
    async fn fetch_all_pools(&self) -> Result<Vec<PoolState>>;
    
    // ========== EVENT HANDLING ==========
    
    /// Process parsed event and update state
    fn process_swap_event(&mut self, event: SwapEvent) -> Result<PriceUpdate>;
    
    // ========== PRICE OPERATIONS ==========
    
    /// Calculate price from pool state
    fn calculate_price(&self, pool: &PoolState) -> Result<Price>;
    
    /// Get current price for a token pair
    fn get_price(&self, pair: &TokenPair) -> Option<Price>;
    
    /// Get all current prices
    fn get_all_prices(&self) -> HashMap<TokenPair, Price>;
    
    // ========== HEALTH & SYNC ==========
    
    /// Perform health check
    async fn heartbeat(&mut self) -> Result<HealthStatus>;
    
    /// Check if DEX is healthy
    fn is_healthy(&self) -> bool;
    
    /// Get last update time
    fn last_update_time(&self) -> std::time::Instant;
    
    /// Check if state is stale
    fn is_state_stale(&self) -> bool {
        self.last_update_time().elapsed() > std::time::Duration::from_secs(3600) // 1 hour
    }
    
    /// Get monitored pools for this DEX
    fn get_monitored_pools(&self) -> Vec<PoolId>;
}