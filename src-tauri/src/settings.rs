use std::{path::PathBuf, sync::Mutex};

use serde::{Deserialize, Deserializer, Serialize};
use tauri::AppHandle;

use crate::storage;

use crate::keys::{CAPS_LOCK, MAX_KEYCODE, MODIFIERS};
const MAX_GROUPS: usize = 3;
/// Enough for names, jargon and product spellings without bloating the prompt the
/// on-device model has to read on every dictation.
const MAX_TERMS: usize = 40;
const MAX_TERM_LENGTH: usize = 64;
const MAX_SNIPPETS: usize = 50;
const MAX_TRIGGER_LENGTH: usize = 48;
const MAX_SNIPPET_LENGTH: usize = 2000;
const MAX_APP_RULES: usize = 30;
const MAX_APP_NAME_LENGTH: usize = 64;
const MAX_TRANSFORMS: usize = 12;
const MAX_TRANSFORM_NAME_LENGTH: usize = 32;
const MAX_TRANSFORM_LENGTH: usize = 300;
/// Styles an app rule can pick. "off" types the plain transcript in that app.
pub const APP_STYLES: [&str; 6] = ["casual", "formal", "email", "code", "notes", "off"];

/// A spoken phrase that is typed as saved text, like "my email" for an address.
#[derive(Clone, Serialize, Deserialize)]
pub struct Snippet {
    pub trigger: String,
    pub text: String,
}

/// How Enhance should write for one app, matched on the app's name or bundle id.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppRule {
    pub app: String,
    pub style: String,
}

/// A saved Command Mode instruction: say its name, or pick it from the menu bar.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Transform {
    pub name: String,
    pub instruction: String,
}

impl Transform {
    /// The saved instruction a spoken Command Mode request names, if any: "formal",
    /// "make it formal" and "Formal." all pick the transform called Formal.
    pub fn resolve<'a>(spoken: &str, transforms: &'a [Transform]) -> Option<&'a str> {
        let words = |text: &str| -> Vec<String> {
            text.split(|c: char| !c.is_alphanumeric())
                .filter(|word| !word.is_empty())
                .map(str::to_lowercase)
                .collect()
        };
        let mut spoken = words(spoken);
        const LEAD_INS: [&str; 7] = ["please", "make", "it", "apply", "use", "do", "transform"];
        while spoken.first().is_some_and(|word| LEAD_INS.contains(&word.as_str())) {
            spoken.remove(0);
        }
        transforms
            .iter()
            .find(|transform| !spoken.is_empty() && words(&transform.name) == spoken)
            .map(|transform| transform.instruction.as_str())
    }
}

fn default_transforms() -> Vec<Transform> {
    [
        ("Formal", "Rewrite this in a formal, professional tone."),
        ("Shorter", "Make this shorter and more direct. Keep every important point."),
        ("Fix grammar", "Fix grammar, spelling and punctuation only. Change nothing else."),
        ("Bullet points", "Turn this into a concise bulleted list."),
        ("English", "Translate this into English."),
    ]
    .into_iter()
    .map(|(name, instruction)| Transform { name: name.into(), instruction: instruction.into() })
    .collect()
}

/// Which model turns speech into text. Parakeet is the fast default for 25 European
/// languages; Whisper covers 99 languages at the cost of a 1.6 GB download.
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SpeechModel {
    #[default]
    Parakeet,
    Whisper,
}

impl<'de> Deserialize<'de> for SpeechModel {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match String::deserialize(deserializer).as_deref() {
            Ok("whisper") => Self::Whisper,
            _ => Self::Parakeet,
        })
    }
}

/// How far Enhance is allowed to go. Standard is what Parla has always done; Light
/// only takes out what nobody meant to say, and Polished may reshape sentences.
#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CleanupLevel {
    Light,
    Standard,
    Polished,
}

impl Default for CleanupLevel {
    fn default() -> Self {
        Self::Standard
    }
}

impl CleanupLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Standard => "standard",
            Self::Polished => "polished",
        }
    }
}

