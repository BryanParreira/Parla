use serde::{Deserialize, Serialize};
use swift_rs::{swift, Bool, Int, SRString};

swift!(fn parla_model_status() -> SRString);
swift!(fn parla_prepare_model() -> Bool);
swift!(fn parla_start(enhance: Bool, quick: Bool, terms: &SRString, match_app: Bool, live: Bool, device: &SRString) -> SRString);
swift!(fn parla_input_devices() -> SRString);
swift!(fn parla_partial() -> SRString);
swift!(fn parla_selected_text() -> SRString);
swift!(fn parla_run_command(instruction: &SRString, passage: &SRString) -> SRString);
swift!(fn parla_stop() -> SRString);
swift!(fn parla_cancel() -> Bool);
swift!(fn parla_level() -> f32);
swift!(fn parla_mic_permission() -> Int);
swift!(fn parla_request_mic() -> Bool);
swift!(fn parla_request_accessibility() -> Bool);
swift!(fn parla_play_cue(start: Bool) -> Bool);
swift!(fn parla_duck_audio(enable: Bool) -> Bool);
swift!(fn parla_float_overlay(window: Int) -> Bool);

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    pub state: String,
    pub progress: Option<f64>,
    pub message: Option<String>,
    #[serde(default)]
    pub streaming: String,
    #[serde(default)]
    pub punctuation: String,
    #[serde(default)]
    pub punctuation_progress: Option<f64>,
    #[serde(default)]
    pub enhance: String,
}

impl ModelStatus {
    pub fn error(message: String) -> Self {
        Self {
            state: "error".into(),
            progress: None,
            message: Some(message),
            streaming: "error".into(),
            punctuation: "error".into(),
            punctuation_progress: None,
            enhance: "unavailable".into(),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transcript {
    pub text: String,
    pub language: Option<String>,
    pub raw: Option<String>,
    pub transcribe_ms: Option<u64>,
    pub enhance_ms: Option<u64>,
    #[cfg_attr(not(test), allow(dead_code))]
    pub audio_ms: Option<u64>,
    error: Option<String>,
}

pub fn status() -> ModelStatus {
    let raw = unsafe { parla_model_status() };
    serde_json::from_str(raw.as_str()).unwrap_or_else(|e| ModelStatus::error(e.to_string()))
}

pub fn prepare() {
    let _ = unsafe { parla_prepare_model() };
}

// Dictionary terms cross as one newline-separated string rather than a shared array,
// which keeps the bridge to the single string type swift-rs already passes.
// An empty device UID records from the system default input.
pub fn start(
    enhance: bool,
    quick: bool,
    dictionary: &[String],
    match_app: bool,
    live: bool,
    device: Option<&str>,
) -> Result<(), String> {
    let terms = SRString::from(dictionary.join("\n").as_str());
    let device = SRString::from(device.unwrap_or(""));
    let error = unsafe { parla_start(enhance, quick, &terms, match_app, live, &device) };
    match error.as_str() {
        "" => Ok(()),
        message => Err(message.to_string()),
    }
}

pub fn stop() -> Result<Transcript, String> {
    parse_transcript(unsafe { parla_stop() })
}

fn parse_transcript(raw: SRString) -> Result<Transcript, String> {
    let mut transcript: Transcript =
        serde_json::from_str(raw.as_str()).map_err(|e| e.to_string())?;
    match transcript.error.take() {
        Some(error) => Err(error),
        None => Ok(transcript),
    }
}

#[derive(Serialize, Deserialize)]
pub struct InputDevice {
    pub uid: String,
    pub name: String,
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
    let _ = unsafe { parla_duck_audio(enable) };
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
