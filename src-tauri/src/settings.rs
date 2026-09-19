use std::{path::PathBuf, sync::Mutex};

use serde::{Deserialize, Deserializer, Serialize};
use tauri::AppHandle;

use crate::storage;

/// macOS virtual keycode for Caps Lock. It latches instead of reporting a clean
/// press and release, so it is never part of a shortcut.
pub const CAPS_LOCK: u16 = 57;
/// Command, Shift, Option, Control and Globe/Fn, left and right.
pub const MODIFIERS: [u16; 9] = [54, 55, 56, 58, 59, 60, 61, 62, 63];
const MAX_GROUPS: usize = 3;
const MAX_KEYCODE: u16 = 127;
/// Enough for names, jargon and product spellings without bloating the prompt the
/// on-device model has to read on every dictation.
const MAX_TERMS: usize = 40;
const MAX_TERM_LENGTH: usize = 64;
const MAX_SNIPPETS: usize = 50;
const MAX_TRIGGER_LENGTH: usize = 48;
const MAX_SNIPPET_LENGTH: usize = 2000;

/// A spoken phrase that is typed as saved text, like "my email" for an address.
#[derive(Clone, Serialize, Deserialize)]
pub struct Snippet {
    pub trigger: String,
    pub text: String,
}

/// A hold-to-talk shortcut. Every group has to be satisfied at once, and a group is
/// satisfied by any of its keycodes, which is what lets "either Option key" and
/// "the right Option key" share one shape.
#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct Hotkey {
    pub groups: Vec<Vec<u16>>,
}

impl Default for Hotkey {
    fn default() -> Self {
        Self {
            groups: vec![vec![58, 61]],
        }
    }
}

impl Hotkey {
    /// Every keycode the shortcut may involve; anything else pressed alongside it
    /// means the user is typing a shortcut rather than dictating.
    pub fn keys(&self) -> Vec<u16> {
        self.groups.concat()
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.groups.is_empty() {
            return Err("Record a shortcut first.".into());
        }
        if self.groups.len() > MAX_GROUPS {
            return Err(format!("Use at most {MAX_GROUPS} keys."));
        }
        if self
            .groups
            .iter()
            .any(|group| group.is_empty() || group.iter().any(|key| *key > MAX_KEYCODE))
        {
            return Err("That shortcut has a key Parla can't watch.".into());
        }
        if self.keys().contains(&CAPS_LOCK) {
            return Err("Caps Lock stays on when pressed, so it can't be held to talk.".into());
        }
        // Without a modifier the key could never be typed normally again.
        if !self
            .groups
            .iter()
            .any(|group| group.iter().all(|key| MODIFIERS.contains(key)))
        {
            return Err("Include a modifier like ⌘, ⌥, ⌃, ⇧ or fn.".into());
        }
        Ok(())
    }
}

// Builds from either the current shape or a name saved by an earlier build, and falls
// back to the default rather than refusing to load the rest of the settings.
impl<'de> Deserialize<'de> for Hotkey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Stored {
            Named(String),
            Groups { groups: Vec<Vec<u16>> },
        }

        let hotkey = match Stored::deserialize(deserializer) {
            Ok(Stored::Named(name)) => named(&name),
            Ok(Stored::Groups { groups }) => Hotkey { groups },
            Err(_) => Hotkey::default(),
        };
        Ok(match hotkey.validate() {
            Ok(()) => hotkey,
            Err(_) => Hotkey::default(),
        })
    }
}

fn named(name: &str) -> Hotkey {
    let groups = match name {
        "right_option" => vec![vec![61]],
        "right_command" => vec![vec![54]],
        "control" => vec![vec![59, 62]],
        "fn" => vec![vec![63]],
        _ => vec![vec![58, 61]],
    };
    Hotkey { groups }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub hotkey: Hotkey,
    /// Tapping the dictation key twice starts a recording that keeps going after the
    /// key is let go.
    pub hands_free: bool,
    /// Optional second shortcut that pastes the last transcript again, for when the
    /// first paste landed in the wrong window.
    pub repeat_hotkey: Option<Hotkey>,
    /// Optional hold-to-talk shortcut whose words are an instruction applied to the
    /// selected text instead of text to paste.
    pub command_hotkey: Option<Hotkey>,
    /// Names and jargon the transcriber tends to get wrong, passed to Enhance.
    pub dictionary: Vec<String>,
    /// Lets Enhance adjust to the app being dictated into, judged by its bundle id only.
    pub app_aware_tone: bool,
    /// Runs the streaming model during recording to show words as they are spoken.
    /// Off by default because it competes with the final transcription for the
    /// Neural Engine.
    pub live_preview: bool,
    /// Ends a hands-free recording after a stretch of silence.
    pub auto_stop_silence: bool,
    /// Core Audio UID of the microphone to record from; `None` follows the system.
    pub input_device: Option<String>,
    pub snippets: Vec<Snippet>,
    pub sounds: bool,
    pub mute_other_audio: bool,
    pub enhance: bool,
    /// Pastes a transcript without the Enhance pass when it has no fillers or
    /// restarts to remove, which saves around half a second.
    pub quick_enhance: bool,
    pub onboarded: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: Hotkey::default(),
            hands_free: true,
            repeat_hotkey: None,
            command_hotkey: None,
            dictionary: Vec::new(),
            app_aware_tone: true,
            live_preview: false,
            auto_stop_silence: true,
            input_device: None,
            snippets: Vec::new(),
            sounds: true,
            mute_other_audio: true,
            enhance: true,
            quick_enhance: true,
            onboarded: false,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        self.hotkey.validate()?;
        if let Some(repeat) = &self.repeat_hotkey {
            repeat.validate()?;
            if *repeat == self.hotkey {
                return Err("That is already the dictation key.".into());
            }
        }
        if let Some(command) = &self.command_hotkey {
            command.validate()?;
            if *command == self.hotkey {
                return Err("That is already the dictation key.".into());
            }
            if self.repeat_hotkey.as_ref() == Some(command) {
                return Err("That key already pastes the last dictation.".into());
            }
        }
        Ok(())
    }

    /// Trims the dictionary down to something the model can read quickly, so a long
    /// paste into the settings field can't slow every dictation down.
    pub fn sanitize(&mut self) {
        let mut terms: Vec<String> = Vec::new();
        for term in self.dictionary.drain(..) {
            let term = term.trim().chars().take(MAX_TERM_LENGTH).collect::<String>();
            if !term.is_empty() && !terms.iter().any(|kept| kept.eq_ignore_ascii_case(&term)) {
                terms.push(term);
            }
        }
        terms.truncate(MAX_TERMS);
        self.dictionary = terms;

        let mut snippets: Vec<Snippet> = Vec::new();
        for snippet in self.snippets.drain(..) {
            let trigger = snippet.trigger.trim().chars().take(MAX_TRIGGER_LENGTH).collect::<String>();
            let text = snippet.text.chars().take(MAX_SNIPPET_LENGTH).collect::<String>();
            if !trigger.is_empty()
                && !text.trim().is_empty()
                && !snippets.iter().any(|kept| kept.trigger.eq_ignore_ascii_case(&trigger))
            {
                snippets.push(Snippet { trigger, text });
            }
        }
        snippets.truncate(MAX_SNIPPETS);
        self.snippets = snippets;
    }
}

