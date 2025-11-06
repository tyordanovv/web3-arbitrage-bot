use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;

use crate::{dex::state::{DexConfig, SyncSettings}, types::{BotError, DexId, FeeStructure, Network, PathFinderConfig, Result, TokenInfo, TokenPair}};

/// Main application configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub active_network: String,
    pub networks: HashMap<String, NetworkConfig>,
    pub arbitrage: ArbitrageConfig,
    pub execution: ExecutionConfig,
    pub sync: SyncConfig,
    pub logging: LoggingConfig,
}

impl AppConfig {
    /// Get the active network config
    pub fn get_active_network(&self) -> Result<&NetworkConfig> {
        self.networks.get(&self.active_network)
            .ok_or_else(|| BotError::Config(
                format!("Active network '{}' not found in config", self.active_network)
            ))
    }
    
    /// Get all enabled DEXs for active network
    pub fn get_enabled_dexes(&self) -> Result<Vec<&DexConfigRaw>> {
        let network = self.get_active_network()?;
        Ok(network.dexes.values()
            .filter(|d| d.enabled)
            .collect())
    }
    
    /// Get all tokens for active network
    pub fn get_all_tokens(&self) -> Result<Vec<TokenInfo>> {
        let network = self.get_active_network()?;
        let mut tokens = Vec::new();
        let mut seen = std::collections::HashSet::new();
        
        for dex in network.dexes.values() {
            for token in dex.tokens.values() {
                if seen.insert(token.address.clone()) {
                    tokens.push(token.to_token_info());
                }
            }
        }
        
        Ok(tokens)
    }
}

/// Raw Network configuration which comes from the config file
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NetworkConfig {
    pub network_type: String,
    pub rpc_url: String,
    pub ws_url: String,
    #[serde(default = "default_timeout")]
    pub request_timeout_secs: u64,
    #[serde(default = "default_retries")]
    pub max_retries: u32,
    pub dexes: HashMap<String, DexConfigRaw>,
}

fn default_timeout() -> u64 { 30 }
fn default_retries() -> u32 { 3 }

impl NetworkConfig {
    /// Parse network type into Network enum
    pub fn get_network_type(&self) -> Result<Network> {
        match self.network_type.as_str() {
            "sui-mainnet" | "mainnet-sui" => Ok(Network::SuiMainnet),
            "sui-testnet" | "testnet-sui" => Ok(Network::SuiTestnet),
            // "sui-devnet" | "devnet-sui" => Ok(Network::SuiDevnet), 
            // "aptos-mainnet" | "mainnet-aptos" => Ok(Network::AptosMainnet),
            // "aptos-testnet" | "testnet-aptos" => Ok(Network::AptosTestnet),
            _ => Err(BotError::Config(format!("Unknown network type: {}", self.network_type))),
        }
    }
    
    /// Check if this is a Sui network
    pub fn is_sui(&self) -> bool {
        self.network_type.to_lowercase().contains("sui")
    }
    
    /// Check if this is an Aptos network
    pub fn is_aptos(&self) -> bool {
        self.network_type.to_lowercase().contains("aptos")
    }
}

/// Raw DEX configuration which comes from the config file
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DexConfigRaw {
    pub name: String,
    pub package_id: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub pools: HashMap<String, String>,
    pub fees: FeeConfig,
    pub tokens: HashMap<String, TokenConfig>,
}

fn default_enabled() -> bool { true }

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FeeConfig {
    pub default_fee: String,
    #[serde(default)]
    pub tiers: HashMap<String, String>,
    pub protocol_fee: Option<String>,
}

