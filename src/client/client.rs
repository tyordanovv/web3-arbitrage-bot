use async_trait::async_trait;
use sui_sdk::{rpc_types::{SuiObjectData, SuiObjectDataOptions}, types::base_types::ObjectID};

use crate::types::{Network, Result};

use super::sui_rpc::SuiRpcClient;

#[async_trait]
pub trait RpcClient: Send + Sync {
    async fn batch_get_objects(
        &self,
        object_ids: Vec<ObjectID>,
        options: Option<SuiObjectDataOptions>,
        batch_size: usize,
        delay_ms: u64,
    ) -> Result<Vec<SuiObjectData>>;

    async fn get_object(
        &self,
        object_id: ObjectID,
        options: Option<SuiObjectDataOptions>,
    ) -> Result<Option<SuiObjectData>>;

    fn get_network(&self) -> Network;
}

/// Enum for static dispatch of RPC clients.
/// This avoids dynamic dispatch overhead and allows better compiler optimizations.
/// Add new variants here as you support more blockchain networks.
#[derive(Clone)]
pub enum RpcClientEnum {
    Sui(SuiRpcClient),
    // Ethereum(EthereumRpcClient),
    // Solana(SolanaRpcClient),
}

impl RpcClientEnum {
    /// Batch fetch multiple objects from the blockchain.
    pub async fn batch_get_objects(
        &self,
        object_ids: Vec<ObjectID>,
        options: Option<SuiObjectDataOptions>,
        batch_size: usize,
        delay_ms: u64,
    ) -> Result<Vec<SuiObjectData>> {
        match self {
            RpcClientEnum::Sui(client) => {
                client.batch_get_objects(object_ids, options, batch_size, delay_ms).await
            }
        }
    }

    /// Fetch a single object from the blockchain.
    pub async fn get_object(
        &self,
        object_id: ObjectID,
        options: Option<SuiObjectDataOptions>,
    ) -> Result<Option<SuiObjectData>> {
        match self {
            RpcClientEnum::Sui(client) => client.get_object(object_id, options).await,
        }
    }

    /// Get the network this client is connected to.
    pub fn get_network(&self) -> Network {
        match self {
            RpcClientEnum::Sui(client) => client.get_network(),
        }
    }
}