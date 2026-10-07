pub mod cli;
mod control;
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
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
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
use settings::{CleanupLevel, OtherAudio, Settings, SettingsStore, SpeechModel, Transform};
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
    /// Start a dictation in this mode, from the mode's own shortcut.
    StartMode(String),
    Stop(Mode),
    Cancel(Mode),
    /// Esc: cancels a recording, asking again first when it has run a while.
    Escape,
    /// Start a dictation if none is running, else stop it: deep links and menu bar clicks.
    Toggle,
    /// Start a dictation unless one is running.
    Begin,
    /// Stop the dictation running, if any.
    End,
    /// The mode shortcut: the next mode, for this recording or the next one.
    CycleMode,
    /// Make this mode, by id, the one new dictations use.
    SwitchMode(String),
    /// Show a short message in the pill, for a coding agent asking for attention.
    Notice(String),
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
    /// A short message, such as the mode just picked.
    Notice { text: String },
    /// Something longer is under way, like transcribing a file.
    Working { label: String },
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
    settings.sanitize();
    settings.validate()?;
    let store = app.state::<SettingsStore>();
    let previous = store.get();
    store.set(settings.clone())?;
    // Choosing Whisper is what starts its one-time download.
    if settings.speech_model == SpeechModel::Whisper && previous.speech_model != SpeechModel::Whisper {
        thread::spawn(engine::prepare_whisper);
    }
    applied(&app, &previous, &settings);
    Ok(settings)
}