impl FeeConfig {
    /// Convert to FeeStructure
    pub fn to_fee_structure(&self) -> Result<FeeStructure> {
        let swap_fee_rate = Decimal::from_str(&self.default_fee)
            .map_err(|e| BotError::Config(format!("Invalid fee: {}", e)))?;
        
        let protocol_fee_rate = if let Some(ref pf) = self.protocol_fee {
            Some(Decimal::from_str(pf)
                .map_err(|e| BotError::Config(format!("Invalid protocol fee: {}", e)))?)
        } else {
            None
        };
        
        let mut fee_tiers = HashMap::new();
        for (pair, fee) in &self.tiers {
            let fee_decimal = Decimal::from_str(fee)
                .map_err(|e| BotError::Config(format!("Invalid tier fee: {}", e)))?;
            fee_tiers.insert(pair.clone(), fee_decimal);
        }
        
        Ok(FeeStructure {
            swap_fee_rate,
            protocol_fee_rate,
            fee_tiers,
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TokenConfig {
    pub symbol: String,
    pub address: String,
    pub decimals: u8,
    pub name: Option<String>,
}

impl TokenConfig {
    /// Convert to TokenInfo
    pub fn to_token_info(&self) -> TokenInfo {
        let mut info = TokenInfo::new(
            self.symbol.clone(),
            self.address.clone(),
            self.decimals,
        );
        
        if let Some(ref name) = self.name {
            info = info.with_name(name.clone());
        }
        
        info
    }
}

impl DexConfigRaw {
    /// Convert to DexConfig (used by DexState)
    pub fn to_dex_config(&self, sync_settings: SyncSettings) -> Result<DexConfig> {
        let dex_id = DexId::from_str(&self.name)
            .map_err(|e| BotError::Config(format!("Invalid DEX name: {}", e)))?;
        
        // Build token pairs
        let mut monitored_pairs = Vec::new();
        let mut pool_addresses = HashMap::new();
        
        for (pair_symbol, pool_address) in &self.pools {
            // Parse pair symbol "SUI/USDC"
            let parts: Vec<&str> = pair_symbol.split('/').collect();
            if parts.len() != 2 {
                return Err(BotError::Config(
                    format!("Invalid pair format: {}", pair_symbol)
                ));
            }
            
            // Get token configs
            let base_token = self.tokens.get(parts[0])
                .ok_or_else(|| BotError::Config(
                    format!("Token {} not found in config", parts[0])
                ))?;
            
            let quote_token = self.tokens.get(parts[1])
                .ok_or_else(|| BotError::Config(
                    format!("Token {} not found in config", parts[1])
                ))?;
            
            // Create pair
            let pair = TokenPair::new(
                base_token.to_token_info(),
                quote_token.to_token_info(),
            );
            
            monitored_pairs.push(pair.clone());
            pool_addresses.insert(pair, pool_address.clone());
        }
        
        Ok(DexConfig {
            name: dex_id,
            package_id: self.package_id.clone(),
            monitored_pairs,
            pool_addresses,
            fee_structure: self.fees.to_fee_structure()?,
            sync_settings,
        })
    }
}

// Arbitrage Configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ArbitrageConfig {
    pub min_profit_threshold: String,
    pub min_profit_percent: String,
    pub max_trade_size: String,
    #[serde(default = "default_max_hops")]
    pub max_hops: usize,
    pub max_price_impact_percent: String,
    pub min_liquidity_usd: String,
    #[serde(default)]
    pub enable_cross_chain: bool,
    #[serde(default = "default_stale_threshold")]
    pub stale_price_threshold_ms: u64,
}

fn default_max_hops() -> usize { 4 }
fn default_stale_threshold() -> u64 { 5000 }

impl ArbitrageConfig {
    /// Convert to PathFinderConfig
    pub fn to_path_finder_config(
        &self,
        intermediate_tokens: Vec<TokenInfo>,
        allowed_dexes: Vec<DexId>,
    ) -> Result<PathFinderConfig> {
        Ok(PathFinderConfig {
            max_hops: self.max_hops,
            min_liquidity_per_pool_usd: Decimal::from_str(&self.min_liquidity_usd)
                .map_err(|e| BotError::Config(format!("Invalid min_liquidity: {}", e)))?,
            max_price_impact_percent: Decimal::from_str(&self.max_price_impact_percent)
                .map_err(|e| BotError::Config(format!("Invalid max_price_impact: {}", e)))?,
            intermediate_tokens,
            allowed_dexes,
            allow_cross_chain: self.enable_cross_chain,
            min_profit_threshold: Decimal::from_str(&self.min_profit_threshold)
                .map_err(|e| BotError::Config(format!("Invalid min_profit_threshold: {}", e)))?,
            min_profit_percent: Decimal::from_str(&self.min_profit_percent)
                .map_err(|e| BotError::Config(format!("Invalid min_profit_percent: {}", e)))?,
        })
    }
    
    /// Get minimum profit as Decimal
    pub fn get_min_profit(&self) -> Result<Decimal> {
        Decimal::from_str(&self.min_profit_threshold)
            .map_err(|e| BotError::Config(format!("Invalid min_profit: {}", e)))
    }
    
    /// Get minimum profit percent as Decimal
    pub fn get_min_profit_percent(&self) -> Result<Decimal> {
        Decimal::from_str(&self.min_profit_percent)
            .map_err(|e| BotError::Config(format!("Invalid min_profit_percent: {}", e)))
    }
}

// Transaction execution configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExecutionConfig {
    pub gas_budget: u64,
    pub slippage_tolerance: String,
    #[serde(default = "default_dry_run")]
    pub dry_run_only: bool,
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_executions: usize,
    #[serde(skip)]
    pub wallet_private_key: Option<String>,
}

fn default_dry_run() -> bool { true }
fn default_max_concurrent() -> usize { 3 }

impl ExecutionConfig {
    /// Get slippage tolerance as Decimal
    pub fn get_slippage_tolerance(&self) -> Result<Decimal> {
        Decimal::from_str(&self.slippage_tolerance)
            .map_err(|e| BotError::Config(format!("Invalid slippage: {}", e)))
    }
}

/// Sync configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SyncConfig {
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval_secs: u64,
    #[serde(default = "default_heartbeat_timeout")]
    pub heartbeat_timeout_secs: u64,
    #[serde(default = "default_periodic_interval")]
    pub periodic_sync_interval_secs: u64,
    #[serde(default = "default_enable_fallback")]
    pub enable_fallback_polling: bool,
}

fn default_heartbeat_interval() -> u64 { 10 }
fn default_heartbeat_timeout() -> u64 { 30 }
fn default_periodic_interval() -> u64 { 300 }
fn default_enable_fallback() -> bool { true }

impl SyncConfig {
    /// Convert to SyncSettings (used by DexConfig)
    pub fn to_sync_settings(&self) -> SyncSettings {
        SyncSettings {
            heartbeat_interval_secs: self.heartbeat_interval_secs,
            heartbeat_timeout_secs: self.heartbeat_timeout_secs,
            periodic_sync_interval_secs: self.periodic_sync_interval_secs,
            enable_fallback_polling: self.enable_fallback_polling,
        }
    }
}

/// Logging configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default)]
    pub log_to_file: bool,
    #[serde(default = "default_log_path")]
    pub log_file_path: String,
}

