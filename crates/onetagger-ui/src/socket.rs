use std::collections::HashMap;
use anyhow::Error;
use axum::extract::ws::WebSocket;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::path::{Path, PathBuf};
use onetagger_renamer::ac::Autocomplete;
use onetagger_renamer::docs::FullDocs;
use onetagger_renamer::{Renamer, TemplateParser, RenamerConfig};
use serde_json::{Value, json};
use serde::{Serialize, Deserialize};
use dunce::canonicalize;
use onetagger_tag::{TagChanges, Tag, Field};
use onetagger_tagger::{TaggerConfig, AudioFileInfo, TrackMatch};
use onetagger_autotag::{Tagger, AudioFileInfoImpl, TaggerConfigExt, AUTOTAGGER_PLATFORMS};
use onetagger_autotag::audiofeatures::{AudioFeaturesConfig, AudioFeatures};
use onetagger_platforms::spotify::Spotify;
use onetagger_player::{AudioSources, AudioPlayer};
use onetagger_shared::{Settings, COMMIT};
use onetagger_playlist::UIPlaylist;

use futures_util::StreamExt;

use crate::StartContext;
use crate::sender::MessageSender;
use crate::tagflow;
use crate::tageditor::TagEditor;
use crate::browser::{FileBrowser, FolderBrowser};


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
enum Action {
    Init,
    Exit,
    SaveSettings { settings: Value },
    LoadSettings,
    DefaultCustomPlatformSettings,
    Browse { path: Option<String>, context: Option<String> },
    Browser { url: String },
    OpenSettingsFolder,
    OpenFolder { path: PathBuf },
    OpenFile { path: PathBuf },
    DeleteFiles { paths: Vec<String> },
    GetLog,
    GeneratePlaylist { paths: Vec<String> },

    LoadPlatforms,
    StartTagging { config: TaggerConfigs, playlist: Option<UIPlaylist> },
    StopTagging,
    ConfigCallback { config: Value, platform: String, id: String },
    RepoManifest,
    #[serde(rename_all = "camelCase")]
    InstallPlatform { id: String, version: String, is_native: bool },

    Waveform { path: PathBuf },
    PlayerStart { path: PathBuf, autoplay: bool },
    #[serde(rename_all = "camelCase")]
    WaveformGet { track_id: i64 },
    PlayerLoad { path: PathBuf },
    PlayerPlay, 
    PlayerPause,
    PlayerSeek { pos: u64 },
    PlayerVolume { volume: f32 },
    PlayerStop,

    #[serde(rename_all = "camelCase")]
    SpotifyAuthorize { client_id: String, client_secret: String },
    SpotifyAuthorized,

    TagEditorFolder { path: Option<String>, subdir: Option<String>, recursive: Option<bool>  },
    TagEditorLoad { path: PathBuf },
    TagEditorSave { changes: TagChanges },

    RenamerSyntaxHighlight { template: String },
    RenamerAutocomplete { template: String },
    RenamerPreview { config: RenamerConfig },
    RenamerStart { config: RenamerConfig },

    FolderBrowser { path: PathBuf, child: String, base: bool },

    ManualTag { config: TaggerConfig, path: PathBuf },
    ManualTagApply { matches: Vec<TrackMatch>, path: PathBuf, config: TaggerConfig },

