use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::{engine::Transcript, storage};

const LIMIT: usize = 1000;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: u64,
    pub text: String,
    /// BCP-47 code recognised from the transcript, absent when it was too short or
    /// too ambiguous to call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<String>,
    pub created_at: u64,
    pub duration_ms: u64,
    pub words: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcribe_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enhance_ms: Option<u64>,
    /// From letting go of the key to the text being pasted: the wait the user feels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
    /// Name of the app the text was dictated into. The name only, and only so the
    /// stats can show where dictation is actually being used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
    /// The style Parla used there, such as "email", so it's clear what was detected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    /// The name of the mode that wrote it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    /// What the cleanup model was asked, when a mode added instructions or context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// The kinds of context that went with it: "selection", "clipboard", "app".
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub context: Vec<String>,
    /// The file it was transcribed from, when it came from one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Whether its audio is still kept. Worked out when the list is read, never stored.
    #[serde(default, skip_deserializing)]
    pub audio: bool,
}

pub struct HistoryStore {
    path: PathBuf,
    entries: Mutex<Vec<Entry>>,
}

impl HistoryStore {
    pub fn load(app: &AppHandle) -> Self {
        let path = storage::path(app, "history.json");
        let entries = Mutex::new(storage::read(&path));
        Self { path, entries }
    }

    pub fn list(&self) -> Vec<Entry> {
        self.lock().clone()
    }

    /// The list with each entry's `audio` flag set from the recordings on disk.
    pub fn list_with_audio(&self, recordings: &Path) -> Vec<Entry> {
        let kept = kept_recordings(recordings);
        let mut entries = self.list();
        for entry in &mut entries {
            entry.audio = kept.contains(&entry.id);
        }
        entries
    }

    pub fn get(&self, id: u64) -> Option<Entry> {
        self.lock().iter().find(|entry| entry.id == id).cloned()
    }

    /// Replaces an entry with a new version of itself, after it was processed again.
    pub fn replace(&self, entry: Entry) -> Result<(), String> {
        let mut entries = self.lock();
        let Some(slot) = entries.iter_mut().find(|kept| kept.id == entry.id) else {
            return Err("That dictation is no longer in the history.".into());
        };
        *slot = entry;
        storage::write(&self.path, &*entries)
    }

    pub fn latest(&self) -> Option<Entry> {
        self.lock().first().cloned()
    }

    /// An id for the entry about to be added, so its audio can be saved under it first.
    pub fn next_id(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or_default();
        // Two entries in the same millisecond would otherwise share an id.
        let newest = self.lock().first().map_or(0, |entry| entry.id);
        now.max(newest + 1)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add(
        &self,
        id: u64,
        transcript: &Transcript,
        duration_ms: u64,
        latency_ms: Option<u64>,
        app: Option<String>,
        mode: Option<String>,
        source: Option<String>,
    ) -> Entry {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or_default();
        let entry = Entry {
            id,
            text: transcript.text.clone(),
            language: transcript.language.clone(),
            raw: transcript.raw.clone(),
            created_at: now,
            duration_ms,
            words: transcript.text.split_whitespace().count(),
            transcribe_ms: transcript.transcribe_ms,
            enhance_ms: transcript.enhance_ms,
            latency_ms,
            app,
            style: transcript.style.clone(),
            mode,
            prompt: transcript.prompt.clone(),
            context: transcript.context.clone().unwrap_or_default(),
            source,
            audio: false,
        };

        let mut entries = self.lock();
        entries.insert(0, entry.clone());
        entries.truncate(LIMIT);
        let _ = storage::write(&self.path, &*entries);
        entry
    }

    /// Adds entries from a backup that aren't here already, keeping newest first.
    pub fn merge(&self, incoming: Vec<Entry>) -> Result<(), String> {
        let mut entries = self.lock();
        let known: HashSet<u64> = entries.iter().map(|entry| entry.id).collect();
        entries.extend(incoming.into_iter().filter(|entry| !known.contains(&entry.id)));
        entries.sort_by(|a, b| b.id.cmp(&a.id));
        entries.truncate(LIMIT);
        storage::write(&self.path, &*entries)
    }

    pub fn remove(&self, id: u64) -> Result<(), String> {
        let mut entries = self.lock();
        entries.retain(|entry| entry.id != id);
        storage::write(&self.path, &*entries)
    }

    pub fn clear(&self) -> Result<(), String> {
        let mut entries = self.lock();
        entries.clear();
        storage::write(&self.path, &*entries)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Entry>> {
        self.entries.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Where an entry's audio is kept.
pub fn recording_path(recordings: &Path, id: u64) -> PathBuf {
    recordings.join(format!("{id}.wav"))
}

/// The ids of the entries whose audio is on disk.
fn kept_recordings(recordings: &Path) -> HashSet<u64> {
    fs::read_dir(recordings)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|file| {
            let name = file.file_name().into_string().ok()?;
            name.strip_suffix(".wav")?.parse().ok()
        })
        .collect()
}

/// Deletes kept audio older than `days`, or all of it when `days` is 0.
pub fn prune_recordings(recordings: &Path, days: u32) {
    let horizon = SystemTime::now()
        .checked_sub(Duration::from_secs(u64::from(days) * 24 * 60 * 60))
        .and_then(|at| at.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as u64);
    for id in kept_recordings(recordings) {
        if days == 0 || id < horizon {
            let _ = fs::remove_file(recording_path(recordings, id));
        }
    }
}