fn default_log_level() -> String { "info".into() }
fn default_log_path() -> String { "logs/arbitrage.log".into() }

/// Load configuration from file and environment
pub fn load_config() -> Result<AppConfig> {
    dotenv::dotenv().ok();
    
    let config_path = std::env::var("CONFIG_PATH")
        .unwrap_or_else(|_| "config/config.toml".to_string());
    
    let mut config = load_from_file(&config_path)?;
    
    // Override with environment variables
    apply_env_overrides(&mut config)?;
    
    // Validate configuration
    validate_config(&config)?;
    
    Ok(config)
}

/// Load config from TOML file
fn load_from_file(path: &str) -> Result<AppConfig> {
    if !Path::new(path).exists() {
        return Err(BotError::Config(format!("Config file not found: {}", path)));
    }
    
    let content = std::fs::read_to_string(path)
        .map_err(|e| BotError::Config(format!("Failed to read config: {}", e)))?;
    
    let config: AppConfig = toml::from_str(&content)
        .map_err(|e| BotError::Config(format!("Failed to parse config: {}", e)))?;
    
    Ok(config)
}

/// Override config with environment variables
fn apply_env_overrides(config: &mut AppConfig) -> Result<()> {
    if let Ok(network) = std::env::var("ACTIVE_NETWORK") {
        config.active_network = network;
    }
    
    let active = config.active_network.clone();
    
    // Override network settings
    if let Some(network_config) = config.networks.get_mut(&active) {
        if let Ok(rpc_url) = std::env::var("RPC_URL") {
            network_config.rpc_url = rpc_url;
        }
        
        if let Ok(ws_url) = std::env::var("WS_URL") {
            network_config.ws_url = ws_url;
        }
    }
    
    // Execution overrides
    if let Ok(key) = std::env::var("WALLET_PRIVATE_KEY") {
        config.execution.wallet_private_key = Some(key);
    }
    
    if let Ok(dry_run) = std::env::var("DRY_RUN_ONLY") {
        config.execution.dry_run_only = dry_run.parse().unwrap_or(true);
    }
    
    // Logging overrides
    if let Ok(level) = std::env::var("RUST_LOG") {
        config.logging.level = level;
    }
    
    Ok(())
}

/// Validate configuration
/// 
/// - Check active network exists
/// - Check at least one DEX is enabled
/// - Validate network URLs
/// - Validate execution config
/// - Validate arbitrage thresholds
fn validate_config(config: &AppConfig) -> Result<()> {
    let network = config.get_active_network()?;
    
    let enabled_dexes: Vec<_> = network.dexes.values()
        .filter(|d| d.enabled)
        .collect();
    
    if enabled_dexes.is_empty() {
        return Err(BotError::Config(
            format!("No DEXs enabled in network '{}'", config.active_network)
        ));
    }
    
    if network.rpc_url.is_empty() {
        return Err(BotError::Config(
            format!("RPC URL not set for network '{}'", config.active_network)
        ));
    }
    
    if network.ws_url.is_empty() {
        return Err(BotError::Config(
            format!("WebSocket URL not set for network '{}'", config.active_network)
        ));
    }
    
    if !config.execution.dry_run_only && config.execution.wallet_private_key.is_none() {
        return Err(BotError::Config(
            "Wallet private key required when dry_run_only is false".into()
        ));
    }
    
    config.arbitrage.get_min_profit()?;
    config.arbitrage.get_min_profit_percent()?;
    
    Ok(())
}