    // FlowTag
    TagFolderAdd { path: String },
    TagFolderRemove { id: i64 },
    TagFolderList,
    TagLibraryScan { full: Option<bool> },
    TagTracksLoad,
    TagPoolLoad,
    TagCreate { name: String, color: Option<String> },
    #[serde(rename_all = "camelCase")]
    TagUpdate { id: i64, name: Option<String>, color: Option<String>, group_id: Option<i64>, clear_group: Option<bool>, keybind: Option<Value>, clear_keybind: Option<bool> },
    TagDelete { id: i64 },
    GroupCreate { name: String, color: String },
    GroupUpdate { id: i64, name: Option<String>, color: Option<String>, position: Option<i64> },
    GroupDelete { id: i64 },
    #[serde(rename_all = "camelCase")]
    TrackAddTag { track_id: i64, name: String },
    #[serde(rename_all = "camelCase")]
    TrackRemoveTag { track_id: i64, tag_id: i64 },
    #[serde(rename_all = "camelCase")]
    FlushHint { track_id: Option<i64> },
    FlushStatusGet,
    FlushRetry,
    RekordboxExport { mode: String },
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
enum TaggerConfigs {
    AutoTagger(TaggerConfig), 
    AudioFeatures(AudioFeaturesConfig)
}

impl TaggerConfigs {
    // Print to log for later easier debug
    pub fn debug_print(&self) {
        match self {
            TaggerConfigs::AutoTagger(c) => {
                let mut c = c.clone();
                // don't leak secrets
                c.custom = HashMap::new().into();
                c.spotify = None;
                info!("AutoTagger config: {:?}", c);
            },
            TaggerConfigs::AudioFeatures(c) => {
                info!("AudioFeatures Config: {:?}", c);
            }
        }
    }
}

/// Path of the file loaded in any connection's player; the flush worker must
/// not write to a file that is (or may still be) held open for playback.
static CURRENT_PLAYER_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Whether `path` is the file currently loaded for playback
pub(crate) fn is_currently_playing(path: &str) -> bool {
    CURRENT_PLAYER_PATH
        .lock()
        .map(|p| p.as_deref() == Some(Path::new(path)))
        .unwrap_or(false)
}

fn set_currently_playing(path: &Path) {
    if let Ok(mut p) = CURRENT_PLAYER_PATH.lock() {
        *p = Some(path.to_owned());
    }
}

// Shared variables in socket
pub(crate) struct SocketContext {
    player: AudioPlayer,
    spotify: Option<Spotify>,
    start_context: StartContext,
    pub(crate) sender: MessageSender,
    pub(crate) db: Arc<onetagger_db::Db>,
    pub(crate) flush_wake: std::sync::mpsc::Sender<()>,
}

impl SocketContext {
    pub fn new(
        start_context: StartContext,
        sender: MessageSender,
        db: Arc<onetagger_db::Db>,
        flush_wake: std::sync::mpsc::Sender<()>,
    ) -> SocketContext {
        SocketContext {
            player: AudioPlayer::new(),
            spotify: None,
            start_context,
            sender,
            db,
            flush_wake,
        }
    }

