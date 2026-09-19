use std::{ffi::c_void, thread, time::Duration};

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
    press_cmd(KEYCODE_V)?;

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
    let copied = match press_cmd(KEYCODE_C) {
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

pub fn accessibility_granted() -> bool {
    unsafe { AXIsProcessTrusted() }
}

fn press_cmd(keycode: u16) -> Result<(), String> {
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
            CGEventSetFlags(down, FLAG_COMMAND);
            CGEventSetFlags(up, FLAG_COMMAND);
            CGEventPost(HID_EVENT_TAP, down);
            thread::sleep(Duration::from_millis(15));
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
