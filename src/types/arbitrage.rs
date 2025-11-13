use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::types::{BotError, DexId, MIN_PROFIT_PERCENT, Network, PoolId, Result, Timestamp, TokenInfo, TokenPair, now};

/// A single hop in an arbitrage path
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageHop {
    pub dex_id: DexId,
    pub pool_id: PoolId,
    pub pair: TokenPair,
    pub sell_base: bool,
    pub token_in: TokenInfo,
    pub token_out: TokenInfo,
    pub amount_in: u64,
    pub expected_amount_out: u64,
    pub min_amount_out: u64,
    pub price_impact: Decimal,
    pub fee_rate: Decimal,
}

impl ArbitrageHop {
    /// Get human-readable description
    pub fn description(&self) -> String {
        format!(
            "{} {} → {} on {}",
            self.token_in.symbol,
            self.token_out.symbol,
            self.dex_id,
            if self.sell_base { "SELL" } else { "BUY" }
        )
    }
}

/// Complete arbitrage path across multiple hops
/// Example: USDC -> SUI -> BTC -> USDC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitragePath {
    pub path_id: String,
    pub start_token: TokenInfo,
    pub end_token: TokenInfo,
    pub hops: Vec<ArbitrageHop>,
    pub initial_amount: u64,
    pub expected_final_amount: u64,
    pub min_final_amount: u64,
    pub calculated_at: Timestamp,
    pub networks: Vec<Network>,
}

impl ArbitragePath {
    /// Check if this is a closed loop (ends with same token)
    pub fn is_closed_loop(&self) -> bool {
        self.start_token.address == self.end_token.address
    }
    
    /// Get number of hops
    pub fn hop_count(&self) -> usize {
        self.hops.len()
    }
    
    /// Check if path is triangular (3 hops)
    pub fn is_triangular(&self) -> bool {
        self.hop_count() == 3 && self.is_closed_loop()
    }
    
    /// Calculate gross profit (before gas)
    pub fn gross_profit(&self) -> i64 {
        self.expected_final_amount as i64 - self.initial_amount as i64
    }
    
    /// Calculate gross profit as Decimal (accounting for decimals)
    pub fn gross_profit_decimal(&self) -> Decimal {
        let initial = self.start_token.to_decimal(self.initial_amount);
        let final_amount = self.end_token.to_decimal(self.expected_final_amount);
        final_amount - initial
    }
    
    /// Calculate profit percentage
    pub fn profit_percent(&self) -> Decimal {
        if self.initial_amount == 0 {
            return Decimal::ZERO;
        }
        
        let profit = self.gross_profit() as f64;
        let initial = self.initial_amount as f64;
        
        Decimal::from_f64_retain(profit / initial * 100.0).unwrap_or(Decimal::ZERO)
    }
    
    /// Get all tokens in the path
    pub fn all_tokens(&self) -> Vec<TokenInfo> {
        let mut tokens = vec![self.start_token.clone()];
        for hop in &self.hops {
            tokens.push(hop.token_out.clone());
        }
        tokens
    }
    
    /// Get human-readable path description
    /// Example: "USDC -> SUI (Cetus) -> BTC (Turbos) -> USDC (Cetus)"
    pub fn path_description(&self) -> String {
        let mut desc = self.start_token.symbol.clone();
        
        for hop in &self.hops {
            desc.push_str(&format!(
                " → {} ({})",
                hop.token_out.symbol,
                hop.dex_id
            ));
        }
        
        desc
    }
    
    /// Check if path is stale
    pub fn is_stale(&self, max_age_ms: u64) -> bool {
        now() - self.calculated_at > max_age_ms
    }
    
    /// Validate path integrity
    pub fn validate(&self) -> Result<()> {
        if self.hops.is_empty() {
            return Err(BotError::InvalidState(
                "Path has no hops".into()
            ));
        }
        
        // Check hops are connected
        for i in 0..self.hops.len() - 1 {
            let current_out = &self.hops[i].token_out;
            let next_in = &self.hops[i + 1].token_in;
            
            if current_out.address != next_in.address {
                return Err(BotError::InvalidState(
                    format!(
                        "Hop {} output ({}) doesn't match hop {} input ({})",
                        i, current_out.symbol, i + 1, next_in.symbol
                    )
                ));
            }
        }
        
        // Check first hop input matches start token
        if self.hops[0].token_in.address != self.start_token.address {
            return Err(BotError::InvalidState(
                "First hop input doesn't match start token".into()
            ));
        }
        
        // Check last hop output matches end token
        if self.hops.last().unwrap().token_out.address != self.end_token.address {
            return Err(BotError::InvalidState(
                "Last hop output doesn't match end token".into()
            ));
        }
        
        Ok(())
    }
}

