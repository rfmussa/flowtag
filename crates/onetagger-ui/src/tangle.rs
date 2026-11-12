//! WebSocket glue for the Tangle sync worker. The token never appears in replies or logs.

use std::path::PathBuf;
use std::sync::mpsc::Sender;
use anyhow::Error;
use onetagger_shared::Settings;
use onetagger_tangle::client::{Client, DEFAULT_BASE};
use onetagger_tangle::error::TangleError;
use onetagger_tangle::token::{KeychainToken, TokenStore};
use onetagger_tangle::worker::{self, Command};
use serde_json::json;

use crate::sender::MessageSender;
use crate::socket::SocketContext;

/// Start the worker; a Tangle problem must never stop FlowTag from starting.
pub(crate) fn spawn_worker(
    db_path: PathBuf,
    events: tokio::sync::broadcast::Sender<String>,
) -> Option<Sender<Command>> {
    let enabled = Settings::load()
        .map(|s| s.ui["flowtag"]["tangleSync"].as_bool().unwrap_or(false))
        .unwrap_or(false);
    let spawned = worker::spawn(
        db_path,
        DEFAULT_BASE.to_string(),
        Box::new(KeychainToken),
        enabled,
        move |value| {
            events.send(value.to_string()).ok();
        },
    );
    match spawned {
        Ok(tx) => Some(tx),
        Err(e) => {
            error!("Failed starting Tangle worker, continuing without Tangle: {e}");
            None
        }
    }
}

/// Short stable code per error variant for the UI.
pub(crate) fn error_code(e: &TangleError) -> String {
    match e {
        TangleError::InvalidToken => "invalid_token".to_string(),
        TangleError::BackupRequired => "backup_and_restore_required".to_string(),
        TangleError::RateLimited { .. } => "rate_limited".to_string(),
        TangleError::Network(_) => "network".to_string(),
        TangleError::Server(_) => "server".to_string(),
        TangleError::ListNotFound => "list_not_found".to_string(),
        TangleError::ItemNotFound => "item_not_found".to_string(),
        TangleError::Other { code, .. } => code.clone(),
    }
}

fn send_error(sender: &MessageSender, code: &str) -> Result<(), Error> {
    sender.send(json!({ "action": "tangleError", "code": code }))
}

/// Forward a command to the worker; replies `unavailable` when there is none.
pub(crate) fn command(context: &SocketContext, cmd: Command) -> Result<(), Error> {
    forward(&context.sender, context.tangle.as_ref(), cmd)
}

fn forward(sender: &MessageSender, tangle: Option<&Sender<Command>>, cmd: Command) -> Result<(), Error> {
    match tangle {
        Some(tx) if tx.send(cmd).is_ok() => Ok(()),
        _ => send_error(sender, "unavailable"),
    }
}

// The async handlers take owned clones: SocketContext is not Sync, so it cannot be held across an await.

/// Verify the token with `/me`, then store it and wake the worker.
pub(crate) async fn connect(
    sender: MessageSender,
    tangle: Option<Sender<Command>>,
    token: String,
) -> Result<(), Error> {
    if tangle.is_none() {
        return send_error(&sender, "unavailable");
    }
    let token = token.trim().to_string();
    if token.is_empty() {
        return send_error(&sender, "empty_token");
    }
    // reqwest::blocking and Keychain access must stay off the async runtime
    let result = tokio::task::spawn_blocking(move || -> Result<String, String> {
        let me = Client::new(DEFAULT_BASE, &token).me().map_err(|e| error_code(&e))?;
        KeychainToken.set(&token).map_err(|e| {
            warn!("Storing Tangle token in Keychain failed: {e}");
            "keychain".to_string()
        })?;
        Ok(me.user_name)
    })
    .await;
    match result {
        Ok(Ok(user)) => {
            forward(&sender, tangle.as_ref(), Command::TokenChanged)?;
            sender.send(json!({ "action": "tangleConnected", "user": user }))
        }
        Ok(Err(code)) => send_error(&sender, &code),
        Err(e) => {
            warn!("Tangle connect task failed: {e}");
            send_error(&sender, "internal")
        }
    }
}

/// Remove the stored token and wake the worker.
pub(crate) async fn disconnect(sender: MessageSender, tangle: Option<Sender<Command>>) -> Result<(), Error> {
    if tangle.is_none() {
        return send_error(&sender, "unavailable");
    }
    match tokio::task::spawn_blocking(|| KeychainToken.delete()).await {
        Ok(Ok(())) => forward(&sender, tangle.as_ref(), Command::TokenChanged),
        Ok(Err(e)) => {
            warn!("Deleting Tangle token from Keychain failed: {e}");
            send_error(&sender, "keychain")
        }
        Err(e) => {
            warn!("Tangle disconnect task failed: {e}");
            send_error(&sender, "internal")
        }
    }
}
