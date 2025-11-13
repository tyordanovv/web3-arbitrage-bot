use async_trait::async_trait;
use tokio::sync::{RwLock, mpsc};
use tracing::info;
use std::{collections::HashMap, sync::Arc};

use crate::{dex::manager::DexManager, event::websocket::{DefaultWebSocketManager, WebSocketManager}, types::{DexId, RawEvent, Result, SwapEvent}, utils::config::NetworkConfig};

#[async_trait]
pub trait EventProcessor: Send + Sync {
    /// Start processing events for all registered DEXs
    async fn start(&mut self) -> Result<()>;
    
    /// Stop all event processing
    async fn stop(&mut self) -> Result<()>;
    
    /// Subscribe to parsed swap events
    fn subscribe_swap_events(&self) -> mpsc::Receiver<SwapEvent>;
    
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

// Default implementation
pub struct DefaultEventProcessor {
    dex_manager: Arc<RwLock<DexManager>>,
    websocket_managers: HashMap<DexId, Box<dyn WebSocketManager>>,
    swap_sender: mpsc::Sender<SwapEvent>,
    swap_receiver: mpsc::Receiver<SwapEvent>,
    processor_tasks: HashMap<DexId, tokio::task::JoinHandle<()>>,
    is_running: bool,
    network_config: NetworkConfig,
}

impl DefaultEventProcessor {
    pub fn new(
        dex_manager: Arc<RwLock<DexManager>>,
        network_config: NetworkConfig,
    ) -> Self {
        let (swap_sender, swap_receiver) = mpsc::channel(1000);
        
        Self {
            dex_manager,
            websocket_managers: HashMap::new(),
            swap_sender,
            swap_receiver,
            processor_tasks: HashMap::new(),
            is_running: false,
            network_config
        }
    }
    
    /// Initialize WebSocket managers for all enabled DEXs
    pub async fn initialize_websockets(&mut self, dex_ids: Vec<DexId>) -> Result<()> {
        info!("Initializing WebSocket managers for DEXs: {:?}", dex_ids);
        for dex_id in dex_ids {
            let ws_manager = Box::new(DefaultWebSocketManager::new(
                dex_id,
                self.network_config.ws_url.to_string(),
            ));
            self.websocket_managers.insert(dex_id, ws_manager);
            info!("WebSocket manager initialized for DEX {}", dex_id);
        }
        Ok(())
    }
    
    async fn start_dex_processor(&mut self, dex_id: DexId) -> Result<()> {
        info!("Starting event processor for DEX {}", dex_id);
        Ok(())
    }
    
    fn parse_raw_event(dex_id: DexId, raw_event: RawEvent) -> Result<SwapEvent> {
        // TODO: Implement DEX-specific event parsing
        // This would convert raw WebSocket data to normalized SwapEvent
        todo!("Parse raw event for {}", dex_id)
    }

    async fn get_enabled_dex_ids(&self) -> Result<Vec<DexId>> {
        let manager = self.dex_manager.read().await;
        Ok(manager.healthy_dexes())
    }
}

#[async_trait]
impl EventProcessor for DefaultEventProcessor {
    async fn start(&mut self) -> Result<()> {
        info!("Starting Event Processor...");
        if self.is_running {
            info!("Event processor already running");
            return Ok(());
        }
        
        self.is_running = true;
        
        let enabled_dex_ids = self.get_enabled_dex_ids().await?;
        
        self.initialize_websockets(enabled_dex_ids).await?;
        
        for dex_id in self.websocket_managers.keys().cloned().collect::<Vec<_>>() {
            self.start_dex_processor(dex_id).await?;
        }
        
        tracing::info!("Event processor started with {} DEXs", self.websocket_managers.len());
        Ok(())
    }
    
    async fn stop(&mut self) -> Result<()> {
        if !self.is_running {
            return Ok(());
        }
        
        self.is_running = false;
        
        // Stop all WebSocket connections
        for ws_manager in self.websocket_managers.values_mut() {
            ws_manager.disconnect().await?;
        }
        
        // Cancel all processor tasks
        for (_, task) in self.processor_tasks.drain() {
            task.abort();
        }
        
        tracing::info!("Event processor stopped");
        Ok(())
    }
    
    fn subscribe_swap_events(&self) -> mpsc::Receiver<SwapEvent> {
        todo!("Return receiver for parsed swap events")
    }
    
    async fn get_status(&self) -> HashMap<DexId, ProcessorStatus> {
        todo!("Return status for each DEX processor")
    }
}