/// What has to happen outside the settings file when a setting changes.
fn applied(app: &AppHandle, previous: &Settings, next: &Settings) {
    if next.input_device != previous.input_device || next.preferred_mics != previous.preferred_mics {
        engine::warm_microphone(microphone(next).as_deref());
    }
    if next.unload_after_minutes != previous.unload_after_minutes {
        engine::set_idle_unload(next.unload_after_minutes);
    }
    if next.keep_audio_days != previous.keep_audio_days {
        history::prune_recordings(&recordings_dir(app), next.keep_audio_days);
        let _ = app.emit("history-changed", ());
    }
    if next.tray_click_records != previous.tray_click_records {
        if let Some(tray) = app.tray_by_id("parla") {
            let _ = tray.set_show_menu_on_left_click(!next.tray_click_records);
        }
    }
    refresh_tray(app);
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
fn history_list(app: AppHandle) -> Vec<Entry> {
    app.state::<HistoryStore>().list_with_audio(&recordings_dir(&app))
}

#[tauri::command]
fn history_delete(app: AppHandle, id: u64) -> Result<(), String> {
    app.state::<HistoryStore>().remove(id)?;
    let _ = std::fs::remove_file(history::recording_path(&recordings_dir(&app), id));
    let _ = app.emit("history-changed", ());
    refresh_tray(&app);
    Ok(())
}

#[tauri::command]
fn history_clear(app: AppHandle) -> Result<(), String> {
    app.state::<HistoryStore>().clear()?;
    history::prune_recordings(&recordings_dir(&app), 0);
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

/// One file at a time: the speech model can only do one thing at once anyway.
static FILE_BUSY: AtomicBool = AtomicBool::new(false);

/// The request that runs a file or a history entry through `mode`.
fn process_request<'a>(settings: &'a Settings, mode: &'a settings::ModeConfig) -> engine::ProcessRequest<'a> {
    engine::ProcessRequest {
        enhance: settings.enhance,
        terms: &settings.dictionary,
        level: settings.cleanup_level.as_str(),
        formatting: false,
        mode,
        whisper: settings.speech_model == SpeechModel::Whisper,
        language: settings.language.as_deref(),
        trim: settings.trim_silence,
    }
}

/// Transcribes an audio or video file through the active mode, keeps it in the history
/// and copies it. Runs on its own thread so dictation keeps answering the keys.
fn transcribe_file(app: &AppHandle, path: PathBuf) {
    if FILE_BUSY.swap(true, Ordering::SeqCst) {
        let _ = app.emit("dictation", DictationEvent::Notice { text: "Already transcribing a file".into() });
        return;
    }
    let app = app.clone();
    thread::spawn(move || {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let done = Arc::new(AtomicBool::new(false));
        place_overlay(&app);
        let reporter = {
            let (app, done, name) = (app.clone(), done.clone(), name.clone());
            thread::spawn(move || {
                while !done.load(Ordering::Relaxed) {
                    let percent = (engine::file_progress() * 100.0).round();
                    let label = if percent > 0.0 {
                        format!("Transcribing {name} · {percent}%")
                    } else {
                        format!("Transcribing {name}…")
                    };
                    set_tray_status(&app, TrayStatus::Processing);
                    let _ = app.emit("dictation", DictationEvent::Working { label });
                    thread::sleep(Duration::from_millis(400));
                }
            })
        };
        let settings = app.state::<SettingsStore>().get();
        let mode = settings.active().clone();
        let result = engine::process_file(&path, &process_request(&settings, &mode));
        done.store(true, Ordering::Relaxed);
        let _ = reporter.join();
        FILE_BUSY.store(false, Ordering::SeqCst);

        let event = match result {
            Ok(transcript) if transcript.text.trim().is_empty() => DictationEvent::Notice {
                text: format!("No speech found in {name}"),
            },
            Ok(transcript) => {
                let history = app.state::<HistoryStore>();
                let id = history.next_id();
                let duration = transcript.audio_ms.unwrap_or_default();
                history.add(id, &transcript, duration, None, None, Some(mode.name.clone()), Some(name));
                let _ = app.emit("history-changed", ());
                refresh_tray(&app);
                match app.clipboard().write_text(transcript.text.clone()) {
                    Ok(()) => DictationEvent::Copied { text: transcript.text },
                    Err(message) => DictationEvent::Error { message: message.to_string() },
                }
            }
            Err(message) => DictationEvent::Error { message },
        };
        set_tray_status(&app, TrayStatus::from_event(&event));
        let _ = app.emit("dictation", event);
    });
}

/// From the History page: a file dropped on the window, or picked with the button.
#[tauri::command]
fn transcribe_path(app: AppHandle, path: Option<String>) {
    thread::spawn(move || {
        let path = match path {
            Some(path) => Some(PathBuf::from(path)),
            None => engine::choose_file("media"),
        };
        if let Some(path) = path {
            transcribe_file(&app, path);
        }
    });
}

/// Runs a history entry through a mode again: from its kept audio when there is some,
/// else from what was transcribed the first time.
#[tauri::command]
async fn history_reprocess(app: AppHandle, id: u64, mode: String) -> Result<Entry, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let history = app.state::<HistoryStore>();
        let entry = history.get(id).ok_or("That dictation is no longer in the history.")?;
        let settings = app.state::<SettingsStore>().get();
        let chosen = settings.mode(&mode).ok_or("That mode no longer exists.")?.clone();
        let request = process_request(&settings, &chosen);
        let audio = history::recording_path(&recordings_dir(&app), id);
        let transcript = if audio.exists() {
            engine::process_file(&audio, &request)?
        } else {
            let original = entry.raw.clone().unwrap_or_else(|| entry.text.clone());
            engine::clean_text(&original, &request)?
        };
        if transcript.text.trim().is_empty() {
            return Err("Nothing came back. The recording may be silent.".into());
        }
        let mut text = transcript.text.clone();
        if settings.spoken_formatting && audio.exists() {
            text = formatting::apply(&text);
        }
        let updated = Entry {
            words: text.split_whitespace().count(),
            raw: transcript.raw.clone().or(entry.raw.clone()).filter(|raw| *raw != text),
            text,
            language: transcript.language.clone().or(entry.language.clone()),
            transcribe_ms: transcript.transcribe_ms.or(entry.transcribe_ms),
            enhance_ms: transcript.enhance_ms,
            style: transcript.style.clone(),
            mode: Some(chosen.name.clone()),
            prompt: transcript.prompt.clone(),
            context: Vec::new(),
            ..entry
        };
        history.replace(updated.clone())?;
        let _ = app.emit("history-changed", ());
        refresh_tray(&app);
        Ok(updated)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// A kept recording as a data URL the History page can play.
#[tauri::command]
fn history_audio(app: AppHandle, id: u64) -> Option<String> {
    let bytes = std::fs::read(history::recording_path(&recordings_dir(&app), id)).ok()?;
    Some(format!("data:audio/wav;base64,{}", control::base64(&bytes)))
}

#[derive(Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Backup {
    app: String,
    version: String,
    created_at: u64,
    settings: Settings,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    history: Option<Vec<Entry>>,
}

/// Saves the settings, modes, words and snippets, and optionally the history, as one
/// file in Downloads.
#[tauri::command]
fn export_backup(app: AppHandle, include_history: bool) -> Result<String, String> {
    let backup = Backup {
        app: "Parla".into(),
        version: app.package_info().version.to_string(),
        created_at: app.state::<HistoryStore>().next_id(),
        settings: app.state::<SettingsStore>().get(),
        history: include_history.then(|| app.state::<HistoryStore>().list()),
    };
    let json = serde_json::to_string_pretty(&backup).map_err(|e| e.to_string())?;
    let stamp = chrono_date();
    save_export(app, format!("Parla backup {stamp}.json"), json)
}

/// Today as YYYY-MM-DD, without pulling in a date library for one file name.
fn chrono_date() -> String {
    let days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() / 86_400)
        .unwrap_or_default() as i64;
    // Howard Hinnant's days-to-civil.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Restores a backup the user picks. The settings go through the same checks as any
/// change; history, when the file has one, is merged in without duplicates.
#[tauri::command]
async fn import_backup(app: AppHandle) -> Result<bool, String> {
    let Some(path) = tauri::async_runtime::spawn_blocking(|| engine::choose_file("json"))
        .await
        .map_err(|e| e.to_string())?
    else {
        return Ok(false);
    };
    let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let backup: Backup = serde_json::from_str(&raw).map_err(|_| "That isn't a Parla backup.".to_string())?;
    if backup.app != "Parla" {
        return Err("That isn't a Parla backup.".into());
    }
    let mut settings = backup.settings;
    // Shortcuts recorded on another kind of keyboard wouldn't mean the same keys.
    settings.onboarded = app.state::<SettingsStore>().get().onboarded;
    update_settings(app.clone(), settings)?;
    if let Some(entries) = backup.history {
        app.state::<HistoryStore>().merge(entries)?;
        let _ = app.emit("history-changed", ());
    }
    let _ = app.emit("settings-changed", ());
    Ok(true)
}

#[tauri::command]
fn open_data_folder(app: AppHandle) -> Result<(), String> {
    let folder = app.path().app_data_dir().map_err(|e| e.to_string())?;
    reveal(&folder)
}

fn reveal(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let opener = "open";
    #[cfg(target_os = "windows")]
    let opener = "explorer";
    #[cfg(target_os = "linux")]
    let opener = "xdg-open";
    std::process::Command::new(opener).arg(path).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Puts `parla` on the PATH by linking to this app's own binary.
#[tauri::command]
fn install_cli() -> Result<String, String> {
    cli::install()
}

#[derive(Serialize)]
struct AgentSetup {
    folder: String,
    commands: Vec<String>,
}

/// Writes the Claude Code plugin to Parla's data folder and returns the commands that
/// install it. Nothing is changed in Claude Code itself until the user runs them.
#[tauri::command]
fn claude_code_setup(app: AppHandle) -> Result<AgentSetup, String> {
    let folder = storage::path(&app, "claude-code");
    cli::write_claude_plugin(&folder)?;
    let shown = folder.to_string_lossy().into_owned();
    Ok(AgentSetup {
        commands: vec![
            format!("claude plugin marketplace add \"{shown}\""),
            "claude plugin install parla@parla".into(),
        ],
        folder: shown,
    })
}

/// parla:// links, for Shortcuts, Raycast, Alfred and scripts, and files opened with
/// Parla from Finder.
fn handle_url(app: &AppHandle, url: &tauri::Url) {
    if url.scheme() == "file" {
        if let Ok(path) = url.to_file_path() {
            transcribe_file(app, path);
        }
        return;
    }
    if url.scheme() != "parla" {
        return;
    }
    let route = format!("{}{}", url.host_str().unwrap_or(""), url.path()).trim_end_matches('/').to_string();
    let query = |key: &str| url.query_pairs().find(|(k, _)| k == key).map(|(_, v)| v.into_owned());
    match route.as_str() {
        "record" | "record/toggle" => send_to_worker(app, Command::Toggle),
        "record/start" => send_to_worker(app, Command::Begin),
        "record/stop" => send_to_worker(app, Command::End),
        "record/cancel" => send_to_worker(app, Command::Cancel(Mode::Dictate)),
        "settings" => show_page(app, "settings"),
        "history" => show_page(app, "history"),
        "modes" => show_page(app, "modes"),
        "paste-last" => send_to_worker(app, Command::Repeat),
        _ => {
            let wanted = query("key").or_else(|| query("name")).or_else(|| route.strip_prefix("mode/").map(str::to_string));
            if route == "mode" || route.starts_with("mode/") {
                if let Some(mode) = wanted.and_then(|name| app.state::<SettingsStore>().get().find_mode(&name).map(|m| m.id.clone())) {
                    send_to_worker(app, Command::SwitchMode(mode));
                }
            } else if route == "transcribe" {
                if let Some(path) = query("path") {
                    transcribe_file(app, PathBuf::from(path));
                }
            }
        }
    }
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
        let mut session = Session::new(app, later);
        loop {
            let command = if session.started.is_some() {
                match rx.recv_timeout(LEVEL_INTERVAL) {
                    Ok(command) => command,
                    Err(RecvTimeoutError::Timeout) => {
                        session.tick();
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
            session.handle(command);
        }
    });

    tx
}

/// Within this long a second Esc throws a long recording away.
const ESC_CONFIRM: Duration = Duration::from_secs(3);
/// Recordings shorter than this are thrown away on the first Esc.
const ESC_ASK_AFTER: Duration = Duration::from_secs(30);

/// The dictation worker's state between commands.
struct Session {
    app: AppHandle,
    later: mpsc::Sender<Command>,
    // The mode travels with every stop and cancel, so letting go of one shortcut can
    // never end, or paste, a recording the other one started.
    started: Option<(Instant, Mode)>,
    // Read when recording starts, while the app being dictated into is still the one
    // in front; by the time it ends the user may have switched away.
    target_app: Option<String>,
    /// The writing mode this recording is in, by id, once the engine has picked it.
    mode: Option<String>,
    /// Where this recording's audio is kept, and the history id it is kept under.
    audio: Option<(u64, PathBuf)>,
    live: bool,
    last_partial: String,
    // What undo would delete. Taken rather than read, so one press can never eat
    // text Parla did not paste.
    last_paste: Option<String>,
    /// When Esc was pressed once on a long recording, asking for a second press.
    esc_pending: Option<Instant>,
}

impl Session {
    fn new(app: AppHandle, later: mpsc::Sender<Command>) -> Self {
        Self {
            app,
            later,
            started: None,
            target_app: None,
            mode: None,
            audio: None,
            live: false,
            last_partial: String::new(),
            last_paste: None,
            esc_pending: None,
        }
    }

    fn emit(&self, event: DictationEvent) {
        set_tray_status(&self.app, TrayStatus::from_event(&event));
        let _ = self.app.emit("dictation", event);
    }

    fn settings(&self) -> Settings {
        self.app.state::<SettingsStore>().get()
    }

    /// Runs while recording: the level meter and the live preview.
    fn tick(&mut self) {
        let _ = self.app.emit_to("overlay", "level", engine::level());
        if self.live {
            let partial = engine::partial();
            if partial != self.last_partial {
                let _ = self.app.emit_to("overlay", "partial", &partial);
                self.last_partial = partial;
            }
        }
    }

    fn handle(&mut self, command: Command) {
        // Links, the CLI and a menu bar click start and stop without saying which kind of
        // recording is running; they mean the dictation one.
        let command = match (command, self.started) {
            (Command::Toggle | Command::Begin, None) => Command::Start(Mode::Dictate),
            (Command::Toggle | Command::End, Some((_, mode))) => Command::Stop(mode),
            (command, _) => command,
        };
        match (command, self.started) {
            (Command::Start(mode), None) => self.start(mode, None),
            (Command::StartMode(id), None) => self.start(Mode::Dictate, Some(id)),
            (Command::Stop(mode), Some((at, active))) if mode == active => self.stop(mode, at),
            (Command::Cancel(mode), Some((_, active))) if mode == active => self.cancel(),
            (Command::Escape, Some((at, _))) => self.escape(at),
            (Command::CycleMode, _) => self.cycle_mode(),
            (Command::SwitchMode(id), _) => self.switch_mode(&id),
            (Command::Notice(text), _) => {
                place_overlay(&self.app);
                play_cue(&self.app, true);
                let _ = self.app.emit("dictation", DictationEvent::Notice { text });
            }
            (Command::Repeat, None) => match self.app.state::<HistoryStore>().latest() {
                Some(entry) => {
                    let event = deliver(&self.app, entry.text, &mut self.last_paste);
                    self.emit(event)
                }
                None => self.emit(DictationEvent::Empty),
            },
            // Only ever offered for text Parla itself pasted, and only once, so a
            // second press can't start deleting what the user wrote.
            (Command::Undo, None) => match self.last_paste.take() {
                Some(text) => self.emit(match paste::undo(&text) {
                    Ok(()) => DictationEvent::Undone,
                    Err(message) => DictationEvent::Error { message },
                }),
                None => self.emit(DictationEvent::Empty),
            },
            // The menu comes first and the selection is read after it, by which
            // time the shortcut's keys are up and can't bend the copy keystroke.
            (Command::PickTransform, None) => {
                let transforms = self.settings().transforms;
                let names: Vec<&str> = transforms.iter().map(|t| t.name.as_str()).collect();
                if let Some(transform) = engine::pick(&names).and_then(|index| transforms.get(index)) {
                    place_overlay(&self.app);
                    self.emit(DictationEvent::Processing);
                    let event = apply_instruction(&self.app, &transform.instruction, &mut self.last_paste);
                    self.emit(event);
                }
            }
            (Command::Transform(instruction), None) => {
                place_overlay(&self.app);
                self.emit(DictationEvent::Processing);
                let event = apply_instruction(&self.app, &instruction, &mut self.last_paste);
                self.emit(event);
            }
            // Only the field the dictation went into is worth reading, and never a
            // password field or a sensitive app.
            (Command::Learn { pasted, app: dictated_into }, _) => {
                let settings = self.settings();
                if settings.learn_words
                    && dictated_into.is_some()
                    && engine::frontmost_app() == dictated_into
                    && engine::sensitive_context().is_none()
                {
                    let fixes = learn::corrections(&pasted, &engine::focused_text());
                    if self.app.state::<SuggestionStore>().record(&fixes, &settings.dictionary) {
                        let _ = self.app.emit("suggestions-changed", ());
                    }
                }
            }
            _ => {}
        }
    }

    fn start(&mut self, mode: Mode, forced: Option<String>) {
        let settings = self.settings();
        // Checked before the microphone opens, so a refused recording never
        // reaches the Neural Engine at all.
        if let Some(reason) = settings.pause_in_sensitive_apps.then(engine::sensitive_context).flatten() {
            self.emit(DictationEvent::Blocked { reason });
            return;
        }
        let microphone = microphone(&settings);
        let options = engine_options(&settings, mode, microphone.as_deref(), forced.as_deref());
        if let Err(message) = engine::start(&options) {
            self.emit(DictationEvent::Error { message });
            return;
        }
        self.started = Some((Instant::now(), mode));
        self.live = settings.live_preview;
        self.last_partial.clear();
        self.esc_pending = None;
        self.mode = None;
        // Audio is only kept for dictations, under the id their history entry will get.
        self.audio = (mode == Mode::Dictate && settings.keep_audio_days > 0).then(|| {
            let id = self.app.state::<HistoryStore>().next_id();
            (id, history::recording_path(&recordings_dir(&self.app), id))
        });
        // The pill and the sound come the moment the microphone is live; reading the
        // app and cursor happens while the user is already talking.
        place_overlay(&self.app);
        play_cue(&self.app, true);
        self.emit(DictationEvent::Recording { command: mode == Mode::Command });
        duck_other_audio(&self.app);
        self.target_app = engine::frontmost_app();
        if let Some(reply) = engine::start_context(&options) {
            self.show_mode(&settings, &reply, forced.is_some());
            self.mode = Some(reply.mode);
        }
    }

    /// Tells the pill which mode is listening, unless it is the plain Default one, and
    /// what context it picked up.
    fn show_mode(&self, settings: &Settings, reply: &engine::ContextReply, always: bool) {
        if let Some(mode) = settings.mode(&reply.mode) {
            if always || mode.id != settings::DEFAULT_MODE {
                let _ = self.app.emit_to("overlay", "mode", &mode.name);
            }
        }
        if !reply.captured.is_empty() {
            let _ = self.app.emit_to("overlay", "context", &reply.captured);
        }
    }

    fn stop(&mut self, mode: Mode, at: Instant) {
        let released = Instant::now();
        let settings = self.settings();
        // Read before anything slow happens, while the user still holds it.
        let send = mode == Mode::Dictate
            && settings.shift_to_send
            && keys::held().shift()
            && !settings.hotkey.keys().iter().any(|key| keys::SHIFT.contains(key));
        self.started = None;
        engine::duck_audio(false);
        play_cue(&self.app, false);
        self.emit(DictationEvent::Processing);
        let duration_ms = at.elapsed().as_millis() as u64;
        let audio = self.audio.take();

        match (engine::stop(audio.as_ref().map(|(_, path)| path.as_path())), mode) {
            (Ok(transcript), _) if transcript.text.trim().is_empty() => {
                if let Some((_, path)) = &audio {
                    let _ = std::fs::remove_file(path);
                }
                self.emit(DictationEvent::Empty)
            }
            (Ok(mut transcript), Mode::Dictate) => {
                if settings.spoken_formatting {
                    transcript.text = formatting::apply(&transcript.text);
                }
                transcript.text = snippets::expand(&transcript.text, &settings.snippets);
                // The spaces are typed but not kept: history holds the dictation
                // itself, not where it happened to land.
                let typed = spaced(&transcript);
                // Paste before bookkeeping so the text lands as soon as it exists.
                let delivery = paste::insert(&self.app, &typed);
                let latency_ms = released.elapsed().as_millis() as u64;
                if send && matches!(delivery, Ok(paste::Delivery::Pasted)) {
                    let _ = paste::press_return();
                }
                let app_name = self.target_app.take();
                let mode_name = transcript
                    .mode
                    .as_deref()
                    .or(self.mode.as_deref())
                    .and_then(|id| settings.mode(id))
                    .map(|mode| mode.name.clone());
                let history = self.app.state::<HistoryStore>();
                let id = audio.as_ref().map_or_else(|| history.next_id(), |(id, _)| *id);
                history.add(id, &transcript, duration_ms, Some(latency_ms), app_name.clone(), mode_name, None);
                if settings.keep_audio_days > 0 {
                    history::prune_recordings(&recordings_dir(&self.app), settings.keep_audio_days);
                }
                let _ = self.app.emit("history-changed", ());
                refresh_tray(&self.app);
                remember_paste(&mut self.last_paste, &delivery, &typed);
                if settings.learn_words && matches!(delivery, Ok(paste::Delivery::Pasted)) {
                    let later = self.later.clone();
                    let pasted = transcript.text.clone();
                    thread::spawn(move || {
                        thread::sleep(LEARN_DELAY);
                        let _ = later.send(Command::Learn { pasted, app: app_name });
                    });
                }
                self.emit(finish_delivery(delivery, transcript.text));
            }
            (Ok(transcript), Mode::Command) => {
                let event = apply_command(&self.app, &transcript.text, &mut self.last_paste);
                self.emit(event)
            }
            (Err(message), _) => self.emit(DictationEvent::Error { message }),
        }
    }

    fn cancel(&mut self) {
        self.started = None;
        self.target_app = None;
        self.esc_pending = None;
        if let Some((_, path)) = self.audio.take() {
            let _ = std::fs::remove_file(path);
        }
        engine::duck_audio(false);
        engine::cancel();
        self.emit(DictationEvent::Cancelled);
    }

    /// Esc throws a recording away. One that has run a while is worth a second press,
    /// so a stray Esc can't lose minutes of talking.
    fn escape(&mut self, at: Instant) {
        let confirmed = self.esc_pending.is_some_and(|asked| asked.elapsed() <= ESC_CONFIRM);
        if at.elapsed() < ESC_ASK_AFTER || confirmed {
            self.cancel();
            hotkey::reset();
        } else {
            self.esc_pending = Some(Instant::now());
            let _ = self.app.emit_to("overlay", "notice", "Press Esc again to discard");
        }
    }

    /// The mode shortcut: mid-recording it moves this dictation to the next mode;
    /// otherwise it changes the mode new dictations use.
    fn cycle_mode(&mut self) {
        let settings = self.settings();
        let current = match (self.started, &self.mode) {
            (Some((_, Mode::Dictate)), Some(id)) => id.clone(),
            (Some(_), _) => return,
            (None, _) => settings.active_mode.clone(),
        };
        let next = settings.next_mode(&current).id.clone();
        self.switch_mode(&next);
    }

    fn switch_mode(&mut self, id: &str) {
        let settings = self.settings();
        let Some(mode) = settings.mode(id) else { return };
        if matches!(self.started, Some((_, Mode::Dictate))) {
            let microphone = microphone(&settings);
            let options = engine_options(&settings, Mode::Dictate, microphone.as_deref(), Some(id));
            if let Some(reply) = engine::start_context(&options) {
                self.show_mode(&settings, &reply, true);
                self.mode = Some(reply.mode);
            }
            return;
        }
        let name = mode.name.clone();
        let id = mode.id.clone();
        change_settings(&self.app, |settings| settings.active_mode = id);
        place_overlay(&self.app);
        let _ = self.app.emit("dictation", DictationEvent::Notice { text: format!("{name} mode") });
    }
}

/// The microphone to record from. Plugged-in devices are only looked up when the user
/// ranked some, so the usual start costs nothing extra.
fn microphone(settings: &Settings) -> Option<String> {
    if settings.input_device.is_some() || settings.preferred_mics.is_empty() {
        return settings.input_device.clone();
    }
    settings.microphone(&engine::input_devices())
}

fn recordings_dir(app: &AppHandle) -> PathBuf {
    storage::path(app, "recordings")
}

fn engine_options<'a>(
    settings: &'a Settings,
    mode: Mode,
    device: Option<&'a str>,
    forced: Option<&'a str>,
) -> engine::StartOptions<'a> {
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
            device,
            level: settings.cleanup_level.as_str(),
            use_context: settings.use_context,
            formatting: settings.spoken_formatting,
            whisper,
            language,
            soft_voice: settings.soft_voice,
            trim: settings.trim_silence,
            boost: settings.boost_input,
            modes: &settings.modes,
            mode_rules: settings::mode_rules_text(&settings.modes),
            active_mode: &settings.active_mode,
            forced_mode: forced,
        },
        // The spoken instruction is used word for word, so cleanup, the dictionary,
        // context, tone matching and modes would only get in its way.
        Mode::Command => engine::StartOptions {
            enhance: false,
            quick: false,
            dictionary: &[],
            match_app: false,
            rules: String::new(),
            live: settings.live_preview,
            device,
            level: CleanupLevel::default().as_str(),
            use_context: false,
            formatting: false,
            whisper,
            language,
            soft_voice: settings.soft_voice,
            trim: settings.trim_silence,
            boost: settings.boost_input,
            modes: &settings.modes[..1],
            mode_rules: String::new(),
            active_mode: settings::DEFAULT_MODE,
            forced_mode: Some(settings::DEFAULT_MODE),
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
    let quiet = settings.mute_other_audio && settings.other_audio != OtherAudio::Keep;
    if quiet || settings.pause_media {
        engine::quiet_other_audio(quiet, settings.pause_media, settings.other_audio == OtherAudio::Lower);
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
    // Elsewhere the window's always-on-top setting is all there is.
    #[cfg(target_os = "macos")]
    let _ = app.run_on_main_thread(move || {
        if let Ok(window) = overlay.ns_window() {
            engine::float_overlay(window);
        }
    });
    #[cfg(not(target_os = "macos"))]
    let _ = overlay;
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
/// How long the menu bar shows a dictation has landed.
const TRAY_DONE: Duration = Duration::from_secs(2);

/// What the menu bar icon says Parla is doing, as a coloured dot on the icon.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TrayStatus {
    Idle,
    /// The speech model is still loading: yellow.
    Loading,
    /// Recording: red.
    Recording,
    /// Transcribing or cleaning up: blue.
    Processing,
    /// Typed or copied a moment ago: green.
    Done,
    /// Leave the icon as it is.
    Unchanged,
}

impl TrayStatus {
    fn from_event(event: &DictationEvent) -> Self {
        match event {
            DictationEvent::Recording { .. } => Self::Recording,
            DictationEvent::Processing | DictationEvent::Working { .. } => Self::Processing,
            DictationEvent::Done { .. } | DictationEvent::Copied { .. } => Self::Done,
            DictationEvent::Notice { .. } => Self::Unchanged,
            _ => Self::Idle,
        }
    }

    fn dot(self) -> Option<[u8; 3]> {
        match self {
            Self::Loading => Some([0xF2, 0xB8, 0x3A]),
            Self::Recording => Some([0xF0, 0x4A, 0x45]),
            Self::Processing => Some([0x3B, 0x8E, 0xF5]),
            Self::Done => Some([0x34, 0xC7, 0x59]),
            Self::Idle | Self::Unchanged => None,
        }
    }

    fn tooltip(self) -> &'static str {
        match self {
            Self::Loading => "Parla — loading the speech model",
            Self::Recording => "Parla — listening",
            Self::Processing => "Parla — transcribing",
            _ => "Parla",
        }
    }
}

/// Bumped on every change, so a delayed return to idle can't undo a newer status.
static TRAY_GENERATION: AtomicU64 = AtomicU64::new(0);
static TRAY_LOADING: AtomicBool = AtomicBool::new(false);
/// Whether the icon is showing idle (or loading), as opposed to a dictation's status.
static TRAY_IDLE: AtomicBool = AtomicBool::new(true);

// The overlay can sit on another display or behind a full-screen app, so the menu bar
// also shows when the microphone is live, and what Parla is doing.
fn set_tray_status(app: &AppHandle, status: TrayStatus) {
    if status == TrayStatus::Unchanged {
        return;
    }
    let generation = TRAY_GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
    draw_tray(app, status);
    if status == TrayStatus::Done {
        let app = app.clone();
        thread::spawn(move || {
            thread::sleep(TRAY_DONE);
            if TRAY_GENERATION.load(Ordering::Relaxed) == generation {
                draw_tray(&app, TrayStatus::Idle);
            }
        });
    }
}

fn draw_tray(app: &AppHandle, status: TrayStatus) {
    let Some(tray) = app.tray_by_id("parla") else {
        return;
    };
    TRAY_IDLE.store(status == TrayStatus::Idle, Ordering::Relaxed);
    // Idle while the model loads still says so.
    let status = if status == TrayStatus::Idle && TRAY_LOADING.load(Ordering::Relaxed) {
        TrayStatus::Loading
    } else {
        status
    };
    let bytes = if status == TrayStatus::Recording { TRAY_RECORDING_ICON } else { TRAY_ICON };
    match status.dot() {
        None => {
            if let Ok(icon) = tray_image(bytes) {
                let _ = tray.set_icon(Some(icon));
                let _ = tray.set_icon_as_template(true);
            }
        }
        Some(color) => {
            if let Ok(icon) = tray_status_image(bytes, color) {
                let _ = tray.set_icon(Some(icon));
                let _ = tray.set_icon_as_template(false);
            }
        }
    }
    let _ = tray.set_tooltip(Some(status.tooltip()));
}

/// The icon with a coloured dot in its bottom-right corner. A coloured image can't be
/// a template one, so the glyph is drawn in the menu bar's own text colour instead.
fn tray_status_image(bytes: &[u8], color: [u8; 3]) -> tauri::Result<Image<'static>> {
    let image = Image::from_bytes(bytes)?;
    let (width, height) = (image.width() as f32, image.height() as f32);
    let light = !cfg!(target_os = "macos") || engine::menu_bar_dark();
    let ink = if light { 255 } else { 0 };
    let radius = width * 0.17;
    let ring = radius + width * 0.05;
    let (cx, cy) = (width - radius - 1.0, height - radius - 1.0);
    let mut rgba = image.rgba().to_vec();
    for (index, pixel) in rgba.chunks_exact_mut(4).enumerate() {
        let x = (index as f32 % width) + 0.5;
        let y = (index as f32 / width).floor() + 0.5;
        let distance = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
        if distance <= radius {
            let edge = (radius - distance).clamp(0.0, 1.0);
            pixel[..3].copy_from_slice(&color);
            pixel[3] = (edge * 255.0) as u8;
        } else if distance <= ring {
            pixel[3] = 0;
        } else {
            pixel[..3].fill(ink);
        }
    }
    Ok(Image::new_owned(rgba, image.width(), image.height()))
}

