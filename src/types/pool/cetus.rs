use sui_sdk::rpc_types::SuiObjectData;
use crate::types::{BotError, ChainAddress, DexId, Result, SuiAddress, Timestamp, TokenInfo, extractor::FieldExtractor, now, pool_parser::PoolParser, pool_state::{PoolId, PoolState}};

pub struct CetusPoolParser;

impl CetusPoolParser {
    const POOL_TYPE_IDENTIFIER: &'static str = "clmm::pool::Pool";
    
    pub fn new() -> Self {
        Self
    }
}

impl PoolParser for CetusPoolParser {
    fn dex_id(&self) -> DexId {
        DexId::Cetus
    }
    
    fn can_parse(&self, sui_object: &SuiObjectData) -> bool {
        FieldExtractor::has_type_suffix(sui_object, Self::POOL_TYPE_IDENTIFIER)
    }
    
    fn parse(&self, sui_object: &SuiObjectData) -> Result<PoolState> {
        let extractor = FieldExtractor::new(sui_object)?;
        
        // Extract pool data - adjust field names based on actual Cetus structure
        let pool_id = PoolId::Sui(SuiAddress::new(sui_object.object_id));
        let reserve_a = extractor.get_decimal_from_u128("coin_a")?;
        let reserve_b = extractor.get_decimal_from_u128("coin_b")?;
        let liquidity = extractor.get_decimal_from_u128("liquidity")?;
        let fee_rate = extractor.get_decimal_from_u64("fee_rate")?;
        
        // Extract token info from type parameters
        let type_str = sui_object.type_.as_ref()
            .ok_or_else(|| BotError::Parse("Missing type".to_string()))?
            .to_string();
        
        let (token_a, token_b) = Self::extract_token_types(&type_str)?;
        
        Ok(PoolState {
            dex_id: DexId::Cetus,
            pool_id,
            token_a,
            token_b,
            reserve_a,
            reserve_b,
            liquidity,
            fee_rate,
            block_timestamp: now(),
        })
    }
}

impl CetusPoolParser {
    fn extract_token_types(type_str: &str) -> Result<(TokenInfo, TokenInfo)> {
        let start = type_str.find('<')
            .ok_or_else(|| BotError::Parse("No type parameters found".to_string()))?;
        let end = type_str.rfind('>')
            .ok_or_else(|| BotError::Parse("Unclosed type parameters".to_string()))?;
        
        let params = &type_str[start + 1..end];
        
        let tokens: Vec<&str> = params.split(',').map(|s| s.trim()).collect();
        
        if tokens.len() != 2 {
            return Err(BotError::Parse(
                format!("Expected 2 type parameters, found {}", tokens.len())
            ));
        }
        
        let token_a = Self::parse_token_info(tokens[0])?;
        let token_b = Self::parse_token_info(tokens[1])?;
        
        Ok((token_a, token_b))
    }
    
    fn parse_token_info(token_type: &str) -> Result<TokenInfo> {
        // Expected format: "0xADDRESS::module::TokenName"
        let parts: Vec<&str> = token_type.split("::").collect();
        
        if parts.len() < 3 {
            return Err(BotError::Parse(
                format!("Invalid token type format: {}", token_type)
            ));
        }
        
        let address = parts[0].trim();
        let module = parts[1].trim();
        let name = parts[2].trim();
        
        if !address.starts_with("0x") {
            return Err(BotError::Parse(
                format!("Invalid address format: {}", address)
            ));
        }  
        Ok(TokenInfo {
            address: Some(address.to_string()),
            symbol: name.to_string(),
            name: Some(format!("{}::{}", module, name)),
            decimals: 9, // todo move to token metadata
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_cetus_pool_from_json() {
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
                    "coin_a": "5041265070",
                    "coin_b": "3143745307052",
                    "liquidity": "125087290394",
                    "fee_rate": "2500",
                    "is_pause": false
                }
            }
        });

        let sui_object: SuiObjectData = serde_json::from_value(json_data)
            .expect("Failed to deserialize SuiObjectData");

        let parser = CetusPoolParser::new();
        let result = parser.parse(&sui_object);

        assert!(result.is_ok(), "Parse failed: {:?}", result.err());
        
        let pool_state = result.unwrap();
        // Verify dex id
        assert_eq!(pool_state.dex_id, DexId::Cetus);
        // Verify reserves
        assert_eq!(pool_state.reserve_a.to_string(), "5041265070");
        assert_eq!(pool_state.reserve_b.to_string(), "3143745307052");
        // Verify liquidity
        assert_eq!(pool_state.liquidity.to_string(), "125087290394");
        // Verify fee rate
        assert_eq!(pool_state.fee_rate.to_string(), "2500");
        // Verify token addresses are extracted correctly
        assert!(pool_state.token_a.address.unwrap().contains("usdc"));
        assert!(pool_state.token_b.address.unwrap().contains("hasui"));
    }

    #[test]
    fn test_extract_token_types() {
        let type_str = "0x1eabed72c53feb3805120a081dc15963c204dc8d091542592abaf7a35689b2fb::pool::Pool<0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7::usdc::USDC, 0xbde4ba4c2e274a60ce15c1cfff9e5c42e41654ac8b6d906a57efa4bd3c29f47d::hasui::HASUI>";
        
        let result = CetusPoolParser::extract_token_types(type_str);
        assert!(result.is_ok(), "Failed to extract token types: {:?}", result.err());
        
        let (token_a, token_b) = result.unwrap();
        
        assert_eq!(token_a.symbol, "USDC");
        assert_eq!(token_b.symbol, "HASUI");
        assert_eq!(token_a.name, Some("usdc::USDC".to_string()));
        assert_eq!(token_b.name, Some("hasui::HASUI".to_string()));
    }
    
    #[test]
    fn test_parse_token_info() {
        let token_type = "0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7::usdc::USDC";
        let result = CetusPoolParser::parse_token_info(token_type);
        
        assert!(result.is_ok());
        let token = result.unwrap();
        assert_eq!(token.symbol, "USDC");
        assert_eq!(token.name, Some("usdc::USDC".to_string()));
    }
    
    #[test]
    fn test_parse_token_info_invalid() {
        let invalid_types = vec![
            "invalid",
            "0x123::only_two",
            "no_address::module::name",
        ];
        
        for invalid in invalid_types {
            let result = CetusPoolParser::parse_token_info(invalid);
            assert!(result.is_err(), "Should fail for: {}", invalid);
        }
    }
    
    #[test]
    fn test_extract_token_types_missing_brackets() {
        let type_str = "Pool without generics";
        let result = CetusPoolParser::extract_token_types(type_str);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_extract_token_types_wrong_param_count() {
        let type_str = "Pool<0x123::token::A>";
        let result = CetusPoolParser::extract_token_types(type_str);
        assert!(result.is_err());
    }
}