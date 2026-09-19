mod engine;
mod history;
mod hotkey;
mod paste;
mod settings;
mod snippets;
mod storage;

use std::{
    sync::mpsc::{self, RecvTimeoutError},
    thread,
    time::{Duration, Instant},
};

use history::{Entry, HistoryStore};
use hotkey::CaptureState;
use serde::Serialize;
use settings::{Settings, SettingsStore};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, PhysicalPosition, WindowEvent,
};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_clipboard_manager::ClipboardExt;

const LEVEL_INTERVAL: Duration = Duration::from_millis(33);
const OVERLAY_BOTTOM_MARGIN: f64 = 16.0;

/// What a recording is for. Dictation pastes what was said; a command applies what was
/// said to the selected text.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Dictate,
    Command,
}

enum Command {
    Start(Mode),
    Stop(Mode),
    Cancel(Mode),
    /// Paste the last transcript again, for when the first one landed in the wrong
    /// window.
    Repeat,
}

#[derive(Clone, Serialize)]
#[serde(tag = "phase", rename_all = "lowercase")]
enum DictationEvent {
    Recording { command: bool },
    Processing,
    Done { text: String },
    Copied { text: String },
    Empty,
    Cancelled,
    Error { message: String },
}

#[derive(Serialize)]
struct Permissions {
    microphone: &'static str,
    accessibility: bool,
}

#[tauri::command]
async fn model_status() -> engine::ModelStatus {
    tauri::async_runtime::spawn_blocking(engine::status)
        .await
        .unwrap_or_else(|e| engine::ModelStatus::error(e.to_string()))
}

#[tauri::command]
async fn prepare_model() {
    let _ = tauri::async_runtime::spawn_blocking(engine::prepare).await;
}

#[tauri::command]
fn permissions() -> Permissions {
    Permissions {
        microphone: engine::microphone_permission(),
        accessibility: paste::accessibility_granted(),
    }
}

#[tauri::command]
async fn request_microphone() -> bool {
    tauri::async_runtime::spawn_blocking(engine::request_microphone)
        .await
        .unwrap_or(false)
}

