//! `parla` on the command line: search and read the history, see stats, manage the
//! dictionary and snippets, control a running Parla, and serve all of it to AI
//! assistants over MCP. Everything is read from Parla's own files on this computer;
//! requests that change something go through the running app when there is one, so
//! it never writes an older copy over them.

use std::{
    collections::BTreeMap,
    io::{BufRead, Write},
    path::{Path, PathBuf},
};

use serde_json::{json, Value};

use crate::{
    history::Entry,
    settings::{Settings, Snippet},
};

const IDENTIFIER: &str = "com.bryanparreira.parla";
const VERSION: &str = env!("CARGO_PKG_VERSION");

const COMMANDS: &[&str] = &[
    "help", "--help", "-h", "search", "history", "read", "stats", "modes", "export", "vocab",
    "snippets", "record", "start", "stop", "cancel", "switch", "settings", "transcribe", "notify", "status",
    "mcp", "install", "setup-claude", "version", "--version",
];

const HELP: &str = "parla — Parla from the command line

Usage: parla <command> [options]

History
  search <words…>          Dictations containing every word (--limit N, --json)
  history                  Recent dictations (--limit N, --since YYYY-MM-DD, --mode NAME, --json)
  read [ID]                One dictation, the latest by default (--raw for the transcript before cleanup)
  stats                    Words, speed and where you dictate (--json)
  export                   Everything, oldest first (--format markdown|jsonl)

Words and modes
  vocab [list|add|remove] <word>        Your words, spelled your way
  snippets [list|set|remove] <phrase> [text]
  modes                    Your modes, the active one marked

Control the running app
  record | start | stop | cancel
  switch <mode>            Make a mode the active one
  transcribe <file>        Transcribe an audio or video file
  notify <message>         Show a message in Parla's pill (--title T, --from-hook for coding agents)
  settings                 Open Settings
  status                   Whether Parla is running, and its mode

Other
  mcp                      Serve the history, words and snippets to an AI assistant over stdio
  install                  Put `parla` on your PATH
  setup-claude             Write the Claude Code plugin and print how to install it
  version

Nothing here goes online. The history never leaves this computer, except what you
choose to hand an AI assistant over MCP.";

/// Runs a command when Parla was started as one, returning its exit code. Anything else,
/// including the arguments macOS adds when it opens an app, starts the app.
pub fn run_from_args() -> Option<i32> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let first = args.first()?;
    if !COMMANDS.contains(&first.as_str()) {
        return None;
    }
    let result = run(&args);
    Some(match result {
        Ok(output) => {
            if !output.is_empty() {
                println!("{output}");
            }
            0
        }
        Err(message) => {
            eprintln!("parla: {message}");
            1
        }
    })
}

/// Options and plain arguments, so "--limit 5 foo" and "foo --limit 5" read the same.
struct Args {
    plain: Vec<String>,
    options: BTreeMap<String, String>,
    flags: Vec<String>,
}

const VALUED: &[&str] = &["--limit", "--since", "--mode", "--format", "--title"];

impl Args {
    fn parse(raw: &[String]) -> Self {
        let mut args = Args { plain: Vec::new(), options: BTreeMap::new(), flags: Vec::new() };
        let mut iter = raw.iter();
        while let Some(arg) = iter.next() {
            if VALUED.contains(&arg.as_str()) {
                if let Some(value) = iter.next() {
                    args.options.insert(arg.clone(), value.clone());
                }
            } else if arg.starts_with("--") {
                args.flags.push(arg.clone());
            } else {
                args.plain.push(arg.clone());
            }
        }
        args
    }

    fn flag(&self, name: &str) -> bool {
        self.flags.iter().any(|flag| flag == name)
    }

    fn limit(&self, default: usize) -> usize {
        self.options.get("--limit").and_then(|n| n.parse().ok()).unwrap_or(default)
    }
}

