#[macro_use] extern crate log;
#[macro_use] extern crate anyhow;
#[macro_use] extern crate include_dir;
#[macro_use] extern crate onetagger_shared;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use anyhow::Error;
use axum::body::Body;
use axum::extract::{Query, WebSocketUpgrade, State, Request};
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::response::IntoResponse;
use axum::Router;
use axum::routing::get;
use include_dir::Dir;
use onetagger_player::AudioSources;
use serde::{Serialize, Deserialize};
use std::io::Cursor;
use std::path::Path;
use image::{ImageFormat, ImageReader};
use onetagger_tag::Tag;
use tokio::runtime::Builder;
use tokio::net::TcpListener;
use onetagger_shared::{PORT, WEBSERVER_CALLBACKS, Settings};

pub mod analysis;
pub mod media;
pub mod socket;
pub mod sender;
pub mod browser;
pub mod tageditor;
pub mod stage;
pub mod tagflow;
pub mod tangle;
pub mod precompute;
pub(crate) mod preload;

static CLIENT_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../client/dist");

// Should have data from arguments and other flags (eg. port / host in future)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartContext {
    pub server_mode: bool,
    pub start_path: Option<String>,
    pub expose: bool,
    pub browser: bool,
}

/// Shared axum state for the websocket route
#[derive(Clone)]
struct WsState {
    context: StartContext,
    db: Arc<onetagger_db::Db>,
    /// Flush/scan events broadcast to every connection
    events: tokio::sync::broadcast::Sender<String>,
    /// Wakes the flush worker (e.g. after a flushHint)
    flush_wake: std::sync::mpsc::Sender<()>,
    /// Preloaded source for the selected track
    preload: preload::PreloadSlot,
    /// Tangle sync worker; `None` if it could not start
    tangle: Option<std::sync::mpsc::Sender<onetagger_tangle::worker::Command>>,
}

fn start_async_runtime(context: StartContext) -> Result<(), Error> {
    let expose = context.expose;
    // Library database is opened once and shared by all connections
    let db_path = Settings::get_folder()?.join("onetagger.db");
    let db = onetagger_db::Db::open(&db_path).map_err(|e| {
        error!("Failed opening library database at {:?}: {e}", db_path);
        e
    })?;

    // Background flusher: writes DB tag changes into the audio files.
    // Events go to every connection; the currently playing file is skipped.
    let (events, _) = tokio::sync::broadcast::channel::<String>(256);
    // OS media keys (play/pause/next from the GUI shell) push into the same channel
    media::set_media_broadcast(events.clone());
    let events_tx = events.clone();
    let preload = preload::PreloadSlot::new();
    let preload_for_flush = preload.clone();
    let flush_wake = onetagger_db::flush::FlushWorker::spawn(
        db_path.clone(),
        move |event| {
            // A flush rewrites the file: any preloaded source may be stale
            preload_for_flush.clear();
            let message = serde_json::json!({
                "action": "trackFlushed",
                "trackId": event.track_id,
                "ok": event.ok,
                "error": event.error,
            });
            events_tx.send(message.to_string()).ok();
        },
        |path| socket::is_currently_playing(path),
    )?;

    let tangle = tangle::spawn_worker(db_path, events.clone());

    let db = Arc::new(db);
    precompute::spawn_waveform_precompute(db.clone());
    let state = WsState { context, db, events, flush_wake, preload, tangle };
    Builder::new_multi_thread().enable_all().build()?.block_on(async move {
        // Register routes
        let app = Router::new()
            .route("/thumb", get(get_thumb))
            .route("/audio", get(get_audio))
            .route("/ws", get(get_ws))
            .route("/spotify", get(get_spotify_callback))
            .route("/{*path}", get(get_static_file))
            .route("/", get(get_static_file))
            .with_state(state);

        // Start http server
        let host = match expose {
            true => format!("0.0.0.0:{PORT}"),
            false => format!("127.0.0.1:{PORT}")
        };
        info!("Starting web server on: http://{host}");
        let listener = TcpListener::bind(host).await?;
        axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await?;

        Ok::<(), Error>(())
    })?;
    Ok(())
}

