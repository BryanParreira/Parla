//! Which keys are held right now, read by polling rather than by intercepting keystrokes,
//! so recording a shortcut and holding one to talk need no keyboard hook. Keycodes are
//! the platform's own: macOS virtual keycodes, Windows virtual-key codes, X11 keycodes.

/// A snapshot of every held key, taken once per poll.
#[derive(Default, Clone, Copy)]
pub struct Held([u64; 4]);

impl Held {
    fn set(&mut self, key: u16) {
        if key < 256 {
            self.0[key as usize / 64] |= 1 << (key % 64);
        }
    }

    pub fn contains(&self, key: u16) -> bool {
        key < 256 && self.0[key as usize / 64] & (1 << (key % 64)) != 0
    }

    /// The held keys a shortcut may use, Caps Lock excepted since it latches.
    pub fn keys(&self) -> impl Iterator<Item = u16> + '_ {
        (0..256u16).filter(|key| self.contains(*key) && *key != CAPS_LOCK)
    }
}

#[cfg(target_os = "macos")]
mod platform {
    /// Caps Lock latches instead of reporting a clean press and release.
    pub const CAPS_LOCK: u16 = 57;
    /// Command, Shift, Option, Control and Globe/Fn, left and right.
    pub const MODIFIERS: &[u16] = &[54, 55, 56, 58, 59, 60, 61, 62, 63];
    // Keycodes above this are function keys the HID system never reports as held.
    pub const MAX_KEYCODE: u16 = 127;
    /// Either Option key.
    pub const DEFAULT_HOTKEY: &[&[u16]] = &[&[58, 61]];
    pub const MODIFIER_HINT: &str = "Include a modifier like ⌘, ⌥, ⌃, ⇧ or fn.";
    pub const SHIFT: &[u16] = &[56, 60];
    pub const ESCAPE: u16 = 53;
    const HID_SYSTEM_STATE: i32 = 1;

    pub fn held() -> super::Held {
        let mut held = super::Held::default();
        for key in 0..=MAX_KEYCODE {
            if unsafe { CGEventSourceKeyState(HID_SYSTEM_STATE, key) } {
                held.set(key);
            }
        }
        held
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventSourceKeyState(state_id: i32, key: u16) -> bool;
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;

    pub const CAPS_LOCK: u16 = 0x14;
    /// Shift, Control, Alt and Windows, left and right.
    pub const MODIFIERS: &[u16] = &[0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0x5B, 0x5C];
    pub const MAX_KEYCODE: u16 = 0xFE;
    /// Control and Windows together, as in other dictation apps: Control alone is too
    /// common in shortcuts, and Alt alone opens app menus when it is let go.
    pub const DEFAULT_HOTKEY: &[&[u16]] = &[&[0xA2, 0xA3], &[0x5B, 0x5C]];
    pub const MODIFIER_HINT: &str = "Include a modifier like Ctrl, Alt, Shift or Windows.";
    pub const SHIFT: &[u16] = &[0xA0, 0xA1];
    pub const ESCAPE: u16 = 0x1B;
    // Mouse buttons, and the side-blind Shift, Control and Alt codes that shadow the
    // left and right ones, are never shortcut keys.
    const SKIPPED: &[u16] = &[0x01, 0x02, 0x04, 0x05, 0x06, 0x10, 0x11, 0x12];

    pub fn held() -> super::Held {
        let mut held = super::Held::default();
        for key in 1..=MAX_KEYCODE {
            if !SKIPPED.contains(&key) && unsafe { GetAsyncKeyState(key as i32) } as u16 & 0x8000 != 0 {
                held.set(key);
            }
        }
        held
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use std::sync::Mutex;

    use x11rb::{protocol::xproto::ConnectionExt, rust_connection::RustConnection};

    pub const CAPS_LOCK: u16 = 66;
    /// Shift, Control, Alt, AltGr and Super, left and right, on a standard layout.
    pub const MODIFIERS: &[u16] = &[50, 62, 37, 105, 64, 108, 92, 133, 134];
    pub const MAX_KEYCODE: u16 = 255;
    /// Control and Super together, as on Windows.
    pub const DEFAULT_HOTKEY: &[&[u16]] = &[&[37, 105], &[133, 134]];
    pub const MODIFIER_HINT: &str = "Include a modifier like Ctrl, Alt, Shift or Super.";
    pub const SHIFT: &[u16] = &[50, 62];
    pub const ESCAPE: u16 = 9;

    // One connection for the life of the app. Wayland sessions have no X server for
    // Parla to read, so the shortcut only works under X11 or XWayland's view of X apps.
    static CONNECTION: Mutex<Option<RustConnection>> = Mutex::new(None);

    pub fn held() -> super::Held {
        let mut held = super::Held::default();
        let mut connection = CONNECTION.lock().unwrap();
        if connection.is_none() {
            *connection = x11rb::connect(None).ok().map(|(c, _)| c);
        }
        let Some(conn) = connection.as_ref() else { return held };
        match conn.query_keymap().ok().and_then(|cookie| cookie.reply().ok()) {
            Some(reply) => {
                for (byte, bits) in reply.keys.iter().enumerate() {
                    for bit in 0..8 {
                        if bits & (1 << bit) != 0 {
                            held.set((byte * 8 + bit) as u16);
                        }
                    }
                }
            }
            // The X server went away; reconnect on the next poll.
            None => *connection = None,
        }
        held
    }
}

pub use platform::{held, CAPS_LOCK, DEFAULT_HOTKEY, ESCAPE, MAX_KEYCODE, MODIFIERS, MODIFIER_HINT, SHIFT};

impl Held {
    pub fn shift(&self) -> bool {
        SHIFT.iter().any(|key| self.contains(*key))
    }
}
