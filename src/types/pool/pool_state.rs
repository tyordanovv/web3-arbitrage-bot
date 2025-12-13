use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sui_sdk::rpc_types::SuiObjectData;
use tracing::{debug, warn};
use serde_json::Value;

use crate::types::cetus::CetusPoolParser;
use crate::types::pool_parser::PoolParser;
use crate::types::{ChainAddress, DexId, SuiAddress, Timestamp, TokenInfo, now};
use crate::types::error::BotError;

pub type PoolId = ChainAddress;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolState {
    pub dex_id: DexId,
    pub pool_id: PoolId,
    pub token_a: TokenInfo,
    pub token_b: TokenInfo,
    pub reserve_a: Decimal,
    pub reserve_b: Decimal,
    pub liquidity: Decimal,
    pub fee_rate: Decimal,
    pub block_timestamp: Timestamp,
}

impl PoolState {
    pub fn new(
        dex_id: DexId,
        pool_id: PoolId,
        token_a: TokenInfo,
        token_b: TokenInfo,
    ) -> Self {
        Self {
            dex_id,
            pool_id,
            token_a,
            token_b,
            reserve_a: Decimal::ZERO,
            reserve_b: Decimal::ZERO,
            liquidity: Decimal::ZERO,
            fee_rate: Decimal::ZERO,
            block_timestamp: now(),
        }
    }

    // Utility methods
    pub fn spot_price_a_to_b(&self) -> Decimal {
        if self.reserve_a.is_zero() {
            return Decimal::ZERO;
        }
        self.reserve_b / self.reserve_a
    }

    pub fn spot_price_b_to_a(&self) -> Decimal {
        if self.reserve_b.is_zero() {
            return Decimal::ZERO;
        }
        self.reserve_a / self.reserve_b
    }

    pub fn constant_product(&self) -> Decimal {
        self.reserve_a * self.reserve_b
    }
}