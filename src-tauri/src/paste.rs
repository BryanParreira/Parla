use std::{thread, time::Duration};

use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;

pub enum Delivery {
    Pasted,
    Copied,
}

pub fn insert(app: &AppHandle, text: &str) -> Result<Delivery, String> {
    if text.trim().is_empty() {
        return Ok(Delivery::Pasted);
    }

    let clipboard = app.clipboard();

    if !accessibility_granted() {
        clipboard
            .write_text(text.to_string())
            .map_err(|e| e.to_string())?;
        return Ok(Delivery::Copied);
    }

    let previous = clipboard.read_text().ok();
    clipboard
        .write_text(text.to_string())
        .map_err(|e| e.to_string())?;
    native::paste()?;

    // The target app reads the pasteboard asynchronously; restoring too early
    // would paste the old contents instead.
    thread::sleep(Duration::from_millis(200));
    if let Some(previous) = previous {
        let _ = clipboard.write_text(previous);
    }

    Ok(Delivery::Pasted)
}

/// The text selected in the frontmost app. Accessibility answers directly in native
/// apps; browsers and Electron apps often don't expose it, so copying is the fallback.
pub fn selection(app: &AppHandle) -> String {
    let direct = crate::engine::selected_text();
    if !direct.trim().is_empty() || !accessibility_granted() {
        return direct;
    }

    let clipboard = app.clipboard();
    let previous = clipboard.read_text().ok();
    // Clearing first means an app that ignores the copy can't hand back stale contents.
    let _ = clipboard.write_text(String::new());
    let copied = match native::copy() {
        Ok(()) => {
            thread::sleep(Duration::from_millis(150));
            clipboard.read_text().unwrap_or_default()
        }
        Err(_) => String::new(),
    };
    if let Some(previous) = previous {
        let _ = clipboard.write_text(previous);
    }
    copied
}

/// Takes a paste back out by deleting exactly what was inserted. Only sound while the
/// text is still the last thing in the field, which is why the caller offers it once and
/// forgets the text afterwards.
pub fn undo(text: &str) -> Result<(), String> {
    if !accessibility_granted() {
        return Err("Parla needs Accessibility permission to take a dictation back.".into());
    }
    let presses = erase_count(text);
    if presses == 0 {
        return Ok(());
    }
    if presses > MAX_UNDO_PRESSES {
        return Err("That dictation is too long to take back.".into());
    }
    for _ in 0..presses {
        native::delete()?;
    }
    Ok(())
}

/// How many times Delete has to be pressed to remove the text. macOS deletes a whole
/// character with its accents and its emoji modifiers in one press, so the pieces that
/// only ever attach to the character before them are not counted.
fn erase_count(text: &str) -> usize {
    let mut count = 0usize;
    let mut joining = false;
    for character in text.chars() {
        let attaches = matches!(character,
            // Combining marks, variation selectors and emoji skin tones.
            '\u{0300}'..='\u{036F}' | '\u{FE00}'..='\u{FE0F}' | '\u{1F3FB}'..='\u{1F3FF}');
        let zero_width_joiner = character == '\u{200D}';
        if !attaches && !zero_width_joiner && !joining {
            count += 1;
        }
        joining = zero_width_joiner;
    }
    count
}

pub use native::accessibility_granted;

// Past this the deletions would take long enough that the user would type over them.
const MAX_UNDO_PRESSES: usize = 1200;

/// macOS: keystrokes posted through Quartz, which needs the Accessibility permission.
#[cfg(not(desktop_engine))]
mod native {
    use std::{ffi::c_void, thread, time::Duration};

    pub fn accessibility_granted() -> bool {
        unsafe { AXIsProcessTrusted() }
    }

    pub fn paste() -> Result<(), String> {
        press(KEYCODE_V, FLAG_COMMAND, Duration::from_millis(15))
    }

    pub fn copy() -> Result<(), String> {
        press(KEYCODE_C, FLAG_COMMAND, Duration::from_millis(15))
    }

