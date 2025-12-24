use async_trait::async_trait;
use tokio::{sync::{ RwLock, mpsc }, task::JoinHandle};
use tracing::{error, info, warn};
use std::{collections::HashMap, sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering}}};

use crate::{event::{connection::DexConnection, metrics::ProcessorMetrics}, sync::update_producer::UpdateProducer, types::{DexId, Network, Result}, utils::config::{DexConfig, NetworkConfig}};

pub struct EventProcessor {
    connections: RwLock<HashMap<DexId, Arc<DexConnection>>>,
}

impl EventProcessor {
    pub async fn new(
        update_producer: Arc<UpdateProducer>,
        network: NetworkConfig,
    ) -> Result<Self> {
        let dex_ids: Vec<DexId> = network.dexes
            .iter()
            .filter(|d| d.enabled)
            .map(|d| d.id)
            .collect();

        let metrics = Arc::new(ProcessorMetrics::new(&dex_ids));

        let mut map = HashMap::new();

        for dex in network.dexes.into_iter().filter(|d| d.enabled) {
            let conn = DexConnection::new(
                dex.id,
                dex.clone(),
                Arc::clone(&update_producer),
                Arc::clone(&metrics),
            );

            map.insert(dex.id, Arc::new(conn));
        }

        Ok(Self {
            connections: RwLock::new(map),
        })
    }

    pub async fn start(&self) {
        for (id, conn) in self.connections.read().await.iter() {
            if let Err(e) = conn.start().await {
                error!("Failed to start {:?}: {}", id, e);
            }
        }
        info!("EventProcessor started");
    }

    pub async fn stop(&self) {
        for (id, conn) in self.connections.read().await.iter() {
            if let Err(e) = conn.stop().await {
                error!("Failed to stop {:?}: {}", id, e);
            }
        }
        info!("EventProcessor stopped");
    }
}

#[derive(Debug, Clone)]
pub struct ConnectionStatus {
    pub is_running: bool,
    pub last_event_time: Option<u64>,
    pub events_processed: u64,
}