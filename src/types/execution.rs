use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::types::{ArbitrageOpportunity, BotError, Timestamp, now};

/// Result of executing an arbitrage trade
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Original opportunity that was executed
    pub opportunity: ArbitrageOpportunity,
    
    /// Execution status
    pub status: ExecutionStatus,
    
    /// Transaction digest if successful
    pub transaction_digest: Option<String>,
    
    /// Actual amounts at each hop (may differ from expected due to slippage)
    pub actual_amounts: HashMap<usize, u64>, // hop_index -> actual_amount
    
    /// Gas used for the transaction
    pub gas_used: u64,
    
    /// Actual profit after all costs
    pub actual_profit: Decimal,
    
    /// Execution duration in milliseconds
    pub execution_duration_ms: u64,
    
    /// Timestamp when execution started
    pub started_at: Timestamp,
    
    /// Timestamp when execution completed
    pub completed_at: Timestamp,
    
    /// Error details if execution failed
    pub error: Option<String>,
    
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl ExecutionResult {
    /// Create a new execution result
    pub fn new(opportunity: ArbitrageOpportunity) -> Self {
        let started_at = now();
        Self {
            opportunity,
            status: ExecutionStatus::Pending,
            transaction_digest: None,
            actual_amounts: HashMap::new(),
            gas_used: 0,
            actual_profit: Decimal::ZERO,
            execution_duration_ms: 0,
            started_at,
            completed_at: started_at,
            error: None,
            metadata: HashMap::new(),
        }
    }
    
    /// Mark execution as successful
    pub fn success(
        mut self,
        transaction_digest: String,
        actual_amounts: HashMap<usize, u64>,
        gas_used: u64,
        actual_profit: Decimal,
    ) -> Self {
        self.status = ExecutionStatus::Success;
        self.transaction_digest = Some(transaction_digest);
        self.actual_amounts = actual_amounts;
        self.gas_used = gas_used;
        self.actual_profit = actual_profit;
        self.completed_at = now();
        self.execution_duration_ms = self.completed_at - self.started_at;
        self
    }
    
    /// Mark execution as failed
    pub fn failure(mut self, error: BotError) -> Self {
        self.status = ExecutionStatus::Failed;
        self.error = Some(error.to_string());
        self.completed_at = now();
        self.execution_duration_ms = self.completed_at - self.started_at;
        self
    }
    
    /// Mark execution as simulated (dry run)
    pub fn simulated(
        mut self,
        actual_amounts: HashMap<usize, u64>,
        gas_used: u64,
        actual_profit: Decimal,
    ) -> Self {
        self.status = ExecutionStatus::Simulated;
        self.actual_amounts = actual_amounts;
        self.gas_used = gas_used;
        self.actual_profit = actual_profit;
        self.completed_at = now();
        self.execution_duration_ms = self.completed_at - self.started_at;
        self
    }
    
    /// Get profit percentage based on initial amount
    pub fn profit_percentage(&self) -> Decimal {
        if self.opportunity.path.initial_amount == 0 {
            return Decimal::ZERO;
        }
        
        let initial_decimal = self.opportunity.path.start_token
            .to_decimal(self.opportunity.path.initial_amount);
            
        if initial_decimal.is_zero() {
            return Decimal::ZERO;
        }
        
        (self.actual_profit / initial_decimal) * Decimal::from(100)
    }
    
    /// Get execution summary
    pub fn summary(&self) -> String {
        match self.status {
            ExecutionStatus::Success => {
                format!(
                    "SUCCESS: Profit {} {} ({:.4}%) in {}ms - TX: {}",
                    self.actual_profit,
                    self.opportunity.path.start_token.symbol,
                    self.profit_percentage(),
                    self.execution_duration_ms,
                    self.transaction_digest.as_ref().map(|s| &s[..8]).unwrap_or("unknown")
                )
            }
            ExecutionStatus::Failed => {
                format!(
                    "FAILED: {} in {}ms - Error: {}",
                    self.opportunity.path.path_description(),
                    self.execution_duration_ms,
                    self.error.as_ref().map(|e| e.to_string()).unwrap_or("Unknown".into())
                )
            }
            ExecutionStatus::Simulated => {
                format!(
                    "SIMULATED: Profit {} {} ({:.4}%)",
                    self.actual_profit,
                    self.opportunity.path.start_token.symbol,
                    self.profit_percentage()
                )
            }
            ExecutionStatus::Pending => {
                format!("PENDING: {}", self.opportunity.path.path_description())
            }
        }
    }
}

/// Execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    /// Execution is pending
    Pending,
    /// Execution completed successfully
    Success,
    /// Execution failed
    Failed,
    /// Dry run simulation
    Simulated,
}

/// Statistics for execution performance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStats {
    pub total_executions: u64,
    pub successful_executions: u64,
    pub failed_executions: u64,
    pub total_profit: Decimal,
    pub avg_execution_time_ms: u64,
    pub success_rate: f64,
    pub last_execution_time: Option<Timestamp>,
}

impl ExecutionStats {
    pub fn new() -> Self {
        Self {
            total_executions: 0,
            successful_executions: 0,
            failed_executions: 0,
            total_profit: Decimal::ZERO,
            avg_execution_time_ms: 0,
            success_rate: 0.0,
            last_execution_time: None,
        }
    }
}

impl Default for ExecutionStats {
    fn default() -> Self {
        Self::new()
    }
}