use sui_sdk::rpc_types::SuiObjectData;
use crate::types::{DexId, Result, extractor::FieldExtractor, pool_parser::PoolParser, pool_state::PoolState};

pub struct TurbosPoolParser;

impl TurbosPoolParser {
    const POOL_TYPE_IDENTIFIER: &'static str = "pool::Pool";
    
    pub fn new() -> Self {
        Self
    }
}

impl PoolParser for TurbosPoolParser {
    fn dex_id(&self) -> DexId {
        DexId::Turbos
    }
    
    fn can_parse(&self, sui_object: &SuiObjectData) -> bool {
        FieldExtractor::has_type_suffix(sui_object, Self::POOL_TYPE_IDENTIFIER)
    }
    
    fn parse(&self, sui_object: &SuiObjectData) -> Result<PoolState> {
        let extractor = FieldExtractor::new(sui_object)?;
        
        // TODO
        
        todo!("Implement Turbos pool parsing")
    }
}