// A level written by a newer build falls back to Standard rather than making the whole
// settings file unreadable.
impl<'de> Deserialize<'de> for CleanupLevel {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match String::deserialize(deserializer).as_deref() {
            Ok("light") => Self::Light,
            Ok("polished") => Self::Polished,
            _ => Self::Standard,
        })
    }
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
            groups: crate::keys::DEFAULT_HOTKEY.iter().map(|group| group.to_vec()).collect(),
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
            return Err(crate::keys::MODIFIER_HINT.into());
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
    /// Optional shortcut that takes the last pasted text back out again, for when a
    /// dictation landed somewhere it shouldn't have.
    pub undo_hotkey: Option<Hotkey>,
    /// Optional shortcut that opens a menu of the transforms at the selected text.
    pub transform_hotkey: Option<Hotkey>,
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
    /// Boosts the microphone so a whispered dictation is heard like a normal one.
    pub soft_voice: bool,
    pub snippets: Vec<Snippet>,
    pub sounds: bool,
    pub mute_other_audio: bool,
    /// Pauses music and video while dictating and plays them again afterwards.
    pub pause_media: bool,
    pub enhance: bool,
    /// Pastes a transcript without the Enhance pass when it has no fillers or
    /// restarts to remove, which saves around half a second.
    pub quick_enhance: bool,
    /// How much Enhance is allowed to change.
    pub cleanup_level: CleanupLevel,
    /// Refuses to record into password fields and banking apps, where a stray
    /// dictation would paste something that shouldn't be spoken aloud.
    pub pause_in_sensitive_apps: bool,
    /// Reads the text just before the cursor so a dictation carries on the sentence.
    pub use_context: bool,
    /// Turns "new line", "comma" and the like into the characters.
    pub spoken_formatting: bool,
    /// The user's own per-app styles, checked before the built-in ones.
    pub app_rules: Vec<AppRule>,
    pub transforms: Vec<Transform>,
    /// Notices words the user fixes after a paste and suggests them for the dictionary.
    pub learn_words: bool,
    /// Asks GitHub once a day whether a newer Parla exists. Off unless the user opts in,
    /// since it is the only request Parla would make on its own.
    pub check_updates: bool,
    pub speech_model: SpeechModel,
    /// Language code Whisper is told to expect; `None` detects it.
    pub language: Option<String>,
    pub onboarded: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: Hotkey::default(),
            hands_free: true,
            repeat_hotkey: None,
            command_hotkey: None,
            undo_hotkey: None,
            transform_hotkey: None,
            dictionary: Vec::new(),
            app_aware_tone: true,
            live_preview: false,
            auto_stop_silence: true,
            input_device: None,
            soft_voice: false,
            snippets: Vec::new(),
            sounds: true,
            mute_other_audio: true,
            pause_media: true,
            enhance: true,
            quick_enhance: true,
            cleanup_level: CleanupLevel::Standard,
            pause_in_sensitive_apps: true,
            use_context: true,
            spoken_formatting: true,
            app_rules: Vec::new(),
            transforms: default_transforms(),
            learn_words: true,
            check_updates: false,
            speech_model: SpeechModel::Parakeet,
            language: None,
            onboarded: false,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        self.hotkey.validate()?;
        // Each optional shortcut has to clear the dictation key and every optional one
        // named before it, so no two shortcuts can ever fire on the same keys.
        let optional = [
            ("pastes the last dictation", &self.repeat_hotkey),
            ("is Command Mode", &self.command_hotkey),
            ("undoes the last paste", &self.undo_hotkey),
            ("opens the transform menu", &self.transform_hotkey),
        ];
        for (index, (_, hotkey)) in optional.iter().enumerate() {
            let Some(hotkey) = hotkey.as_ref() else { continue };
            hotkey.validate()?;
            if *hotkey == self.hotkey {
                return Err("That is already the dictation key.".into());
            }
            for (name, earlier) in optional.iter().take(index) {
                if earlier.as_ref() == Some(hotkey) {
                    return Err(format!("That key already {name}."));
                }
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

        let mut rules: Vec<AppRule> = Vec::new();
        for rule in self.app_rules.drain(..) {
            let app = rule.app.trim().chars().take(MAX_APP_NAME_LENGTH).collect::<String>();
            if !app.is_empty()
                && APP_STYLES.contains(&rule.style.as_str())
                && !rules.iter().any(|kept| kept.app.eq_ignore_ascii_case(&app))
            {
                rules.push(AppRule { app, style: rule.style });
            }
        }
        rules.truncate(MAX_APP_RULES);
        self.app_rules = rules;

        let mut transforms: Vec<Transform> = Vec::new();
        for transform in self.transforms.drain(..) {
            let name = transform.name.trim().chars().take(MAX_TRANSFORM_NAME_LENGTH).collect::<String>();
            let instruction = transform
                .instruction
                .trim()
                .chars()
                .take(MAX_TRANSFORM_LENGTH)
                .collect::<String>();
            if !name.is_empty()
                && !instruction.is_empty()
                && !transforms.iter().any(|kept| kept.name.eq_ignore_ascii_case(&name))
            {
                transforms.push(Transform { name, instruction });
            }
        }
        transforms.truncate(MAX_TRANSFORMS);
        self.transforms = transforms;

        // Language codes are two or three letters; anything else means "detect".
        self.language = self
            .language
            .take()
            .map(|code| code.trim().to_lowercase())
            .filter(|code| (2..=3).contains(&code.len()) && code.chars().all(|c| c.is_ascii_lowercase()));
    }

    /// The app rules as the engine reads them: one "app<TAB>style" per line.
    pub fn app_rules_text(&self) -> String {
        self.app_rules
            .iter()
            .map(|rule| format!("{}\t{}", rule.app.replace(['\t', '\n'], " "), rule.style))
            .collect::<Vec<_>>()
            .join("\n")
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

    // Keycodes are macOS ones; each platform numbers keys its own way.
    #[cfg(target_os = "macos")]
    #[test]
    fn reads_names_saved_by_earlier_builds() {
        assert_eq!(parse("\"option\"").groups, vec![vec![58, 61]]);
        assert_eq!(parse("\"fn\"").groups, vec![vec![63]]);
        assert_eq!(parse("\"right_option\"").groups, vec![vec![61]]);
        // A combo saved as a name by a build that never shipped it.
        assert_eq!(parse("\"ctrl+space\"").groups, Hotkey::default().groups);
    }

    // Keycodes are macOS ones; each platform numbers keys its own way.
    #[cfg(target_os = "macos")]
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

    // Keycodes are macOS ones; each platform numbers keys its own way.
    #[cfg(target_os = "macos")]
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

    // Keycodes are macOS ones; each platform numbers keys its own way.
    #[cfg(target_os = "macos")]
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

    // Keycodes are macOS ones; each platform numbers keys its own way.
    #[cfg(target_os = "macos")]
    #[test]
    fn refuses_an_undo_shortcut_that_clashes_with_another() {
        let shared = Hotkey { groups: vec![vec![54]] };
        let settings = Settings {
            command_hotkey: Some(shared.clone()),
            undo_hotkey: Some(shared),
            ..Settings::default()
        };
        assert!(settings.validate().is_err());

        let settings = Settings {
            undo_hotkey: Some(Hotkey { groups: vec![vec![54]] }),
            ..Settings::default()
        };
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn falls_back_to_the_usual_cleanup_when_the_level_is_unknown() {
        let read = |json: &str| serde_json::from_str::<CleanupLevel>(json).unwrap();
        assert_eq!(read("\"light\"").as_str(), "light");
        assert_eq!(read("\"polished\"").as_str(), "polished");
        // Written by a newer build, or by hand.
        assert_eq!(read("\"exuberant\"").as_str(), "standard");
        assert_eq!(read("7").as_str(), "standard");
        // Settings saved before the level existed keep behaving as they did.
        let settings: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings.cleanup_level.as_str(), "standard");
        assert!(settings.pause_in_sensitive_apps);
        assert!(settings.undo_hotkey.is_none());
    }

    #[test]
    fn new_settings_have_safe_defaults_for_older_files() {
        let settings: Settings = serde_json::from_str("{}").unwrap();
        assert!(settings.use_context && settings.spoken_formatting && settings.learn_words);
        assert!(!settings.check_updates, "update checks go online, so they stay opt-in");
        assert!(settings.speech_model == SpeechModel::Parakeet);
        assert_eq!(settings.transforms.len(), 5);
        let unknown: Settings = serde_json::from_str(r#"{"speechModel":"gpt"}"#).unwrap();
        assert!(unknown.speech_model == SpeechModel::Parakeet);
    }

    #[test]
    fn cleans_up_app_rules_transforms_and_the_language() {
        let mut settings = Settings {
            app_rules: vec![
                AppRule { app: " Slack ".into(), style: "casual".into() },
                AppRule { app: "slack".into(), style: "formal".into() },
                AppRule { app: "Mail".into(), style: "shouty".into() },
                AppRule { app: "  ".into(), style: "code".into() },
            ],
            transforms: vec![
                Transform { name: "Formal".into(), instruction: "Be formal.".into() },
                Transform { name: "formal".into(), instruction: "Again.".into() },
                Transform { name: "Empty".into(), instruction: " ".into() },
            ],
            language: Some(" JA ".into()),
            ..Settings::default()
        };
        settings.sanitize();
        assert_eq!(settings.app_rules.len(), 1);
        assert_eq!(settings.app_rules[0].app, "Slack");
        assert_eq!(settings.transforms.len(), 1);
        assert_eq!(settings.language.as_deref(), Some("ja"));
        assert_eq!(settings.app_rules_text(), "Slack\tcasual");

        let mut bad = Settings { language: Some("english".into()), ..Settings::default() };
        bad.sanitize();
        assert!(bad.language.is_none());
    }

    #[test]
    fn a_spoken_transform_name_picks_its_instruction() {
        let transforms = default_transforms();
        let formal = Some("Rewrite this in a formal, professional tone.");
        assert_eq!(Transform::resolve("Formal.", &transforms), formal);
        assert_eq!(Transform::resolve("make it formal", &transforms), formal);
        assert_eq!(Transform::resolve("Fix grammar", &transforms).is_some(), true);
        // Anything else is a one-off instruction and is used as spoken.
        assert_eq!(Transform::resolve("make it formal and short", &transforms), None);
        assert_eq!(Transform::resolve("please", &transforms), None);
    }

    // Keycodes are macOS ones; each platform numbers keys its own way.
    #[cfg(target_os = "macos")]
    #[test]
    fn rejects_shortcuts_that_would_swallow_typing() {
        assert!(Hotkey { groups: vec![vec![49]] }.validate().is_err());
        assert!(Hotkey { groups: vec![vec![CAPS_LOCK]] }.validate().is_err());
        assert!(Hotkey { groups: vec![vec![55], vec![56], vec![49], vec![48]] }.validate().is_err());
        assert!(Hotkey { groups: vec![vec![59, 62], vec![49]] }.validate().is_ok());
    }
}