/// Arbitrage opportunity (profitable path)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageOpportunity {
    pub path: ArbitragePath,
    pub gross_profit: Decimal,
    pub estimated_gas_cost: Decimal,
    pub total_dex_fees: Decimal,
    pub net_profit: Decimal,
    pub net_profit_percent: Decimal,
    pub discovered_at: Timestamp,
}

impl ArbitrageOpportunity {
    /// Check if opportunity is still profitable
    pub fn is_profitable(&self, min_profit: Decimal) -> bool {
        self.net_profit >= min_profit
    }
    
    /// Pretty print summary
    pub fn summary(&self) -> String {
        format!(
            "Opportunity: {}\n\
             Profit: {} {} ({:.2}%)\n\
             Gas: {} {}\n\
             Net: {} {}\n\
             Path: {}",
            self.path.path_id,
            self.gross_profit,
            self.path.start_token.symbol,
            self.net_profit_percent,
            self.estimated_gas_cost,
            self.path.start_token.symbol,
            self.net_profit,
            self.path.start_token.symbol,
            self.path.path_description()
        )
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use crate::types::{DEX_SWAP_FEE_RATE, DexId, Network, TokenInfo, TokenPair, now};

    use super::*;
    
    #[test]
    fn test_triangular_arbitrage() {
        // Setup tokens
        let usdc = TokenInfo::new("USDC", "0x2::usdc::USDC", 6);
        let sui = TokenInfo::new("SUI", "0x2::sui::SUI", 9);
        let btc = TokenInfo::new("BTC", "0x2::btc::BTC", 8);
        
        // Create triangular path: USDC → SUI → BTC → USDC
        let path = ArbitragePath {
            path_id: "triangle-1".into(),
            start_token: usdc.clone(),
            end_token: usdc.clone(),
            hops: vec![
                ArbitrageHop {
                    dex_id: DexId::Cetus,
                    pool_id: "pool1".into(),
                    pair: TokenPair::new(sui.clone(), usdc.clone()),
                    sell_base: false, // Buy SUI with USDC
                    token_in: usdc.clone(),
                    token_out: sui.clone(),
                    amount_in: 1000_000000, // 1000 USDC
                    expected_amount_out: 500_000000000, // 500 SUI
                    min_amount_out: 495_000000000,
                    price_impact: MIN_PROFIT_PERCENT,
                    fee_rate: DEX_SWAP_FEE_RATE,
                },
                ArbitrageHop {
                    dex_id: DexId::Cetus,
                    pool_id: "pool2".into(),
                    pair: TokenPair::new(btc.clone(), sui.clone()),
                    sell_base: false, // Buy BTC with SUI
                    token_in: sui.clone(),
                    token_out: btc.clone(),
                    amount_in: 500_000000000,
                    expected_amount_out: 1_50000000, // 1.5 BTC
                    min_amount_out: 1_48500000,
                    price_impact: MIN_PROFIT_PERCENT,
                    fee_rate: DEX_SWAP_FEE_RATE,
                },
                ArbitrageHop {
                    dex_id: DexId::Cetus,
                    pool_id: "pool3".into(),
                    pair: TokenPair::new(btc.clone(), usdc.clone()),
                    sell_base: true, // Sell BTC for USDC
                    token_in: btc.clone(),
                    token_out: usdc.clone(),
                    amount_in: 1_50000000,
                    expected_amount_out: 1010_000000, // 1010 USDC
                    min_amount_out: 1005_000000,
                    price_impact: MIN_PROFIT_PERCENT,
                    fee_rate: DEX_SWAP_FEE_RATE,
                },
            ],
            initial_amount: 1000_000000,
            expected_final_amount: 1010_000000,
            min_final_amount: 1005_000000,
            calculated_at: now(),
            networks: vec![Network::SuiTestnet],
        };
        
        // Validate
        assert!(path.validate().is_ok());
        assert!(path.is_triangular());
        assert!(path.is_closed_loop());
        
        // Check profit
        let profit = path.gross_profit_decimal();
        assert_eq!(profit, Decimal::from(10)); // 10 USDC profit
        
        println!("Path: {}", path.path_description());
        println!("Profit: {:.2}%", path.profit_percent());
    }
}