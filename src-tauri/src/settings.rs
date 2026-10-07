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
const MAX_MODES: usize = 16;
const MAX_MODE_NAME_LENGTH: usize = 32;
const MAX_MODE_INSTRUCTIONS: usize = 1500;
const MAX_MODE_EXAMPLES: usize = 3;
const MAX_EXAMPLE_LENGTH: usize = 600;
const MAX_MODE_APPS: usize = 20;
const MAX_MICS: usize = 12;
/// How long recordings may be kept, in days. Past a year nobody is coming back for them.
const MAX_KEEP_AUDIO_DAYS: u32 = 365;
const MAX_IDLE_MINUTES: u32 = 240;
/// What a mode starts from. "default" is Parla as it has always worked; the others pin a
/// style, and "custom" runs the user's own instructions.
pub const MODE_PRESETS: [&str; 6] = ["default", "voice", "message", "email", "note", "custom"];
pub const DEFAULT_MODE: &str = "default";

/// One dictated input and the text the user wants out of it, shown to the model so it
/// copies the pattern instead of guessing from a description.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ModeExample {
    pub input: String,
    pub output: String,
}

/// What a mode lets its cleanup see besides the dictation itself. All of it is read
/// through Accessibility or the pasteboard on this Mac and only ever reaches the
/// on-device model.
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct ModeContext {
    /// The text selected when recording started.
    pub selection: bool,
    /// Text copied up to three seconds before recording, or while it ran.
    pub clipboard: bool,
    /// The app, its window title, the date and time, and the user's name.
    pub app: bool,
}

/// A way of turning speech into text: plain transcription, a chat message, an email, a
/// note, or the user's own instructions. Picked by hand, by its own shortcut, or by the
/// app or website being dictated into.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct ModeConfig {
    /// Stable key, so renaming a mode keeps its shortcut and deep links working.
    pub id: String,
    pub name: String,
    pub preset: String,
    /// The custom preset's own instructions, or extra guidance for the others.
    pub instructions: String,
    pub examples: Vec<ModeExample>,
    pub context: ModeContext,
    /// App names, bundle id fragments or website domains that switch to this mode.
    pub apps: Vec<String>,
    /// Starts a recording straight in this mode.
    pub hotkey: Option<Hotkey>,
    /// Language code the result is written in, translating when needed; None keeps the
    /// language spoken.
    pub translate: Option<String>,
    /// Overrides the cleanup level; None follows the Writing settings.
    pub level: Option<CleanupLevel>,
}

impl Default for ModeConfig {
    fn default() -> Self {
        Self {
            id: DEFAULT_MODE.into(),
            name: "Default".into(),
            preset: DEFAULT_MODE.into(),
            instructions: String::new(),
            examples: Vec::new(),
            context: ModeContext::default(),
            apps: Vec::new(),
            hotkey: None,
            translate: None,
            level: None,
        }
    }
}

// The Mac engine reads presets in Swift; these are the Rust engine's view of them.
#[cfg_attr(not(desktop_engine), allow(dead_code))]
impl ModeConfig {
    fn preset(id: &str, name: &str, preset: &str) -> Self {
        Self { id: id.into(), name: name.into(), preset: preset.into(), ..Self::default() }
    }

    /// Whether this mode's text goes through Enhance at all.
    pub fn cleans_up(&self) -> bool {
        self.preset != "voice"
    }

    /// The style a preset pins, overriding whatever the app would pick.
    pub fn style(&self) -> Option<&'static str> {
        match self.preset.as_str() {
            "message" => Some("casual"),
            "email" => Some("email"),
            "note" => Some("notes"),
            _ => None,
        }
    }
}

fn default_modes() -> Vec<ModeConfig> {
    vec![
        ModeConfig::default(),
        ModeConfig::preset("voice", "Voice to Text", "voice"),
        ModeConfig::preset("message", "Message", "message"),
        ModeConfig::preset("email", "Email", "email"),
        ModeConfig::preset("note", "Note", "note"),
    ]
}