    /// Queue a message for this connection (safe to call from any task/thread via a cloned sender)
    pub(crate) fn send(&self, message: impl Serialize) -> Result<(), Error> {
        self.sender.send(message)
    }
}


/// Reply to init call
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InitData {
    action: &'static str,
    version: &'static str,
    os: &'static str,
    arch: &'static str,
    custom_platform_compat: i32,
    start_context: StartContext,
    renamer_docs: FullDocs,
    commit: &'static str,
    work_dir: PathBuf,
    data_dir: PathBuf
}

impl InitData {
    /// Create new default instance
    pub fn new(start_context: StartContext) -> InitData {
        InitData {
            action: "init",
            version: onetagger_shared::VERSION,
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
            custom_platform_compat: onetagger_tagger::custom::CUSTOM_PLATFORM_COMPATIBILITY,
            start_context,
            renamer_docs: FullDocs::get().html(),
            commit: COMMIT,
            work_dir: std::env::current_dir().unwrap_or_default(),
            data_dir: Settings::get_folder().unwrap_or_default(),
        }
    }
}

pub(crate) async fn handle_ws_connection(
    websocket: WebSocket,
    context: StartContext,
    db: Arc<onetagger_db::Db>,
    events: tokio::sync::broadcast::Sender<String>,
    flush_wake: std::sync::mpsc::Sender<()>,
) -> Result<(), Error> {
    // Split the socket: all writes go through a dedicated writer task, so handlers
    // and background jobs can push messages concurrently with the read loop.
    let (sink, mut stream) = websocket.split();
    let sender = MessageSender::spawn(sink);

    // Forward broadcast events (flusher results) to this connection
    let mut events_rx = events.subscribe();
    let events_sender = sender.clone();
    tokio::spawn(async move {
        loop {
            match events_rx.recv().await {
                Ok(text) => {
                    if events_sender.send_text(text).is_err() {
                        break;
                    }
                }
                // Missed some events under load: keep forwarding newer ones
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    let mut context = SocketContext::new(context, sender, db, flush_wake);

    while let Some(message) = stream.next().await {
        match message {
            Ok(msg) => {
                match msg.to_text() {
                    Ok(text) => {
                        // Handle the WS message
                        match handle_message(text, &mut context).await {
                            Ok(_) => {},
                            Err(err) => {
                                // Send error to UI
                                error!("Websocket: {:?}, Data: {}", err, text);
                                context.send(json!({
                                    "action": "error",
                                    "message": &format!("{}", err)
                                })).ok();
                            }
                        }
                    },
                    Err(e) => warn!("WebSocket Message is not text: {e}"),
                }
            }

            Err(e) => {
                warn!("WebSocket error: {e}");
            }
        }

    }

    Ok(())
}

async fn handle_message(text: &str, context: &mut SocketContext) -> Result<(), Error> {
    // Parse JSON
    let action: Action = serde_json::from_str(text)?;
    match action {
        // Get initial info
        Action::Init => {
            context.send(InitData::new(context.start_context.clone())).ok();
        },
        Action::Exit => std::process::exit(0),
        Action::SaveSettings { settings } => Settings::from_ui(&settings).save()?,
        Action::LoadSettings => match Settings::load() {
            Ok(settings) => {
                context.send(json!({
                    "action": "loadSettings",
                    "settings": settings.ui
                })).ok();
            }
            // Ignore settings if they don't exist (might be initial load)
            Err(e) => error!("Failed loading settings, using defaults. {}", e)
        },
        // Get the default custom platform options
        Action::DefaultCustomPlatformSettings => {
            context.send(json!({
                "action": "defaultCustomPlatformSettings",
                "custom": TaggerConfig::custom_default().custom
            })).ok();
        }
        // Browse for folder
        Action::Browse { path, context: browse_context } => {
            let mut initial = path.unwrap_or(".".to_string());
            if initial.is_empty() || !Path::new(&initial).exists() {
                initial = ".".to_string()
            }
            if let Some(path) = tinyfiledialogs::select_folder_dialog("Select path", &initial) {
                context.send(json!({
                    "action": "browse",
                    "path": path,
                    "context": browse_context
                })).ok();
            }
        },
        // Get 1t Log
        Action::GetLog => {
            log::logger().flush();
            let log = std::fs::read_to_string(&Settings::get_folder()?.join("onetagger.log"))?;
            context.send(json!({
                "action": "log",
                "log": log
            })).ok();
        },
        // Open URL in external browser
        Action::Browser { url } => { webbrowser::open(&url)?; },
        Action::OpenSettingsFolder => opener::open(Settings::get_folder()?.to_str().unwrap())?,
        Action::OpenFolder { path } => { opener::open(&path).ok(); },
        Action::OpenFile { path } => { opener::open(&path).ok(); },
        Action::DeleteFiles { paths } => { trash::delete_all(&paths)?; },

        Action::GeneratePlaylist { paths } => {
            let playlist = onetagger_playlist::create_m3u_playlist(&paths.into_iter().map(|i| i.into()).collect::<Vec<_>>());
            if let Some(path) = tinyfiledialogs::save_file_dialog_with_filter(
                "Save playlist", 
                &std::env::current_dir()?.to_string_lossy().to_string(), 
                &["m3u", "m3u8"], 
                "Save playlist"
            ) {
                std::fs::write(&path, playlist)?;
                context.send(json!({
                    "action": "notify",
                    "message": format!("Playlist saved to: {path}")
                })).ok();
            }
        }

        Action::LoadPlatforms => {
            let platforms = tokio::task::spawn_blocking(|| {
                let mut platforms = AUTOTAGGER_PLATFORMS.lock().unwrap();
                platforms.load_all();
                platforms.platforms.iter().map(|p| p.info.clone()).collect::<Vec<_>>()
            }).await?;
            context.send(json!({
                "action": "loadPlatforms",
                "platforms": platforms
            })).ok();
        },
        Action::ConfigCallback { config, platform, id } => {
            let platform_clone = platform.clone();
            let response = tokio::task::spawn_blocking(move || {
                if let Some(p) = AUTOTAGGER_PLATFORMS.lock().unwrap().get_builder(&platform) {
                    Some(p.config_callback(&id, config))
                } else {
                    None
                }
            }).await?;
            if let Some(r) = response {
                context.send(json!({
                    "action": "configCallback",
                    "platform": platform_clone,
                    "response": r
                })).ok();
            }
        }
        Action::StartTagging { config, playlist } => {
            config.debug_print();

            // Load playlist
            let mut files = if let Some(playlist) = playlist {
                playlist.get_files()?
            } else { vec![] };
            let mut file_count = files.len();
            let mut folder_path = None;
            let tagger_finished = Arc::new(Mutex::new(None));
            // Load taggers
            let (tagger_type, rx) = match config {
                TaggerConfigs::AutoTagger(c) => {
                    // Load file list
                    if files.is_empty() {
                        let path = c.path.as_ref().map(|p| p.to_owned()).unwrap_or_default();
                        files = AudioFileInfo::get_file_list(&path, c.include_subfolders);
                        file_count = files.len();
                        folder_path = Some(path);
                    }
                    let rx = Tagger::tag_files(&c, files, tagger_finished.clone());
                    ("autoTagger", rx)
                },
                TaggerConfigs::AudioFeatures(c) => {
                    if files.is_empty() {
                        let path = c.path.as_ref().map(|i| i.to_owned()).unwrap_or_default().to_owned();
                        files = AudioFileInfo::get_file_list(&path, c.include_subfolders);
                        folder_path = Some(path);
                        file_count = files.len();
                    }
                    // Authorize spotify
                    let spotify = context.spotify.as_ref().ok_or(anyhow!("Spotify unauthorized!"))?.to_owned().to_owned();
                    let rx = AudioFeatures::start_tagging(c.clone(), spotify, files);
                    ("audioFeatures", rx)
                },
            };

            // Drain tagging progress on a blocking task: the status receiver blocks
            // between updates, and inline it would pin this connection (and a runtime
            // worker thread) for the whole run — StopTagging could never be received.
            let sender = context.sender.clone();
            tokio::task::spawn_blocking(move || {
                let start = timestamp!();
                sender.send(json!({
                    "action": "startTagging",
                    "files": file_count,
                    "type": tagger_type
                })).ok();
                for status in rx {
                    sender.send(json!({
                        "action": "taggingProgress",
                        "status": status
                    })).ok();
                }
                info!("Tagging finished, took: {} seconds.", (timestamp!() - start) / 1000);
                sender.send(json!({
                    "action": "taggingDone",
                    "path": folder_path,
                    "data": *tagger_finished.lock().unwrap()
                })).ok();
            });
        },
        Action::StopTagging => {
            onetagger_autotag::STOP_TAGGING.store(true, Ordering::SeqCst);
        },
        Action::Waveform { path } => {
            // Decode + stream on a blocking task so the read loop stays responsive;
            // bars are pushed straight to the writer task (no per-bar client ack).
            let sender = context.sender.clone();
            tokio::task::spawn_blocking(move || {
                let result: Result<(), Error> = (|| {
                    let source = AudioSources::from_path(&path)?;
                    let (waveform_rx, cancel_tx) = source.generate_waveform(180)?;
                    for wave in waveform_rx {
                        let sent = sender.send(json!({
                            "action": "waveformWave",
                            "wave": wave
                        }));
                        // Connection gone: stop generating
                        if sent.is_err() {
                            cancel_tx.send(true).ok();
                            return Ok(());
                        }
                    }
                    sender.send(json!({ "action": "waveformDone" })).ok();
                    Ok(())
                })();
                if let Err(e) = result {
                    error!("Waveform generation failed: {e}");
                    sender.send(json!({
                        "action": "error",
                        "message": format!("Waveform failed: {e}")
                    })).ok();
                }
            });
        },
        // Load + optionally start playback in one round trip (FlowTag)
        Action::PlayerStart { path, autoplay } => {
            set_currently_playing(&path);
            // Decoder open touches the file: keep it off the async threads
            let source = tokio::task::spawn_blocking(move || AudioSources::from_path(&path)).await??;
            context.send(json!({
                "action": "playerStarted",
                "duration": source.duration() as u64
            })).ok();
            context.player.load_file(source);
            if autoplay {
                context.player.play();
            }
        },
        // Cached waveform by track id (FlowTag)
        Action::WaveformGet { track_id } => tagflow::waveform_get(context, track_id)?,
        // Load player file
        Action::PlayerLoad { path } => {
            // Protect the file from the flush worker while it may be held for playback
            set_currently_playing(&path);
            let source = AudioSources::from_path(&path)?;
            // Meta
            let tag = Tag::load_file(&path, false)?;
            let title = tag.tag().get_field(Field::Title).map(|i| i.first().map(String::from)).flatten();
            let artists = tag.tag().get_field(Field::Artist).unwrap_or(vec![]);
            // Send to UI
            context.send(json!({
                "action": "playerLoad",
                "title": title,
                "artists": artists,
                "duration": source.duration() as u64
            })).ok();
            // Load
            context.player.load_file(source);
        },
        //  Controls
        Action::PlayerPlay => context.player.play(),
        Action::PlayerPause => context.player.pause(),
        Action::PlayerSeek { pos } => {
            context.send(json!({
                "action": "playerSync",
                "playing": context.player.seek(pos)
            })).ok();
        },
        Action::PlayerVolume { volume } => context.player.volume(volume),
        Action::PlayerStop => {
            context.player.stop();
            // Release the flush-worker guard: the file handle is dropped with the sink
            if let Ok(mut p) = CURRENT_PLAYER_PATH.lock() {
                *p = None;
            }
        },
        Action::SpotifyAuthorize { client_id, client_secret } => {
            // Authorize cached
            if let Some(spotify) = Spotify::try_cached_token(&client_id, &client_secret) {
                context.spotify = Some(spotify);
            // Authorize new
            } else {
                let (auth_url, client) = Spotify::generate_auth_url(&client_id, &client_secret)?;
                webbrowser::open(&auth_url)?;
                let spotify = tokio::task::spawn_blocking(move || {
                    Spotify::auth_server(client)
                }).await??;
                context.spotify = Some(spotify);
            }
            context.send(json!({
                "action": "spotifyAuthorized",
                "value": true
            })).ok();
            debug!("Spotify Authorized!");
        },
        // Check if authorized
        Action::SpotifyAuthorized => {
            context.send(json!({
                "action": "spotifyAuthorized",
                "value": context.spotify.is_some()
            })).ok();
        },
        Action::TagEditorFolder { path, subdir, recursive } => {
            let recursive = recursive.unwrap_or(false);
            let (new_path, files) = FileBrowser::list_dir_or_default(path.clone().map(|p| PathBuf::from(p)), subdir, true, true, recursive)?;
            context.send(json!({
                "action": "tagEditorFolder",
                "files": files,
                "path": new_path,
                // Stateless
                "recursive": recursive
            })).ok();
        },
        // Load tags of file
        Action::TagEditorLoad { path } => {
            let data = TagEditor::load_file(&path)?;
            context.send(json!({
                "action": "tagEditorLoad",
                "data": data
            })).ok();
        },
        // Save changes
        Action::TagEditorSave { changes } => {
            let _tag = changes.commit()?;
            context.send(json!({
                "action": "tagEditorSave"
            })).ok();
        },
        // Syntax highlight for renamer
        Action::RenamerSyntaxHighlight { template } => {
            let renamer = Renamer::new(TemplateParser::parse(&template));
            let html = renamer.generate_html(&template);
            context.send(json!({
                "action": "renamerSyntaxHighlight",
                "html": html
            })).ok();
        },
        // Autocomplete data
        Action::RenamerAutocomplete { template } => {
            let ac = Autocomplete::parse(&template);
            let suggestions = ac.suggest_html();
            context.send(json!({
                "action": "renamerAutocomplete",
                "suggestions": suggestions,
                "offset": ac.suggestion_offset()
            })).ok();
        },
        // Generate new names but don't rename
        Action::RenamerPreview { config } => {
            let mut renamer = Renamer::new(TemplateParser::parse(&config.template));
            let files = AudioFileInfo::load_files_iter(&config.path, config.subfolders, None, None);
            let files = renamer.generate(files.take(3), &config).unwrap_or(vec![]);
            context.send(json!({
                "action": "renamerPreview",
                "files": files,
            })).ok();
        },
        // Start renamer
        Action::RenamerStart { config } => {
            let mut renamer = Renamer::new(TemplateParser::parse(&config.template));
            let files = AudioFileInfo::load_files_iter(&config.path, config.subfolders, None, None);
            let files = renamer.generate(files, &config)?;
            renamer.rename(&files, &config)?;
            context.send(json!({
                "action": "renamerDone",
            })).ok();
        },
        // File browser list dir
        Action::FolderBrowser { path, child , base } => {
            // Windows root dir override
            let path = if cfg!(windows) && path.to_string_lossy() == "/" {
                if child.is_empty() {
                    PathBuf::from("/".to_string())
                } else {
                    PathBuf::from(format!("{}\\", child))
                }
            } else {
                canonicalize(PathBuf::from(path).join(child))?
            };

            let e = match base {
                true => FolderBrowser::generate_base(&path)?,
                false => FolderBrowser::list_dir(&path)?
            };

            context.send(json!({
                "action": "folderBrowser",
                "entry": e,
                "base": base,
                "path": path
            })).ok();
        },

        // Manually tag a file
        Action::ManualTag { config, path } => {
            // Log config
            info!("Manual tag starting for path: {path:?}");
            TaggerConfigs::AutoTagger(config.clone()).debug_print();

            let rx = tokio::task::spawn_blocking(move || {
                onetagger_autotag::manual_tagger(path, &config)
            }).await.unwrap()?;

            for (platform, r) in rx {
                match r {
                    Ok(matches) => {
                        context.send(json!({
                            "action": "manualTag",
                            "platform": platform,
                            "status": "ok",
                            "matches": matches
                        })).ok();
                    },
                    Err(e) => {
                        context.send(json!({
                            "action": "manualTag",
                            "platform": platform,
                            "status": "error",
                            "error": e.to_string()
                        })).ok();
                    },
                }
            }

            // On done
            context.send(json!({
                "action": "manualTagDone"
            })).ok();
        },
        // Apply the tags from manual tagger
        Action::ManualTagApply { matches, path, config } => {
            match onetagger_autotag::manual_tagger_apply(matches, path, &config) {
                Ok(_) => {
                    context.send(json!({
                        "action": "manualTagApplied",
                        "status": "ok"
                    })).ok();
                },
                Err(e) => {
                    error!("Failed applying manual tag: {e}");
                    context.send(json!({
                        "action": "manualTagApplied",
                        "status": "error",
                        "error": e.to_string()
                    })).ok();
                },
            }
        },


        Action::RepoManifest => {
            // Fetch before sending: holding a context borrow across the await would
            // require SocketContext: Sync (AudioPlayer's channel is not)
            let manifest = onetagger_autotag::repo::fetch_manifest_async().await?;
            context.send(json!({
                "action": "repoManifest",
                "manifest": manifest
            })).ok();
        },
        Action::InstallPlatform { id, version, is_native } => {
            match onetagger_autotag::repo::install_platform(&id, &version, is_native) {
                Ok(_) => context.send(json!({
                    "action": "installPlatform",
                    "status": "ok"
                })).ok(),
                Err(e) => {
                    error!("Failed installing platform {id}@{version}: {e}");
                    context.send(json!({
                        "action": "installPlatform",
                        "status": "error",
                        "error": e.to_string()
                    })).ok()
                },
            };
        },

        // FlowTag
        Action::TagFolderAdd { path } => tagflow::folder_add(context, &path)?,
        Action::TagFolderRemove { id } => tagflow::folder_remove(context, id)?,
        Action::TagFolderList => tagflow::folder_list(context)?,
        Action::TagLibraryScan { full } => tagflow::library_scan(context, full.unwrap_or(false)),
        Action::TagTracksLoad => tagflow::tracks_load(context)?,
        Action::TagPoolLoad => tagflow::pool_load(context)?,
        Action::TagCreate { name, color } => tagflow::tag_create(context, &name, color.as_deref())?,
        Action::TagUpdate { id, name, color, group_id, clear_group, keybind, clear_keybind } => {
            tagflow::tag_update(context, id, tagflow::TagPatch {
                name, color, group_id, clear_group, keybind, clear_keybind
            })?
        },
        Action::TagDelete { id } => tagflow::tag_delete(context, id)?,
        Action::GroupCreate { name, color } => tagflow::group_create(context, &name, &color)?,
        Action::GroupUpdate { id, name, color, position } => {
            tagflow::group_update(context, id, name.as_deref(), color.as_deref(), position)?
        },
        Action::GroupDelete { id } => tagflow::group_delete(context, id)?,
        Action::TrackAddTag { track_id, name } => tagflow::track_add_tag(context, track_id, &name)?,
        Action::TrackRemoveTag { track_id, tag_id } => tagflow::track_remove_tag(context, track_id, tag_id)?,
        Action::FlushHint { track_id } => tagflow::flush_hint(context, track_id)?,
        Action::FlushStatusGet => tagflow::flush_status_get(context)?,
        Action::FlushRetry => tagflow::flush_retry(context)?,
        Action::RekordboxExport { mode } => tagflow::rekordbox_export(context, &mode)?,
    }
   
    Ok(())
}