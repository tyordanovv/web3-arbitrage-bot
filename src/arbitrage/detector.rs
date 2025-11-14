use std::sync::{Arc};
use tokio::sync::{mpsc, RwLock};

use async_trait::async_trait;
use tracing::info;

use crate::{arbitrage::calculator::ArbitrageCalculator, dex::manager::DexManager, types::{ArbitrageOpportunity, Result}};

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
    dex_manager: Arc<RwLock<DexManager>>,
    calculator: Box<dyn ArbitrageCalculator>,
    is_running: bool,
    stats: DetectionStats,
}

impl DefaultArbitrageDetector {
    pub fn new(
        dex_manager: Arc<RwLock<DexManager>>,
        calculator: Box<dyn ArbitrageCalculator>,
    ) -> Self {        
        Self {
            dex_manager,
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
        // todo!("Receive next opportunity from channel");
        None
    }
    
    fn get_stats(&self) -> DetectionStats {
        self.stats.clone()
    }
}