// src/config.rs
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use crate::types::{BotError, DexId, MIN_PROFIT_PERCENT, Network, Result, TokenInfo};

/// Simple, focused configuration
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
}

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
    pub intermediate_tokens: Vec<TokenInfo>,
    pub allowed_dexes: Vec<DexId>,
    pub allow_cross_chain: bool,
    pub min_profit_threshold: Decimal,
    pub min_profit_percent: Decimal,
}

impl Default for ArbitrageConfig {
    fn default() -> Self {
        Self {
            max_hops: 4,
            min_liquidity_per_pool_usd: Decimal::from(10000),
            max_price_impact_percent: Decimal::from(5),
            intermediate_tokens: vec![],
            allowed_dexes: DexId::all(),
            allow_cross_chain: false,
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

impl Default for Config {
    fn default() -> Self {
        Self {
            network: NetworkConfig::default(),
            arbitrage: ArbitrageConfig::default(),
            execution: ExecutionConfig::default(),
            validation: ValidationConfig::default(),
            logging: LoggingConfig::default()
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

    /// Load config from file or use defaults
    pub fn load() -> Result<Self> {
        // Try to load from config file
        if let Ok(config) = Self::load_from_file("config.toml") {
            return Ok(config);
        }
        
        // Fall back to environment variables + defaults
        let mut config = Self::default();
        config.apply_env_vars()?;
        Ok(config)
    }
    
    /// Load from TOML file
    fn load_from_file(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content).unwrap();
        Ok(config)
    }
    
    /// Apply environment variable overrides
    fn apply_env_vars(&mut self) -> Result<()> {
        if let Ok(rpc_url) = std::env::var("RPC_URL") {
            self.network.rpc_url = rpc_url;
        }
        
        if let Ok(ws_url) = std::env::var("WS_URL") {
            self.network.ws_url = ws_url;
        }
        
        if let Ok(private_key) = std::env::var("PRIVATE_KEY") {
            self.execution.private_key = Some(private_key);
            self.execution.dry_run = false;
        }
        
        Ok(())
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
        
        if self.enabled_dexes().is_empty() {
            return Err(BotError::Config("No DEXs enabled".into()));
        }
        
        Ok(())
    }
}