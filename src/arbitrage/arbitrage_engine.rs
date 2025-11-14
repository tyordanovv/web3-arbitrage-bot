use tracing::{ info, debug, warn };

use crate::{arbitrage::{detector::ArbitrageDetector, validator::OpportunityValidator}, event::processor::EventProcessor, execution::executor::TradeExecutor, types::{ArbitrageOpportunity, ExecutionResult, Result}};
use std::time::Duration;

pub struct ArbitrageEngine {
    // Components
    event_processor: Box<dyn EventProcessor>,
    detector: Box<dyn ArbitrageDetector>,
    executor: Box<dyn TradeExecutor>,
    validator: Box<dyn OpportunityValidator>,
    
    // State
    is_running: bool,
    stats: EngineStats,
}

#[derive(Debug, Clone)]
pub struct EngineStats {
    pub opportunities_found: u64,
    pub opportunities_executed: u64,
    pub execution_successes: u64,
    pub execution_failures: u64,
    pub total_profit: f64,
    pub start_time: std::time::Instant,
}

impl Default for EngineStats {
    fn default() -> Self {
        Self {
            opportunities_found: 0,
            opportunities_executed: 0,
            execution_successes: 0,
            execution_failures: 0,
            total_profit: 0.0,
            start_time: std::time::Instant::now(),
        }
    }
}

impl ArbitrageEngine {
    pub fn new(
        event_processor: Box<dyn EventProcessor>,
        detector: Box<dyn ArbitrageDetector>,
        executor: Box<dyn TradeExecutor>,
        validator: Box<dyn OpportunityValidator>,
    ) -> Self {
        Self {
            event_processor,
            detector,
            executor,
            validator,
            is_running: false,
            stats: EngineStats::default(),
        }
    }
    
    /// Start the complete arbitrage engine - SIMPLE POLLING
    pub async fn start(&mut self) -> Result<()> {
        info!("🚀 Starting Arbitrage Engine...");
        
        if self.is_running {
            warn!("Engine is already running");
            return Ok(());
        }
        
        self.is_running = true;
        self.stats.start_time = std::time::Instant::now();
        
        self.event_processor.start().await?;
        info!("Event processor started");
        
        self.run_main_loop().await?;

        info!("Arbitrage Engine stopped");
        Ok(())
    }
    
    async fn run_main_loop(&mut self) -> Result<()> {
        let mut interval = tokio::time::interval(Duration::from_millis(20)); // 50Hz
        
        while self.is_running {
            tokio::select! {
                _ = interval.tick() => {
                    let opportunity = self.detector.next_opportunity().await;
                    // TODO process opportunities
                }
                _ = self.check_shutdown_signal() => {
                    self.stop().await?;
                    self.is_running = false;
                    break;
                }
            }
        }
        
        Ok(())
    }
    
    /// Process batch of opportunities
    async fn process_opportunity(&mut self, opportunity: Option<ArbitrageOpportunity>) {
        
    }
    
    /// Handle execution results
    async fn handle_execution_result(&mut self, result: ExecutionResult) {
        info!("Execution result: {}", result.summary());
    }
    
    /// Check for shutdown signal
    async fn check_shutdown_signal(&self) {
        tokio::signal::ctrl_c().await.ok();
        info!("Shutdown signal received");
    }
    
    /// Stop the engine gracefully
    pub async fn stop(&mut self) -> Result<()> {
        if !self.is_running {
            return Ok(());
        }
        
        info!("Stopping Arbitrage Engine...");
        self.is_running = false;
        
        // Stop components
        self.event_processor.stop().await?;
        
        info!("Final stats: {:?}", self.stats);
        Ok(())
    }
    
    /// Get engine statistics
    pub fn get_stats(&self) -> &EngineStats {
        &self.stats
    }
    
    /// Check if engine is running
    pub fn is_running(&self) -> bool {
        self.is_running
    }
}

// Simple builder
pub struct ArbitrageEngineBuilder {
    event_processor: Option<Box<dyn EventProcessor>>,
    detector: Option<Box<dyn ArbitrageDetector>>,
    executor: Option<Box<dyn TradeExecutor>>,
    validator: Option<Box<dyn OpportunityValidator>>,
}

impl ArbitrageEngineBuilder {
    pub fn new() -> Self {
        Self {
            event_processor: None,
            detector: None,
            executor: None,
            validator: None,
        }
    }
    
    pub fn with_event_processor(mut self, processor: Box<dyn EventProcessor>) -> Self {
        self.event_processor = Some(processor);
        self
    }
    
    pub fn with_detector(mut self, detector: Box<dyn ArbitrageDetector>) -> Self {
        self.detector = Some(detector);
        self
    }
    
    pub fn with_executor(mut self, executor: Box<dyn TradeExecutor>) -> Self {
        self.executor = Some(executor);
        self
    }
    
    pub fn with_validator(mut self, validator: Box<dyn OpportunityValidator>) -> Self {
        self.validator = Some(validator);
        self
    }
    
    pub fn build(self) -> Result<ArbitrageEngine> {
        Ok(ArbitrageEngine::new(
            self.event_processor.expect("Event processor is required"),
            self.detector.expect("Detector is required"),
            self.executor.expect("Executor is required"),
            self.validator.expect("Validator is required"),
        ))
    }
}

impl Default for ArbitrageEngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}