fn run(raw: &[String]) -> Result<String, String> {
    let command = raw[0].as_str();
    let args = Args::parse(&raw[1..]);
    let json = args.flag("--json");
    match command {
        "help" | "--help" | "-h" => Ok(HELP.into()),
        "version" | "--version" => Ok(format!("parla {VERSION}")),
        "search" => {
            let found = search(&history()?, &args.plain.join(" "), args.limit(20));
            Ok(if json { to_json(&found) } else { list_text(&found) })
        }
        "history" => {
            let entries = recent(&history()?, &args);
            Ok(if json { to_json(&entries) } else { list_text(&entries) })
        }
        "read" => {
            let entries = history()?;
            let entry = match args.plain.first() {
                Some(id) => entries.iter().find(|e| e.id.to_string() == *id),
                None => entries.first(),
            }
            .ok_or("No dictation with that id.")?;
            Ok(if json {
                to_json(entry)
            } else if args.flag("--raw") {
                entry.raw.clone().unwrap_or_else(|| entry.text.clone())
            } else {
                entry.text.clone()
            })
        }
        "stats" => {
            let stats = stats(&history()?);
            Ok(if json { stats.to_string() } else { stats_text(&stats) })
        }
        "modes" => {
            let settings = settings()?;
            let counts = mode_counts(&history()?);
            if json {
                return Ok(json!(settings
                    .modes
                    .iter()
                    .map(|m| json!({
                        "id": m.id, "name": m.name, "preset": m.preset,
                        "active": m.id == settings.active_mode,
                        "dictations": counts.get(&m.name).copied().unwrap_or(0),
                    }))
                    .collect::<Vec<_>>())
                .to_string());
            }
            Ok(settings
                .modes
                .iter()
                .map(|m| {
                    let mark = if m.id == settings.active_mode { "*" } else { " " };
                    format!("{mark} {:<20} {:>5} dictations  ({})", m.name, counts.get(&m.name).unwrap_or(&0), m.id)
                })
                .collect::<Vec<_>>()
                .join("\n"))
        }
        "export" => {
            let mut entries = history()?;
            entries.reverse();
            Ok(match args.options.get("--format").map(String::as_str) {
                Some("jsonl") => entries.iter().map(|e| serde_json::to_string(e).unwrap_or_default()).collect::<Vec<_>>().join("\n"),
                _ => markdown(&entries),
            })
        }
        "vocab" => vocab(&args),
        "snippets" => snippets(&args),
        "record" => control(json!({ "cmd": "record" })),
        "start" => control(json!({ "cmd": "start" })),
        "stop" => control(json!({ "cmd": "stop" })),
        "cancel" => control(json!({ "cmd": "cancel" })),
        "settings" => control(json!({ "cmd": "settings" })),
        "status" => {
            let reply = send(&json!({ "cmd": "status" }))?;
            Ok(format!("Parla is running. Model: {}. Mode: {}.", reply["model"].as_str().unwrap_or("?"), reply["mode"].as_str().unwrap_or("?")))
        }
        "switch" => {
            let mode = args.plain.join(" ");
            if mode.is_empty() {
                return run(&["modes".into()]);
            }
            control(json!({ "cmd": "switch", "mode": mode }))
        }
        "transcribe" => {
            let file = args.plain.first().ok_or("Name the file to transcribe.")?;
            let path = std::fs::canonicalize(file).map_err(|_| format!("Can't find {file}."))?;
            control(json!({ "cmd": "transcribe", "path": path }))
        }
        "notify" => {
            let text = if args.flag("--from-hook") { hook_message()? } else { args.plain.join(" ") };
            let text = match args.options.get("--title") {
                // "Claude Code needs your OK" reads as one line; a quote needs the colon.
            Some(title) if text.starts_with(char::is_lowercase) => format!("{title} {text}"),
            Some(title) if !text.is_empty() => format!("{title}: {text}"),
                Some(title) => title.clone(),
                None => text,
            };
            if text.trim().is_empty() {
                return Ok(String::new());
            }
            // A coding agent's hook must never fail because Parla isn't open.
            match control(json!({ "cmd": "notify", "text": text })) {
                Ok(_) => Ok(String::new()),
                Err(_) if args.flag("--from-hook") => Ok(String::new()),
                Err(error) => Err(error),
            }
        }
        "mcp" => mcp(),
        "install" => install(),
        "setup-claude" => {
            let folder = data_dir()?.join("claude-code");
            write_claude_plugin(&folder)?;
            let shown = folder.to_string_lossy();
            Ok(format!(
                "Plugin written to {shown}\nInstall it with:\n  claude plugin marketplace add \"{shown}\"\n  claude plugin install parla@parla"
            ))
        }
        _ => Ok(HELP.into()),
    }
}

// MARK: - Files

