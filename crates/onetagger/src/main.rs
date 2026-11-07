#![windows_subsystem = "windows"]

#[macro_use] extern crate log;

use anyhow::Error;
use clap::Parser;
use std::path::PathBuf;
use onetagger_shared::Settings;
use wry::{DragDropEvent, WebContext, WebViewBuilder};
use tao::dpi::{LogicalSize, Size};
use tao::event::{StartCause, Event, WindowEvent};
use tao::event_loop::{EventLoopBuilder, ControlFlow};
use tao::window::{WindowBuilder, Icon, Theme};

use onetagger_shared::{VERSION, COMMIT, PORT};
use onetagger_ui::StartContext;

fn main() {
    // Setup 1T
    onetagger_shared::setup();
    let cli = Cli::parse();

    info!("\n\nStarting OneTagger v{VERSION} Commit: {COMMIT} OS: {}\n\n", std::env::consts::OS);
    
    // MacOS
    if !cli.server {
        old_macos_warning().ok();
    }
            
    // Start
    let context = StartContext {
        start_path: cli.path, 
        server_mode: cli.server, 
        expose: cli.expose,
        browser: cli.browser
    };

    // Server mode
    if cli.server {
        onetagger_ui::start_all(context).expect("Failed to start servers!");
        return;
    }

    // GUI Mode
    std::thread::spawn(move || {
        onetagger_ui::start_all(context).expect("Failed to start servers!");
    });
    start_webview().expect("Failed to start webview!");
    debug!("Exitting gracefully...");
}

#[derive(Parser, Debug, Clone)]
#[clap(version)]
struct Cli {
    /// Start in server mode (no webview window)
    #[clap(short = 'S', long)]
    server: bool,

    /// Make the server listen on `0.0.0.0`
    #[clap(long)]
    expose: bool,

    /// Path to music files
    #[clap(short, long)]
    path: Option<String>,

    /// Open in browser
    #[clap(long)]
    browser: bool,
}

/// Show warning for old macOS
#[cfg(target_os = "macos")]
fn old_macos_warning() -> Result<(), Error> {
    use std::process::Command;
    use native_dialog::DialogBuilder;

    // Get version
    let output = Command::new("sw_vers")
        .arg("-productVersion")
        .output()?
        .stdout;
    let version = String::from_utf8(output)?;
    // Show warning
    if version.starts_with("10.") && !version.contains("10.15") {
        let server_version = DialogBuilder::message()
            .set_title("Unsupported version")
            .set_text("In order to use One Tagger on older macOS, install a more recent browser like Google Chrome and set it as default browser. Click Yes to restart One Tagger into server mode and open in browser, No to open normally.")
            .confirm()
            .show()?;

        if server_version {
            Command::new("osascript")
                .arg("-e")
                .arg(format!(
                    "tell application \"Terminal\" to do script \"{} --server --browser\"",
                    std::env::args().next().unwrap()
                ))
                .output()
                .ok();
            std::process::exit(0);
        }
    }
    Ok(())

}

/// Show warning for old macOS
#[cfg(not(target_os = "macos"))]
fn old_macos_warning() -> Result<(), Error> { Ok(()) }


