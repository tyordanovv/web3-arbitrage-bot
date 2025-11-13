use tokio::sync::watch;
use tracing::info;

use crate::{arbitrage::{detector::ArbitrageDetector, validator::OpportunityValidator}, event::processor::{EventProcessor, ProcessorStatus}, execution::executor::TradeExecutor, types::{ArbitrageOpportunity, ExecutionResult, Result}};

pub struct ArbitrageEngine {
    event_processor: Box<dyn EventProcessor>,
    detector: Box<dyn ArbitrageDetector>,
    executor: Box<dyn TradeExecutor>,
    validator: Box<dyn OpportunityValidator>,
    shutdown_sender: watch::Sender<bool>,
}

impl ArbitrageEngine {
    pub fn new(
        event_processor: Box<dyn EventProcessor>,
        detector: Box<dyn ArbitrageDetector>,
        executor: Box<dyn TradeExecutor>,
        validator: Box<dyn OpportunityValidator>,
    ) -> Self {
        let (shutdown_sender, _) = watch::channel(false);
        
        Self {
            event_processor,
            detector,
            executor,
            validator,
            shutdown_sender,
        }
    }
    
    /// Start the complete arbitrage engine with graceful shutdown support
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting Arbitrage Engine...");
        
        // Create shutdown receiver for this run
        let mut shutdown_receiver = self.shutdown_sender.subscribe();
        
        // Start event processing
        self.event_processor.start().await?;
        
        // Start detection loop
        self.detector.start_detection().await?;
        
        // engine loop
        while !*shutdown_receiver.borrow() {
            tokio::select! {
                _ = shutdown_receiver.changed() => {
                    info!("Shutdown signal received, stopping engine...");
                    break;
                }
                opportunity = self.detector.next_opportunity() => {
                    if let Some(opportunity) = opportunity {
                        if self.validator.validate(&opportunity).await {
                            let result = self.executor.execute(opportunity).await;
                            self.handle_execution_result(result).await;
                        }
                    }
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(10)) => {
                    // Continue
                }
            }
        }
        
        self.stop().await?;
        Ok(())
    }
    
    /// Stop the engine gracefully
    pub async fn stop(&mut self) -> Result<()> {
        info!("Stopping Arbitrage Engine gracefully...");
        
        // Send shutdown signal to all components
        let _ = self.shutdown_sender.send(true);
        
        self.detector.stop_detection().await?;
        self.event_processor.stop().await?;
        
        info!("Arbitrage Engine stopped successfully");
        Ok(())
    }
    
    /// Get a shutdown receiver to listen for shutdown signals
    pub fn get_shutdown_receiver(&self) -> watch::Receiver<bool> {
        self.shutdown_sender.subscribe()
    }
    
    async fn handle_execution_result(&self, result: ExecutionResult) {
        info!("Execution result: {}", result.summary());
    }
}