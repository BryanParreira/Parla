mod engine;
mod formatting;
mod history;
mod hotkey;
mod keys;
mod learn;
mod paste;
mod settings;
mod snippets;
mod storage;
#[cfg(target_os = "macos")]
mod updater;
#[cfg(not(target_os = "macos"))]
mod desktop_update;

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, RecvTimeoutError},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

use history::{Entry, HistoryStore};
use hotkey::CaptureState;
use serde::Serialize;
use learn::{Suggestion, SuggestionStore};
use settings::{CleanupLevel, Settings, SettingsStore, SpeechModel, Transform};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, PhysicalPosition, WindowEvent,
};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_clipboard_manager::ClipboardExt;

const LEVEL_INTERVAL: Duration = Duration::from_millis(33);
const OVERLAY_BOTTOM_MARGIN: f64 = 16.0;
/// Long enough for the user to fix a word or two after a paste, short enough that they
/// are probably still in the same field.
const LEARN_DELAY: Duration = Duration::from_secs(15);
const RELEASES_PAGE: &str = "https://github.com/BryanParreira/Parla/releases";

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
    /// Delete the text Parla pasted last, for when it landed somewhere it shouldn't
    /// have.
    Undo,
    /// Apply a saved instruction to the selected text, picked from the menu bar.
    Transform(String),
    /// Show the transforms in a menu at the selection and apply the one picked.
    PickTransform,
    /// Look at the field a dictation went into for words the user has since fixed.
    Learn { pasted: String, app: Option<String> },
}

/// Lets the menu bar hand work to the dictation worker, which owns the paste.
struct Worker(Mutex<mpsc::Sender<Command>>);

#[derive(Serialize)]
struct UpdateInfo {
    available: bool,
    version: String,
    url: String,
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
    /// The last paste was taken back out again.
    Undone,
    /// Recording was refused because of what has focus right now.
    Blocked { reason: String },
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
    #[cfg(target_os = "macos")]
    return std::process::Command::new("open")
        .arg(format!(
            "x-apple.systempreferences:com.apple.preference.security?{anchor}"
        ))
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string());
    // Windows has a microphone privacy page; typing into other apps needs no permission.
    #[cfg(target_os = "windows")]
    return match anchor {
        "Privacy_Microphone" => desktop_update::open_url("ms-settings:privacy-microphone"),
        _ => Ok(()),
    };
    #[cfg(target_os = "linux")]
    {
        let _ = anchor;
        Ok(())
    }
}

#[tauri::command]
fn get_settings(store: tauri::State<SettingsStore>) -> Settings {
    store.get()
}

#[tauri::command]
fn update_settings(app: AppHandle, mut settings: Settings) -> Result<Settings, String> {
    settings.validate()?;
    settings.sanitize();
    let store = app.state::<SettingsStore>();
    let previous = store.get();
    store.set(settings.clone())?;
    // Choosing Whisper is what starts its one-time download.
    if settings.speech_model == SpeechModel::Whisper && previous.speech_model != SpeechModel::Whisper {
        thread::spawn(engine::prepare_whisper);
    }
    if settings.input_device != previous.input_device {
        engine::warm_microphone(settings.input_device.as_deref());
    }
    refresh_tray(&app);
    Ok(settings)
}

#[derive(Clone, Serialize)]
#[serde(tag = "phase", rename_all = "lowercase")]
enum UpdateProgress {
    Downloading { progress: f64 },
    Verifying,
    Restarting,
}

/// Downloads `url` to `file`, reporting progress to the window as it goes.
fn download_with_progress(handle: &AppHandle, url: &str, file: &std::path::Path) -> Result<(), String> {
    let done = Arc::new(AtomicBool::new(false));
    let reporter = {
        let done = done.clone();
        let handle = handle.clone();
        thread::spawn(move || {
            while !done.load(Ordering::Relaxed) {
                let progress = engine::download_progress();
                let _ = handle.emit("update-progress", UpdateProgress::Downloading { progress });
                thread::sleep(Duration::from_millis(200));
            }
        })
    };
    let downloaded = engine::download(url, file);
    done.store(true, Ordering::Relaxed);
    let _ = reporter.join();
    downloaded
}