/// Start webview window
pub fn start_webview() -> Result<(), Error> {
    // Setup wry
    let event_loop = EventLoopBuilder::with_user_event().build();
    let proxy = event_loop.create_proxy();
    // Kept for the event loop itself (later clones are consumed by handlers)
    let loop_proxy = proxy.clone();
    let window = WindowBuilder::new()
        .with_title("FlowTag")
        .with_min_inner_size(Size::Logical(LogicalSize::new(1150.0, 550.0)))
        .with_inner_size(Size::Logical(LogicalSize::new(1280.0, 720.0)))
        .with_resizable(true)
        .with_window_icon(Some(Icon::from_rgba(include_bytes!("../../../assets/64x64.bin").to_vec(), 64, 64).unwrap()))
        .with_theme(Some(Theme::Dark))
        .build(&event_loop)?;
    window.set_inner_size(Size::Logical(LogicalSize::new(1280.0, 720.0)));
    let mut context = WebContext::new(Some(Settings::get_folder()?.join("webview")));
    let p = proxy.clone();

    // OS media keys + Now Playing: the client's player state arrives over the
    // websocket and is relayed to this (main) thread as a user event
    let mut media_controls = setup_media_controls();
    let media_proxy = proxy.clone();
    onetagger_ui::media::register_media_sink(move |now_playing| {
        media_proxy.send_event(CustomWindowEvent::Media(now_playing)).ok();
    });

    // Register menu for MacOS shortcuts to work
    #[cfg(target_os = "macos")]
    let _menu = {
        use muda::{Menu, Submenu, PredefinedMenuItem};
        let menu = Menu::new();

        let submenu = Submenu::new("Edit", true);
        submenu.append_items(&[
            &PredefinedMenuItem::undo(None),
            &PredefinedMenuItem::redo(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::cut(None),
            &PredefinedMenuItem::copy(None),
            &PredefinedMenuItem::paste(None),
            &PredefinedMenuItem::select_all(None),
        ])?;

        menu.append(&submenu)?;
        menu.init_for_nsapp();
	    debug!("Added menu");

        menu
    };
    
    // Configure
    let mut webview = WebViewBuilder::new_with_web_context(&mut context)
        .with_url(&format!("http://127.0.0.1:{PORT}/"))
        .with_devtools(Settings::load().map(|s| s.devtools()).unwrap_or(false))
        .with_ipc_handler(move |message| {
            let proxy = &p;
            if message.body() == "devtools" {
                proxy.send_event(CustomWindowEvent::DevTools).ok();
            }
            if message.body() == "exit" {
                proxy.send_event(CustomWindowEvent::Exit).ok();
            }
            if message.body() == "toggleMini" {
                proxy.send_event(CustomWindowEvent::ToggleMini).ok();
            }
        });

    // Windows webview2 does NOT support custom DnD, janky workaround
    if cfg!(target_os = "windows") {
        // Handler
        let proxy = proxy.clone();
        let handle_url = move |url: String| -> bool {
            debug!("Navigation/NewWindow to: {url}");
            if url.starts_with("file://") {
                let url = url.replace("file:///", "");
                let path = urlencoding::decode(&url).map(|r| r.to_string()).unwrap_or(url).replace("/", "\\");
                proxy.send_event(CustomWindowEvent::DropFolder(path.into())).ok();
                return false;
            }
            true
        };
        
        // Register
        webview = webview.with_navigation_handler(handle_url.clone());
        webview = webview.with_new_window_req_handler(handle_url);
    }

    // Handle dropped folders (for all other than Windows)
    if cfg!(not(target_os = "windows")) {
        webview = webview.with_drag_drop_handler(move |event| {
            match event {
                DragDropEvent::Drop { mut paths, .. } => {
                    if paths.len() > 1 || paths.is_empty() {
                        warn!("Drop only 1 path!");
                        return true;
                    }
                    let path = paths.remove(0);
                    if path.is_dir() {
                        proxy.send_event(CustomWindowEvent::DropFolder(path)).ok();
                        return true;
                    }
                    if path.is_file() {
                        return false;
                    }
                },
                _ => {}
            }

            true
        });
    }

    // Non-linux Webview
    #[cfg(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "ios",
        target_os = "android",
    ))]
    let webview = webview.build(&window)?;

    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
    )))]
    let webview = {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        let vbox = window.default_vbox().unwrap();
        webview.build_gtk(vbox).unwrap()
    };

    // Event loop
    let main_window_id = window.id();
    let mut mini: Option<(tao::window::Window, wry::WebView)> = None;
    event_loop.run(move |event, target, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(StartCause::Init) => {
                debug!("Started webview!");
            },
            // Check for unsaved progress (mini just closes)
            Event::WindowEvent { event: WindowEvent::CloseRequested, window_id, .. } => {
                if window_id != main_window_id {
                    mini = None;
                    return;
                }
                match webview.evaluate_script("window.onWebviewEvent({\"action\": \"exit\"})") {
                    Ok(_) => {},
                    Err(e) => {
                        warn!("Failed to ask for exit: {e}");
                        *control_flow = ControlFlow::Exit;
                    }
                }

            },
            // Drop folder to client
            Event::UserEvent(CustomWindowEvent::DropFolder(path)) => {
                match webview.evaluate_script(&format!("window.onWebviewEvent({{\"action\": \"browse\", \"path\": \"{}\"}})", path.to_string_lossy().replace("\\", "\\\\").replace("\"", "\\\""))) {
                    Ok(_) => {},
                    Err(e) => error!("Failed executing JS on webview: {e}"),
                }
            },
            // Open devtools
            Event::UserEvent(CustomWindowEvent::DevTools) => {
                webview.open_devtools();
            },
            // Mirror the client player into the OS Now Playing panel
            Event::UserEvent(CustomWindowEvent::Media(np)) => {
                if let Some(controls) = media_controls.as_mut() {
                    let cover = format!("http://127.0.0.1:{PORT}/thumb?path={}", urlencoding::encode(&np.path));
                    controls.set_metadata(souvlaki::MediaMetadata {
                        title: Some(&np.title),
                        artist: Some(&np.artist),
                        album: None,
                        duration: Some(std::time::Duration::from_millis(np.duration_ms)),
                        cover_url: Some(&cover),
                    }).ok();
                    let progress = Some(souvlaki::MediaPosition(std::time::Duration::from_millis(np.position_ms)));
                    let playback = match np.playing {
                        true => souvlaki::MediaPlayback::Playing { progress },
                        false => souvlaki::MediaPlayback::Paused { progress },
                    };
                    controls.set_playback(playback).ok();
                }
            },
            // Exit from UI
            Event::UserEvent(CustomWindowEvent::Exit) => {
                debug!("Exitting webview loop...");
                *control_flow = ControlFlow::Exit;
            },
            // Floating miniplayer
            Event::UserEvent(CustomWindowEvent::ToggleMini) => {
                if mini.is_some() {
                    mini = None;
                } else {
                    match create_mini(target, loop_proxy.clone()) {
                        Ok(m) => mini = Some(m),
                        Err(e) => error!("Failed creating miniplayer: {e}"),
                    }
                }
            },
            Event::UserEvent(CustomWindowEvent::DragMini) => {
                if let Some((w, _)) = &mini {
                    w.drag_window().ok();
                }
            },
            _ => ()
        }

    });

}