fn data_dir() -> Result<PathBuf, String> {
    // For pointing at a test build's data instead of the installed app's.
    if let Some(dir) = std::env::var_os("PARLA_DATA_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let base = home.map(|home| home.join("Library/Application Support"));
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(target_os = "linux")]
    let base = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).or(home.map(|home| home.join(".local/share")));
    base.map(|base| base.join(IDENTIFIER)).ok_or_else(|| "Can't find Parla's data folder.".into())
}

fn history() -> Result<Vec<Entry>, String> {
    let path = data_dir()?.join("history.json");
    match std::fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str(&raw).map_err(|e| format!("The history file can't be read: {e}")),
        Err(_) => Ok(Vec::new()),
    }
}

fn settings() -> Result<Settings, String> {
    let path = data_dir()?.join("settings.json");
    let mut settings: Settings = std::fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default();
    settings.sanitize();
    Ok(settings)
}

/// Changes the settings: through the running app when there is one, else in the file.
fn change(request: Value, local: impl FnOnce(&mut Settings)) -> Result<String, String> {
    match send(&request) {
        Ok(reply) if reply["ok"] == json!(true) => Ok(String::new()),
        Ok(reply) => Err(reply["error"].as_str().unwrap_or("Parla refused that.").to_string()),
        Err(Unreachable) => {
            let mut settings = settings()?;
            local(&mut settings);
            settings.sanitize();
            settings.validate()?;
            let path = data_dir()?.join("settings.json");
            crate::storage::write(&path, &settings)?;
            Ok(String::new())
        }
    }
}

struct Unreachable;

impl From<Unreachable> for String {
    fn from(_: Unreachable) -> Self {
        "Parla isn't running. Open it and try again.".into()
    }
}

/// Sends one request to the running app. Unreachable means it isn't running.
#[cfg(unix)]
fn send(request: &Value) -> Result<Value, Unreachable> {
    use std::{io::BufReader, os::unix::net::UnixStream};
    let path = data_dir().map_err(|_| Unreachable)?.join(crate::control::SOCKET);
    let mut stream = UnixStream::connect(path).map_err(|_| Unreachable)?;
    writeln!(stream, "{request}").map_err(|_| Unreachable)?;
    let mut line = String::new();
    BufReader::new(&stream).read_line(&mut line).map_err(|_| Unreachable)?;
    serde_json::from_str(&line).map_err(|_| Unreachable)
}

#[cfg(not(unix))]
fn send(_request: &Value) -> Result<Value, Unreachable> {
    Err(Unreachable)
}

/// A request only the running app can carry out.
fn control(request: Value) -> Result<String, String> {
    let reply = send(&request)?;
    if reply["ok"] == json!(true) {
        Ok(String::new())
    } else {
        Err(reply["error"].as_str().unwrap_or("Parla refused that.").to_string())
    }
}

// MARK: - History

fn search(entries: &[Entry], query: &str, limit: usize) -> Vec<Entry> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    entries
        .iter()
        .filter(|entry| {
            let haystack = format!("{} {}", entry.text, entry.raw.as_deref().unwrap_or("")).to_lowercase();
            !words.is_empty() && words.iter().all(|word| haystack.contains(word))
        })
        .take(limit)
        .cloned()
        .collect()
}

fn recent(entries: &[Entry], args: &Args) -> Vec<Entry> {
    let since = args.options.get("--since").and_then(|date| day_start(date));
    let mode = args.options.get("--mode").map(|m| m.to_lowercase());
    entries
        .iter()
        .filter(|entry| since.map_or(true, |since| entry.created_at >= since))
        .filter(|entry| {
            mode.as_ref().map_or(true, |wanted| entry.mode.as_deref().unwrap_or("Default").to_lowercase() == *wanted)
        })
        .take(args.limit(20))
        .cloned()
        .collect()
}

/// Milliseconds at the start of a YYYY-MM-DD day, in UTC.
fn day_start(date: &str) -> Option<u64> {
    let mut parts = date.split('-').map(|p| p.parse::<i64>().ok());
    let (year, month, day) = (parts.next()??, parts.next()??, parts.next()??);
    // Howard Hinnant's days-from-civil.
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u64::try_from(days * 86_400_000).ok()
}

