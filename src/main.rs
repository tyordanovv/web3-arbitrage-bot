use std::sync::Arc;

use arbitrage_bot::{arbitrage::{arbitrage_engine::{ArbitrageEngine, ArbitrageEngineBuilder}, calculator::{ArbitrageCalculator, DefaultArbitrageCalculator}, detector::{ArbitrageDetector, DefaultArbitrageDetector}, validator::{DefaultOpportunityValidator, OpportunityValidator}}, dex::manager::DexManager, event::processor::{DefaultEventProcessor, EventProcessor}, execution::executor::{DefaultTradeExecutor, TradeExecutor}, sync::synchronizer::SyncOrchestratorBuilder, types::Result, utils::{config::Config, logger::init}};
use tokio::sync::RwLock;
use tracing::{info, error};

#[tokio::main]
async fn main() -> Result<()> {
    init();
    info!("Starting Arbitrage Bot");
    
    let config = Config::load()?;
    config.validate()?;
    
    let dex_manager = DexManager::with_config(
        config.sync_config().max_pools_per_dex,
        config.sync_config().state_ttl(),
    );    let dex_manager = Arc::new(RwLock::new(dex_manager));
    
    let orchestrator = Arc::new(SyncOrchestratorBuilder::new()
        .with_dex_manager(dex_manager.clone())
        .with_rpc_endpoint(config.network_config().rpc_url.clone())
        .with_config(config.sync_config().clone())
        .build()?
    );
    
    info!("Performing initial state synchronization...");
    if let Err(e) = orchestrator.initialize().await {
        error!("Initial synchronization failed: {}", e);
        return Err(e);
    }
    info!("Initial synchronization completed successfully");

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
    
    let engine = ArbitrageEngineBuilder::new()
        .with_event_processor(event_processor)
        .with_detector(detector)
        .with_executor(executor)
        .with_validator(validator)
        .with_sync_orchestrator(orchestrator)
        .build()?;

    setup_graceful_shutdown(engine).await
}

/// Handle graceful shutdown
async fn setup_graceful_shutdown(mut engine: ArbitrageEngine) -> Result<()> {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl-C, shutting down gracefully...");
        },
        _ = terminate => {
            info!("Received SIGTERM, shutting down gracefully...");
        },
        result = engine.start() => {
            if let Err(e) = result {
                error!("Engine stopped with error: {}", e);
            }
        }
    }

    // Perform graceful shutdown
    if let Err(e) = engine.stop().await {
        error!("Error during shutdown: {}", e);
        return Err(e);
    }

    info!("Arbitrage Bot shutdown complete");
    Ok(())
}