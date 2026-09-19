use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
    },
    thread,
    time::{Duration, Instant},
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::{
    settings::{Hotkey, SettingsStore, CAPS_LOCK},
    Command, Mode,
};

const POLL_INTERVAL: Duration = Duration::from_millis(12);
// Brief taps are usually the start of a shortcut like Option+Arrow, not dictation.
const HOLD_THRESHOLD: Duration = Duration::from_millis(150);
// Two taps closer together than this are a deliberate double-tap rather than two
// unrelated brushes against the key.
const DOUBLE_TAP_WINDOW: Duration = Duration::from_millis(400);
// A hands-free recording nobody stopped is almost certainly forgotten, and the audio
// buffer grows for as long as it runs.
const MAX_HANDS_FREE: Duration = Duration::from_secs(300);
// Long enough to stop and think mid-sentence, short enough that a forgotten recording
// ends on its own.
const SILENCE_STOP: Duration = Duration::from_secs(8);
// Microphone RMS above this counts as the user talking. Room noise usually sits well
// below it and speech well above.
const VOICE_LEVEL: f32 = 0.006;
const HID_SYSTEM_STATE: i32 = 1;
// Keycodes above this are function keys the HID system never reports as held.
const MAX_KEYCODE: u16 = 127;

/// Set while the settings window is recording a new shortcut. The same poll loop
/// reports the keys being held instead of starting dictation, so recording needs no
/// event tap and no extra permission.
#[derive(Default)]
pub struct CaptureState {
    active: AtomicBool,
}

impl CaptureState {
    pub fn set(&self, active: bool) {
        self.active.store(active, Ordering::Relaxed);
    }

    fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }
}

#[derive(Clone, Serialize)]
struct CaptureEvent {
    codes: Vec<u16>,
    /// True once every key has been let go, which is when the combination is complete.
    done: bool,
}

fn is_down(key: u16) -> bool {
    unsafe { CGEventSourceKeyState(HID_SYSTEM_STATE, key) }
}

fn pressed_keys() -> Vec<u16> {
    (0..=MAX_KEYCODE)
        .filter(|key| *key != CAPS_LOCK && is_down(*key))
        .collect()
}

fn is_held(hotkey: &Hotkey) -> bool {
    !hotkey.groups.is_empty()
        && hotkey
            .groups
            .iter()
            .all(|group| group.iter().any(|key| is_down(*key)))
}

/// Any other key held alongside the shortcut means the user is typing something like
/// Option+Arrow, not talking. Only worth scanning for while the shortcut itself is held.
fn is_interfering(hotkey: &Hotkey) -> bool {
    let keys = hotkey.keys();
    (0..=MAX_KEYCODE)
        .filter(|key| *key != CAPS_LOCK && !keys.contains(key))
        .any(is_down)
}

#[derive(Clone, Copy)]
enum State {
    Idle,
    Pending(Instant),
    Active,
    /// Recording started by a double-tap, which outlives the key being let go.
    Latched(Instant),
    /// Hands-free recording whose stopping keypress is still down.
    Stopping,
    Blocked,
}

#[derive(Default)]
struct Watcher {
    state: Option<State>,
    last_tap: Option<Instant>,
    /// When the microphone last heard speech during a hands-free recording.
    last_voice: Option<Instant>,
}

impl Watcher {
    fn recording(&self) -> bool {
        matches!(
            self.state,
            Some(State::Active) | Some(State::Latched(_)) | Some(State::Stopping)
        )
    }

    fn reset(&mut self) {
        self.state = None;
        self.last_tap = None;
        self.last_voice = None;
    }

    /// True once a hands-free recording has gone quiet for long enough to end it.
    fn gone_silent(&mut self) -> bool {
        let now = Instant::now();
        if crate::engine::level() >= VOICE_LEVEL {
            self.last_voice = Some(now);
        }
        self.last_voice
            .is_some_and(|at| now.duration_since(at) >= SILENCE_STOP)
    }