fn list_text(entries: &[Entry]) -> String {
    if entries.is_empty() {
        return "Nothing found.".into();
    }
    entries
        .iter()
        .map(|entry| {
            let line: String = entry.text.split_whitespace().collect::<Vec<_>>().join(" ");
            let short: String = line.chars().take(100).collect();
            let more = if line.chars().count() > 100 { "…" } else { "" };
            format!("{}  {}{}", entry.id, short, more)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn to_json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string_pretty(value).unwrap_or_default()
}

fn markdown(entries: &[Entry]) -> String {
    let mut out = String::from("# Parla history\n");
    for entry in entries {
        out.push_str(&format!("\n## {}\n\n{}\n", entry.id, entry.text));
    }
    out
}

fn mode_counts(entries: &[Entry]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for entry in entries {
        *counts.entry(entry.mode.clone().unwrap_or_else(|| "Default".into())).or_insert(0) += 1;
    }
    counts
}

fn stats(entries: &[Entry]) -> Value {
    let words: usize = entries.iter().map(|e| e.words).sum();
    // Transcribed files weren't typed by hand anyway, so they don't count towards speed.
    let speaking_ms: u64 = entries.iter().filter(|e| e.source.is_none()).map(|e| e.duration_ms).sum();
    let spoken_words: usize = entries.iter().filter(|e| e.source.is_none()).map(|e| e.words).sum();
    let wpm = if speaking_ms > 0 { spoken_words as f64 / (speaking_ms as f64 / 60_000.0) } else { 0.0 };
    let latencies: Vec<u64> = entries.iter().filter_map(|e| e.latency_ms).collect();
    let mut sorted = latencies.clone();
    sorted.sort_unstable();
    let median = sorted.get(sorted.len() / 2).copied();
    let mut apps: BTreeMap<String, usize> = BTreeMap::new();
    for entry in entries {
        if let Some(app) = &entry.app {
            *apps.entry(app.clone()).or_insert(0) += 1;
        }
    }
    let mut top: Vec<(String, usize)> = apps.into_iter().collect();
    top.sort_by(|a, b| b.1.cmp(&a.1));
    top.truncate(5);
    json!({
        "dictations": entries.len(),
        "words": words,
        "wordsPerMinute": wpm.round() as u64,
        // Typing runs at about 40 words a minute.
        "minutesSaved": ((spoken_words as f64 / 40.0) - speaking_ms as f64 / 60_000.0).max(0.0).round() as u64,
        "medianLatencyMs": median,
        "modes": mode_counts(entries),
        "topApps": top.into_iter().map(|(app, count)| json!({ "app": app, "dictations": count })).collect::<Vec<_>>(),
    })
}

fn stats_text(stats: &Value) -> String {
    let mut lines = vec![
        format!("Dictations       {}", stats["dictations"]),
        format!("Words            {}", stats["words"]),
        format!("Words a minute   {}", stats["wordsPerMinute"]),
        format!("Minutes saved    {}", stats["minutesSaved"]),
    ];
    if let Some(ms) = stats["medianLatencyMs"].as_u64() {
        lines.push(format!("Typed in         {ms} ms (median)"));
    }
    if let Some(apps) = stats["topApps"].as_array().filter(|apps| !apps.is_empty()) {
        lines.push("Top apps".into());
        for app in apps {
            lines.push(format!("  {:<20} {}", app["app"].as_str().unwrap_or(""), app["dictations"]));
        }
    }
    lines.join("\n")
}

// MARK: - Words and snippets

fn vocab(args: &Args) -> Result<String, String> {
    let action = args.plain.first().map(String::as_str).unwrap_or("list");
    let term = args.plain.get(1..).map(|rest| rest.join(" ")).unwrap_or_default();
    match action {
        "list" => Ok(settings()?.dictionary.join("\n")),
        "add" if !term.is_empty() => change(json!({ "cmd": "vocab-add", "term": term }), |s| {
            if !s.dictionary.iter().any(|kept| kept.eq_ignore_ascii_case(&term)) {
                s.dictionary.push(term.clone());
            }
        }),
        "remove" if !term.is_empty() => change(json!({ "cmd": "vocab-remove", "term": term }), |s| {
            s.dictionary.retain(|kept| !kept.eq_ignore_ascii_case(&term))
        }),
        _ => Err("Use: parla vocab [list | add <word> | remove <word>]".into()),
    }
}

fn snippets(args: &Args) -> Result<String, String> {
    let action = args.plain.first().map(String::as_str).unwrap_or("list");
    match action {
        "list" => Ok(settings()?
            .snippets
            .iter()
            .map(|s| format!("{} → {}", s.trigger, s.text.replace('\n', "⏎")))
            .collect::<Vec<_>>()
            .join("\n")),
        "set" if args.plain.len() >= 3 => {
            let trigger = args.plain[1].clone();
            let text = args.plain[2..].join(" ");
            change(json!({ "cmd": "snippet-set", "trigger": trigger, "text": text }), |s| {
                s.snippets.retain(|kept| !kept.trigger.eq_ignore_ascii_case(&trigger));
                s.snippets.push(Snippet { trigger: trigger.clone(), text: text.clone() });
            })
        }
        "remove" if args.plain.len() >= 2 => {
            let trigger = args.plain[1..].join(" ");
            change(json!({ "cmd": "snippet-remove", "trigger": trigger }), |s| {
                s.snippets.retain(|kept| !kept.trigger.eq_ignore_ascii_case(&trigger))
            })
        }
        _ => Err("Use: parla snippets [list | set <phrase> <text> | remove <phrase>]".into()),
    }
}

// MARK: - Coding agents

/// What a Claude Code hook has to say, read from the JSON it pipes in.
fn hook_message() -> Result<String, String> {
    let mut input = String::new();
    std::io::stdin().read_to_string_lossy(&mut input);
    let event: Value = serde_json::from_str(&input).unwrap_or(Value::Null);
    let first_line = |text: &str| -> String {
        let line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
        let short: String = line.chars().take(70).collect();
        if line.chars().count() > 70 { format!("{short}…") } else { short }
    };
    Ok(match event["hook_event_name"].as_str() {
        Some("Stop") => match event["last_assistant_message"].as_str() {
            Some(message) if !message.trim().is_empty() => format!("Done — {}", first_line(message)),
            _ => "Done".into(),
        },
        Some("Notification") => match event["notification_type"].as_str() {
            Some("permission_prompt") => "needs your OK".into(),
            Some("idle_prompt") => "is waiting for you".into(),
            _ => event["message"].as_str().map(first_line).unwrap_or_else(|| "Needs you".into()),
        },
        _ => event["message"].as_str().map(first_line).unwrap_or_default(),
    })
}

trait ReadLossy {
    fn read_to_string_lossy(&mut self, out: &mut String);
}

impl<R: std::io::Read> ReadLossy for R {
    fn read_to_string_lossy(&mut self, out: &mut String) {
        let mut bytes = Vec::new();
        let _ = self.read_to_end(&mut bytes);
        out.push_str(&String::from_utf8_lossy(&bytes));
    }
}

/// The plugin that lets Claude Code tap Parla on the shoulder: a note in the pill when
/// it needs an answer or finishes a turn, and the history over MCP. It runs this very
/// binary, so nothing has to be on the PATH.
pub fn write_claude_plugin(folder: &Path) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe = exe.to_string_lossy().into_owned();
    let write = |path: PathBuf, value: Value| -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())
    };
    let description = "Parla tells you in its pill when Claude Code needs you or is done, and gives Claude your dictation history, words and snippets over MCP. All local.";
    write(
        folder.join(".claude-plugin/marketplace.json"),
        json!({
            "name": "parla",
            "description": "Parla, on-device dictation",
            "owner": { "name": "Parla" },
            "plugins": [{ "name": "parla", "source": "./plugin", "description": description }],
        }),
    )?;
    write(
        folder.join("plugin/.claude-plugin/plugin.json"),
        json!({ "name": "parla", "version": VERSION, "description": description, "author": { "name": "Bryan Bernardo Parreira" } }),
    )?;
    let hook = |title: &str| {
        json!({ "type": "command", "command": exe, "args": ["notify", "--from-hook", "--title", title], "timeout": 5 })
    };
    write(
        folder.join("plugin/hooks/hooks.json"),
        json!({
            "hooks": {
                "Notification": [{
                    "matcher": "permission_prompt|idle_prompt|elicitation_dialog|agent_needs_input",
                    "hooks": [hook("Claude Code")],
                }],
                "Stop": [{ "hooks": [hook("Claude Code")] }],
            }
        }),
    )?;
    write(
        folder.join("plugin/.mcp.json"),
        json!({ "mcpServers": { "parla": { "command": exe, "args": ["mcp"] } } }),
    )
}

