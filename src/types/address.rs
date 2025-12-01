use std::fmt;

use serde::{Deserialize, Serialize};
use sui_sdk::types::base_types::ObjectID;
use std::str::FromStr;

use crate::types::{BotError, Network};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ChainAddress {
    Sui(SuiAddress),
    Aptos(AptosAddress),
}

impl ChainAddress {
    pub fn network(&self) -> Network {
        match self {
            ChainAddress::Sui(_) => Network::SuiMainnet,
            ChainAddress::Aptos(_) => Network::AptosMainnet,
        }
    }

    pub fn as_sui_object_id(&self) -> Option<ObjectID> {
        match self {
            ChainAddress::Sui(sui_addr) => Some(sui_addr.inner()),
            ChainAddress::Aptos(_) => None,
        }
    }
    
    pub fn as_aptos_address(&self) -> Option<&str> {
        match self {
            ChainAddress::Aptos(aptos_addr) => Some(aptos_addr.inner()),
            ChainAddress::Sui(_) => None,
        }
    }
}

impl fmt::Display for ChainAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChainAddress::Sui(addr) => write!(f, "Sui:{}", addr),
            ChainAddress::Aptos(addr) => write!(f, "Aptos:{}", addr),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct SuiAddress(ObjectID);

impl SuiAddress {
    pub fn new(id: ObjectID) -> Self {
        Self(id)
    }

    pub fn from_str(hex_str: &str) -> Result<Self> {
        Self::new(ObjectID::from_hex_literal(hex_str).map_err(|e| BotError::Parse(format!("Could not parse {}"))));
    }

    pub fn random() -> Self {
        Self(ObjectID::random())
    }
    
    pub fn inner(&self) -> ObjectID {
        self.0
    }
}

impl FromStr for SuiAddress {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let id = ObjectID::from_hex_literal(s)
            .map_err(|e| anyhow::anyhow!("invalid Sui address '{}': {}", s, e))?;

        Ok(SuiAddress::new(id))
    }
}