/// Downloads, checks and installs an update, then quits so the new version can open.
#[cfg(target_os = "macos")]
#[tauri::command]
async fn install_update(app: AppHandle, url: String) -> Result<(), String> {
    let target = updater::installed_bundle()
        .ok_or("Only the installed Parla can update itself. Download the update instead.")?;
    if !is_release_url(&url) || !url.ends_with(".dmg") {
        return Err("That isn't a Parla release.".into());
    }
    let current = app.package_info().version.to_string();
    let handle = app.clone();
    let work = std::env::temp_dir().join(format!("parla-update-{}", std::process::id()));

    let result = tauri::async_runtime::spawn_blocking({
        let work = work.clone();
        move || -> Result<(), String> {
            let _ = std::fs::remove_dir_all(&work);
            std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
            let dmg = work.join("Parla.dmg");
            download_with_progress(&handle, &url, &dmg)?;

            let _ = handle.emit("update-progress", UpdateProgress::Verifying);
            let new_app = updater::extract(&dmg, &work)?;
            updater::verify(&new_app, &current)?;
            let _ = handle.emit("update-progress", UpdateProgress::Restarting);
            updater::schedule_swap(std::process::id(), &new_app, &target, &work)
        }
    })
    .await
    .map_err(|e| e.to_string())?;

    match result {
        Ok(()) => {
            app.exit(0);
            Ok(())
        }
        Err(message) => {
            let _ = std::fs::remove_dir_all(&work);
            Err(message)
        }
    }
}

/// Downloads the installer (Windows) or AppImage (Linux) and hands over to it.
#[cfg(not(target_os = "macos"))]
#[tauri::command]
async fn install_update(app: AppHandle, url: String) -> Result<(), String> {
    desktop_update::supported()?;
    if !is_release_url(&url) {
        return Err("That isn't a Parla release.".into());
    }
    let name = desktop_update::file_name(&url)?;
    let handle = app.clone();
    let work = std::env::temp_dir().join(format!("parla-update-{}", std::process::id()));
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let _ = std::fs::remove_dir_all(&work);
        std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
        let file = work.join(name);
        download_with_progress(&handle, &url, &file)?;
        let _ = handle.emit("update-progress", UpdateProgress::Restarting);
        desktop_update::install(&file, std::process::id())
    })
    .await
    .map_err(|e| e.to_string())??;
    app.exit(0);
    Ok(())
}

#[tauri::command]
fn suggestions_list(store: tauri::State<SuggestionStore>) -> Vec<Suggestion> {
    store.list()
}

#[tauri::command]
fn suggestion_resolve(app: AppHandle, word: String, accepted: bool) -> Result<(), String> {
    app.state::<SuggestionStore>().resolve(&word, accepted)?;
    let _ = app.emit("suggestions-changed", ());
    Ok(())
}

#[tauri::command]
async fn check_for_update(app: AppHandle) -> Result<UpdateInfo, String> {
    let release = tauri::async_runtime::spawn_blocking(engine::latest_release)
        .await
        .map_err(|e| e.to_string())??;
    let current = app.package_info().version.to_string();
    // Release tags have lagged the app version before (v1.2.1 shipped Parla_1.3.0), so
    // the DMG's own name is the better source when it has one.
    let version = release
        .download
        .as_deref()
        .and_then(version_in_file_name)
        .unwrap_or(release.version);
    Ok(UpdateInfo {
        available: newer(&version, &current),
        url: release.download.unwrap_or(release.url),
        version,
    })
}

/// "…/Parla_1.3.0_aarch64.dmg" → "1.3.0".
fn version_in_file_name(url: &str) -> Option<String> {
    let name = url.rsplit('/').next()?;
    name.split('_')
        .find(|part| part.split('.').count() == 3 && part.split('.').all(|n| n.parse::<u64>().is_ok()))
        .map(str::to_string)
}

