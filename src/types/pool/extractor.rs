use sui_sdk::rpc_types::{SuiMoveStruct, SuiMoveValue, SuiObjectData, SuiParsedData};
use std::collections::BTreeMap;
use rust_decimal::{Decimal, prelude::FromPrimitive};
use crate::types::{Result, BotError};

/// Minimal, strict field extractor for fast pool parsing
pub struct FieldExtractor<'a> {
    fields: &'a BTreeMap<String, SuiMoveValue>,
    object_id: String,
}

impl<'a> FieldExtractor<'a> {
    pub fn new(sui_object: &'a SuiObjectData) -> Result<Self> {
        let fields = Self::extract_fields(sui_object)?;
        Ok(Self {
            fields,
            object_id: sui_object.object_id.to_string(),
        })
    }

    fn extract_fields(
        sui_object: &'a SuiObjectData,
    ) -> Result<&'a BTreeMap<String, SuiMoveValue>> {
        let content = sui_object.content.as_ref()
            .ok_or_else(|| BotError::Parse("Missing content".to_string()))?;

        let move_obj = match content {
            SuiParsedData::MoveObject(obj) => obj,
            _ => return Err(BotError::Parse("Not a Move object".to_string())),
        };

        match &move_obj.fields {
            SuiMoveStruct::WithFields(fields)
            | SuiMoveStruct::WithTypes { fields, .. } => Ok(fields),
            _ => Err(BotError::Parse("Unexpected struct format".to_string())),
        }
    }

    /// Strict u64 extractor
    pub fn get_u64(&self, field: &str) -> Result<u64> {
        let v = self.fields.get(field).ok_or_else(|| {
            BotError::Parse(format!("Missing u64 field '{}' in {}", field, self.object_id))
        })?;

        match v {
            SuiMoveValue::Number(n) => {
                u64::try_from(*n).map_err(|_| BotError::Parse(format!(
                    "Overflow converting '{}' to u64 in {}",
                    field, self.object_id
                )))
            }
            SuiMoveValue::String(s) => s.parse::<u64>().map_err(|_| {
                BotError::Parse(format!(
                    "Invalid string '{}' for u64 field '{}' in {}",
                    s, field, self.object_id
                ))
            }),
            other => Err(BotError::Parse(format!(
                "Invalid type {:?} for u64 field '{}' in {}",
                other, field, self.object_id
            ))),
        }
    }

    /// Strict u128 extractor
    pub fn get_u128(&self, field: &str) -> Result<u128> {
        let v = self.fields.get(field).ok_or_else(|| {
            BotError::Parse(format!("Missing u128 field '{}' in {}", field, self.object_id))
        })?;

        match v {
            SuiMoveValue::Number(n) => {
                u128::try_from(*n).map_err(|_| BotError::Parse(format!(
                    "Overflow converting '{}' to u128 in {}",
                    field, self.object_id
                )))
            }
            SuiMoveValue::String(s) => s.parse::<u128>().map_err(|_| {
                BotError::Parse(format!(
                    "Invalid string '{}' for u128 field '{}' in {}",
                    s, field, self.object_id
                ))
            }),
            other => Err(BotError::Parse(format!(
                "Invalid type {:?} for u128 field '{}' in {}",
                other, field, self.object_id
            ))),
        }
    }

    pub fn get_decimal_from_u64(&self, field: &str) -> Result<Decimal> {
        Ok(Decimal::from(self.get_u64(field)?))
    }

    pub fn get_decimal_from_u128(&self, field: &str) -> Result<Decimal> {
        let val = self.get_u128(field)?;
        Decimal::from_u128(val).ok_or_else(|| {
            BotError::Parse(format!("Failed Decimal conversion for '{}'", field))
        })
    }

    pub fn has_type_suffix(sui_object: &SuiObjectData, suffix: &str) -> bool {
        sui_object.type_
            .as_ref()
            .map(|t| t.to_string().contains(suffix))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use sui_sdk::rpc_types::{SuiMoveStruct, SuiMoveValue, SuiObjectData, SuiParsedData};
    use std::collections::BTreeMap;

    fn mock_object() -> SuiObjectData {
        let json_data = json!({
            "objectId": "0x7d44018fbc32f456b6d0122206041a2cc159bdde32911b4be94a4e5840890764",
            "version": "699348628",
            "digest": "2YTH8GyiYhEiMJ2FQXKiDkL8yxN2cuZAX8sMMeC7tEJN",
            "type": "0x1eabed72c53feb3805120a081dc15963c204dc8d091542592abaf7a35689b2fb::pool::Pool<0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7::usdc::USDC, 0xbde4ba4c2e274a60ce15c1cfff9e5c42e41654ac8b6d906a57efa4bd3c29f47d::hasui::HASUI>",
            "content": {
                "dataType": "moveObject",
                "type": "0x1eabed72c53feb3805120a081dc15963c204dc8d091542592abaf7a35689b2fb::pool::Pool<0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7::usdc::USDC, 0xbde4ba4c2e274a60ce15c1cfff9e5c42e41654ac8b6d906a57efa4bd3c29f47d::hasui::HASUI>",
                "hasPublicTransfer": true,
                "fields": {
                    "coin_a": "42",
                    "coin_b": "3143745307052",
                    "liquidity": "125087290394",
                    "u64_overflow": "18446744073709551625", // 18446744073709551615 + 10
                    "fee_rate": "2500",
                    "is_pause": false
                }
            }
        });

        serde_json::from_value(json_data).expect("Failed to deserialize SuiObjectData")
    }

    #[test]
    fn test_get_u64_success() {
        let obj = mock_object();
        let fx = FieldExtractor::new(&obj).unwrap();

        assert_eq!(fx.get_u64("coin_a").unwrap(), 42);
    }

    #[test]
    fn test_get_u128_success() {
        let obj = mock_object();
        let fx = FieldExtractor::new(&obj).unwrap();

        assert_eq!(fx.get_u128("liquidity").unwrap(), 125087290394);
    }

    #[test]
    fn test_string_numeric_field() {
        let obj = mock_object();
        let fx = FieldExtractor::new(&obj).unwrap();

        assert_eq!(fx.get_u64("coin_a").unwrap(), 42);
    }

    #[test]
    fn test_missing_field() {
        let obj = mock_object();
        let fx = FieldExtractor::new(&obj).unwrap();

        assert!(fx.get_u64("missing").is_err());
    }

    #[test]
    fn test_invalid_type() {
        let obj = mock_object();
        let fx = FieldExtractor::new(&obj).unwrap();

        assert!(fx.get_u64("is_pause").is_err());
    }

    #[test]
    fn test_overflow_u64() {
        let obj = mock_object();
        let fx = FieldExtractor::new(&obj).unwrap();

        assert!(fx.get_u64("u64_overflow").is_err());
    }

    #[test]
    fn test_decimal_from_u128() {
        let obj = mock_object();
        let fx = FieldExtractor::new(&obj).unwrap();

        assert_eq!(fx.get_decimal_from_u128("liquidity").unwrap().to_string(), "125087290394");
    }
}
