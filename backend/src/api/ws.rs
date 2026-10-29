//! WebSocket live-update endpoint.
//!
//! Protocol: one JSON object per line, envelope: `{"kind": "...", "data": {...}}`
//! Mirroring `events::WsEvent`. Bounded broadcast buffer (1024); a client that
//! falls behind receives one `{"kind":"lagged"}` message and is disconnected
//! (the server never buffers unboundedly per client).

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use tokio::sync::broadcast::error::RecvError;

use crate::app::AppState;
use crate::events::WsEvent;

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(st): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, st))
}

async fn handle_socket(mut socket: WebSocket, st: AppState) {
    let mut rx = st.bus.subscribe();
    // First message: hello with subscription confirmation.
    let hello = serde_json::json!({
        "kind": "hello",
        "data": {
            "version": st.version(),
            "mode": st.cfg.application.mode.to_string(),
            "subscribed": ["market", "equity", "signal", "order", "fill", "risk", "strategy", "health", "backtest"],
        }
    });
    if socket.send(Message::Text(hello.to_string().into())).await.is_err() {
        return;
    }

    loop {
        tokio::select! {
            incoming = socket.next() => {
                match incoming {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {} // ignore client pings/pings handled below
                    Some(Err(_)) => break,
                }
            }
            ev = rx.recv() => {
                match ev {
                    Ok(ev) => {
                        let text = serde_json::to_string(&ev).unwrap_or_else(|_| "{}".into());
                        if socket.send(Message::Text(text.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(RecvError::Lagged(_)) => {
                        // Client too slow: tell it, then drop it (documented).
                        let _ = socket
                            .send(Message::Text(
                                serde_json::json!({"kind":"lagged"}).to_string().into(),
                            ))
                            .await;
                        break;
                    }
                    Err(RecvError::Closed) => break,
                }
            }
        }
    }
    // Drain the socket politely.
    let _ = socket.close().await;
}

// Keep WsEvent reachable from this module for documentation accuracy.
#[allow(dead_code)]
fn _event_types(_: WsEvent) {}