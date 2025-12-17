use std::sync::Arc;

use async_trait::async_trait;
use tracing::{debug, info};

use crate::{arbitrage::calculator::ArbitrageCalculator, sync::reader::StateReader, types::{ArbitrageOpportunity, Result}};

#[async_trait]
pub trait ArbitrageDetector: Send + Sync {
    async fn next_opportunity(&mut self) -> Option<ArbitrageOpportunity>;
    fn get_stats(&self) -> DetectionStats;
}

#[derive(Debug, Clone)]
pub struct DetectionStats {
    pub scans_performed: u64,
    pub opportunities_found: u64,
    pub avg_scan_duration_ms: u64,
    pub last_scan_timestamp: u64,
}

pub struct DefaultArbitrageDetector {
    state_reader: Arc<StateReader>,
    calculator: Box<dyn ArbitrageCalculator>,
    is_running: bool,
    stats: DetectionStats,
}

impl DefaultArbitrageDetector {
    pub fn new(
        state_reader: Arc<StateReader>,
        calculator: Box<dyn ArbitrageCalculator>,
    ) -> Self {
        Self {
            state_reader,
            calculator,
            is_running: false,
            stats: DetectionStats {
                scans_performed: 0,
                opportunities_found: 0,
                avg_scan_duration_ms: 0,
                last_scan_timestamp: 0,
            },
        }
    }
}

#[async_trait]
impl ArbitrageDetector for DefaultArbitrageDetector {
    async fn next_opportunity(&mut self) -> Option<ArbitrageOpportunity> {
        let scan_start = std::time::Instant::now();

        // Get snapshot - LOCK-FREE READ
        let snapshot_start = std::time::Instant::now();
        let snapshot = self.state_reader.get_snapshot();
        let snapshot_read_time = snapshot_start.elapsed();

        info!(
            pool_count = snapshot.pool_count,
            dex_count = snapshot.dex_count,
            snapshot_age_ms = snapshot.snapshot_time.elapsed().as_millis(),
            read_time_ns = snapshot_read_time.as_nanos(),
            "Retrieved pool snapshot for arbitrage detection"
        );

        // TODO: Implement actual arbitrage detection algorithm
        // For now, this is a placeholder that demonstrates lock-free access

        let scan_duration = scan_start.elapsed();

        // Update stats
        self.stats.scans_performed += 1;
        self.stats.last_scan_timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Update rolling average
        let current_avg = self.stats.avg_scan_duration_ms;
        let new_duration_ms = scan_duration.as_millis() as u64;
        self.stats.avg_scan_duration_ms =
            (current_avg * (self.stats.scans_performed - 1) + new_duration_ms)
                / self.stats.scans_performed;

        debug!(
            scan_duration_us = scan_duration.as_micros(),
            avg_scan_duration_ms = self.stats.avg_scan_duration_ms,
            scans_performed = self.stats.scans_performed,
            "Arbitrage scan completed"
        );

        None
    }

    fn get_stats(&self) -> DetectionStats {
        self.stats.clone()
    }
}