    fn tick(
        &mut self,
        hotkey: &Hotkey,
        hands_free: bool,
        auto_stop: bool,
        mode: Mode,
        commands: &Sender<Command>,
    ) {
        let held = is_held(hotkey);
        let blocked = held && is_interfering(hotkey);
        let send = |command| {
            let _ = commands.send(command);
        };

        self.state = Some(match self.state.unwrap_or(State::Idle) {
            State::Idle if held && blocked => State::Blocked,
            State::Idle if held => State::Pending(Instant::now()),
            State::Idle => State::Idle,

            // A tap too short to dictate with. A second one right after starts a
            // recording that carries on once the key is released.
            State::Pending(_) if !held => {
                let double = hands_free
                    && self
                        .last_tap
                        .is_some_and(|at| at.elapsed() <= DOUBLE_TAP_WINDOW);
                self.last_tap = if double { None } else { Some(Instant::now()) };
                if double {
                    send(Command::Start(mode));
                    // Counting from the start means a latch nobody speaks into ends too.
                    self.last_voice = Some(Instant::now());
                    State::Latched(Instant::now())
                } else {
                    State::Idle
                }
            }
            State::Pending(_) if blocked => State::Blocked,
            State::Pending(since) if since.elapsed() >= HOLD_THRESHOLD => {
                self.last_tap = None;
                send(Command::Start(mode));
                State::Active
            }
            pending @ State::Pending(_) => pending,

            State::Active if !held => {
                send(Command::Stop(mode));
                State::Idle
            }
            State::Active if blocked => {
                send(Command::Cancel(mode));
                State::Blocked
            }
            State::Active => State::Active,

            // Pressing the key again ends a hands-free recording, but only once it is
            // released, so the keypress itself never lands in the transcript.
            State::Latched(_) if held => State::Stopping,
            State::Latched(since) if since.elapsed() >= MAX_HANDS_FREE => {
                send(Command::Stop(mode));
                State::Idle
            }
            State::Latched(_) if auto_stop && self.gone_silent() => {
                send(Command::Stop(mode));
                State::Idle
            }
            latched @ State::Latched(_) => latched,

            State::Stopping if !held => {
                self.last_tap = None;
                send(Command::Stop(mode));
                State::Idle
            }
            State::Stopping => State::Stopping,

            State::Blocked if !held => State::Idle,
            State::Blocked => State::Blocked,
        });
    }
}

/// Fires once per hold, for shortcuts that trigger an action instead of opening and
/// closing a recording.
#[derive(Default)]
struct TapWatcher {
    state: Option<State>,
}

impl TapWatcher {
    fn tick(&mut self, hotkey: &Hotkey, commands: &Sender<Command>, command: fn() -> Command) {
        let held = is_held(hotkey);
        let blocked = held && is_interfering(hotkey);

        self.state = Some(match self.state.unwrap_or(State::Idle) {
            State::Idle if held && blocked => State::Blocked,
            State::Idle if held => State::Pending(Instant::now()),
            State::Pending(_) if !held => State::Idle,
            State::Pending(_) if blocked => State::Blocked,
            State::Pending(since) if since.elapsed() >= HOLD_THRESHOLD => {
                let _ = commands.send(command());
                // Waiting for the release keeps one hold from firing on every tick.
                State::Active
            }
            pending @ State::Pending(_) => pending,
            State::Active if !held => State::Idle,
            State::Blocked if !held => State::Idle,
            other => other,
        });
    }
}

// A lone modifier can't be registered as a global hotkey, so its state is polled
// instead. Any other key joining in means the user is typing a shortcut.
pub fn spawn(app: AppHandle, commands: Sender<Command>) {
    thread::spawn(move || {
        let mut dictation = Watcher::default();
        let mut command = Watcher::default();
        let mut repeat = TapWatcher::default();
        let mut captured: Vec<u16> = Vec::new();

        loop {
            thread::sleep(POLL_INTERVAL);

            if app.state::<CaptureState>().is_active() {
                // A dictation already under way would otherwise be transcribed and
                // pasted using the keys the user is only trying to record.
                if dictation.recording() {
                    let _ = commands.send(Command::Cancel(Mode::Dictate));
                }
                if command.recording() {
                    let _ = commands.send(Command::Cancel(Mode::Command));
                }
                dictation.reset();
                command.reset();
                repeat = TapWatcher::default();
                capture(&app, &mut captured);
                continue;
            }
            captured.clear();

            let settings = app.state::<SettingsStore>().get();
            dictation.tick(
                &settings.hotkey,
                settings.hands_free,
                settings.auto_stop_silence,
                Mode::Dictate,
                &commands,
            );
            match &settings.command_hotkey {
                // Latching a command would leave nothing to say when it should end.
                Some(hotkey) => command.tick(hotkey, false, false, Mode::Command, &commands),
                // A shortcut removed mid-recording still has to end that recording.
                None if command.recording() => {
                    let _ = commands.send(Command::Cancel(Mode::Command));
                    command.reset();
                }
                None => {}
            }
            if let Some(hotkey) = &settings.repeat_hotkey {
                repeat.tick(hotkey, &commands, || Command::Repeat);
            }
        }
    });
}

// Keys are accumulated rather than sampled, so pressing Control then Space records
// both even though they are never pressed on exactly the same tick.
fn capture(app: &AppHandle, captured: &mut Vec<u16>) {
    let pressed = pressed_keys();

    if pressed.is_empty() {
        if !captured.is_empty() {
            emit(app, captured, true);
            captured.clear();
        }
        return;
    }

    let before = captured.len();
    for key in pressed {
        if !captured.contains(&key) {
            captured.push(key);
        }
    }
    if captured.len() != before {
        emit(app, captured, false);
    }
}

fn emit(app: &AppHandle, codes: &[u16], done: bool) {
    let _ = app.emit(
        "hotkey-capture",
        CaptureEvent {
            codes: codes.to_vec(),
            done,
        },
    );
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventSourceKeyState(state_id: i32, key: u16) -> bool;
}
