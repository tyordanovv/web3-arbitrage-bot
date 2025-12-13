use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};
use std::str::FromStr;

use crate::types::{BotError, DexId, MIN_PROFIT_PERCENT, Network, Result, TokenInfo};

/// Network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub network: Network,
    pub rpc_url: String,
    pub ws_url: String,
    pub dexes: Vec<DexConfig>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            network: Network::SuiTestnet,
            rpc_url: "https://fullnode.testnet.sui.io:443".into(),
            ws_url: "wss://fullnode.testnet.sui.io:443".into(),
            dexes: vec![],
        }
    }
}

/// Simplified DEX config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DexConfig {
    pub id: DexId,
    pub package_id: String,
    pub event_type: String,
    pub enabled: bool,
    pub pools: Vec<PoolConfig>,
}

/// Pool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    pub address: String,
    pub token_a: TokenInfo,
    pub token_b: TokenInfo,
}

/// Configuration for path finding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageConfig {
    pub max_hops: usize,
    pub min_liquidity_per_pool_usd: Decimal,
    pub max_price_impact_percent: Decimal,
    pub min_profit_threshold: Decimal,
    pub min_profit_percent: Decimal,
}

impl Default for ArbitrageConfig {
    fn default() -> Self {
        Self {
            max_hops: 4,
            min_liquidity_per_pool_usd: Decimal::from(10000),
            max_price_impact_percent: Decimal::from(5),
            min_profit_threshold: Decimal::from(1),
            min_profit_percent: MIN_PROFIT_PERCENT,
        }
    }
}

/// Execution configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionConfig {
    /// Dry run mode (don't execute real transactions)
    pub dry_run: bool,
    
    /// Wallet private key (if not dry run)
    pub private_key: Option<String>,
    
    /// Gas budget per transaction
    pub gas_budget: u64,
    
    /// Slippage tolerance percentage
    pub slippage_tolerance_percent: Decimal,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            dry_run: true,
            private_key: None,
            gas_budget: 10_000_000,
            slippage_tolerance_percent: Decimal::from_str("1.0").unwrap(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationConfig {
    /// Maximum age of opportunity in milliseconds before considering stale
    pub max_opportunity_age_ms: u64,
    
    /// Minimum liquidity in USD for a pool to be considered
    pub min_pool_liquidity_usd: Decimal,
    
    /// Maximum price divergence percentage from expected
    pub max_price_divergence_percent: Decimal,
    
    /// Whether to re-validate with current state before execution
    pub revalidate_before_execution: bool,
    
    /// Maximum gas cost as percentage of profit
    pub max_gas_cost_percent: Decimal,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            max_opportunity_age_ms: 2000,
            min_pool_liquidity_usd: Decimal::from(1000),
            max_price_divergence_percent: Decimal::from_str("5.0").unwrap(),
            revalidate_before_execution: true,
            max_gas_cost_percent: Decimal::from_str("50.0").unwrap(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub enable_metrics: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".into(),
            enable_metrics: true,
        }
    }
}

/// Consolidated synchronization configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    /// Maximum number of pools per DEX
    pub max_pools_per_dex: usize,
    
    /// Enable periodic state synchronization
    pub enable_periodic_sync: bool,
    
    /// Normal sync interval in seconds
    pub sync_interval_seconds: u64,
    
    /// Emergency sync interval in seconds (used when issues occur)
    pub emergency_sync_interval_seconds: u64,
    
    /// State time-to-live in seconds (when state is considered stale)
    pub state_ttl_seconds: u64,
    
    /// Batch size for pool synchronization
    pub batch_size: usize,
    
    /// Maximum retry attempts for failed syncs
    pub max_retries: u32,
    
    /// Delay between retries in seconds
    pub retry_delay_seconds: u64,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            max_pools_per_dex: 1000,
            enable_periodic_sync: true,
            sync_interval_seconds: 3600,        // 1 hour
            emergency_sync_interval_seconds: 300, // 5 minutes
            state_ttl_seconds: 3600,            // 1 hour
            batch_size: 10,
            max_retries: 3,
            retry_delay_seconds: 5,
        }
    }
}

impl SyncConfig {
    // Helper methods to convert to Duration
    pub fn sync_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.sync_interval_seconds)
    }
    
    pub fn emergency_sync_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.emergency_sync_interval_seconds)
    }
    
    pub fn state_ttl(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.state_ttl_seconds)
    }
    
    pub fn retry_delay(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.retry_delay_seconds)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Network settings
    pub network: NetworkConfig,
    
    /// Arbitrage settings
    pub arbitrage: ArbitrageConfig,
    
    /// Execution settings
    pub execution: ExecutionConfig,

    /// Opportunity validation settings
    pub validation: ValidationConfig,

    /// Logging settings
    pub logging: LoggingConfig, 

    /// State synchronization settings (replaces both DexManagerConfig and SyncConfig)
    pub sync: SyncConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            network: NetworkConfig::default(),
            arbitrage: ArbitrageConfig::default(),
            execution: ExecutionConfig::default(),
            validation: ValidationConfig::default(),
            logging: LoggingConfig::default(),
            sync: SyncConfig::default(),
        }
    }
}

impl Config {
    pub fn network_config(&self) -> &NetworkConfig {
        &self.network
    }
    
    pub fn arbitrage_config(&self) -> &ArbitrageConfig {
        &self.arbitrage
    }
    
    pub fn validation_config(&self) -> &ValidationConfig {
        &self.validation
    }
    
    pub fn execution_config(&self) -> &ExecutionConfig {
        &self.execution
    }

