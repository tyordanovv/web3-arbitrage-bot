use sui_sdk::rpc_types::SuiEvent;
use serde_json::Value;

use crate::types::{DexId, Result, SwapDelta, now};
use crate::types::pool_state::PoolId;
use crate::types::SuiAddress;

pub fn parse_sui_event(dex: DexId, ev: &SuiEvent) -> Result<SwapDelta> {
    let json = ev.parsed_json.as_ref()
        .ok_or_else(|| crate::types::BotError::Parse("missing parsed_json".into()))?;

    match dex {
        DexId::Cetus => parse_cetus(json, ev),
        DexId::Turbos => parse_turbos(json, ev),
        _ => Err(crate::types::BotError::Parse("unsupported dex".into())),
    }
}

fn base(
    dex: DexId,
    pool: &str,
    ev: &SuiEvent,
    amount_in: u64,
    amount_out: u64,
    base_to_quote: bool,
) -> Result<SwapDelta> {
    Ok(SwapDelta {
        dex_id: dex,
        pool_id: PoolId::Sui(SuiAddress::from_str(pool)?),
        amount_in,
        amount_out,
        base_to_quote,
        timestamp: ev.timestamp_ms.unwrap_or_else(now()),
        transaction_digest: ev.id.tx_digest.to_string(),
        sender: ev.sender.map(|s| s.to_string()),
        block_height: ev.checkpoint.map(|c| c.value()),
        sequence: ev.id.event_seq.map(|s| s.value()),
    })
}

fn parse_cetus(j: &Value, ev: &SuiEvent) -> Result<SwapDelta> {
    let pool = j["pool_id"].as_str().ok_or(parse("pool_id"))?;
    let amount_in = j["amount_in"].as_str().and_then(|v| v.parse().ok()).unwrap_or(0);
    let amount_out = j["amount_out"].as_str().and_then(|v| v.parse().ok()).unwrap_or(0);

    let base_to_quote = j["token_a_in"]
        .as_str()
        .and_then(|v| v.parse::<u64>().ok())
        .map(|v| v > 0)
        .unwrap_or(false);

    base(DexId::Cetus, pool, ev, amount_in, amount_out, base_to_quote)
}

fn parse_turbos(j: &Value, ev: &SuiEvent) -> Result<SwapDelta> {
    let pool = j["pool_id"].as_str().ok_or(parse("pool_id"))?;
    base(
        DexId::Turbos,
        pool,
        ev,
        j["amount_in"].as_u64().unwrap_or(0),
        j["amount_out"].as_u64().unwrap_or(0),
        j["a_to_b"].as_bool().unwrap_or(false),
    )
}

fn parse(field: &str) -> crate::types::BotError {
    crate::types::BotError::Parse(format!("missing {}", field))
}