#[tauri::command]
fn open_privacy_settings(pane: String) -> Result<(), String> {
    let anchor = match pane.as_str() {
        "microphone" => "Privacy_Microphone",
        "accessibility" => "Privacy_Accessibility",
        _ => return Err(format!("Unknown settings pane: {pane}")),
    };
    std::process::Command::new("open")
        .arg(format!(
            "x-apple.systempreferences:com.apple.preference.security?{anchor}"
        ))
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_settings(store: tauri::State<SettingsStore>) -> Settings {
    store.get()
}

#[tauri::command]
fn update_settings(store: tauri::State<SettingsStore>, mut settings: Settings) -> Result<Settings, String> {
    settings.validate()?;
    settings.sanitize();
    store.set(settings.clone())?;
    Ok(settings)
}

// While this is on the dictation key is only reported, never acted on, so the user can
// record a shortcut that overlaps with the one already set.
#[tauri::command]
fn set_hotkey_capture(state: tauri::State<CaptureState>, active: bool) {
    state.set(active);
}

#[tauri::command]
fn history_list(store: tauri::State<HistoryStore>) -> Vec<Entry> {
    store.list()
}

#[tauri::command]
fn history_delete(app: AppHandle, id: u64) -> Result<(), String> {
    app.state::<HistoryStore>().remove(id)?;
    let _ = app.emit("history-changed", ());
    Ok(())
}

#[tauri::command]
fn history_clear(app: AppHandle) -> Result<(), String> {
    app.state::<HistoryStore>().clear()?;
    let _ = app.emit("history-changed", ());
    Ok(())
}

#[tauri::command]
async fn input_devices() -> Vec<engine::InputDevice> {
    tauri::async_runtime::spawn_blocking(engine::input_devices)
        .await
        .unwrap_or_default()
}

/// Writes an export to Downloads and shows it in Finder. The frontend builds the file,
/// so this only needs a safe name and somewhere to put it.
#[tauri::command]
fn save_export(app: AppHandle, name: String, contents: String) -> Result<String, String> {
    let safe: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || " -_.".contains(c) { c } else { '-' })
        .collect();
    let safe = safe.trim_start_matches('.');
    if safe.is_empty() {
        return Err("That file name can't be used.".into());
    }
    let folder = app.path().download_dir().map_err(|e| e.to_string())?;
    let path = folder.join(safe);
    std::fs::write(&path, contents).map_err(|e| e.to_string())?;
    let _ = std::process::Command::new("open").arg("-R").arg(&path).spawn();
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
fn copy_text(app: AppHandle, text: String) -> Result<(), String> {
    app.clipboard().write_text(text).map_err(|e| e.to_string())
}

// Only an explicit prompt request adds Parla to the Accessibility list in System
// Settings; checking trust alone leaves the user nothing to switch on.
#[tauri::command]
fn request_accessibility() -> bool {
    engine::request_accessibility()
}

#[tauri::command]
fn launch_at_login(app: AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
fn set_launch_at_login(app: AppHandle, enabled: bool) -> Result<(), String> {
    let launcher = app.autolaunch();
    let result = if enabled {
        launcher.enable()
    } else {
        launcher.disable()
    };
    result.map_err(|e| e.to_string())
}

// Key presses and releases can arrive faster than the engine starts, so a single
// thread handles them in order instead of racing async tasks.
fn spawn_worker(app: AppHandle) -> mpsc::Sender<Command> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let emit = |event: DictationEvent| {
            set_tray_recording(&app, matches!(event, DictationEvent::Recording { .. } | DictationEvent::Processing));
            let _ = app.emit("dictation", event);
        };
        // The mode travels with every stop and cancel, so letting go of one shortcut can
        // never end, or paste, a recording the other one started.
        let mut started: Option<(Instant, Mode)> = None;
        let mut live = false;
        let mut last_partial = String::new();

        loop {
            let command = if started.is_some() {
                match rx.recv_timeout(LEVEL_INTERVAL) {
                    Ok(command) => command,
                    Err(RecvTimeoutError::Timeout) => {
                        let _ = app.emit_to("overlay", "level", engine::level());
                        if live {
                            let partial = engine::partial();
                            if partial != last_partial {
                                let _ = app.emit_to("overlay", "partial", &partial);
                                last_partial = partial;
                            }
                        }
                        continue;
                    }
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            } else {
                match rx.recv() {
                    Ok(command) => command,
                    Err(_) => break,
                }
            };

            match (command, started) {
                (Command::Start(mode), None) => {
                    let settings = app.state::<SettingsStore>().get();
                    match start_engine(&settings, mode) {
                        Ok(()) => {
                            started = Some((Instant::now(), mode));
                            live = settings.live_preview;
                            last_partial.clear();
                            place_overlay(&app);
                            play_cue(&app, true);
                            duck_other_audio(&app);
                            emit(DictationEvent::Recording {
                                command: mode == Mode::Command,
                            });
                        }
                        Err(message) => emit(DictationEvent::Error { message }),
                    }
                }
                (Command::Stop(mode), Some((at, active))) if mode == active => {
                    let released = Instant::now();
                    started = None;
                    engine::duck_audio(false);
                    play_cue(&app, false);
                    emit(DictationEvent::Processing);
                    let duration_ms = at.elapsed().as_millis() as u64;

                    match (engine::stop(), mode) {
                        (Ok(transcript), _) if transcript.text.trim().is_empty() => {
                            emit(DictationEvent::Empty)
                        }
                        (Ok(mut transcript), Mode::Dictate) => {
                            let snippets = app.state::<SettingsStore>().get().snippets;
                            transcript.text = snippets::expand(&transcript.text, &snippets);
                            // Paste before bookkeeping so the text lands as soon as it exists.
                            let delivery = paste::insert(&app, &transcript.text);
                            let latency_ms = released.elapsed().as_millis() as u64;
                            app.state::<HistoryStore>().add(&transcript, duration_ms, latency_ms);
                            let _ = app.emit("history-changed", ());
                            emit(finish_delivery(delivery, transcript.text));
                        }
                        (Ok(transcript), Mode::Command) => {
                            emit(apply_command(&app, &transcript.text))
                        }
                        (Err(message), _) => emit(DictationEvent::Error { message }),
                    }
                }
                (Command::Repeat, None) => match app.state::<HistoryStore>().latest() {
                    Some(entry) => emit(deliver(&app, entry.text)),
                    None => emit(DictationEvent::Empty),
                },
                (Command::Cancel(mode), Some((_, active))) if mode == active => {
                    started = None;
                    engine::duck_audio(false);
                    engine::cancel();
                    emit(DictationEvent::Cancelled);
                }
                _ => {}
            }
        }
    });

    tx
}

fn start_engine(settings: &Settings, mode: Mode) -> Result<(), String> {
    match mode {
        Mode::Dictate => engine::start(
            settings.enhance,
            settings.quick_enhance,
            &settings.dictionary,
            settings.app_aware_tone,
            settings.live_preview,
            settings.input_device.as_deref(),
        ),
        // The spoken instruction is used word for word, so cleanup, the dictionary and
        // tone matching would only get in its way.
        Mode::Command => engine::start(
            false,
            false,
            &[],
            false,
            settings.live_preview,
            settings.input_device.as_deref(),
        ),
    }
}

