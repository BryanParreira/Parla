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
    #[cfg_attr(not(test), allow(dead_code))]
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