/// Serve assets file
async fn get_static_file(request: Request<Body>) -> impl IntoResponse {
    let mut path = request.uri().to_string();
    // Index HTML
    if path == "/" {
        path = "/index.html".to_string();
    }
    path = path[1..].to_string();

    // Static files
    if let Some(file) = CLIENT_DIR.get_file(&path) {
        let mime = mime_guess::from_path(&path).first().unwrap_or(mime::APPLICATION_OCTET_STREAM);
        return (StatusCode::OK, [(CONTENT_TYPE, mime.to_string())], file.contents().to_vec());
    }

    (StatusCode::NOT_FOUND, [(CONTENT_TYPE, "text/plain".to_string())], "Not found".as_bytes().to_vec())
}

#[derive(Debug, Clone, Deserialize)]
struct GetQueryPath {
    path: String
}

/// Load album art from tag and downscale to a small JPEG thumbnail
fn load_thumbnail(path: impl AsRef<Path>) -> Result<Vec<u8>, Error> {
    let tag_wrap = Tag::load_file(path, false)?;
    let tag = tag_wrap.tag();
    let pictures = tag.get_art();
    let picture = pictures.first().ok_or(anyhow!("Missing album art!"))?;
    let img = ImageReader::new(Cursor::new(&picture.data)).with_guessed_format()?.decode()?;
    let scaled = img.thumbnail_exact(50, 50);
    let mut out = vec![];
    scaled.write_to(&mut Cursor::new(&mut out), ImageFormat::Jpeg)?;
    Ok(out)
}

/// Serve thumbnail
async fn get_thumb(Query(GetQueryPath { path }): Query<GetQueryPath>) -> impl IntoResponse {
    match load_thumbnail(&path) {
        Ok(art) => (StatusCode::OK, [(CONTENT_TYPE, "image/jpeg".to_string())], art),
        Err(e) => {
            warn!("Error loading album art: {} File: {}", e, path);
            (StatusCode::NOT_FOUND, [(CONTENT_TYPE, "text/plain".to_string())], format!("Error loading album art: {} File: {}", e, path).into_bytes())
        }
    }
}

/// Serve audio
async fn get_audio(Query(GetQueryPath { path }): Query<GetQueryPath>) -> impl IntoResponse {
    let data = tokio::task::spawn_blocking(move || {
        match AudioSources::from_path(&path).map(|s| s.generate_wav()) {
            Ok(Ok(wav)) => wav,
            Ok(Err(e)) => {
                warn!("Failed generating wav: {e}");
                vec![]
            },
            Err(e) => {
                warn!("Failed opening audio file {path}: {e}");
                vec![]
            }
        }
    }).await.unwrap_or(vec![]);

    // Empty 404 on error
    if data.is_empty() {
        return (StatusCode::NOT_FOUND, [(CONTENT_TYPE, "text/plain")], vec![]);
    }
    (StatusCode::OK, [(CONTENT_TYPE, "audio/wav")], data)
}

/// WS connection
async fn get_ws(ws: WebSocketUpgrade, State(state): State<WsState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| {
        debug!("WS Connected!");
        async move {
            match socket::handle_ws_connection(socket, state.context, state.db, state.events, state.flush_wake, state.preload, state.tangle).await {
                Ok(_) => {},
                Err(e) => warn!("WS connection error: {e}"),
            }
            debug!("WS Disconnected!");
        }
    })
}

/// Spotify token callback
async fn get_spotify_callback(request: Request<Body>) -> impl IntoResponse {
    info!("Got Spotify token from callback");
    WEBSERVER_CALLBACKS.lock().unwrap().insert("spotify".to_string(), request.uri().to_string());
    (StatusCode::OK, [(CONTENT_TYPE, "text/html")], include_str!("../../../assets/spotify_callback.html"))
}

// Start everything
pub fn start_all(context: StartContext) -> Result<(), Error> {
    if context.expose {
        warn!("Server is exposed to public!");
    }

    // Open in browser with 1s delay to allow the srever to load
    if context.browser {
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(1));
            webbrowser::open(&format!("http://127.0.0.1:{PORT}")).ok();
        });
    }

    start_async_runtime(context.clone())?;
    Ok(())
}

