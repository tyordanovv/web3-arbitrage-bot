use std::{sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering}}, time::{Duration, Instant}};

use futures::lock::Mutex;
use sui_sdk::{SuiClientBuilder, rpc_types::SuiEvent};
use tracing::{debug, error, info};
use tokio::task::JoinHandle;

use crate::{event::{metrics::ProcessorMetrics, parse_sui_event}, sync::update_producer::{PoolUpdate, UpdateProducer}, types::{DexId, Result, SuiAddress, SwapDelta, pool_state::PoolId}, utils::config::DexConfig};

pub struct DexConnection {
    dex_id: DexId,
    config: DexConfig,
    update_producer: Arc<UpdateProducer>,
    metrics: Arc<ProcessorMetrics>,

    handle: Mutex<Option<JoinHandle<()>>>,
    running: AtomicBool,
}

impl DexConnection {
    pub fn new(
        dex_id: DexId,
        config: DexConfig,
        update_producer: Arc<UpdateProducer>,
        metrics: Arc<ProcessorMetrics>,
    ) -> Self {
        Self {
            dex_id,
            config,
            update_producer,
            metrics,
            handle: Mutex::new(None),
            running: AtomicBool::new(false),
        }
    }

    pub async fn start(&self) -> Result<()> {
        if self.running.swap(true, Ordering::SeqCst) {
            return Ok(());
        }

        self.metrics.increment_active_connections();

        let dex_id = self.dex_id;
        let config = self.config.clone();
        let producer = Arc::clone(&self.update_producer);
        let metrics = Arc::clone(&self.metrics);

        let handle = tokio::spawn(async move {
            websocket_supervisor(dex_id, config, producer, metrics).await;
        });

        *self.handle.lock().await = Some(handle);
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        self.running.store(false, Ordering::SeqCst);

        if let Some(handle) = self.handle.lock().await.take() {
            handle.abort();
        }

        self.metrics.decrement_active_connections();
        Ok(())
    }
}

async fn websocket_supervisor(
    dex_id: DexId,
    config: DexConfig,
    producer: Arc<UpdateProducer>,
    metrics: Arc<ProcessorMetrics>,
) {
    let mut backoff = Duration::from_secs(1);
    let max_backoff = Duration::from_secs(30);

    loop {
        match websocket_session(dex_id, &config, &producer, &metrics).await {
            Ok(_) => {
                backoff = Duration::from_secs(1);
                break;
            }
            Err(e) => {
                error!("{:?} WS error: {}", dex_id, e);
                metrics.record_reconnection(dex_id);

                tokio::time::sleep(backoff).await;
                backoff = std::cmp::min(backoff * 2, max_backoff);
            }
        }
    }
}

async fn websocket_session(
    dex_id: DexId,
    config: &DexConfig,
    producer: &Arc<UpdateProducer>,
    metrics: &Arc<ProcessorMetrics>,
) -> Result<()> {
    info!("Connecting WS for {:?}", dex_id);

    let endpoints = config.network.endpoints();

    let sui = SuiClientBuilder::default()
        .ws_url(&endpoints.ws_url)
        .build(&endpoints.rpc_url) 
        .await
        .map_err(|e| crate::types::BotError::WebSocket(e.to_string()))?;

    let filter = sui_sdk::rpc_types::EventFilter::MoveEventType(
        format!("{}::{}", config.package_id, config.event_type),
    );

    let mut sub = sui
        .subscribe_event(filter)
        .await
        .map_err(|e| crate::types::BotError::WebSocket(e.to_string()))?;

    info!("Subscribed {:?} events", dex_id);

    while let Some(event) = sub.next().await {
        let total_start = Instant::now();

        match event {
            Ok(ev) => {
                metrics.record_event_received(dex_id);

                let parse_start = Instant::now();
                match parse_sui_event(dex_id, &ev) {
                    Ok(delta) => {
                        metrics.record_parse_time(dex_id, parse_start.elapsed());

                        let update_start = Instant::now();
                        if let Err(e) = producer
                            .update_multiple_pools(vec![PoolUpdate::Delta(delta)])
                            .await
                        {
                            error!("UpdateProducer error {:?}: {}", dex_id, e);
                            continue;
                        }

                        metrics.record_update_producer_time(update_start.elapsed());
                        metrics.record_event_processed(dex_id);
                        metrics.record_total_processing_time(dex_id, total_start.elapsed());

                        if total_start.elapsed().as_micros() > 10_000 {
                            debug!("Slow {:?}: {}µs", dex_id, total_start.elapsed().as_micros());
                        }
                    }
                    Err(e) => {
                        metrics.record_parse_error(dex_id);
                        debug!("Parse error {:?}: {}", dex_id, e);
                    }
                }
            }
            Err(e) => {
                return Err(crate::types::BotError::WebSocket(e.to_string()));
            }
        }
    }

    Ok(())
}