use async_trait::async_trait;
use tokio::{sync::{ RwLock, mpsc }, task::JoinHandle};
use tracing::{info, warn};
use std::{collections::HashMap, sync::Arc};

use crate::{
    dex::manager::DexManager, 
    event::{context::WsContext, websocket::{DexWebSocket, WebSocketManager}}, 
    types::{ BotError, DexId, Network, RawEvent, Result, SwapEvent }, 
    utils::config::{DexConfig, NetworkConfig}
};

#[async_trait]
pub trait EventProcessor: Send + Sync {
    /// Start processing events for all registered DEXs
    async fn start(&mut self) -> Result<()>;
    
    /// Stop all event processing
    async fn stop(&mut self) -> Result<()>;
    
    /// Get processing status for each DEX
    async fn get_status(&self) -> HashMap<DexId, ProcessorStatus>;
}

#[derive(Debug, Clone)]
pub struct ProcessorStatus {
    pub is_running: bool,
    pub events_processed: u64,
    pub last_event_time: Option<u64>,
    pub error_count: u64,
}

pub struct DefaultEventProcessor {
    dex_manager: Arc<RwLock<DexManager>>,
    ws_managers: HashMap<DexId, JoinHandle<()>>,
    is_running: bool,
    network_config: NetworkConfig,
    dex_configs: HashMap<DexId, DexConfig>,
}

impl DefaultEventProcessor {
    pub fn new(
        dex_manager: Arc<RwLock<DexManager>>,
        network_config: NetworkConfig,
    ) -> Self {      
        let dex_configs = network_config.dexes
            .iter()
            .map(|config| (config.id, config.clone()))
            .collect();

        Self {
            dex_manager,
            ws_managers: HashMap::new(),
            is_running: false,
            network_config,
            dex_configs,
        }
    }

    /// Initialize WebSocket managers for all enabled DEXs
    async fn initialize_websockets(&mut self, event_sender: mpsc::Sender<(DexId, RawEvent)>) -> Result<()> {        
        let enabled_dexes = self.get_enabled_dex_ids().await?;
        info!("Found {} enabled DEXs: {:?}", enabled_dexes.values().flatten().count(), enabled_dexes);
        
        if enabled_dexes.is_empty() {
            warn!("No DEXs enabled - check your DexManager configuration");
            return Ok(());
        }   

        for (network, dex_ids) in self.get_enabled_dex_ids().await? {
            for dex_id in dex_ids {
                info!("Initializing WS for DEX {:?} on network {:?}", dex_id, network);
                
                let mut ws = self.build_ws_manager_from_config(&dex_id, network)?;
                let sender = event_sender.clone();
                
                let handle = tokio::spawn(async move {
                    let ctx = Arc::new(WsContext {
                        dex_id,
                        network,
                        tx: sender,
                    });
                    ws.connect(ctx).await.ok();
                });
                
                self.ws_managers.insert(dex_id, handle);
            }
        }
        Ok(())
    }

    async fn process_event(&self, dex_id: DexId, raw_event: RawEvent) -> Result<SwapEvent> {
        info!("Processing event for DEX {:?}: {:?}", dex_id, raw_event);
        Ok(SwapEvent::new())
    }

    async fn get_enabled_dex_ids(&self) -> Result<HashMap<Network, Vec<DexId>>> {
        let manager = self.dex_manager.read().await;
        Ok(manager.healthy_dexes())
    }

    fn build_ws_manager_from_config(
        &self, 
        dex_id: &DexId, 
        network: Network
    ) -> Result<Box<dyn WebSocketManager>> {
        let dex_config = self.dex_configs
            .get(dex_id)
            .ok_or_else(|| BotError::Config(format!("No config found for DEX: {:?}", dex_id)))?;

        match network {
            Network::SuiMainnet => Ok(Box::new(DexWebSocket::new(
                &self.network_config.ws_url,
                &dex_config.package_id,
                &dex_config.event_type,
            ))),
            _ => {
                // TODO: Implement AptosDexWs
                Err(BotError::Config("Aptos not yet implemented".to_string()))
            }
        }
    }
}

#[async_trait]
impl EventProcessor for DefaultEventProcessor {
    async fn start(&mut self) -> Result<()> {
        info!("Starting Event Processor...");
        if self.is_running {
            return Ok(());
        }
        
        self.is_running = true;
        let (event_sender, mut event_receiver) = mpsc::channel(5000);

        self.initialize_websockets(event_sender).await?;
        
        let dex_manager = self.dex_manager.clone();
        tokio::spawn(async move {
            info!("Event processing loop started");
            
            while let Some((dex_id, raw_event)) = event_receiver.recv().await {
                info!("Processing event for {:?}", dex_id);
                // TODO
            }
            
            info!("Event processing loop stopped");
        });
                
        info!("Event processor started");
        Ok(())
    }
    
    async fn stop(&mut self) -> Result<()> {
        if !self.is_running {
            return Ok(());
        }
        info!("Stopping Event Processor...");
        
        self.is_running = false;
        
        // Stop all WebSocket connections
        
        info!("Event processor stopped");
        Ok(())
    }
    
    async fn get_status(&self) -> HashMap<DexId, ProcessorStatus> {
        todo!("Return status for each DEX processor")
    }
}