    pub fn delete() -> Result<(), String> {
        press(KEYCODE_DELETE, 0, UNDO_KEY_GAP)
    }

fn press(keycode: u16, flags: u64, gap: Duration) -> Result<(), String> {
    unsafe {
        let source = CGEventSourceCreate(EVENT_SOURCE_HID_SYSTEM_STATE);
        if source.is_null() {
            return Err("Could not create a keyboard event source.".into());
        }

        let down = CGEventCreateKeyboardEvent(source, keycode, true);
        let up = CGEventCreateKeyboardEvent(source, keycode, false);
        let result = if down.is_null() || up.is_null() {
            Err("Could not create the keystroke.".into())
        } else {
            // Overrides whatever modifiers are still physically held from the hotkey.
            CGEventSetFlags(down, flags);
            CGEventSetFlags(up, flags);
            CGEventPost(HID_EVENT_TAP, down);
            thread::sleep(gap);
            CGEventPost(HID_EVENT_TAP, up);
            Ok(())
        };

        for event in [down, up, source] {
            if !event.is_null() {
                CFRelease(event);
            }
        }
        result
    }
}

const KEYCODE_C: u16 = 8;
const KEYCODE_V: u16 = 9;
const KEYCODE_DELETE: u16 = 51;
// Long enough for the app in front to see every press, short enough that taking a whole
// paragraph back still feels immediate.
const UNDO_KEY_GAP: Duration = Duration::from_micros(900);
const FLAG_COMMAND: u64 = 1 << 20;
const HID_EVENT_TAP: u32 = 0;
const EVENT_SOURCE_HID_SYSTEM_STATE: i32 = 1;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventSourceCreate(state_id: i32) -> *mut c_void;
    fn CGEventCreateKeyboardEvent(source: *mut c_void, keycode: u16, key_down: bool)
        -> *mut c_void;
    fn CGEventSetFlags(event: *mut c_void, flags: u64);
    fn CGEventPost(tap: u32, event: *mut c_void);
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(cf: *mut c_void);
}
}

/// Windows and Linux: keystrokes through enigo. Windows needs no permission; Linux needs
/// an X11 session, since Wayland doesn't let apps type into other apps.
#[cfg(desktop_engine)]
mod native {
    use std::{thread, time::Duration};

    use enigo::{Direction, Enigo, Key, Keyboard, Settings};

    // The modifier that pastes and copies: Command when testing on a Mac.
    const SHORTCUT: Key = if cfg!(target_os = "macos") { Key::Meta } else { Key::Control };

    fn keyboard() -> Result<Enigo, String> {
        Enigo::new(&Settings::default()).map_err(|e| format!("Couldn't type into other apps: {e}"))
    }

    pub fn accessibility_granted() -> bool {
        keyboard().is_ok()
    }

    fn chord(letter: char) -> Result<(), String> {
        let mut keys = keyboard()?;
        let fail = |e: enigo::InputError| e.to_string();
        keys.key(SHORTCUT, Direction::Press).map_err(fail)?;
        thread::sleep(Duration::from_millis(10));
        let pressed = keys.key(Key::Unicode(letter), Direction::Click).map_err(fail);
        thread::sleep(Duration::from_millis(10));
        keys.key(SHORTCUT, Direction::Release).map_err(fail)?;
        pressed
    }

    pub fn paste() -> Result<(), String> {
        chord('v')
    }

    pub fn copy() -> Result<(), String> {
        chord('c')
    }

    pub fn delete() -> Result<(), String> {
        keyboard()?.key(Key::Backspace, Direction::Click).map_err(|e| e.to_string())?;
        thread::sleep(Duration::from_micros(900));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_what_one_delete_press_removes() {
        assert_eq!(erase_count(""), 0);
        assert_eq!(erase_count("Meet at 3."), 10);
        // Accented letters arrive precomposed, so each is one press.
        assert_eq!(erase_count("olá"), 3);
        // A combining accent rides along with the letter it sits on.
        assert_eq!(erase_count("ola\u{0301}"), 3);
        // Skin tone and joined emoji come out in one press each.
        assert_eq!(erase_count("\u{1F44B}\u{1F3FD}"), 1);
        assert_eq!(erase_count("\u{1F468}\u{200D}\u{1F4BB}"), 1);
    }
}