/// Lines the engine reads to pick a mode by app or site: "match<TAB>mode id".
pub fn mode_rules_text(modes: &[ModeConfig]) -> String {
    modes
        .iter()
        .flat_map(|mode| mode.apps.iter().map(move |app| format!("{}\t{}", app.replace(['\t', '\n'], " "), mode.id)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// What other audio does while Parla listens.
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum OtherAudio {
    Keep,
    Lower,
    #[default]
    Mute,
}

impl<'de> Deserialize<'de> for OtherAudio {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match String::deserialize(deserializer).as_deref() {
            Ok("keep") => Self::Keep,
            Ok("lower") => Self::Lower,
            _ => Self::Mute,
        })
    }
}

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
    pub modes: Vec<ModeConfig>,
    /// The mode used when no app rule or mode shortcut picks another.
    pub active_mode: String,
    /// Held while recording, tapping cycles through the modes; or pressed while idle
    /// to switch the active one.
    pub mode_hotkey: Option<Hotkey>,
    /// Holding Shift as the dictation ends presses Return after the paste.
    pub shift_to_send: bool,
    /// Esc throws a recording away, asking again for one longer than 30 seconds.
    pub esc_cancels: bool,
    /// Microphones to prefer, best first, when no single one is chosen.
    pub preferred_mics: Vec<String>,
    /// Microphones never offered, like virtual devices nobody talks into.
    pub hidden_mics: Vec<String>,
    /// Turns the microphone's input volume up while recording, then puts it back.
    pub boost_input: bool,
    /// Turned down or silenced, when `mute_other_audio` is on.
    pub other_audio: OtherAudio,
    /// Cuts long silences out before transcribing, which keeps Whisper from inventing
    /// words for them.
    pub trim_silence: bool,
    /// Unloads the big speech models after this many idle minutes; 0 keeps them loaded.
    pub unload_after_minutes: u32,
    /// Keeps each dictation's audio for this many days, so it can be played back and
    /// transcribed again; 0 keeps none.
    pub keep_audio_days: u32,
    /// Clicking the menu bar icon starts and stops a recording; right-click opens the menu.
    pub tray_click_records: bool,
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
            modes: default_modes(),
            active_mode: DEFAULT_MODE.into(),
            mode_hotkey: None,
            shift_to_send: true,
            esc_cancels: true,
            preferred_mics: Vec::new(),
            hidden_mics: Vec::new(),
            boost_input: false,
            other_audio: OtherAudio::Mute,
            trim_silence: true,
            unload_after_minutes: 0,
            keep_audio_days: 0,
            tray_click_records: false,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        self.hotkey.validate()?;
        // Each optional shortcut has to clear the dictation key and every optional one
        // named before it, so no two shortcuts can ever fire on the same keys.
        let mut optional: Vec<(String, &Option<Hotkey>)> = vec![
            ("pastes the last dictation".into(), &self.repeat_hotkey),
            ("is Command Mode".into(), &self.command_hotkey),
            ("undoes the last paste".into(), &self.undo_hotkey),
            ("opens the transform menu".into(), &self.transform_hotkey),
            ("switches modes".into(), &self.mode_hotkey),
        ];
        for mode in &self.modes {
            optional.push((format!("starts {}", mode.name), &mode.hotkey));
        }
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

    /// The mode a recording uses when nothing more specific picks one.
    pub fn active(&self) -> &ModeConfig {
        self.mode(&self.active_mode).unwrap_or(&self.modes[0])
    }

    pub fn mode(&self, id: &str) -> Option<&ModeConfig> {
        self.modes.iter().find(|mode| mode.id == id)
    }

    /// A mode named in a deep link or on the command line: its id, or its name in any case.
    pub fn find_mode(&self, wanted: &str) -> Option<&ModeConfig> {
        let wanted = wanted.trim();
        self.mode(wanted).or_else(|| self.modes.iter().find(|mode| mode.name.eq_ignore_ascii_case(wanted)))
    }

    /// The mode after `id`, wrapping around, for cycling with the mode shortcut.
    pub fn next_mode(&self, id: &str) -> &ModeConfig {
        let index = self.modes.iter().position(|mode| mode.id == id).unwrap_or(0);
        &self.modes[(index + 1) % self.modes.len()]
    }

    /// The microphone to record from: the one chosen, else the most preferred one that
    /// is plugged in, else the system default.
    pub fn microphone(&self, available: &[crate::engine::InputDevice]) -> Option<String> {
        let present = |uid: &String| available.iter().any(|device| &device.uid == uid);
        self.input_device
            .clone()
            .or_else(|| self.preferred_mics.iter().find(|uid| present(uid)).cloned())
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
        self.language = self.language.take().and_then(language_code);

        self.sanitize_modes();

        for list in [&mut self.preferred_mics, &mut self.hidden_mics] {
            let mut kept: Vec<String> = Vec::new();
            for uid in list.drain(..) {
                if !uid.trim().is_empty() && !kept.contains(&uid) {
                    kept.push(uid);
                }
            }
            kept.truncate(MAX_MICS);
            *list = kept;
        }
        self.keep_audio_days = self.keep_audio_days.min(MAX_KEEP_AUDIO_DAYS);
        self.unload_after_minutes = self.unload_after_minutes.min(MAX_IDLE_MINUTES);
    }

    /// Keeps the Default mode first and every mode well formed, so the rest of Parla can
    /// always find one to use.
    fn sanitize_modes(&mut self) {
        let clip = |text: &str, limit: usize| text.trim().chars().take(limit).collect::<String>();
        let mut modes: Vec<ModeConfig> = Vec::new();
        for mode in self.modes.drain(..) {
            let id = clip(&mode.id, MAX_MODE_NAME_LENGTH);
            let name = clip(&mode.name, MAX_MODE_NAME_LENGTH);
            if id.is_empty() || name.is_empty() || modes.iter().any(|kept| kept.id == id) {
                continue;
            }
            let preset = if MODE_PRESETS.contains(&mode.preset.as_str()) { mode.preset } else { "custom".into() };
            let examples = mode
                .examples
                .into_iter()
                .map(|example| ModeExample {
                    input: clip(&example.input, MAX_EXAMPLE_LENGTH),
                    output: clip(&example.output, MAX_EXAMPLE_LENGTH),
                })
                .filter(|example| !example.input.is_empty() && !example.output.is_empty())
                .take(MAX_MODE_EXAMPLES)
                .collect();
            let mut apps: Vec<String> = Vec::new();
            for app in mode.apps {
                let app = clip(&app, MAX_APP_NAME_LENGTH);
                if !app.is_empty() && !apps.iter().any(|kept| kept.eq_ignore_ascii_case(&app)) {
                    apps.push(app);
                }
            }
            apps.truncate(MAX_MODE_APPS);
            modes.push(ModeConfig {
                id,
                name,
                preset,
                instructions: clip(&mode.instructions, MAX_MODE_INSTRUCTIONS),
                examples,
                apps,
                translate: mode.translate.and_then(language_code),
                ..mode
            });
        }
        // The Default mode is how Parla worked before modes existed, so there is always one.
        match modes.iter().position(|mode| mode.id == DEFAULT_MODE) {
            Some(0) => {}
            Some(index) => {
                let default = modes.remove(index);
                modes.insert(0, default);
            }
            None => modes.insert(0, ModeConfig::default()),
        }
        modes[0].preset = DEFAULT_MODE.into();
        modes.truncate(MAX_MODES);
        self.modes = modes;
        if self.mode(&self.active_mode).is_none() {
            self.active_mode = DEFAULT_MODE.into();
        }
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

/// Two or three lower-case letters, or nothing.
fn language_code(code: String) -> Option<String> {
    let code = code.trim().to_lowercase();
    ((2..=3).contains(&code.len()) && code.chars().all(|c| c.is_ascii_lowercase())).then_some(code)
}

pub struct SettingsStore {
    path: PathBuf,
    current: Mutex<Settings>,
}

impl SettingsStore {
    pub fn load(app: &AppHandle) -> Self {
        let path = storage::path(app, "settings.json");
        let mut settings: Settings = storage::read(&path);
        // Files from before modes existed load with an empty list.
        settings.sanitize();
        let current = Mutex::new(settings);
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

    #[test]
    fn older_files_get_the_built_in_modes() {
        let mut settings: Settings = serde_json::from_str("{}").unwrap();
        settings.sanitize();
        assert_eq!(settings.modes[0].id, DEFAULT_MODE);
        assert_eq!(settings.modes.len(), 5);
        assert_eq!(settings.active().id, DEFAULT_MODE);
        assert!(settings.shift_to_send && settings.esc_cancels && settings.trim_silence);
        assert!(settings.keep_audio_days == 0 && !settings.boost_input, "privacy-sensitive extras stay off");
    }

    #[test]
    fn keeps_modes_well_formed() {
        let custom = |id: &str, name: &str, preset: &str| ModeConfig {
            id: id.into(),
            name: name.into(),
            preset: preset.into(),
            ..ModeConfig::default()
        };
        let mut settings = Settings {
            modes: vec![
                custom("reply", " Reply ", "made-up"),
                custom("reply", "Duplicate", "custom"),
                custom("", "No id", "custom"),
                ModeConfig { translate: Some(" ES ".into()), ..custom("es", "Spanish", "message") },
            ],
            active_mode: "gone".into(),
            ..Settings::default()
        };
        settings.sanitize();
        let ids: Vec<&str> = settings.modes.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["default", "reply", "es"]);
        assert_eq!(settings.modes[1].name, "Reply");
        assert_eq!(settings.modes[1].preset, "custom");
        assert_eq!(settings.modes[2].translate.as_deref(), Some("es"));
        assert_eq!(settings.active_mode, DEFAULT_MODE);
        assert_eq!(settings.next_mode("es").id, DEFAULT_MODE);
        assert_eq!(settings.find_mode("spanish").map(|m| m.id.as_str()), Some("es"));
        assert_eq!(mode_rules_text(&settings.modes), "");
    }

    // Keycodes are macOS ones; each platform numbers keys its own way.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_mode_shortcut_must_be_its_own() {
        let mut settings = Settings { mode_hotkey: Some(Hotkey { groups: vec![vec![54]] }), ..Settings::default() };
        settings.modes[1].hotkey = Some(Hotkey { groups: vec![vec![54]] });
        assert!(settings.validate().is_err());
        settings.modes[1].hotkey = Some(Hotkey { groups: vec![vec![59, 62], vec![18]] });
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn prefers_ranked_microphones_that_are_plugged_in() {
        use crate::engine::InputDevice;
        let device = |uid: &str| InputDevice { uid: uid.into(), name: uid.into() };
        let settings = Settings { preferred_mics: vec!["usb".into(), "built-in".into()], ..Settings::default() };
        assert_eq!(settings.microphone(&[device("built-in")]).as_deref(), Some("built-in"));
        assert_eq!(settings.microphone(&[device("usb"), device("built-in")]).as_deref(), Some("usb"));
        assert_eq!(settings.microphone(&[device("airpods")]), None);
    }
}
