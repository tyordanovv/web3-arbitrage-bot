use async_trait::async_trait;
use rust_decimal::Decimal;

use crate::{types::{ArbitrageOpportunity, ExecutionResult, Result}, utils::config::ExecutionConfig};

#[async_trait]
pub trait TradeExecutor: Send + Sync {
    async fn execute(&mut self, opportunity: ArbitrageOpportunity) -> ExecutionResult;
}

pub struct DefaultTradeExecutor {
    _config: ExecutionConfig,
}

impl DefaultTradeExecutor {
    pub fn new(config: ExecutionConfig) -> Self {
        Self {
            _config : config,
        }
    }
    
    async fn simulate_transaction(&self, _opportunity: &ArbitrageOpportunity) -> Result<(u64, Decimal)> {
        // TODO: Implement transaction simulation
        // Return (gas_used, simulated_profit)
        todo!()
    }
    
    async fn execute_transaction(&self, _opportunity: &ArbitrageOpportunity) -> Result<(String, u64, Decimal)> {
        // TODO: Implement actual transaction execution
        // Return (transaction_digest, gas_used, actual_profit)
        todo!()
    }
}

#[async_trait]
impl TradeExecutor for DefaultTradeExecutor {
    async fn execute(&mut self, _opportunity: ArbitrageOpportunity) -> ExecutionResult {
        todo!()
    }
}