//! The dictation engine behind one interface. macOS runs the Swift engine (Parakeet on
//! the Neural Engine, Apple Intelligence for Enhance); Windows and Linux run the Rust one
//! (Parakeet through sherpa-onnx, a small local model through llama.cpp).

use serde::{Deserialize, Serialize};

#[cfg(not(desktop_engine))]
mod mac;
#[cfg(not(desktop_engine))]
pub use mac::*;

#[cfg(desktop_engine)]
mod desktop;
// The Mac lays emails out in Swift; this is the same layout for the Rust engine.
#[cfg_attr(not(desktop_engine), allow(dead_code))]
mod email;
#[cfg(desktop_engine)]
pub use desktop::*;

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
    #[serde(default)]
    pub whisper: String,
    #[serde(default)]
    pub whisper_progress: Option<f64>,
    #[serde(default)]
    pub whisper_message: Option<String>,
    /// The local Enhance model's download on Windows and Linux.
    #[serde(default)]
    pub enhance_progress: Option<f64>,
    #[serde(default)]
    pub enhance_message: Option<String>,
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
            whisper: "idle".into(),
            whisper_progress: None,
            whisper_message: None,
            enhance_progress: None,
            enhance_message: None,
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Transcript {
    pub text: String,
    pub language: Option<String>,
    pub raw: Option<String>,
    pub transcribe_ms: Option<u64>,
    pub enhance_ms: Option<u64>,
    #[cfg_attr(any(not(test), desktop_engine), allow(dead_code))]
    pub audio_ms: Option<u64>,
    /// The text lands straight after a word, so a space goes in front of it.
    #[serde(default)]
    pub leading_space: Option<bool>,
    /// The text lands straight before a word, so a space goes after it.
    #[serde(default)]
    pub trailing_space: Option<bool>,
    /// The style used for the app or site, such as "email".
    #[serde(default)]
    pub style: Option<String>,
    /// The mode that handled it, by id.
    #[serde(default)]
    pub mode: Option<String>,
    /// What the cleanup model was asked, when a mode added instructions or context.
    #[serde(default)]
    pub prompt: Option<String>,
    /// The kinds of context that went with it: "selection", "clipboard", "app".
    #[serde(default)]
    pub context: Option<Vec<String>>,
    #[cfg_attr(desktop_engine, allow(dead_code))]
    pub(super) error: Option<String>,
}


/// How a dictation should be handled, fixed when recording starts. The Rust engine has
/// no live preview, cursor context, Whisper or soft-voice boost yet.
#[cfg_attr(desktop_engine, allow(dead_code))]
pub struct StartOptions<'a> {
    pub enhance: bool,
    pub quick: bool,
    pub dictionary: &'a [String],
    pub match_app: bool,
    /// The user's app rules, one "app\tstyle" per line.
    pub rules: String,
    pub live: bool,
    pub device: Option<&'a str>,
    pub level: &'a str,
    pub use_context: bool,
    pub formatting: bool,
    pub whisper: bool,
    /// A language code to force, or empty to detect it.
    pub language: &'a str,
    /// Boosts the microphone for dictating in a whisper.
    pub soft_voice: bool,
    /// Cuts long silences out before transcribing.
    pub trim: bool,
    /// Turns the microphone's input volume up while recording.
    pub boost: bool,
    pub modes: &'a [crate::settings::ModeConfig],
    /// "match\tmode id" lines that pick a mode by app or website.
    pub mode_rules: String,
    pub active_mode: &'a str,
    /// A mode asked for by its own shortcut, which wins over the app rules.
    pub forced_mode: Option<&'a str>,
}

/// The mode a recording ended up in, and the context it collected when it started.
#[derive(Deserialize, Default, Clone)]
pub struct ContextReply {
    pub mode: String,
    #[serde(default)]
    pub captured: Vec<String>,
}

/// A file, a kept recording or a history entry to run through a mode.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(desktop_engine, allow(dead_code))]
pub struct ProcessRequest<'a> {
    pub enhance: bool,
    pub terms: &'a [String],
    pub level: &'a str,
    pub formatting: bool,
    pub mode: &'a crate::settings::ModeConfig,
    pub whisper: bool,
    pub language: Option<&'a str>,
    pub trim: bool,
}


#[derive(Deserialize)]
#[cfg_attr(desktop_engine, allow(dead_code))]
pub struct Release {
    pub version: String,
    pub url: String,
    pub download: Option<String>,
    pub error: Option<String>,
}


#[derive(Serialize, Deserialize)]
pub struct InputDevice {
    pub uid: String,
    pub name: String,
}