/// Whether version `a` is newer than `b`, comparing "1.10.0" as numbers.
fn newer(a: &str, b: &str) -> bool {
    let parts = |v: &str| -> Vec<u64> {
        v.split(['.', '-']).map_while(|part| part.parse().ok()).collect()
    };
    parts(a) > parts(b)
}

/// Parla's own GitHub releases, and nothing else. GitHub treats the repository name
/// case-insensitively, so the check does too.
fn is_release_url(url: &str) -> bool {
    url.to_lowercase().starts_with(&RELEASES_PAGE.to_lowercase()) && !url.contains(char::is_whitespace)
}

// Only ever opens Parla's own release downloads, whatever the frontend asks for.
#[tauri::command]
fn open_release(url: String) -> Result<(), String> {
    let target = if is_release_url(&url) { url } else { RELEASES_PAGE.to_string() };
    #[cfg(target_os = "macos")]
    return std::process::Command::new("open")
        .arg(target)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string());
    #[cfg(not(target_os = "macos"))]
    return desktop_update::open_url(&target);
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
    refresh_tray(&app);
    Ok(())
}

#[tauri::command]
fn history_clear(app: AppHandle) -> Result<(), String> {
    app.state::<HistoryStore>().clear()?;
    let _ = app.emit("history-changed", ());
    refresh_tray(&app);
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
    let later = tx.clone();

    thread::spawn(move || {
        let emit = |event: DictationEvent| {
            set_tray_recording(&app, matches!(event, DictationEvent::Recording { .. } | DictationEvent::Processing));
            let _ = app.emit("dictation", event);
        };
        // The mode travels with every stop and cancel, so letting go of one shortcut can
        // never end, or paste, a recording the other one started.
        let mut started: Option<(Instant, Mode)> = None;
        // Read when recording starts, while the app being dictated into is still the one
        // in front; by the time it ends the user may have switched away.
        let mut target_app: Option<String> = None;
        let mut live = false;
        let mut last_partial = String::new();
        // What undo would delete. Taken rather than read, so one press can never eat
        // text Parla did not paste.
        let mut last_paste: Option<String> = None;

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
                    // Checked before the microphone opens, so a refused recording never
                    // reaches the Neural Engine at all.
                    if let Some(reason) = settings
                        .pause_in_sensitive_apps
                        .then(engine::sensitive_context)
                        .flatten()
                    {
                        emit(DictationEvent::Blocked { reason });
                        continue;
                    }
                    let options = engine_options(&settings, mode);
                    match engine::start(&options) {
                        Ok(()) => {
                            started = Some((Instant::now(), mode));
                            live = settings.live_preview;
                            last_partial.clear();
                            // The pill and the sound come the moment the microphone is
                            // live; reading the app and cursor happens while the user
                            // is already talking.
                            place_overlay(&app);
                            play_cue(&app, true);
                            emit(DictationEvent::Recording {
                                command: mode == Mode::Command,
                            });
                            duck_other_audio(&app);
                            target_app = engine::frontmost_app();
                            engine::start_context(&options);
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
                            let settings = app.state::<SettingsStore>().get();
                            if settings.spoken_formatting {
                                transcript.text = formatting::apply(&transcript.text);
                            }
                            transcript.text = snippets::expand(&transcript.text, &settings.snippets);
                            // The spaces are typed but not kept: history holds the dictation
                            // itself, not where it happened to land.
                            let typed = spaced(&transcript);
                            // Paste before bookkeeping so the text lands as soon as it exists.
                            let delivery = paste::insert(&app, &typed);
                            let latency_ms = released.elapsed().as_millis() as u64;
                            let app_name = target_app.take();
                            app.state::<HistoryStore>().add(
                                &transcript,
                                duration_ms,
                                latency_ms,
                                app_name.clone(),
                            );
                            let _ = app.emit("history-changed", ());
                            refresh_tray(&app);
                            remember_paste(&mut last_paste, &delivery, &typed);
                            if settings.learn_words && matches!(delivery, Ok(paste::Delivery::Pasted)) {
                                let later = later.clone();
                                let pasted = transcript.text.clone();
                                thread::spawn(move || {
                                    thread::sleep(LEARN_DELAY);
                                    let _ = later.send(Command::Learn { pasted, app: app_name });
                                });
                            }
                            emit(finish_delivery(delivery, transcript.text));
                        }
                        (Ok(transcript), Mode::Command) => {
                            emit(apply_command(&app, &transcript.text, &mut last_paste))
                        }
                        (Err(message), _) => emit(DictationEvent::Error { message }),
                    }
                }
                (Command::Repeat, None) => match app.state::<HistoryStore>().latest() {
                    Some(entry) => emit(deliver(&app, entry.text, &mut last_paste)),
                    None => emit(DictationEvent::Empty),
                },
                // Only ever offered for text Parla itself pasted, and only once, so a
                // second press can't start deleting what the user wrote.
                (Command::Undo, None) => match last_paste.take() {
                    Some(text) => emit(match paste::undo(&text) {
                        Ok(()) => DictationEvent::Undone,
                        Err(message) => DictationEvent::Error { message },
                    }),
                    None => emit(DictationEvent::Empty),
                },
                // The menu comes first and the selection is read after it, by which
                // time the shortcut's keys are up and can't bend the copy keystroke.
                (Command::PickTransform, None) => {
                    let transforms = app.state::<SettingsStore>().get().transforms;
                    let names: Vec<&str> = transforms.iter().map(|t| t.name.as_str()).collect();
                    if let Some(transform) = engine::pick(&names).and_then(|index| transforms.get(index)) {
                        place_overlay(&app);
                        emit(DictationEvent::Processing);
                        emit(apply_instruction(&app, &transform.instruction, &mut last_paste));
                    }
                }
                (Command::Transform(instruction), None) => {
                    place_overlay(&app);
                    emit(DictationEvent::Processing);
                    emit(apply_instruction(&app, &instruction, &mut last_paste));
                }
                // Only the field the dictation went into is worth reading, and never a
                // password field or a sensitive app.
                (Command::Learn { pasted, app: dictated_into }, _) => {
                    let settings = app.state::<SettingsStore>().get();
                    if settings.learn_words
                        && dictated_into.is_some()
                        && engine::frontmost_app() == dictated_into
                        && engine::sensitive_context().is_none()
                    {
                        let fixes = learn::corrections(&pasted, &engine::focused_text());
                        if app.state::<SuggestionStore>().record(&fixes, &settings.dictionary) {
                            let _ = app.emit("suggestions-changed", ());
                        }
                    }
                }
                (Command::Cancel(mode), Some((_, active))) if mode == active => {
                    started = None;
                    target_app = None;
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

fn engine_options(settings: &Settings, mode: Mode) -> engine::StartOptions<'_> {
    let whisper = settings.speech_model == SpeechModel::Whisper;
    let language = if whisper { settings.language.as_deref().unwrap_or("") } else { "" };
    match mode {
        Mode::Dictate => engine::StartOptions {
            enhance: settings.enhance,
            quick: settings.quick_enhance,
            dictionary: &settings.dictionary,
            match_app: settings.app_aware_tone,
            rules: settings.app_rules_text(),
            live: settings.live_preview,
            device: settings.input_device.as_deref(),
            level: settings.cleanup_level.as_str(),
            use_context: settings.use_context,
            formatting: settings.spoken_formatting,
            whisper,
            language,
            soft_voice: settings.soft_voice,
        },
        // The spoken instruction is used word for word, so cleanup, the dictionary,
        // context and tone matching would only get in its way.
        Mode::Command => engine::StartOptions {
            enhance: false,
            quick: false,
            dictionary: &[],
            match_app: false,
            rules: String::new(),
            live: settings.live_preview,
            device: settings.input_device.as_deref(),
            level: CleanupLevel::default().as_str(),
            use_context: false,
            formatting: false,
            whisper,
            language,
            soft_voice: settings.soft_voice,
        },
    }
}

// A spoken name of a saved transform stands for its full instruction.
fn apply_command(
    app: &AppHandle,
    spoken: &str,
    last_paste: &mut Option<String>,
) -> DictationEvent {
    let transforms = app.state::<SettingsStore>().get().transforms;
    let instruction = Transform::resolve(spoken, &transforms).unwrap_or(spoken).to_string();
    apply_instruction(app, &instruction, last_paste)
}

// The result is pasted over the text that is still selected, so the selection is
// rewritten in place.
fn apply_instruction(
    app: &AppHandle,
    instruction: &str,
    last_paste: &mut Option<String>,
) -> DictationEvent {
    let selection = paste::selection(app);
    if selection.trim().is_empty() {
        return DictationEvent::Error {
            message: "Select the text you want to change first.".into(),
        };
    }
    match engine::run_command(instruction, &selection) {
        Ok(text) => deliver(app, text, last_paste),
        Err(message) => DictationEvent::Error { message },
    }
}

/// The dictation with the spaces that join it to the words on either side of the cursor.
fn spaced(transcript: &engine::Transcript) -> String {
    let text = &transcript.text;
    let front = transcript.leading_space == Some(true) && !text.starts_with(char::is_whitespace);
    let back = transcript.trailing_space == Some(true) && !text.ends_with(char::is_whitespace);
    format!("{}{text}{}", if front { " " } else { "" }, if back { " " } else { "" })
}

fn deliver(app: &AppHandle, text: String, last_paste: &mut Option<String>) -> DictationEvent {
    let delivery = paste::insert(app, &text);
    remember_paste(last_paste, &delivery, &text);
    finish_delivery(delivery, text)
}

/// Only a paste can be taken back: text that was copied to the clipboard instead was
/// never typed anywhere, and undoing a failed paste would delete the user's own words.
fn remember_paste(
    last_paste: &mut Option<String>,
    delivery: &Result<paste::Delivery, String>,
    text: &str,
) {
    *last_paste = match delivery {
        Ok(paste::Delivery::Pasted) => Some(text.to_string()),
        _ => None,
    };
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
    let settings = app.state::<SettingsStore>().get();
    if settings.mute_other_audio || settings.pause_media {
        engine::quiet_other_audio(settings.mute_other_audio, settings.pause_media);
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
    float_overlay(app);
}

// Raised again on every recording, since switching Spaces or entering full screen can
// leave the pill ordered behind the app in front.
fn float_overlay(app: &AppHandle) {
    let Some(overlay) = app.get_webview_window("overlay") else {
        return;
    };
    let _ = app.run_on_main_thread(move || {
        if let Ok(window) = overlay.ns_window() {
            engine::float_overlay(window);
        }
    });
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
    if let Ok(icon) = tray_image(bytes) {
        let _ = tray.set_icon(Some(icon));
        let _ = tray.set_icon_as_template(true);
    }
    let _ = tray.set_tooltip(Some(if recording { "Parla — listening" } else { "Parla" }));
}

/// The tray icons are black template images, which macOS recolors for the menu bar.
/// Windows and Linux draw icons as they are, on dark taskbars, so they are made white.
fn tray_image(bytes: &[u8]) -> tauri::Result<Image<'static>> {
    let image = Image::from_bytes(bytes)?;
    if cfg!(target_os = "macos") {
        return Ok(image.to_owned());
    }
    let mut rgba = image.rgba().to_vec();
    for pixel in rgba.chunks_exact_mut(4) {
        pixel[..3].fill(255);
    }
    Ok(Image::new_owned(rgba, image.width(), image.height()))
}

const TRANSFORM_PREFIX: &str = "transform:";
const RECENT_PREFIX: &str = "recent:";
const MIC_PREFIX: &str = "mic:";
const TOGGLE_PREFIX: &str = "toggle:";
const RECENT_COUNT: usize = 5;
const RECENT_LABEL_CHARS: usize = 48;

/// "⌥ Option", "⌃ Control + Space": the shortcut as the menu shows it, or None when it
/// uses a key the menu has no name for.
fn hotkey_label(hotkey: &settings::Hotkey) -> Option<String> {
    let name = |group: &Vec<u16>| -> Option<&'static str> {
        let has = |codes: &[u16]| group.iter().all(|code| codes.contains(code));
        Some(if has(&[54, 55]) {
            "⌘ Command"
        } else if has(&[56, 60]) {
            "⇧ Shift"
        } else if has(&[58, 61]) {
            "⌥ Option"
        } else if has(&[59, 62]) {
            "⌃ Control"
        } else if has(&[63]) {
            "fn"
        } else if has(&[49]) {
            "Space"
        } else {
            return None;
        })
    };
    let names = hotkey.groups.iter().map(name).collect::<Option<Vec<_>>>()?;
    (!names.is_empty()).then(|| names.join(" + "))
}

/// A dictation shortened to one menu line.
fn menu_line(text: &str) -> String {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() <= RECENT_LABEL_CHARS {
        return line;
    }
    let cut: String = line.chars().take(RECENT_LABEL_CHARS - 1).collect();
    format!("{}…", cut.trim_end())
}

// Everything worth reaching without opening the window: the last dictations, the
// transforms, the microphone and the switches people flip most. Opening the menu leaves
// the app in front, and its selection, untouched, so paste and transforms land there.
fn tray_menu<R: tauri::Runtime, M: Manager<R>>(
    manager: &M,
    settings: &Settings,
    recent: &[Entry],
) -> tauri::Result<Menu<R>> {
    use tauri::menu::{CheckMenuItem, IsMenuItem};
    let separator = || PredefinedMenuItem::separator(manager);
    let hint = MenuItem::with_id(
        manager,
        "hint",
        hotkey_label(&settings.hotkey)
            .map(|keys| format!("Hold {keys} to Dictate"))
            .unwrap_or_else(|| "Hold Your Shortcut to Dictate".into()),
        false,
        None::<&str>,
    )?;

    let paste_last =
        MenuItem::with_id(manager, "paste-last", "Paste Last Dictation", !recent.is_empty(), None::<&str>)?;
    let recent_items = recent
        .iter()
        .take(RECENT_COUNT)
        .enumerate()
        .map(|(index, entry)| {
            MenuItem::with_id(manager, format!("{RECENT_PREFIX}{index}"), menu_line(&entry.text), true, None::<&str>)
        })
        .collect::<tauri::Result<Vec<_>>>()?;
    let copy_hint = MenuItem::with_id(manager, "copy-hint", "Click One to Copy It", false, None::<&str>)?;
    let mut recent_refs: Vec<&dyn IsMenuItem<R>> = vec![&copy_hint];
    recent_refs.extend(recent_items.iter().map(|item| item as &dyn IsMenuItem<R>));
    let recent_menu = Submenu::with_items(manager, "Recent Dictations", !recent_items.is_empty(), &recent_refs)?;

    let transform_items = settings
        .transforms
        .iter()
        .enumerate()
        .map(|(index, transform)| {
            MenuItem::with_id(manager, format!("{TRANSFORM_PREFIX}{index}"), &transform.name, true, None::<&str>)
        })
        .collect::<tauri::Result<Vec<_>>>()?;
    let transform_refs: Vec<&dyn IsMenuItem<R>> =
        transform_items.iter().map(|item| item as &dyn IsMenuItem<R>).collect();
    let transforms = Submenu::with_items(manager, "Transform Selection", !transform_items.is_empty(), &transform_refs)?;

    let chosen = settings.input_device.as_deref();
    let devices = engine::input_devices();
    let default_mic = CheckMenuItem::with_id(manager, MIC_PREFIX, "System Default", true, chosen.is_none(), None::<&str>)?;
    let mic_items = devices
        .iter()
        .map(|device| {
            CheckMenuItem::with_id(
                manager,
                format!("{MIC_PREFIX}{}", device.uid),
                &device.name,
                true,
                chosen == Some(device.uid.as_str()),
                None::<&str>,
            )
        })
        .collect::<tauri::Result<Vec<_>>>()?;
    let mut mic_refs: Vec<&dyn IsMenuItem<R>> = vec![&default_mic];
    mic_refs.extend(mic_items.iter().map(|item| item as &dyn IsMenuItem<R>));
    let microphone = Submenu::with_items(manager, "Microphone", true, &mic_refs)?;

    let toggle = |name: &str, label: &str, on: bool| {
        CheckMenuItem::with_id(manager, format!("{TOGGLE_PREFIX}{name}"), label, true, on, None::<&str>)
    };
    let clean_up = toggle("enhance", "Clean Up Text", settings.enhance)?;
    let match_app = toggle("match-app", "Match the App", settings.app_aware_tone)?;
    let pause_media = toggle("pause-media", "Pause Music While Dictating", settings.pause_media)?;
    let whisper_mode = toggle("soft-voice", "Whisper Mode", settings.soft_voice)?;

    let open = MenuItem::with_id(manager, "open", "Open Parla", true, None::<&str>)?;
    let preferences = MenuItem::with_id(manager, "settings", "Settings…", true, Some("CmdOrCtrl+,"))?;
    let updates = MenuItem::with_id(manager, "updates", "Check for Updates…", true, None::<&str>)?;
    let quit = MenuItem::with_id(manager, "quit", "Quit Parla", true, Some("CmdOrCtrl+Q"))?;

    Menu::with_items(
        manager,
        &[
            &hint,
            &separator()?,
            &paste_last,
            &recent_menu,
            &transforms,
            &separator()?,
            &microphone,
            &clean_up,
            &match_app,
            &pause_media,
            &whisper_mode,
            &separator()?,
            &open,
            &preferences,
            &updates,
            &separator()?,
            &quit,
        ],
    )
}

/// Rebuilt whenever what it shows changes: settings, history, or a switch in the menu.
fn refresh_tray(app: &AppHandle) {
    let settings = app.state::<SettingsStore>().get();
    let recent: Vec<Entry> = app.state::<HistoryStore>().list().into_iter().take(RECENT_COUNT).collect();
    if let (Some(tray), Ok(menu)) = (app.tray_by_id("parla"), tray_menu(app, &settings, &recent)) {
        let _ = tray.set_menu(Some(menu));
    }
}

fn send_to_worker(app: &AppHandle, command: Command) {
    let worker = app.state::<Worker>();
    let sender = worker.0.lock().unwrap_or_else(|e| e.into_inner());
    let _ = sender.send(command);
}

/// Opens the window on a page, for the menu's Settings and Check for Updates.
fn show_page(app: &AppHandle, page: &str) {
    show_main(app);
    let _ = app.emit("navigate", page);
}

fn change_settings(app: &AppHandle, change: impl FnOnce(&mut Settings)) {
    let store = app.state::<SettingsStore>();
    let previous = store.get();
    let mut next = previous.clone();
    change(&mut next);
    if store.set(next.clone()).is_ok() {
        if next.input_device != previous.input_device {
            engine::warm_microphone(next.input_device.as_deref());
        }
        let _ = app.emit("settings-changed", ());
    }
    refresh_tray(app);
}

fn on_tray_menu(app: &AppHandle, id: &str) {
    match id {
        "open" => show_main(app),
        "settings" => show_page(app, "settings"),
        "updates" => show_page(app, "settings"),
        "quit" => app.exit(0),
        "paste-last" => send_to_worker(app, Command::Repeat),
        _ => {}
    }
    if let Some(index) = id.strip_prefix(RECENT_PREFIX).and_then(|i| i.parse::<usize>().ok()) {
        if let Some(entry) = app.state::<HistoryStore>().list().get(index) {
            let _ = app.clipboard().write_text(entry.text.clone());
        }
    } else if let Some(index) = id.strip_prefix(TRANSFORM_PREFIX).and_then(|i| i.parse::<usize>().ok()) {
        if let Some(transform) = app.state::<SettingsStore>().get().transforms.get(index) {
            send_to_worker(app, Command::Transform(transform.instruction.clone()));
        }
    } else if let Some(uid) = id.strip_prefix(MIC_PREFIX) {
        let uid = (!uid.is_empty()).then(|| uid.to_string());
        change_settings(app, |settings| settings.input_device = uid);
    } else if let Some(name) = id.strip_prefix(TOGGLE_PREFIX) {
        change_settings(app, |settings| match name {
            "enhance" => settings.enhance = !settings.enhance,
            "match-app" => settings.app_aware_tone = !settings.app_aware_tone,
            "pause-media" => settings.pause_media = !settings.pause_media,
            "soft-voice" => settings.soft_voice = !settings.soft_voice,
            _ => {}
        });
    }
}

fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    let settings = app.state::<SettingsStore>().get();
    let recent: Vec<Entry> = app.state::<HistoryStore>().list().into_iter().take(RECENT_COUNT).collect();
    let menu = tray_menu(app, &settings, &recent)?;

    TrayIconBuilder::with_id("parla")
        .icon(tray_image(TRAY_ICON)?)
        .icon_as_template(true)
        .tooltip("Parla")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| on_tray_menu(app, event.id.as_ref()))
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
            app.manage(SuggestionStore::load(&handle));
            app.manage(CaptureState::default());
            let worker = spawn_worker(handle.clone());
            app.manage(Worker(Mutex::new(worker.clone())));
            hotkey::spawn(handle.clone(), worker);

            if let Some(overlay) = app.get_webview_window("overlay") {
                let _ = overlay.set_ignore_cursor_events(true);
            }
            float_overlay(app.handle());
            build_tray(app)?;
            // Asking at launch is what registers Parla in System Settings' Accessibility
            // list; macOS only shows its own prompt the first time.
            if !paste::accessibility_granted() {
                engine::request_accessibility();
            }
            engine::prepare();
            engine::warm_microphone(app.state::<SettingsStore>().get().input_device.as_deref());
            if app.state::<SettingsStore>().get().speech_model == SpeechModel::Whisper {
                thread::spawn(engine::prepare_whisper);
            }
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
            set_launch_at_login,
            suggestions_list,
            suggestion_resolve,
            check_for_update,
            open_release,
            install_update
        ])
        .build(tauri::generate_context!())
        .expect("error while building Parla");

    app.run(|app, event| match event {
        tauri::RunEvent::Reopen { .. } => show_main(app),
        // Quitting mid-dictation would otherwise leave the Mac silent.
        tauri::RunEvent::Exit => {
            engine::duck_audio(false);
            engine::shutdown();
        }
        _ => {}
    });
}

