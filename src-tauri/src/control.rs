//! A local socket the `parla` command talks to while the app is running, so scripts,
//! coding agents and the MCP server can start a recording, switch modes or change the
//! dictionary without going through the window. It lives in Parla's own data folder,
//! is readable by the user alone, and never listens on the network.

use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::{settings::SettingsStore, Command, Mode};

pub const SOCKET: &str = "parla.sock";

#[derive(Deserialize)]
#[serde(tag = "cmd", rename_all = "kebab-case")]
pub enum Request {
    Record,
    Start,
    Stop,
    Cancel,
    Switch { mode: String },
    Settings,
    Transcribe { path: String },
    Notify { text: String },
    VocabAdd { term: String },
    VocabRemove { term: String },
    SnippetSet { trigger: String, text: String },
    SnippetRemove { trigger: String },
    Status,
}

/// Carries out one request from the command line.
fn answer(app: &AppHandle, request: Request) -> Result<Value, String> {
    let send = |command| crate::send_to_worker(app, command);
    match request {
        Request::Record => send(Command::Toggle),
        Request::Start => send(Command::Begin),
        Request::Stop => send(Command::End),
        Request::Cancel => send(Command::Cancel(Mode::Dictate)),
        Request::Settings => crate::show_page(app, "settings"),
        Request::Notify { text } => send(Command::Notice(text)),
        Request::Transcribe { path } => crate::transcribe_file(app, path.into()),
        Request::Switch { mode } => {
            let id = app
                .state::<SettingsStore>()
                .get()
                .find_mode(&mode)
                .map(|found| found.id.clone())
                .ok_or_else(|| format!("There is no mode called {mode}."))?;
            send(Command::SwitchMode(id));
        }
        Request::VocabAdd { term } => edit(app, |settings| {
            if !settings.dictionary.iter().any(|kept| kept.eq_ignore_ascii_case(&term)) {
                settings.dictionary.push(term);
            }
        })?,
        Request::VocabRemove { term } => {
            edit(app, |settings| settings.dictionary.retain(|kept| !kept.eq_ignore_ascii_case(&term)))?
        }
        Request::SnippetSet { trigger, text } => edit(app, |settings| {
            settings.snippets.retain(|kept| !kept.trigger.eq_ignore_ascii_case(&trigger));
            settings.snippets.push(crate::settings::Snippet { trigger, text });
        })?,
        Request::SnippetRemove { trigger } => {
            edit(app, |settings| settings.snippets.retain(|kept| !kept.trigger.eq_ignore_ascii_case(&trigger)))?
        }
        Request::Status => {
            let settings = app.state::<SettingsStore>().get();
            return Ok(json!({
                "ok": true,
                "model": crate::engine::status().state,
                "mode": settings.active().name,
            }));
        }
    }
    Ok(json!({ "ok": true }))
}

/// Changes the settings the way the window would, so the running app never writes back
/// an older copy over them.
fn edit(app: &AppHandle, change: impl FnOnce(&mut crate::settings::Settings)) -> Result<(), String> {
    let store = app.state::<SettingsStore>();
    let mut next = store.get();
    change(&mut next);
    next.sanitize();
    next.validate()?;
    crate::update_settings(app.clone(), next)?;
    let _ = tauri::Emitter::emit(app, "settings-changed", ());
    Ok(())
}

#[cfg(unix)]
pub fn serve(app: AppHandle) {
    use std::{
        io::{BufRead, BufReader, Write},
        os::unix::{fs::PermissionsExt, net::UnixListener},
    };

    let path = crate::storage::path(&app, SOCKET);
    // Left behind by a Parla that didn't quit cleanly.
    let _ = std::fs::remove_file(&path);
    let Ok(listener) = UnixListener::bind(&path) else { return };
    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let app = app.clone();
            std::thread::spawn(move || {
                let mut line = String::new();
                let mut reader = BufReader::new(&stream);
                if reader.read_line(&mut line).is_err() {
                    return;
                }
                let reply = match serde_json::from_str::<Request>(&line) {
                    Ok(request) => answer(&app, request).unwrap_or_else(|error| json!({ "ok": false, "error": error })),
                    Err(error) => json!({ "ok": false, "error": format!("Unknown request: {error}") }),
                };
                let _ = writeln!(&stream, "{reply}");
            });
        }
    });
}

/// Windows has no Unix sockets; there the command line reads and writes the files.
#[cfg(not(unix))]
pub fn serve(_app: AppHandle) {}

/// Standard base64, for handing a recording to the window as a data URL.
pub fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for (index, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if index <= chunk.len() {
                out.push(ALPHABET[(n >> shift & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn encodes_base64() {
        assert_eq!(super::base64(b""), "");
        assert_eq!(super::base64(b"f"), "Zg==");
        assert_eq!(super::base64(b"fo"), "Zm8=");
        assert_eq!(super::base64(b"foo"), "Zm9v");
        assert_eq!(super::base64(b"foobar"), "Zm9vYmFy");
    }
}
