use std::sync::Arc;
use metrics::{Counter, Gauge, Histogram, counter, gauge, histogram};

use tracing::{debug, info};
use tokio::sync::mpsc;

use crate::types::{BotError, Result, SwapDelta, pool_state::PoolState};

#[derive(Debug, Clone)]
pub enum PoolUpdate {
    /// Full pool state (from RPC fetcher)
    FullState(PoolState),
    /// Delta update (from websocket swap event)
    Delta(SwapDelta),
}

pub struct UpdateProducer {
    sender: mpsc::Sender<Vec<PoolUpdate>>,
    metrics: Arc<UpdateMetrics>,
}

impl UpdateProducer {
    pub fn new(sender: mpsc::Sender<Vec<PoolUpdate>>) -> Self {
        Self {
            sender,
            metrics: Arc::new(UpdateMetrics::new()),
        }
    }

    pub async fn update_multiple_pools(&self, updates: Vec<PoolUpdate>) -> Result<usize> {
        let batch_size = updates.len();
        
        if batch_size == 0 {
            return Ok(0);
        }
        
        info!(batch_size, "Sending pool updates");
        
        self.sender.send(updates).await
            .map_err(|e| BotError::ChannelError(e.to_string()))?;
        
        Ok(batch_size)
    }
}

#[derive(Debug, Clone)]
pub struct UpdateMetrics {
    // Counters
    pub events_received: Counter,
    pub batches_received: Counter,
    pub updates_sent: Counter,
    pub batches_sent: Counter,
    pub duplicates_dropped: Counter,
    pub stale_updates_dropped: Counter,
    
    // Gauges
    pub pending_updates: Gauge,
    pub channel_queue_size: Gauge,
    pub batch_size: Histogram,
    
    // Histograms
    pub processing_latency: Histogram,
    pub batch_build_time: Histogram,
    pub update_age: Histogram,
}

impl UpdateMetrics {
    pub fn new() -> Self {
        Self {
            events_received: counter!("updates.events_received"),
            batches_received: counter!("updates.batches_received"),
            updates_sent: counter!("updates.sent"),
            batches_sent: counter!("updates.batches_sent"),
            duplicates_dropped: counter!("updates.duplicates_dropped"),
            stale_updates_dropped: counter!("updates.stale_dropped"),
            pending_updates: gauge!("updates.pending"),
            channel_queue_size: gauge!("updates.channel_queue_size"),
            batch_size: histogram!("updates.batch_size"),
            processing_latency: histogram!("updates.processing_latency_ms"),
            batch_build_time: histogram!("updates.batch_build_time_ms"),
            update_age: histogram!("updates.age_ms"),
        }
    }
}
