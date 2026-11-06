use anyhow::Error;
use axum::extract::ws::{Message, WebSocket};
use futures_util::stream::SplitSink;
use futures_util::SinkExt;
use serde::Serialize;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

/// Cloneable, thread-safe handle for pushing JSON messages to one websocket connection.
///
/// All writes are funneled through a single writer task, so request handlers and
/// background jobs (scans, flusher, waveform generation) can send concurrently
/// without owning the socket.
#[derive(Clone)]
pub struct MessageSender {
    tx: UnboundedSender<String>,
}

impl MessageSender {
    /// Spawn the writer task draining queued messages into the websocket sink.
    pub fn spawn(mut sink: SplitSink<WebSocket, Message>) -> MessageSender {
        let (tx, mut rx) = unbounded_channel::<String>();
        tokio::spawn(async move {
            while let Some(text) = rx.recv().await {
                if let Err(e) = sink.send(Message::from(text)).await {
                    warn!("Websocket writer failed, stopping: {e}");
                    break;
                }
            }
            // Writer gone: either the connection closed or sending failed.
            // Senders will get an error on their next send.
        });
        MessageSender { tx }
    }

    /// Serialize and queue a message. Fails when serialization fails or the connection is gone.
    pub fn send(&self, message: impl Serialize) -> Result<(), Error> {
        let text = serde_json::to_string(&message)?;
        self.tx.send(text).map_err(|_| anyhow!("Websocket connection closed"))?;
        Ok(())
    }

    /// Whether the connection (writer task) is still alive.
    pub fn is_alive(&self) -> bool {
        !self.tx.is_closed()
    }
}