// MARK: - Installing

/// Links `parla` into a folder on the PATH. /usr/local/bin first, which every shell
/// searches; ~/.local/bin when that isn't writable.
pub fn install() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let home = std::env::var_os("HOME").map(PathBuf::from).ok_or("No home folder.")?;
    let candidates = [PathBuf::from("/usr/local/bin"), home.join(".local/bin")];
    let mut last_error = String::new();
    for folder in candidates {
        let _ = std::fs::create_dir_all(&folder);
        let link = folder.join("parla");
        let _ = std::fs::remove_file(&link);
        #[cfg(unix)]
        let made = std::os::unix::fs::symlink(&exe, &link);
        #[cfg(not(unix))]
        let made = std::fs::copy(&exe, &link).map(|_| ());
        match made {
            Ok(()) => return Ok(link.to_string_lossy().into_owned()),
            Err(error) => last_error = error.to_string(),
        }
    }
    Err(format!("Couldn't install the command: {last_error}"))
}

// MARK: - MCP

const MCP_PROTOCOL: &str = "2025-06-18";

fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": { "type": "object", "properties": properties, "required": required },
    })
}

fn tools() -> Value {
    let text = |d: &str| json!({ "type": "string", "description": d });
    let number = |d: &str| json!({ "type": "integer", "description": d });
    json!([
        tool("search_dictations", "Search the user's Parla dictation history for entries containing every word of the query.",
            json!({ "query": text("Words to look for"), "limit": number("Most results to return, 20 by default") }), &["query"]),
        tool("recent_dictations", "The user's most recent dictations, newest first.",
            json!({ "limit": number("How many, 20 by default"), "since": text("Only from this day on, YYYY-MM-DD"), "mode": text("Only from this mode") }), &[]),
        tool("read_dictation", "One dictation in full by id, with the transcript before cleanup.",
            json!({ "id": text("The dictation's id") }), &["id"]),
        tool("dictation_stats", "How much the user dictates, how fast, in which modes and apps.", json!({}), &[]),
        tool("list_modes", "The user's Parla modes, which shape how dictation is written.", json!({}), &[]),
        tool("list_vocabulary", "Names and words Parla spells the user's way.", json!({}), &[]),
        tool("add_vocabulary", "Add a word or name Parla keeps getting wrong, spelled the right way.",
            json!({ "word": text("The word as it should be written") }), &["word"]),
        tool("remove_vocabulary", "Remove a word from the user's vocabulary.", json!({ "word": text("The word") }), &["word"]),
        tool("list_snippets", "Spoken phrases Parla replaces with saved text.", json!({}), &[]),
        tool("set_snippet", "Make Parla type `text` whenever the user says `phrase`.",
            json!({ "phrase": text("What the user says"), "text": text("What gets typed") }), &["phrase", "text"]),
        tool("remove_snippet", "Remove a snippet.", json!({ "phrase": text("The spoken phrase") }), &["phrase"]),
    ])
}

