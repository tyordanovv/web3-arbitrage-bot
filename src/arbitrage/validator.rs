use std::sync::Arc;

use async_trait::async_trait;
use tracing::debug;

use crate::{sync::reader::StateReader, types::ArbitrageOpportunity, utils::config::ValidationConfig};

#[async_trait]
pub trait OpportunityValidator: Send + Sync {
    async fn validate(&self, opportunity: &ArbitrageOpportunity) -> bool;
}

pub struct DefaultOpportunityValidator {
    state_reader: Arc<StateReader>,
    config: ValidationConfig,
}

impl DefaultOpportunityValidator {
    pub fn new(
        state_reader: Arc<StateReader>,
        config: ValidationConfig,
    ) -> Self {
        Self { state_reader, config }
    }
}

#[async_trait]
impl OpportunityValidator for DefaultOpportunityValidator {
    async fn validate(&self, _opportunity: &ArbitrageOpportunity) -> bool {
        let validation_start = std::time::Instant::now();

        // Get snapshot - LOCK-FREE READ
        let snapshot = self.state_reader.get_snapshot();

        debug!(
            pool_count = snapshot.pool_count,
            snapshot_age_ms = snapshot.snapshot_time.elapsed().as_millis(),
            "Validating opportunity with current state"
        );

        // TODO: Implement validation logic
        // - Check if prices haven't changed significantly
        // - Check if pools have sufficient liquidity
        // - Check if opportunity isn't too stale

        let validation_time = validation_start.elapsed();
        debug!(
            validation_time_us = validation_time.as_micros(),
            "Validation completed"
        );

        false // Placeholder - will return true when implemented
    }
}