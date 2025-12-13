use crate::types::{DexId, Network, RawEvent};
use tokio::sync::mpsc;

pub struct WsContext {
    pub dex_id: DexId,
    pub network: Network,
    pub tx: mpsc::Sender<(DexId, RawEvent)>,
}