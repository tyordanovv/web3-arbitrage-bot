use std::sync::Arc;

use arbitrage_bot::{
    arbitrage::{
        arbitrage_engine::{ ArbitrageEngine, ArbitrageEngineBuilder }, 
        calculator::ArbitrageCalculator, 
        detector::ArbitrageDetector, 
        validator::OpportunityValidator
    }, dex::registry::DexRegistryBuilder, 
    event::processor::EventProcessor, 
    execution::executor::TradeExecutor, 
    sync::{ reader::StateReader, state::StateUpdater, synchronizer::StateSynchronizer, update_producer::UpdateProducer}, 
    types::Result, utils::{config::Config, logger::init}
};
use tokio::sync::mpsc;
use tracing::{info, error};

#[tokio::main]
async fn main() -> Result<()> {
    init();
    info!("Starting Arbitrage Bot");

    let config = Config::load()?;
    config.validate()?;

    // Build core components
    let components = build_components(&config).await?;

    // Initialize state synchronization
    info!("Performing initial state synchronization...");
    components.synchronizer.initialize().await?;
    info!("Initial synchronization completed successfully");

    // Build and start the arbitrage engine
    let synchronizer = components.synchronizer.clone();
    let engine = build_engine(&config, components)?;

    // Run with graceful shutdown
    run_with_shutdown(engine, synchronizer).await
}

struct Components {
    state_reader: Arc<StateReader>,
    update_producer: Arc<UpdateProducer>,
    synchronizer: Arc<StateSynchronizer>,
}

async fn build_components(config: &Config) -> Result<Components> {
    // Build DEX registry
    let dex_registry = Arc::new(
        DexRegistryBuilder::new()
            .with_max_pools_per_dex(config.sync_config().max_pools_per_dex.clone())
            .with_dex_configs(config.network_config().dexes.clone())
            .build()?,
    );

    // Create state management pipeline
    let (batch_tx, batch_rx) = mpsc::channel(100);
    let state_reader = Arc::new(StateReader::new());
    let update_producer = Arc::new(UpdateProducer::new(batch_tx));

    // Start state updater
    let mut state_updater = StateUpdater::new(
        batch_rx,
        state_reader.clone(),
        dex_registry.clone(),
    );
    
    tokio::spawn(async move {
        if let Err(e) = state_updater.run().await {
            error!("StateUpdater crashed: {}", e);
        }
    });

    // Build synchronizer
    let synchronizer = Arc::new(
        StateSynchronizer::builder()
            .with_rpc_endpoint(config.network_config().rpc_url.clone())
            .with_config(config.sync_config().clone())
            .with_dex_registry(dex_registry.clone())
            .with_update_producer(update_producer.clone())
            .build()
            .await?,
    );

    Ok(Components {
        state_reader,
        update_producer,
        synchronizer,
    })
}

fn build_engine(config: &Config, components: Components) -> Result<ArbitrageEngine> {
    let event_processor = EventProcessor::new(components.update_producer, config.network_config().clone());
    let calculator = ArbitrageCalculator::new(config.arbitrage_config().clone());
    let detector = ArbitrageDetector::new(components.state_reader.clone(), calculator);
    let executor = TradeExecutor::new(config.execution_config().clone());
    let validator = OpportunityValidator::new(components.state_reader, config.validation_config().clone());

    ArbitrageEngineBuilder::new()
        .with_event_processor(event_processor)
        .with_detector(detector)
        .with_executor(executor)
        .with_validator(validator)
        .build()
}

async fn run_with_shutdown(
    mut engine: ArbitrageEngine,
    synchronizer: Arc<StateSynchronizer>,
) -> Result<()> {
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

    // Start background synchronization
    let sync_handle = tokio::spawn({
        let synchronizer = synchronizer.clone();
        async move {
            if let Err(e) = synchronizer.run_periodic_sync().await {
                error!("Periodic sync failed: {}", e);
            }
        }
    });

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl-C, shutting down gracefully...");
        },
        _ = terminate => {
            info!("Received SIGTERM, shutting down gracefully...");
        },
        result = engine.run() => {
            if let Err(e) = result {
                error!("Engine stopped with error: {}", e);
            }
        }
    }

    // Cleanup
    sync_handle.abort();
    engine.shutdown().await?;

    info!("Arbitrage Bot shutdown complete");
    Ok(())
}