enum CustomWindowEvent {
    DropFolder(PathBuf),
    DevTools,
    Exit,
    /// Client player state for the OS Now Playing panel
    Media(onetagger_ui::media::NowPlaying),
    /// Open/close the floating miniplayer window
    ToggleMini,
    /// The frameless miniplayer is dragged by its background
    DragMini,
}

/// Create the always-on-top frameless miniplayer window
fn create_mini(
    target: &tao::event_loop::EventLoopWindowTarget<CustomWindowEvent>,
    proxy: tao::event_loop::EventLoopProxy<CustomWindowEvent>,
) -> Result<(tao::window::Window, wry::WebView), Error> {
    let window = WindowBuilder::new()
        .with_title("FlowTag Mini")
        .with_inner_size(Size::Logical(LogicalSize::new(400.0, 178.0)))
        .with_resizable(false)
        .with_decorations(false)
        .with_always_on_top(true)
        .with_theme(Some(Theme::Dark))
        .build(target)?;

    let builder = WebViewBuilder::new()
        .with_url(&format!("http://127.0.0.1:{PORT}/#/mini"))
        .with_ipc_handler(move |message| {
            match message.body().as_str() {
                "closeMini" => { proxy.send_event(CustomWindowEvent::ToggleMini).ok(); },
                "dragMini" => { proxy.send_event(CustomWindowEvent::DragMini).ok(); },
                _ => {}
            }
        });

    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "ios", target_os = "android"))]
    let webview = builder.build(&window)?;
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "ios", target_os = "android")))]
    let webview = {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        let vbox = window.default_vbox().unwrap();
        builder.build_gtk(vbox)?
    };

    Ok((window, webview))
}

/// OS media controls (macOS Now Playing / MPRIS). Created on the main thread
/// beside the event loop; None when unavailable (Windows would need an hwnd).
fn setup_media_controls() -> Option<souvlaki::MediaControls> {
    #[cfg(target_os = "windows")]
    { return None; }
    #[cfg(not(target_os = "windows"))]
    {
        let config = souvlaki::PlatformConfig {
            display_name: "FlowTag",
            dbus_name: "flowtag",
            hwnd: None,
        };
        let mut controls = match souvlaki::MediaControls::new(config) {
            Ok(c) => c,
            Err(e) => {
                warn!("Media controls unavailable: {e:?}");
                return None;
            }
        };
        if let Err(e) = controls.attach(|event| handle_media_event(event)) {
            warn!("Failed attaching media control events: {e:?}");
            return None;
        }
        Some(controls)
    }
}

/// Map OS media commands onto FlowTag client actions (broadcast to all clients)
fn handle_media_event(event: souvlaki::MediaControlEvent) {
    use souvlaki::MediaControlEvent::*;
    match event {
        Play => onetagger_ui::media::media_key("play", None),
        Pause | Stop => onetagger_ui::media::media_key("pause", None),
        Toggle => onetagger_ui::media::media_key("playPause", None),
        Next => onetagger_ui::media::media_key("next", None),
        Previous => onetagger_ui::media::media_key("prev", None),
        SetPosition(souvlaki::MediaPosition(pos)) => {
            onetagger_ui::media::media_key("seek", Some(pos.as_millis() as u64))
        }
        _ => {}
    }
}
