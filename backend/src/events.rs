//! Application event bus: in-process broadcast for WebSocket delivery and
//! internal notifications. Bounded channel; slow consumers receive `Lagged`
//! and are dropped by the WS layer (documented behavior — the server never
//! buffers unboundedly for a single client).

use serde::Serialize;
use tokio::sync::broadcast;

use crate::domain::*;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum WsEvent {
    /// A new demo/provider market observation batch arrived.
    MarketBatch {
        count: usize,
        simulated: bool,
        observed_at: i64,
    },
    /// Latest snapshot for one mint (sent on watchlist updates and ingestion).
    Snapshot(MarketSnapshot),
    Equity(EquitySnapshot),
    Signal(SignalRecord),
    Order(PaperOrder),
    Fill(Fill),
    Strategy { id: String, status: String },
    Risk(RiskEvent),
    Health { provider: String, healthy: bool, provider_up: bool, stale: bool },
    Backtest { id: String, status: String },
}

#[derive(Debug, Clone)]
pub struct EventBus {
    tx: broadcast::Sender<WsEvent>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(1024)
    }
}

impl EventBus {
    pub fn new(capacity: usize) -> EventBus {
        let (tx, _) = broadcast::channel(capacity.max(16));
        EventBus { tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<WsEvent> {
        self.tx.subscribe()
    }

    /// Publish; errors are ignored (no subscribers / channel closed).
    pub fn emit(&self, ev: WsEvent) {
        let _ = self.tx.send(ev);
    }
}