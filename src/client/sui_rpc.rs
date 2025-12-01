use sui_sdk::{SuiClient, SuiClientBuilder, rpc_types::{SuiObjectData, SuiObjectDataOptions}, types::{base_types::ObjectID, object::Object}};
use std::time::Duration;
use tokio::time::sleep;

use crate::types::Result;

/// A custom RPC client that extends SuiClient functionality.
pub struct SuiRpcClient {
    sui_client: SuiClient,
}

impl SuiRpcClient {
    pub async fn new() -> Result<Self> {
        let sui_client = SuiClientBuilder::default()
            .build_mainnet()
            .await?;
        Ok(SuiRpcClient { sui_client })
    }

    /// Fetches multiple objects in batches, with a configurable delay between requests.
    ///
    /// # Arguments
    ///
    /// * `object_ids` - A vector of object IDs to fetch.
    /// * `options` - Options to specify which fields and data to return for each object.
    /// * `batch_size` - The number of objects to fetch in a single RPC call.
    /// * `delay_ms` - The delay in milliseconds to wait after each batch request.
    pub async fn batch_get_objects(
        &self,
        object_ids: Vec<ObjectID>,
        options: Option<SuiObjectDataOptions>,
        batch_size: usize,
        delay_ms: u64,
    ) -> Result<Vec<SuiObjectData>> {
        let mut all_data = Vec::new();
        let delay = Duration::from_millis(delay_ms);

        for chunk in object_ids.chunks(batch_size) {
            let results = self.sui_client.read_api()
                .multi_get_object_with_options(
                    chunk.to_vec(),
                    options.clone().unwrap_or_else(SuiObjectDataOptions::full_content),
                )
                .await?;

            for res in results {
                if let Some(data) = res.data {
                    all_data.push(data);
                }
            }

            if chunk.len() == batch_size {
                sleep(delay).await;
            }
        }

        Ok(all_data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sui_sdk::{
        rpc_types::SuiObjectDataOptions,
        types::base_types::ObjectID,
    };
    use std::str::FromStr;

    const USDC_HASUI_POOL: &str = "0x7d44018fbc32f456b6d0122206041a2cc159bdde32911b4be94a4e5840890764";
    const HASUI_SUI_POOL: &str = "0x871d8a227114f375170f149f7e9d45be822dd003eba225e83c05ac80828596bc";

    #[tokio::test]
    async fn test_client_initialization() {
        let client = SuiRpcClient::new().await;
        assert!(client.is_ok(), "Failed to initialize SuiRpcClient");
    }

    #[tokio::test]
    async fn test_fetch_single_cetus_pool() {
        let client = SuiRpcClient::new().await.expect("Failed to create client");
        
        let pool_id = ObjectID::from_str(USDC_HASUI_POOL)
            .expect("Invalid pool address");
        
        let options = SuiObjectDataOptions::new().with_content();
        
        let results = client.batch_get_objects(
            vec![pool_id],
            Some(options),
            1,
            0,
        ).await;
        
        assert!(results.is_ok(), "Failed to fetch pool data");
        let data = results.unwrap();
        assert_eq!(data.len(), 1, "Expected 1 pool object");
        
        println!("USDC/haSUI Pool data: {:?}", data[0]);
    }

    #[tokio::test]
    async fn test_fetch_both_cetus_pools() {
        let client = SuiRpcClient::new().await.expect("Failed to create client");
        
        let pool_ids = vec![
            ObjectID::from_str(USDC_HASUI_POOL).expect("Invalid USDC/haSUI pool address"),
            ObjectID::from_str(HASUI_SUI_POOL).expect("Invalid haSUI/SUI pool address"),
        ];
        
        let options = SuiObjectDataOptions::full_content();
        
        let results = client.batch_get_objects(
            pool_ids,
            Some(options),
            2,
            0,
        ).await;
        
        assert!(results.is_ok(), "Failed to fetch pool data");
        let data = results.unwrap();
        assert_eq!(data.len(), 2, "Expected 2 pool objects");
        
        println!("Fetched {} Cetus pools", data.len());
        for (i, pool) in data.iter().enumerate() {
            println!("Pool {}: {:?}", i + 1, pool.object_id);
        }
    }

    #[tokio::test]
    async fn test_batch_fetch_with_delay() {
        let client = SuiRpcClient::new().await.expect("Failed to create client");
        
        let pool_ids = vec![
            ObjectID::from_str(USDC_HASUI_POOL).expect("Invalid pool address"),
            ObjectID::from_str(HASUI_SUI_POOL).expect("Invalid pool address"),
        ];
        
        let options = SuiObjectDataOptions::new()
            .with_content()
            .with_owner()
            .with_type();
        
        let start = std::time::Instant::now();
        
        let results = client.batch_get_objects(
            pool_ids,
            Some(options),
            1,
            100,
        ).await;
        
        let duration = start.elapsed();
        
        assert!(results.is_ok(), "Failed to fetch pool data");
        assert!(duration.as_millis() >= 100, "Expected at least 100ms delay");
        
        let data = results.unwrap();
        assert_eq!(data.len(), 2, "Expected 2 pool objects");
        
        println!("Batch fetch completed in {:?}", duration);
    }

    #[tokio::test]
    async fn test_fetch_pool_content_details() {
        let client = SuiRpcClient::new().await.expect("Failed to create client");
        
        let pool_id = ObjectID::from_str(HASUI_SUI_POOL)
            .expect("Invalid pool address");
        
        let options = SuiObjectDataOptions::new()
            .with_content()
            .with_type()
            .with_bcs();
        
        let results = client.batch_get_objects(
            vec![pool_id],
            Some(options),
            1,
            0,
        ).await;
        
        assert!(results.is_ok(), "Failed to fetch pool data");
        let data = results.unwrap();
        
        assert!(!data.is_empty(), "No data returned");
        
        let pool = &data[0];
        println!("Pool Object ID: {}", pool.object_id);
        println!("Pool Type: {:?}", pool.type_);
        println!("Pool Version: {:?}", pool.version);
        
        if let Some(content) = &pool.content {
            println!("Pool Content: {:?}", content);
        }
    }

    #[tokio::test]
    async fn test_empty_object_list() {
        let client = SuiRpcClient::new().await.expect("Failed to create client");
        
        let results = client.batch_get_objects(
            vec![],
            Some(SuiObjectDataOptions::full_content()),
            10,
            0,
        ).await;
        
        assert!(results.is_ok(), "Failed with empty object list");
        let data = results.unwrap();
        assert_eq!(data.len(), 0, "Expected empty result");
    }
}