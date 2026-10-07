//! The Swift engine: Parakeet on the Neural Engine and Apple Intelligence, through the
//! swift-lib package.

use serde::Deserialize;
use swift_rs::{swift, Bool, Int, SRString};

use super::{ContextReply, InputDevice, ModelStatus, ProcessRequest, Release, StartOptions, Transcript};

swift!(fn parla_model_status() -> SRString);
swift!(fn parla_prepare_model() -> Bool);
swift!(fn parla_start(live: Bool, device: &SRString, whisper: Bool, language: &SRString, soft_voice: Bool, trim: Bool, boost: Bool) -> SRString);
swift!(fn parla_start_context(request: &SRString) -> SRString);
swift!(fn parla_note_clipboard() -> Bool);
swift!(fn parla_set_idle_unload(minutes: Int) -> Bool);
swift!(fn parla_process_file(path: &SRString, request: &SRString) -> SRString);
swift!(fn parla_clean_text(text: &SRString, request: &SRString) -> SRString);
swift!(fn parla_file_progress() -> f64);
swift!(fn parla_choose_file(kind: &SRString) -> SRString);
swift!(fn parla_menu_bar_dark() -> Bool);
swift!(fn parla_prepare_whisper() -> Bool);
swift!(fn parla_warm_microphone(device: &SRString) -> Bool);
swift!(fn parla_focused_text() -> SRString);
swift!(fn parla_latest_release() -> SRString);
swift!(fn parla_download(url: &SRString, destination: &SRString) -> SRString);
swift!(fn parla_download_progress() -> f64);
swift!(fn parla_input_devices() -> SRString);
swift!(fn parla_frontmost_app() -> SRString);
swift!(fn parla_sensitive_context() -> SRString);
swift!(fn parla_partial() -> SRString);
swift!(fn parla_selected_text() -> SRString);
swift!(fn parla_pick(names: &SRString) -> Int);
swift!(fn parla_run_command(instruction: &SRString, passage: &SRString) -> SRString);
swift!(fn parla_stop(audio_path: &SRString) -> SRString);
swift!(fn parla_cancel() -> Bool);
swift!(fn parla_level() -> f32);
swift!(fn parla_mic_permission() -> Int);
swift!(fn parla_request_mic() -> Bool);
swift!(fn parla_request_accessibility() -> Bool);
swift!(fn parla_play_cue(start: Bool) -> Bool);
swift!(fn parla_duck_audio(enable: Bool, mute: Bool, pause: Bool, lower: Bool) -> Bool);
swift!(fn parla_float_overlay(window: Int) -> Bool);

pub fn status() -> ModelStatus {
    let raw = unsafe { parla_model_status() };
    serde_json::from_str(raw.as_str()).unwrap_or_else(|e| ModelStatus::error(e.to_string()))
}

pub fn prepare() {
    let _ = unsafe { parla_prepare_model() };
}

// Lists cross as one newline-separated string rather than a shared array, which keeps
// the bridge to the single string type swift-rs already passes. An empty device UID
// records from the system default input.
pub fn start(options: &StartOptions) -> Result<(), String> {
    let device = SRString::from(options.device.unwrap_or(""));
    let language = SRString::from(options.language);
    let error = unsafe {
        parla_start(
            options.live,
            &device,
            options.whisper,
            &language,
            options.soft_voice,
            options.trim,
            options.boost,
        )
    };
    match error.as_str() {
        "" => Ok(()),
        message => Err(message.to_string()),
    }
}

/// Reads the app and cursor context for the cleanup and picks the mode. Called once
/// recording has started, so the first words are never lost to it, and again when the
/// user switches modes mid-recording.
pub fn start_context(options: &StartOptions) -> Option<ContextReply> {
    let request = serde_json::json!({
        "enhance": options.enhance,
        "quick": options.quick,
        "terms": options.dictionary,
        "matchApp": options.match_app,
        "level": options.level,
        "rules": options.rules,
        "useContext": options.use_context,
        "formatting": options.formatting,
        "modes": options.modes,
        "modeRules": options.mode_rules,
        "activeMode": options.active_mode,
        "forcedMode": options.forced_mode,
    });
    let request = SRString::from(request.to_string().as_str());
    serde_json::from_str(unsafe { parla_start_context(&request) }.as_str()).ok()
}

