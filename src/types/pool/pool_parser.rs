use sui_sdk::rpc_types::SuiObjectData;
use crate::types::{DexId, pool_state::PoolState, Result, BotError};

/// Trait for parsing DEX-specific pool data
pub trait PoolParser: Send + Sync {
    /// Parse a SuiObjectData into a PoolState
    fn parse(&self, sui_object: &SuiObjectData) -> Result<PoolState>;
    
    /// Validate if this parser can handle the given object type
    fn can_parse(&self, sui_object: &SuiObjectData) -> bool;
    
    /// Get the DEX ID this parser handles
    fn dex_id(&self) -> DexId;
}

pub struct PoolParserRegistry {
    parsers: Vec<Box<dyn PoolParser>>,
}

impl PoolParserRegistry {
    pub fn new() -> Self {
        Self {
            parsers: Vec::new(),
        }
    }
    
    pub fn register<P: PoolParser + 'static>(mut self, parser: P) -> Self {
        self.parsers.push(Box::new(parser));
        self
    }
    
    /// Parse a SuiObjectData using the first compatible parser
    pub fn parse(&self, sui_object: &SuiObjectData, dex_id: &DexId) -> Result<PoolState> {
        // First try to find parser by DEX ID
        for parser in &self.parsers {
            if parser.dex_id() == *dex_id && parser.can_parse(sui_object) {
                return parser.parse(sui_object);
            }
        }
        
        Err(BotError::Parse(format!("No parser found for DEX {} and object type {:?}", dex_id, sui_object.type_)))
    }
    
    /// Parse multiple objects, filtering out failures
    pub fn parse_batch(&self, objects: Vec<SuiObjectData>, dex_id: &DexId) -> Vec<PoolState> {
        objects
            .iter()
            .filter_map(|obj| {
                match self.parse(obj, dex_id) {
                    Ok(pool) => Some(pool),
                    Err(e) => {
                        tracing::warn!(
                            "Failed to parse pool {}: {}",
                            obj.object_id,
                            e
                        );
                        None
                    }
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    // Mock parser for testing
    struct MockParserA;
    
    impl PoolParser for MockParserA {
        fn parse(&self, sui_object: &SuiObjectData) -> Result<PoolState> {
            use crate::types::{ChainAddress, SuiAddress, TokenInfo, now};
            use crate::types::pool_state::PoolId;
            use rust_decimal::Decimal;
            
            Ok(PoolState {
                dex_id: DexId::Cetus,
                pool_id: PoolId::Sui(SuiAddress::new(sui_object.object_id)),
                token_a: TokenInfo {
                    address: Some("0xaaa".to_string()),
                    symbol: "TOKA".to_string(),
                    name: Some("Token A".to_string()),
                    decimals: 9,
                },
                token_b: TokenInfo {
                    address: Some("0xbbb".to_string()),
                    symbol: "TOKB".to_string(),
                    name: Some("Token B".to_string()),
                    decimals: 9,
                },
                reserve_a: Decimal::new(1000, 0),
                reserve_b: Decimal::new(2000, 0),
                liquidity: Decimal::new(1500, 0),
                fee_rate: Decimal::new(25, 2),
                block_timestamp: now(),
            })
        }
        
        fn can_parse(&self, sui_object: &SuiObjectData) -> bool {
            sui_object.type_.as_ref()
                .map(|t| t.to_string().contains("::pool::Pool"))
                .unwrap_or(false)
        }
        
        fn dex_id(&self) -> DexId {
            DexId::Cetus
        }
    }
    
    struct MockParserB;
    
    impl PoolParser for MockParserB {
        fn parse(&self, _sui_object: &SuiObjectData) -> Result<PoolState> {
            Err(BotError::Parse("Mock parser B always fails".to_string()))
        }
        
        fn can_parse(&self, sui_object: &SuiObjectData) -> bool {
            sui_object.type_.as_ref()
                .map(|t| t.to_string().contains("::other::Pool"))
                .unwrap_or(false)
        }
        
        fn dex_id(&self) -> DexId {
            DexId::Turbos
        }
    }
    
    fn create_cetus_pool_json() -> serde_json::Value {
        json!({
            "objectId": "0x7d44018fbc32f456b6d0122206041a2cc159bdde32911b4be94a4e5840890764",
            "version": "699348628",
            "digest": "2YTH8GyiYhEiMJ2FQXKiDkL8yxN2cuZAX8sMMeC7tEJN",
            "type": "0x1eabed72c53feb3805120a081dc15963c204dc8d091542592abaf7a35689b2fb::pool::Pool<0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7::usdc::USDC, 0xbde4ba4c2e274a60ce15c1cfff9e5c42e41654ac8b6d906a57efa4bd3c29f47d::hasui::HASUI>",
            "content": {
                "dataType": "moveObject",
                "type": "0x1eabed72c53feb3805120a081dc15963c204dc8d091542592abaf7a35689b2fb::pool::Pool<0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7::usdc::USDC, 0xbde4ba4c2e274a60ce15c1cfff9e5c42e41654ac8b6d906a57efa4bd3c29f47d::hasui::HASUI>",
                "hasPublicTransfer": true,
                "fields": {
                    "coin_a": "5041265070",
                    "coin_b": "3143745307052",
                    "liquidity": "125087290394",
                    "fee_rate": "2500",
                    "is_pause": false
                }
            }
        })
    }
    
    fn create_turbos_pool_json() -> serde_json::Value {
        json!({
            "objectId": "0x7d44018fbc32f456b6d0122206041a2cc159bdde32911b4be94a4e5840890765",
            "version": "699348629",
            "digest": "2YTH8GyiYhEiMJ2FQXKiDkL8yxN2cuZAX8sMMeC7tEJF",
            "type": "0x1eabed72c53feb3805120a081dc15963c204dc8d091542592abaf7a35689b2fa::pool::Pool<0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e8::usdc::USDC, 0xbde4ba4c2e274a60ce15c1cfff9e5c42e41654ac8b6d906a57efa4bd3c29f47a::hasui::HASUI>",
            "content": {
                "dataType": "moveObject",
                "type": "0x1eabed72c53feb3805120a081dc15963c204dc8d091542592abaf7a35689b2fa::pool::Pool<0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e8::usdc::USDC, 0xbde4ba4c2e274a60ce15c1cfff9e5c42e41654ac8b6d906a57efa4bd3c29f47a::hasui::HASUI>",
                "hasPublicTransfer": true,
                "fields": {
                    "coin_a": "5041265070",
                    "coin_b": "3143745307052",
                    "liquidity": "125087290394",
                    "fee_rate": "2500",
                    "is_pause": false
                }
            }
        })
    }
    
    #[test]
    fn test_registry_new() {
        let registry = PoolParserRegistry::new();
        assert_eq!(registry.parsers.len(), 0);
    }
    
    #[test]
    fn test_registry_register_single_parser() {
        let registry = PoolParserRegistry::new()
            .register(MockParserA);
        
        assert_eq!(registry.parsers.len(), 1);
    }
    
    #[test]
    fn test_registry_register_multiple_parsers() {
        let registry = PoolParserRegistry::new()
            .register(MockParserA)
            .register(MockParserB);
        
        assert_eq!(registry.parsers.len(), 2);
    }
    
    #[test]
    fn test_parse_with_matching_parser() {
        let registry = PoolParserRegistry::new()
            .register(MockParserA);
        
        let json_data = create_cetus_pool_json();
        let sui_object: SuiObjectData = serde_json::from_value(json_data)
            .expect("Failed to deserialize SuiObjectData");
        
        let result = registry.parse(&sui_object, &DexId::Cetus);
        
        assert!(result.is_ok(), "Parse should succeed: {:?}", result.err());
        let pool_state = result.unwrap();
        assert_eq!(pool_state.dex_id, DexId::Cetus);
        assert_eq!(pool_state.token_a.symbol, "TOKA");
        assert_eq!(pool_state.token_b.symbol, "TOKB");
    }
    
    #[test]
    fn test_parse_with_wrong_dex_id() {
        let registry = PoolParserRegistry::new()
            .register(MockParserA);
        
        let json_data = create_cetus_pool_json();
        let sui_object: SuiObjectData = serde_json::from_value(json_data)
            .expect("Failed to deserialize SuiObjectData");
        
        let result = registry.parse(&sui_object, &DexId::Turbos);
        
        assert!(result.is_err());
        if let Err(BotError::Parse(msg)) = result {
            assert!(msg.contains("No parser found"));
        }
    }
    
    #[test]
    fn test_parse_batch_all_valid() {
        let registry = PoolParserRegistry::new()
            .register(MockParserA);
        
        let json_data1 = create_cetus_pool_json();
        let json_data2 = create_cetus_pool_json();
        
        let sui_object1: SuiObjectData = serde_json::from_value(json_data1)
            .expect("Failed to deserialize SuiObjectData");
        let sui_object2: SuiObjectData = serde_json::from_value(json_data2)
            .expect("Failed to deserialize SuiObjectData");
        
        let objects = vec![sui_object1, sui_object2];
        let results = registry.parse_batch(objects, &DexId::Cetus);
        
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].dex_id, DexId::Cetus);
        assert_eq!(results[1].dex_id, DexId::Cetus);
    }
    
    #[test]
    fn test_parse_batch_empty() {
        let registry = PoolParserRegistry::new()
            .register(MockParserA);
        
        let objects = vec![];
        let results = registry.parse_batch(objects, &DexId::Cetus);
        
        assert_eq!(results.len(), 0);
    }
    
    #[test]
    fn test_multiple_parsers_correct_selection() {
        let registry = PoolParserRegistry::new()
            .register(MockParserA)
            .register(MockParserB);
        
        // Test Cetus pool
        let cetus_json = create_cetus_pool_json();
        let cetus_object: SuiObjectData = serde_json::from_value(cetus_json)
            .expect("Failed to deserialize SuiObjectData");
        
        let result = registry.parse(&cetus_object, &DexId::Cetus);
        assert!(result.is_ok());
        
        let turbos_json = create_turbos_pool_json();
        let turbos_object: SuiObjectData = serde_json::from_value(turbos_json)
            .expect("Failed to deserialize SuiObjectData");
        
        let result = registry.parse(&turbos_object, &DexId::Turbos);
        assert!(result.is_err());
    }
}