fn call_tool(name: &str, input: &Value) -> Result<String, String> {
    let text = |key: &str| input[key].as_str().map(str::to_string).filter(|s| !s.trim().is_empty());
    let limit = input["limit"].as_u64().map_or(20, |n| n as usize);
    let entry_json = |e: &Entry| {
        json!({ "id": e.id.to_string(), "text": e.text, "raw": e.raw, "mode": e.mode, "app": e.app, "words": e.words, "createdAt": e.created_at })
    };
    match name {
        "search_dictations" => {
            let query = text("query").ok_or("Give a query.")?;
            let found = search(&history()?, &query, limit);
            Ok(json!(found.iter().map(entry_json).collect::<Vec<_>>()).to_string())
        }
        "recent_dictations" => {
            let mut raw = vec!["--limit".to_string(), limit.to_string()];
            for key in ["since", "mode"] {
                if let Some(value) = text(key) {
                    raw.push(format!("--{key}"));
                    raw.push(value);
                }
            }
            let entries = recent(&history()?, &Args::parse(&raw));
            Ok(json!(entries.iter().map(entry_json).collect::<Vec<_>>()).to_string())
        }
        "read_dictation" => {
            let id = text("id").ok_or("Give an id.")?;
            let entries = history()?;
            let entry = entries.iter().find(|e| e.id.to_string() == id).ok_or("No dictation with that id.")?;
            Ok(json!({ "entry": entry_json(entry), "prompt": entry.prompt }).to_string())
        }
        "dictation_stats" => Ok(stats(&history()?).to_string()),
        "list_modes" => run(&["modes".into(), "--json".into()]),
        "list_vocabulary" => Ok(json!(settings()?.dictionary).to_string()),
        "add_vocabulary" => vocab(&Args::parse(&["add".into(), text("word").ok_or("Give a word.")?])).map(|_| "Added.".into()),
        "remove_vocabulary" => {
            vocab(&Args::parse(&["remove".into(), text("word").ok_or("Give a word.")?])).map(|_| "Removed.".into())
        }
        "list_snippets" => Ok(json!(settings()?.snippets.iter().map(|s| json!({ "phrase": s.trigger, "text": s.text })).collect::<Vec<_>>()).to_string()),
        "set_snippet" => {
            let phrase = text("phrase").ok_or("Give the phrase.")?;
            let typed = input["text"].as_str().ok_or("Give the text.")?.to_string();
            snippets(&Args::parse(&["set".into(), phrase, typed])).map(|_| "Saved.".into())
        }
        "remove_snippet" => {
            snippets(&Args::parse(&["remove".into(), text("phrase").ok_or("Give the phrase.")?])).map(|_| "Removed.".into())
        }
        _ => Err(format!("Unknown tool {name}.")),
    }
}