#[cfg(test)]
mod tests {
    use super::{engine::Transcript, newer, spaced, version_in_file_name};

    #[test]
    fn compares_versions_as_numbers() {
        assert!(newer("1.10.0", "1.9.2"));
        assert!(newer("1.3.1", "1.3.0"));
        assert!(!newer("1.3.0", "1.3.0"));
        assert!(!newer("1.2.1", "1.3.0"));
    }

    #[test]
    fn reads_the_version_from_the_dmg_name() {
        let url = "https://github.com/BryanParreira/Parla/releases/download/v1.2.1/Parla_1.3.0_aarch64.dmg";
        assert_eq!(version_in_file_name(url).as_deref(), Some("1.3.0"));
        assert_eq!(version_in_file_name("https://x/Parla.dmg"), None);
    }

    #[test]
    fn joins_the_dictation_to_the_words_around_the_cursor() {
        let transcript = |json: &str| serde_json::from_str::<Transcript>(json).unwrap();
        assert_eq!(spaced(&transcript(r#"{"text":"banana"}"#)), "banana");
        assert_eq!(
            spaced(&transcript(r#"{"text":"banana","leadingSpace":true,"trailingSpace":true}"#)),
            " banana "
        );
        assert_eq!(spaced(&transcript(r#"{"text":"banana ","trailingSpace":true}"#)), "banana ");
    }
}
