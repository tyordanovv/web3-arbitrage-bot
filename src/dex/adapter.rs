use async_trait::async_trait;

use crate::{dex::state::DexState, types::{DexId, HealthStatus, PoolId, PoolState, Price, PriceUpdate, RawEvent, Result, SwapEvent, SyncResult, TokenPair}};

#[async_trait]
pub trait DexAdapter: Send + Sync {
    // ========== IDENTITY ==========
    fn dex_id(&self) -> DexId;
    
    // ========== STATE ACCESS ==========
    fn state(&self) -> &DexState;
    fn state_mut(&mut self) -> &mut DexState;
    
    // ========== INITIALIZATION ==========
    async fn initialize(&mut self) -> Result<()>;
    
    // ========== POOL FETCHING ==========
    async fn fetch_pool_state(&self, pool_id: &PoolId) -> Result<PoolState>;
    async fn fetch_all_pools(&self) -> Result<Vec<PoolState>>;
    
    // ========== EVENT HANDLING ==========

    /// Parse raw event from WebSocket
    /// Each DEX has different JSON structure
    fn parse_event(&self, raw: RawEvent) -> Result<SwapEvent>;
    
    /// Process parsed event and update state
    fn process_swap_event(&mut self, event: SwapEvent) -> Result<PriceUpdate>;
    
    // ========== PRICE OPERATIONS ==========
    fn calculate_price(&self, pool: &PoolState) -> Result<Price>;
    fn get_price(&self, pair: &TokenPair) -> Option<Price>;
    
    // ========== HEALTH & SYNC ==========
    async fn heartbeat(&mut self) -> Result<HealthStatus>;
    async fn periodic_sync(&mut self) -> Result<SyncResult>;
    fn is_healthy(&self) -> bool;
}