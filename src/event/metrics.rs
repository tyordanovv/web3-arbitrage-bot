use std::collections::HashMap;
use std::time::Duration;

use metrics::{counter, gauge, histogram};

use crate::types::DexId;
#[derive(Clone)]
pub struct ProcessorMetrics {
    dex_labels: HashMap<DexId, Vec<(&'static str, String)>>,
}

impl ProcessorMetrics {
    pub fn new(dexes: &[DexId]) -> Self {
        Self::describe_metrics();

        let dex_labels = dexes
            .iter()
            .map(|d| (*d, vec![("dex", format!("{:?}", d).to_lowercase())]))
            .collect();

        Self { dex_labels }
    }

    fn labels(&self, dex: DexId) -> Option<&[(&'static str, String)]> {
        self.dex_labels.get(&dex).map(|v| v.as_slice())
    }

    fn describe_metrics() {
        metrics::describe_counter!("arbitrage_bot_events_received_total", "");
        metrics::describe_counter!("arbitrage_bot_events_processed_total", "");
        metrics::describe_counter!("arbitrage_bot_parse_errors_total", "");
        metrics::describe_gauge!("arbitrage_bot_active_connections", "");
        metrics::describe_histogram!(
            "arbitrage_bot_event_processing_latency_microseconds",
            metrics::Unit::Microseconds,
            ""
        );
        metrics::describe_histogram!(
            "arbitrage_bot_parse_time_microseconds",
            metrics::Unit::Microseconds,
            ""
        );
        metrics::describe_histogram!(
            "arbitrage_bot_update_producer_time_microseconds",
            metrics::Unit::Microseconds,
            ""
        );
    }

    pub fn record_event_received(&self, dex: DexId) {
        counter!("arbitrage_bot_events_received_total").increment(1);
        if let Some(l) = self.labels(dex) {
            counter!("arbitrage_bot_dex_events_received_total", l).increment(1);
        }
    }

    pub fn record_event_processed(&self, dex: DexId) {
        counter!("arbitrage_bot_events_processed_total").increment(1);
        if let Some(l) = self.labels(dex) {
            counter!("arbitrage_bot_dex_events_processed_total", l).increment(1);
        }
    }

    pub fn record_parse_error(&self, dex: DexId) {
        counter!("arbitrage_bot_parse_errors_total").increment(1);
        if let Some(l) = self.labels(dex) {
            counter!("arbitrage_bot_dex_parse_errors_total", l).increment(1);
        }
    }

    pub fn record_parse_time(&self, dex: DexId, d: Duration) {
        let us = d.as_micros() as f64;
        histogram!("arbitrage_bot_parse_time_microseconds").record(us);
        if let Some(l) = self.labels(dex) {
            histogram!("arbitrage_bot_dex_parse_time_microseconds", l).record(us);
        }
    }

    pub fn record_update_producer_time(&self, d: Duration) {
        histogram!("arbitrage_bot_update_producer_time_microseconds")
            .record(d.as_micros() as f64);
    }

    pub fn record_total_processing_time(&self, dex: DexId, d: Duration) {
        let us = d.as_micros() as f64;
        histogram!("arbitrage_bot_event_processing_latency_microseconds").record(us);
        if let Some(l) = self.labels(dex) {
            histogram!("arbitrage_bot_dex_processing_latency_microseconds", l).record(us);
        }
    }

    pub fn increment_active_connections(&self) {
        gauge!("arbitrage_bot_active_connections").increment(1.0);
    }

    pub fn decrement_active_connections(&self) {
        gauge!("arbitrage_bot_active_connections").decrement(1.0);
    }

    pub fn record_reconnection(&self, dex: DexId) {
        counter!("arbitrage_bot_reconnections_total").increment(1);
        if let Some(l) = self.labels(dex) {
            counter!("arbitrage_bot_dex_reconnections_total", l).increment(1);
        }
    }
}