/// Shows the menu bar loading until the speech model is ready.
fn watch_model_loading(app: &AppHandle) {
    if engine::status().state == "ready" {
        return;
    }
    TRAY_LOADING.store(true, Ordering::Relaxed);
    draw_tray(app, TrayStatus::Idle);
    let app = app.clone();
    thread::spawn(move || {
        while engine::status().state != "ready" {
            thread::sleep(Duration::from_secs(1));
        }
        TRAY_LOADING.store(false, Ordering::Relaxed);
        // A dictation's own status is left for it to clear.
        if TRAY_IDLE.load(Ordering::Relaxed) {
            draw_tray(&app, TrayStatus::Idle);
        }
    });
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
const MODE_PREFIX: &str = "mode:";
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

    let mode_items = settings
        .modes
        .iter()
        .map(|mode| {
            CheckMenuItem::with_id(
                manager,
                format!("{MODE_PREFIX}{}", mode.id),
                &mode.name,
                true,
                mode.id == settings.active_mode,
                None::<&str>,
            )
        })
        .collect::<tauri::Result<Vec<_>>>()?;
    let mode_refs: Vec<&dyn IsMenuItem<R>> = mode_items.iter().map(|item| item as &dyn IsMenuItem<R>).collect();
    let modes = Submenu::with_items(
        manager,
        format!("Mode: {}", settings.active().name),
        true,
        &mode_refs,
    )?;
    let transcribe = MenuItem::with_id(manager, "transcribe-file", "Transcribe File…", cfg!(target_os = "macos"), None::<&str>)?;

    let chosen = settings.input_device.as_deref();
    // Devices the user hid never show, unless one of them is the chosen one.
    let devices: Vec<engine::InputDevice> = engine::input_devices()
        .into_iter()
        .filter(|device| !settings.hidden_mics.contains(&device.uid) || chosen == Some(device.uid.as_str()))
        .collect();
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
            &modes,
            &transcribe,
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
    next.sanitize();
    if next.validate().is_ok() && store.set(next.clone()).is_ok() {
        applied(app, &previous, &next);
        let _ = app.emit("settings-changed", ());
    } else {
        refresh_tray(app);
    }
}