    pub fn logging_config(&self) -> &LoggingConfig {
        &self.logging
    }

    pub fn sync_config(&self) -> &SyncConfig {
        &self.sync
    }

    /// Load config from file or use defaults
    pub fn load() -> Result<Self> {
        info!("Loading configuration...");
        
        // Try to load from config file
        match Self::load_from_file("config.toml") {
            Ok(config) => {
                info!("Config loaded from config.toml");
                config.log_loaded_config();
                return Ok(config);
            }
            Err(e) => {
                warn!("Failed to load config from file: {}", e);
                warn!("Using default configuration instead");
            }
        }
        
        let mut config = Self::default();
        config.apply_env_vars()?;
        
        info!("Using default configuration with environment overrides");
        config.log_loaded_config();
        
        Ok(config)
    }    
    /// Load from TOML file
    fn load_from_file(path: &str) -> Result<Self> {
        info!("Loading config from: {}", path);
        
        // Check if file exists first
        if !std::path::Path::new(path).exists() {
            return Err(BotError::Config(format!("Config file not found: {}", path)));
        }
        
        // Read the file content
        let content = std::fs::read_to_string(path)
            .map_err(|e| BotError::Config(format!("Failed to read config file {}: {}", path, e)))?;
        
        info!("Config file found, size: {} bytes", content.len());
        
        // Parse the TOML content
        let config: Config = toml::from_str(&content)
            .map_err(|e| {
                error!("Failed to parse TOML config: {}", e);
                BotError::Config(format!("Failed to parse config file {}: {}", path, e))
            })?;
        
        info!("Config parsed successfully from: {}", path);
        Ok(config)
    }
    
    /// Apply environment variable overrides
    fn apply_env_vars(&mut self) -> Result<()> {
        debug!("Applying environment variable overrides...");
        
        if let Ok(rpc_url) = std::env::var("RPC_URL") {
            info!("Overriding RPC_URL from environment: {}", rpc_url);
            self.network.rpc_url = rpc_url;
        }
        
        if let Ok(ws_url) = std::env::var("WS_URL") {
            info!("Overriding WS_URL from environment: {}", ws_url);
            self.network.ws_url = ws_url;
        }
        
        if let Ok(private_key) = std::env::var("PRIVATE_KEY") {
            info!("Private key provided via environment, disabling dry-run mode");
            self.execution.private_key = Some(private_key);
            self.execution.dry_run = false;
        }
        
        Ok(())
    }
    
    /// Log the loaded configuration
    fn log_loaded_config(&self) {
        info!("=== LOADED CONFIGURATION ===");
        info!("Network: {:?}", self.network.network);
        info!("RPC URL: {}", self.network.rpc_url);
        info!("WebSocket URL: {}", self.network.ws_url);
        
        info!("Sync Settings:");
        info!("  Max pools per DEX: {}", self.sync.max_pools_per_dex);
        info!("  Enable periodic sync: {}", self.sync.enable_periodic_sync);
        info!("  Sync interval: {}s", self.sync.sync_interval_seconds);
        info!("  Emergency sync interval: {}s", self.sync.emergency_sync_interval_seconds);
        info!("  State TTL: {}s", self.sync.state_ttl_seconds);
        info!("  Batch size: {}", self.sync.batch_size);
        info!("  Max retries: {}", self.sync.max_retries);
        info!("  Retry delay: {}s", self.sync.retry_delay_seconds);

        
        info!("Configured DEXs ({} total):", self.network.dexes.len());
        for (i, dex) in self.network.dexes.iter().enumerate() {
            info!("  {}. {:?} - enabled: {}, package: {}, event_type: {}", 
                i + 1, dex.id, dex.enabled, dex.package_id, dex.event_type);
            info!("     Pools: {}", dex.pools.len());
            for pool in &dex.pools {
                info!("       - {}: {}/{}", 
                    pool.address, 
                    pool.token_a.symbol, 
                    pool.token_b.symbol);
            }
        }
        
        info!("Enabled DEXs: {}", self.enabled_dexes().len());
        for dex in self.enabled_dexes() {
            info!("  - {:?} (package: {})", dex.id, dex.package_id);
        }
        
        info!("Arbitrage Settings:");
        info!("  Max hops: {}", self.arbitrage.max_hops);
        info!("  Min profit %: {}", self.arbitrage.min_profit_percent);
        
        info!("Execution Settings:");
        info!("  Dry run: {}", self.execution.dry_run);
        info!("  Gas budget: {}", self.execution.gas_budget);
        info!("  Slippage: {}%", self.execution.slippage_tolerance_percent);
        
        info!("Validation Settings:");
        info!("  Max opportunity age: {}ms", self.validation.max_opportunity_age_ms);
        info!("  Min pool liquidity: ${}", self.validation.min_pool_liquidity_usd);
        
        info!("=================================");
    }
    
    /// Get enabled DEXs
    pub fn enabled_dexes(&self) -> Vec<&DexConfig> {
        self.network.dexes.iter().filter(|d| d.enabled).collect()
    }
    
    /// Validate configuration
    pub fn validate(&self) -> Result<()> {        
        if !self.execution.dry_run && self.execution.private_key.is_none() {
            return Err(BotError::Config("Private key required when not in dry-run mode".into()));
        }
        
        let enabled_dexes = self.enabled_dexes();
        if enabled_dexes.is_empty() {
            warn!("No DEXs enabled in configuration!");
            return Err(BotError::Config("No DEXs enabled".into()));
        }
        
        info!("Configuration validation passed");
        info!("Found {} enabled DEXs", enabled_dexes.len());
        
        Ok(())
    }
}