#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(unused_must_use)]

use std::sync::Arc;

use arbitrage_bot::{arbitrage::{arbitrage_engine::ArbitrageEngine, calculator::{ArbitrageCalculator, DefaultArbitrageCalculator}, detector::{ArbitrageDetector, DefaultArbitrageDetector}, validator::{DefaultOpportunityValidator, OpportunityValidator}}, dex::manager::DexManager, event::processor::{DefaultEventProcessor, EventProcessor}, execution::executor::{DefaultTradeExecutor, TradeExecutor}, utils::{config::Config, logger::init}};
use tokio::sync::RwLock;
use tracing::info;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    init();
    info!("Starting Arbitrage Bot");
    
    let config = Config::load()?;
    
    // 1. Create and initialize DexManager
    let mut dex_manager = DexManager::new();
    // TODO: Register DEX adapters here
    // for dex_config in config.enabled_dexes() {
    //     let adapter = create_dex_adapter(dex_config, config.network_config()).await?;
    //     dex_manager.register_adapter(adapter)?;
    // }
    // dex_manager.initialize_all().await?;
    
    let dex_manager = Arc::new(RwLock::new(dex_manager));
    
    // 2. Create components using concrete implementations
    let event_processor = Box::new(DefaultEventProcessor::new(
        dex_manager.clone(),
        config.network_config().clone(),
    )) as Box<dyn EventProcessor>;
    
    let calculator = Box::new(DefaultArbitrageCalculator::new(
        config.arbitrage_config().clone(),
    )) as Box<dyn ArbitrageCalculator>;
    
    let detector = Box::new(DefaultArbitrageDetector::new(
        dex_manager.clone(),
        calculator,
    )) as Box<dyn ArbitrageDetector>;
    
    let executor = Box::new(DefaultTradeExecutor::new(
        config.execution_config().clone(),
    )) as Box<dyn TradeExecutor>;
    
    let validator = Box::new(DefaultOpportunityValidator::new(
        dex_manager.clone(),
        config.validation_config().clone(),
    )) as Box<dyn OpportunityValidator>;
    
    // 3. Create and run engine
    let mut engine = ArbitrageEngine::new(
        event_processor,
        detector,
        executor,
        validator,
    );
    
    tracing::info!("Arbitrage Engine starting...");
    engine.start().await?;
    
    Ok(())
}