fn on_tray_menu(app: &AppHandle, id: &str) {
    match id {
        "open" => show_main(app),
        "settings" => show_page(app, "settings"),
        "updates" => show_page(app, "settings"),
        "quit" => app.exit(0),
        "paste-last" => send_to_worker(app, Command::Repeat),
        "transcribe-file" => {
            let app = app.clone();
            // The file panel waits for the user; the menu's own thread can't.
            thread::spawn(move || {
                if let Some(path) = engine::choose_file("media") {
                    transcribe_file(&app, path);
                }
            });
        }
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
    } else if let Some(mode) = id.strip_prefix(MODE_PREFIX) {
        send_to_worker(app, Command::SwitchMode(mode.to_string()));
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
        .show_menu_on_left_click(!settings.tray_click_records)
        .on_menu_event(|app, event| on_tray_menu(app, event.id.as_ref()))
        .on_tray_icon_event(|tray, event| {
            use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                let app = tray.app_handle();
                if app.state::<SettingsStore>().get().tray_click_records {
                    send_to_worker(app, Command::Toggle);
                }
            }
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
            control::serve(handle.clone());
            // Asking at launch is what registers Parla in System Settings' Accessibility
            // list; macOS only shows its own prompt the first time.
            if !paste::accessibility_granted() {
                engine::request_accessibility();
            }
            engine::prepare();
            let settings = app.state::<SettingsStore>().get();
            engine::warm_microphone(microphone(&settings).as_deref());
            engine::set_idle_unload(settings.unload_after_minutes);
            history::prune_recordings(&recordings_dir(&handle), settings.keep_audio_days);
            if settings.speech_model == SpeechModel::Whisper {
                thread::spawn(engine::prepare_whisper);
            }
            watch_model_loading(&handle);
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
            install_update,
            transcribe_path,
            history_reprocess,
            history_audio,
            export_backup,
            import_backup,
            open_data_folder,
            install_cli,
            claude_code_setup
        ])
        .build(tauri::generate_context!())
        .expect("error while building Parla");

    app.run(|app, event| match event {
        // Clicking the Dock icon. Other platforms reopen the window from the tray.
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Reopen { .. } => show_main(app),
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Opened { urls } => {
            for url in &urls {
                handle_url(app, url);
            }
        }
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