// The result is pasted over the text that is still selected, so the selection is
// rewritten in place.
fn apply_command(app: &AppHandle, instruction: &str) -> DictationEvent {
    let selection = paste::selection(app);
    if selection.trim().is_empty() {
        return DictationEvent::Error {
            message: "Select the text you want to change first.".into(),
        };
    }
    match engine::run_command(instruction, &selection) {
        Ok(text) => deliver(app, text),
        Err(message) => DictationEvent::Error { message },
    }
}

fn deliver(app: &AppHandle, text: String) -> DictationEvent {
    finish_delivery(paste::insert(app, &text), text)
}

fn finish_delivery(delivery: Result<paste::Delivery, String>, text: String) -> DictationEvent {
    match delivery {
        Ok(paste::Delivery::Pasted) => DictationEvent::Done { text },
        Ok(paste::Delivery::Copied) => DictationEvent::Copied { text },
        Err(message) => DictationEvent::Error { message },
    }
}

fn play_cue(app: &AppHandle, start: bool) {
    if app.state::<SettingsStore>().get().sounds {
        engine::play_cue(start);
    }
}

fn duck_other_audio(app: &AppHandle) {
    if app.state::<SettingsStore>().get().mute_other_audio {
        engine::duck_audio(true);
    }
}

// The pill follows the cursor's display so it shows up where the user is looking.
fn place_overlay(app: &AppHandle) {
    let Some(overlay) = app.get_webview_window("overlay") else {
        return;
    };
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|point| app.monitor_from_point(point.x, point.y).ok().flatten())
        .or_else(|| overlay.primary_monitor().ok().flatten());
    let (Some(monitor), Ok(size)) = (monitor, overlay.outer_size()) else {
        return;
    };

    let area = monitor.work_area();
    let margin = (OVERLAY_BOTTOM_MARGIN * monitor.scale_factor()) as i32;
    let x = area.position.x + (area.size.width as i32 - size.width as i32) / 2;
    let y = area.position.y + area.size.height as i32 - size.height as i32 - margin;
    let _ = overlay.set_position(PhysicalPosition::new(x, y));
}

fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

const TRAY_ICON: &[u8] = include_bytes!("../icons/tray-template.png");
const TRAY_RECORDING_ICON: &[u8] = include_bytes!("../icons/tray-recording-template.png");

// The overlay can sit on another display or behind a full-screen app, so the menu bar
// also shows when the microphone is live.
fn set_tray_recording(app: &AppHandle, recording: bool) {
    let Some(tray) = app.tray_by_id("parla") else {
        return;
    };
    let bytes = if recording { TRAY_RECORDING_ICON } else { TRAY_ICON };
    if let Ok(icon) = Image::from_bytes(bytes) {
        let _ = tray.set_icon(Some(icon));
        let _ = tray.set_icon_as_template(true);
    }
    let _ = tray.set_tooltip(Some(if recording { "Parla — listening" } else { "Parla" }));
}

fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Parla", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Parla", true, Some("CmdOrCtrl+Q"))?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &separator, &quit])?;

    TrayIconBuilder::with_id("parla")
        .icon(Image::from_bytes(TRAY_ICON)?)
        .icon_as_template(true)
        .tooltip("Parla")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        // Two copies would both watch the dictation key and paste twice.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main(app)
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let handle = app.handle().clone();
            app.manage(SettingsStore::load(&handle));
            app.manage(HistoryStore::load(&handle));
            app.manage(CaptureState::default());
            hotkey::spawn(handle.clone(), spawn_worker(handle));

            if let Some(overlay) = app.get_webview_window("overlay") {
                let _ = overlay.set_ignore_cursor_events(true);
            }
            build_tray(app)?;
            // Asking at launch is what registers Parla in System Settings' Accessibility
            // list; macOS only shows its own prompt the first time.
            if !paste::accessibility_granted() {
                engine::request_accessibility();
            }
            engine::prepare();
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the main window keeps Parla running in the menu bar so the
            // dictation key keeps working.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            model_status,
            prepare_model,
            permissions,
            request_microphone,
            open_privacy_settings,
            get_settings,
            update_settings,
            set_hotkey_capture,
            history_list,
            history_delete,
            history_clear,
            copy_text,
            input_devices,
            save_export,
            request_accessibility,
            launch_at_login,
            set_launch_at_login
        ])
        .build(tauri::generate_context!())
        .expect("error while building Parla");

    app.run(|app, event| match event {
        tauri::RunEvent::Reopen { .. } => show_main(app),
        // Quitting mid-dictation would otherwise leave the Mac silent.
        tauri::RunEvent::Exit => engine::duck_audio(false),
        _ => {}
    });
}