/// A Model Context Protocol server over stdio: one JSON-RPC message per line.
fn mcp() -> Result<String, String> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            let error = json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": "Parse error" } });
            let _ = writeln!(stdout, "{error}");
            continue;
        };
        // Notifications have no id and get no answer.
        let Some(id) = message.get("id").cloned() else { continue };
        let result = match message["method"].as_str().unwrap_or("") {
            "initialize" => Ok(json!({
                "protocolVersion": message["params"]["protocolVersion"].as_str().unwrap_or(MCP_PROTOCOL),
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "parla", "version": VERSION },
                "instructions": "The user's Parla dictation history, vocabulary and snippets. Everything stays on their computer apart from what you read here.",
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tools() })),
            "tools/call" => {
                let name = message["params"]["name"].as_str().unwrap_or("");
                let input = message["params"].get("arguments").cloned().unwrap_or(json!({}));
                Ok(match call_tool(name, &input) {
                    Ok(text) => json!({ "content": [{ "type": "text", "text": text }] }),
                    Err(error) => json!({ "content": [{ "type": "text", "text": error }], "isError": true }),
                })
            }
            other => Err(json!({ "code": -32601, "message": format!("Method not found: {other}") })),
        };
        let reply = match result {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err(error) => json!({ "jsonrpc": "2.0", "id": id, "error": error }),
        };
        let _ = writeln!(stdout, "{reply}");
        let _ = stdout.flush();
    }
    Ok(String::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: u64, text: &str) -> Entry {
        serde_json::from_value(json!({ "id": id, "text": text, "createdAt": id, "durationMs": 1000, "words": text.split_whitespace().count() })).unwrap()
    }

    #[test]
    fn searches_for_every_word() {
        let entries = vec![entry(3, "Send the quarterly report"), entry(2, "Quarterly numbers look good"), entry(1, "Lunch?")];
        let found = search(&entries, "quarterly report", 10);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, 3);
        assert!(search(&entries, "", 10).is_empty());
    }

    #[test]
    fn reads_options_anywhere() {
        let args = Args::parse(&["foo".into(), "--limit".into(), "5".into(), "--json".into(), "bar".into()]);
        assert_eq!(args.plain, vec!["foo", "bar"]);
        assert_eq!(args.limit(20), 5);
        assert!(args.flag("--json"));
    }

    #[test]
    fn turns_a_date_into_its_first_millisecond() {
        assert_eq!(day_start("1970-01-01"), Some(0));
        assert_eq!(day_start("2026-10-07"), Some(1_791_331_200_000));
        assert_eq!(day_start("nonsense"), None);
    }

    #[test]
    fn lists_its_tools_with_schemas() {
        let tools = tools();
        let names: Vec<&str> = tools.as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"search_dictations") && names.contains(&"set_snippet"));
        assert!(tools.as_array().unwrap().iter().all(|t| t["inputSchema"]["type"] == "object"));
    }
}