/// Parla wrote the clipboard itself, which a mode must never take for something the user
/// copied.
pub fn note_clipboard() {
    let _ = unsafe { parla_note_clipboard() };
}

pub fn set_idle_unload(minutes: u32) {
    let _ = unsafe { parla_set_idle_unload(minutes as Int) };
}

/// Transcribes an audio or video file, or a kept recording, through a mode. Blocks until
/// it is done, which for a long file is minutes.
pub fn process_file(path: &std::path::Path, request: &ProcessRequest) -> Result<Transcript, String> {
    let path = SRString::from(path.to_string_lossy().as_ref());
    let request = SRString::from(serde_json::to_string(request).map_err(|e| e.to_string())?.as_str());
    parse_transcript(unsafe { parla_process_file(&path, &request) })
}

/// Runs text already transcribed through a mode again.
pub fn clean_text(text: &str, request: &ProcessRequest) -> Result<Transcript, String> {
    let text = SRString::from(text);
    let request = SRString::from(serde_json::to_string(request).map_err(|e| e.to_string())?.as_str());
    parse_transcript(unsafe { parla_clean_text(&text, &request) })
}

/// How far the file being processed has got, from 0 to 1.
pub fn file_progress() -> f64 {
    unsafe { parla_file_progress() }
}

/// Asks the user for a file: "media" for audio or video, "json" for a backup.
pub fn choose_file(kind: &str) -> Option<std::path::PathBuf> {
    let path = unsafe { parla_choose_file(&SRString::from(kind)) }.as_str().to_string();
    (!path.is_empty()).then(|| path.into())
}

pub fn menu_bar_dark() -> bool {
    unsafe { parla_menu_bar_dark() }
}

/// Sets up the audio engine ahead of time so pressing the key records right away. The
/// microphone itself stays off until then.
pub fn warm_microphone(device: Option<&str>) {
    let _ = unsafe { parla_warm_microphone(&SRString::from(device.unwrap_or(""))) };
}

pub fn prepare_whisper() {
    let _ = unsafe { parla_prepare_whisper() };
}

/// The focused field's text, or empty when it is a password field or unreadable.
pub fn focused_text() -> String {
    unsafe { parla_focused_text() }.as_str().to_string()
}

/// Downloads `url` to `destination`, blocking until it is done.
pub fn download(url: &str, destination: &std::path::Path) -> Result<(), String> {
    let url = SRString::from(url);
    let destination = SRString::from(destination.to_string_lossy().as_ref());
    match unsafe { parla_download(&url, &destination) }.as_str() {
        "" => Ok(()),
        message => Err(message.to_string()),
    }
}

/// How far the download in progress has got, from 0 to 1.
pub fn download_progress() -> f64 {
    unsafe { parla_download_progress() }
}

pub fn latest_release() -> Result<Release, String> {
    let raw = unsafe { parla_latest_release() };
    let release: Release = serde_json::from_str(raw.as_str()).map_err(|e| e.to_string())?;
    match release.error.clone() {
        Some(error) => Err(error),
        None => Ok(release),
    }
}

/// The name of the app in front, for the history entry. Only the app's own name, never
/// its window or contents.
pub fn frontmost_app() -> Option<String> {
    let name = unsafe { parla_frontmost_app() }.as_str().to_string();
    (!name.is_empty()).then_some(name)
}

/// Why dictating right now would be a bad idea: a password field has focus, or the app
/// in front is one where a stray paste does real damage. Empty means go ahead.
pub fn sensitive_context() -> Option<String> {
    let reason = unsafe { parla_sensitive_context() }.as_str().to_string();
    (!reason.is_empty()).then_some(reason)
}

/// Ends the recording and returns its text, keeping the audio at `audio` when given.
pub fn stop(audio: Option<&std::path::Path>) -> Result<Transcript, String> {
    let path = SRString::from(audio.map(|p| p.to_string_lossy().into_owned()).unwrap_or_default().as_str());
    parse_transcript(unsafe { parla_stop(&path) })
}

