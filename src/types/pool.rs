use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::types::{DexId, PoolId, Timestamp, TokenInfo};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolState {
    pub dex_id: DexId,
    pub pool_id: PoolId,
    pub token_a: TokenInfo,
    pub token_b: TokenInfo,
    pub reserve_a: Decimal,
    pub reserve_b: Decimal,
    pub fee_rate: Decimal,
    pub block_timestamp: Timestamp,
}

impl PoolState {
    pub fn spot_price_a_to_b(&self) -> Decimal {
        self.reserve_b / self.reserve_a
    }

    pub fn spot_price_b_to_a(&self) -> Decimal {
        self.reserve_a / self.reserve_b
    }

    pub fn constant_product(&self) -> Decimal {
        self.reserve_a * self.reserve_b
    }
}
