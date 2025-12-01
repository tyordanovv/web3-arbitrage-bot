use std::sync::Arc;
use sui_sdk::types::base_types::ObjectID;
use tracing::{debug, info, warn, error};

use crate::{client::sui_rpc::SuiRpcClient, types::{BotError, ChainAddress, DexId, Network, Result, cetus::CetusPoolParser, pool_parser::PoolParserRegistry, pool_state::{ PoolId, PoolState}, turbos::TurbosPoolParser}, utils::config::SyncConfig};

pub struct PoolStateFetcher {
    sui_rpc_client: Arc<SuiRpcClient>,
    config: SyncConfig,
    parser_registry: PoolParserRegistry,
}

impl PoolStateFetcher {
    pub fn new(sui_rpc_client: Arc<SuiRpcClient>, config: SyncConfig) -> Self {
        let parser_registry = PoolParserRegistry::new()
            .register(CetusPoolParser::new())
            .register(TurbosPoolParser::new());
        
        Self {
            sui_rpc_client,
            config,
            parser_registry,
        }
    }

    pub async fn fetch_batch(&self, network: &Network, dex_id: &DexId, pool_ids: &[PoolId]) -> Result<Vec<PoolState>> {
        info!("Fetching batch of {} pools for network {:?}", pool_ids.len(), network);
        
        match network {
            Network::SuiMainnet => self.fetch_sui_batch(dex_id, pool_ids).await,
            _ => {
                error!("Network {} not implemented, falling back to individual fetches", network);
                Err(BotError::Sync(format!("Network {} not implemented, falling back to individual fetches", network)))
            }
        }
    }

    async fn fetch_sui_batch(&self, dex_id: &DexId, pool_ids: &[PoolId]) -> Result<Vec<PoolState>> {
        let object_ids: Vec<_> = pool_ids.iter()
            .filter_map(|pool_id| pool_id.as_sui_object_id())
            .collect();

        match self.sui_rpc_client.batch_get_objects(
            object_ids,
            None,
            self.config.batch_size,
            2000
        ).await {
            Ok(sui_objects) => {
                info!("Successfully batch fetched {} pools", sui_objects.len());
                let pool_states = self.parser_registry.parse_batch(sui_objects, dex_id);
                Ok(pool_states)
            }
            Err(e) => {
                warn!("Batch fetch failed: {}", e);
                Err(e)
            }
        }
    }

    async fn fetch_pool_state(&self, pool_id: &PoolId) -> Result<PoolState> {
        // Implementation depends on your specific PoolState parsing logic
        // This should call the appropriate RPC method based on network
        todo!("Implement pool state fetching for specific network")
    }
}

#[cfg(test)]
mod tests {
    use sui_sdk::rpc_types::SuiObjectDataOptions;

    use super::*;
    use std::str::FromStr;

    const CETUS_USDC_HASUI_POOL: &str = "0x7d44018fbc32f456b6d0122206041a2cc159bdde32911b4be94a4e5840890764";
    const CETUS_WAL_SUI_POOL: &str = "0x72f5c6eef73d77de271886219a2543e7c29a33de19a6c69c5cf1899f729c3f17";

    #[tokio::test]
    async fn test_client_creation() {
        let result = SuiRpcClient::new().await;
        assert!(result.is_ok(), "Failed to create SuiRpcClient: {:?}", result.err());
    }

    #[tokio::test]
    async fn test_batch_get_objects_with_cetus_pools() {
        let client = SuiRpcClient::new().await.unwrap();

        let pool_ids = vec![
            ObjectID::from_str(CETUS_USDC_HASUI_POOL).unwrap(),
            ObjectID::from_str(CETUS_WAL_SUI_POOL).unwrap(),
        ];

        let result = client
            .batch_get_objects(pool_ids.clone(), Some(SuiObjectDataOptions::full_content()), 10, 0)
            .await;

        assert!(result.is_ok(), "Failed to fetch pools: {:?}", result.err());
        let pools = result.unwrap();

        assert!(!pools.is_empty(), "Expected to fetch at least one pool");
        println!("Fetched {} pools", pools.len());

        for pool in &pools {
            println!("  - Pool {}: version {}", pool.object_id, pool.version);
        }
    }

    #[tokio::test]
    async fn test_empty_object_list() {
        let client = SuiRpcClient::new().await.unwrap();

        let result = client
            .batch_get_objects(vec![], Some(SuiObjectDataOptions::default()), 10, 0)
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
        println!("Empty list handled correctly");
    }

    #[tokio::test]
    async fn test_batching_with_multiple_pools() {
        let client = SuiRpcClient::new().await.unwrap();

        let pool_ids = vec![
            ObjectID::from_str(CETUS_USDC_HASUI_POOL).unwrap(),
            ObjectID::from_str(CETUS_WAL_SUI_POOL).unwrap(),
        ];

        let start = std::time::Instant::now();
        let result = client
            .batch_get_objects(
                pool_ids,
                Some(SuiObjectDataOptions::full_content()),
                1,
                1000,
            )
            .await;
        let duration = start.elapsed();

        assert!(result.is_ok());
        assert!(duration.as_millis() >= 1000, "Expected at least 1000ms delay");
        println!("Batching with delay works: {}ms", duration.as_millis());
    }

    #[tokio::test]
    async fn test_no_delay_on_last_batch() {
        let client = SuiRpcClient::new().await.unwrap();

        let pool_ids = vec![
            ObjectID::from_str(CETUS_USDC_HASUI_POOL).unwrap(),
            ObjectID::from_str(CETUS_WAL_SUI_POOL).unwrap(),
        ];

        let start = std::time::Instant::now();
        let result = client
            .batch_get_objects(
                pool_ids,
                Some(SuiObjectDataOptions::full_content()),
                10,
                500, 
            )
            .await;
        let duration = start.elapsed();

        assert!(result.is_ok());
        assert!(duration.as_millis() < 400, "Unexpected delay on incomplete batch");
        println!("No delay on last incomplete batch: {}ms", duration.as_millis());
    }
}