fn parse_transcript(raw: SRString) -> Result<Transcript, String> {
    let mut transcript: Transcript =
        serde_json::from_str(raw.as_str()).map_err(|e| e.to_string())?;
    match transcript.error.take() {
        Some(error) => Err(error),
        None => Ok(transcript),
    }
}

pub fn input_devices() -> Vec<InputDevice> {
    serde_json::from_str(unsafe { parla_input_devices() }.as_str()).unwrap_or_default()
}

pub fn partial() -> String {
    unsafe { parla_partial() }.as_str().to_string()
}

pub fn selected_text() -> String {
    unsafe { parla_selected_text() }.as_str().to_string()
}

#[derive(Deserialize)]
struct CommandResult {
    text: String,
    error: Option<String>,
}

/// Shows `names` as a menu at the selected text and returns the index picked, or None
/// when the menu was dismissed.
pub fn pick(names: &[&str]) -> Option<usize> {
    if names.is_empty() {
        return None;
    }
    let joined = SRString::from(names.join("\n").as_str());
    usize::try_from(unsafe { parla_pick(&joined) }).ok()
}

pub fn run_command(instruction: &str, passage: &str) -> Result<String, String> {
    let instruction = SRString::from(instruction);
    let passage = SRString::from(passage);
    let raw = unsafe { parla_run_command(&instruction, &passage) };
    let result: CommandResult = serde_json::from_str(raw.as_str()).map_err(|e| e.to_string())?;
    match result.error {
        Some(error) => Err(error),
        None => Ok(result.text),
    }
}

/// Nothing to free: the Swift engine tears itself down with the process.
pub fn shutdown() {}

pub fn cancel() {
    let _ = unsafe { parla_cancel() };
}

pub fn level() -> f32 {
    unsafe { parla_level() }
}

pub fn microphone_permission() -> &'static str {
    match unsafe { parla_mic_permission() } {
        2 => "granted",
        0 => "undetermined",
        _ => "denied",
    }
}

pub fn request_microphone() -> bool {
    unsafe { parla_request_mic() }
}

pub fn request_accessibility() -> bool {
    unsafe { parla_request_accessibility() }
}

pub fn play_cue(start: bool) {
    let _ = unsafe { parla_play_cue(start) };
}

// Silences whatever else is playing so it neither bleeds into the microphone nor
// talks over the user. Turning it off restores the exact level it replaced.
pub fn duck_audio(enable: bool) {
    let _ = unsafe { parla_duck_audio(enable, false, false, false) };
}

/// Mutes or turns down the speakers and/or pauses whatever music or video is playing,
/// until `duck_audio(false)` puts it all back.
pub fn quiet_other_audio(mute: bool, pause: bool, lower: bool) {
    let _ = unsafe { parla_duck_audio(true, mute, pause, lower) };
}

// Keeps the overlay above every app, full-screen ones included. Must run on the main
// thread.
pub fn float_overlay(window: *mut std::ffi::c_void) {
    let _ = unsafe { parla_float_overlay(window as Int) };
}

#[cfg(test)]
mod tests {
    use super::*;

    swift!(fn parla_benchmark(path: &SRString, enhance: Bool) -> SRString);

    // Measures end-to-end latency on a real recording without a microphone. Needs the
    // Parakeet models on disk:
    //   PARLA_BENCH_AUDIO=clip.wav cargo test --release --lib benchmark -- --ignored --nocapture
    #[test]
    #[ignore]
    fn benchmark() {
        let path = std::env::var("PARLA_BENCH_AUDIO").expect("set PARLA_BENCH_AUDIO");
        let path = SRString::from(path.as_str());

        for (run, enhance) in [false, true, true, true].into_iter().enumerate() {
            let result = parse_transcript(unsafe { parla_benchmark(&path, enhance) });
            println!("status: {}", serde_json::to_string(&status()).unwrap());
            let transcript = result.expect("benchmark failed");
            println!(
                "run {run} enhance={enhance} audio={:?}ms transcribe={:?}ms enhance={:?}ms\n  raw: {:?}\n  out: {}",
                transcript.audio_ms,
                transcript.transcribe_ms,
                transcript.enhance_ms,
                transcript.raw,
                transcript.text
            );
            assert!(!transcript.text.trim().is_empty());
        }
    }
}
