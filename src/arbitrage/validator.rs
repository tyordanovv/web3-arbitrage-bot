use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::{dex::manager::DexManager, types::ArbitrageOpportunity, utils::config::ValidationConfig};

#[async_trait]
pub trait OpportunityValidator: Send + Sync {
    async fn validate(&self, opportunity: &ArbitrageOpportunity) -> bool;
}

pub struct DefaultOpportunityValidator {
    dex_manager: Arc<RwLock<DexManager>>,
    config: ValidationConfig,
}

impl DefaultOpportunityValidator {
    pub fn new(
        dex_manager: Arc<RwLock<DexManager>>,
        config: ValidationConfig,
    ) -> Self {
        Self { dex_manager, config }
    }
}

#[async_trait]
impl OpportunityValidator for DefaultOpportunityValidator {
    async fn validate(&self, _opportunity: &ArbitrageOpportunity) -> bool {
        // Check if prices haven't changed significantly
        // Check if pools have sufficient liquidity
        // Check if opportunity isn't too stale
        todo!("Quick validation logic")
    }
}