pub struct SettingsStore {
    path: PathBuf,
    current: Mutex<Settings>,
}

impl SettingsStore {
    pub fn load(app: &AppHandle) -> Self {
        let path = storage::path(app, "settings.json");
        let current = Mutex::new(storage::read(&path));
        Self { path, current }
    }

    pub fn get(&self) -> Settings {
        self.current.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn set(&self, next: Settings) -> Result<(), String> {
        storage::write(&self.path, &next)?;
        *self.current.lock().unwrap_or_else(|e| e.into_inner()) = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> Hotkey {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn reads_names_saved_by_earlier_builds() {
        assert_eq!(parse("\"option\"").groups, vec![vec![58, 61]]);
        assert_eq!(parse("\"fn\"").groups, vec![vec![63]]);
        assert_eq!(parse("\"right_option\"").groups, vec![vec![61]]);
        // A combo saved as a name by a build that never shipped it.
        assert_eq!(parse("\"ctrl+space\"").groups, Hotkey::default().groups);
    }

    #[test]
    fn falls_back_when_the_stored_shortcut_is_unusable() {
        assert_eq!(parse(r#"{"groups":[[49]]}"#).groups, Hotkey::default().groups);
        assert_eq!(parse(r#"{"groups":[]}"#).groups, Hotkey::default().groups);
        assert_eq!(parse(r#"{"groups":[[59,62],[49]]}"#).groups, vec![vec![59, 62], vec![49]]);
    }

    #[test]
    fn keeps_the_dictionary_short_and_free_of_repeats() {
        let mut settings = Settings {
            dictionary: vec![
                "  Parla  ".into(),
                "parla".into(),
                "".into(),
                "Tauri".into(),
                "x".repeat(200),
            ],
            ..Settings::default()
        };
        settings.sanitize();
        assert_eq!(settings.dictionary.len(), 3);
        assert_eq!(settings.dictionary[0], "Parla");
        assert_eq!(settings.dictionary[2].len(), MAX_TERM_LENGTH);

        let mut many = Settings {
            dictionary: (0..80).map(|n| format!("term{n}")).collect(),
            ..Settings::default()
        };
        many.sanitize();
        assert_eq!(many.dictionary.len(), MAX_TERMS);
    }

    #[test]
    fn refuses_a_repeat_shortcut_that_clashes_with_dictation() {
        let hotkey = Hotkey { groups: vec![vec![58, 61]] };
        let settings = Settings {
            repeat_hotkey: Some(hotkey.clone()),
            hotkey,
            ..Settings::default()
        };
        assert!(settings.validate().is_err());
    }

    #[test]
    fn keeps_every_shortcut_distinct() {
        let repeat = Hotkey { groups: vec![vec![59, 62], vec![49]] };
        let settings = Settings {
            repeat_hotkey: Some(repeat.clone()),
            command_hotkey: Some(repeat),
            ..Settings::default()
        };
        assert!(settings.validate().is_err());

        let settings = Settings {
            command_hotkey: Some(Hotkey { groups: vec![vec![54]] }),
            ..Settings::default()
        };
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn rejects_shortcuts_that_would_swallow_typing() {
        assert!(Hotkey { groups: vec![vec![49]] }.validate().is_err());
        assert!(Hotkey { groups: vec![vec![CAPS_LOCK]] }.validate().is_err());
        assert!(Hotkey { groups: vec![vec![55], vec![56], vec![49], vec![48]] }.validate().is_err());
        assert!(Hotkey { groups: vec![vec![59, 62], vec![49]] }.validate().is_ok());
    }
}