impl fmt::Display for SuiAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<ObjectID> for SuiAddress {
    fn from(id: ObjectID) -> Self {
        Self::new(id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct AptosAddress(String); //TODO

impl AptosAddress {
    pub fn new(id: String) -> Self {
        Self(id)
    }

    pub fn inner(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AptosAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use crate::types::pool_state::PoolId;
    use super::*;
    use sui_sdk::types::base_types::ObjectID;

    fn create_test_sui_object_id() -> ObjectID {
        ObjectID::from_hex_literal("0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef").unwrap()
    }

    fn create_test_aptos_address() -> String {
        "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string()
    }

    #[test]
    fn test_sui_address_creation() {
        let object_id = create_test_sui_object_id();
        let sui_addr = SuiAddress::new(object_id);
        
        assert_eq!(sui_addr.inner(), object_id);
    }

    #[test]
    fn test_sui_address_from_object_id() {
        let object_id = create_test_sui_object_id();
        let sui_addr: SuiAddress = object_id.into();
        
        assert_eq!(sui_addr.inner(), object_id);
    }

    #[test]
    fn test_sui_address_display() {
        let object_id = create_test_sui_object_id();
        let sui_addr = SuiAddress::new(object_id);
        
        let display = format!("{}", sui_addr);
        assert_eq!(display, object_id.to_string());
    }

    #[test]
    fn test_chain_address_sui_creation() {
        let object_id = create_test_sui_object_id();
        let sui_addr = SuiAddress::new(object_id);
        let chain_addr = ChainAddress::Sui(sui_addr);
        
        assert_eq!(chain_addr.network(), Network::SuiMainnet);
    }

    #[test]
    fn test_chain_address_sui_object_id_conversion() {
        let object_id = create_test_sui_object_id();
        let sui_addr = SuiAddress::new(object_id);
        let chain_addr = ChainAddress::Sui(sui_addr);
        
        assert_eq!(chain_addr.as_sui_object_id(), Some(object_id));
        assert!(chain_addr.as_aptos_address().is_none());
    }

    #[test]
    fn test_chain_address_aptos_address_conversion() {
        let addr_str = create_test_aptos_address();
        let aptos_addr = AptosAddress::new(addr_str.clone());
        let chain_addr = ChainAddress::Aptos(aptos_addr);
        
        assert_eq!(chain_addr.as_aptos_address(), Some(addr_str.as_str()));
        assert!(chain_addr.as_sui_object_id().is_none());
    }

    #[test]
    fn test_chain_address_display_sui() {
        let object_id = create_test_sui_object_id();
        let sui_addr = SuiAddress::new(object_id);
        let chain_addr = ChainAddress::Sui(sui_addr);
        
        let display = format!("{}", chain_addr);
        assert!(display.starts_with("Sui:"));
        assert!(display.contains(&object_id.to_string()));
    }

    #[test]
    fn test_chain_address_equality() {
        let object_id = create_test_sui_object_id();
        let addr1 = ChainAddress::Sui(SuiAddress::new(object_id));
        let addr2 = ChainAddress::Sui(SuiAddress::new(object_id));
        
        assert_eq!(addr1, addr2);
    }

    #[test]
    fn test_chain_address_inequality_different_networks() {
        let object_id = create_test_sui_object_id();
        let addr_str = create_test_aptos_address();
        
        let sui_addr = ChainAddress::Sui(SuiAddress::new(object_id));
        let aptos_addr = ChainAddress::Aptos(AptosAddress::new(addr_str));
        
        assert_ne!(sui_addr, aptos_addr);
    }

    #[test]
    fn test_chain_address_inequality_same_network() {
        let object_id1 = create_test_sui_object_id();
        let object_id2 = ObjectID::random();
        
        let addr1 = ChainAddress::Sui(SuiAddress::new(object_id1));
        let addr2 = ChainAddress::Sui(SuiAddress::new(object_id2));
        
        assert_ne!(addr1, addr2);
    }

    #[test]
    fn test_chain_address_hash() {
        use std::collections::HashSet;
        
        let object_id = create_test_sui_object_id();
        let addr1 = ChainAddress::Sui(SuiAddress::new(object_id));
        let addr2 = ChainAddress::Sui(SuiAddress::new(object_id));
        
        let mut set = HashSet::new();
        set.insert(addr1.clone());
        set.insert(addr2.clone());
        
        assert_eq!(set.len(), 1);
        assert!(set.contains(&addr1));
    }

    #[test]
    fn test_chain_address_serialization_deserialization() {
        let object_id = create_test_sui_object_id();
        let original_addr = ChainAddress::Sui(SuiAddress::new(object_id));
        
        // Serialize
        let serialized = serde_json::to_string(&original_addr).unwrap();
        
        // Deserialize
        let deserialized: ChainAddress = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(original_addr, deserialized);
    }

    #[test]
    fn test_chain_address_aptos_serialization_deserialization() {
        let addr_str = create_test_aptos_address();
        let original_addr = ChainAddress::Aptos(AptosAddress::new(addr_str));
        
        // Serialize
        let serialized = serde_json::to_string(&original_addr).unwrap();
        
        // Deserialize
        let deserialized: ChainAddress = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(original_addr, deserialized);
    }

    #[test]
    fn test_pool_id_type_alias() {
        let object_id = create_test_sui_object_id();
        let chain_addr = ChainAddress::Sui(SuiAddress::new(object_id));
        let pool_id: PoolId = chain_addr.clone();
        
        assert_eq!(chain_addr, pool_id);
    }

    #[test]
    fn test_network_enum_matches_chain_address() {
        let sui_addr = ChainAddress::Sui(SuiAddress::new(create_test_sui_object_id()));
        let aptos_addr = ChainAddress::Aptos(AptosAddress::new("0x123".to_string()));
        
        assert_eq!(sui_addr.network(), Network::SuiMainnet);
        assert_eq!(aptos_addr.network(), Network::AptosMainnet);
    }
}