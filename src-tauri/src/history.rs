use std::{
    path::PathBuf,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
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

    pub fn latest(&self) -> Option<Entry> {
        self.lock().first().cloned()
    }

    pub fn add(
        &self,
        transcript: &Transcript,
        duration_ms: u64,
        latency_ms: u64,
        app: Option<String>,
    ) -> Entry {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or_default();
        let entry = Entry {
            id: now,
            text: transcript.text.clone(),
            language: transcript.language.clone(),
            raw: transcript.raw.clone(),
            created_at: now,
            duration_ms,
            words: transcript.text.split_whitespace().count(),
            transcribe_ms: transcript.transcribe_ms,
            enhance_ms: transcript.enhance_ms,
            latency_ms: Some(latency_ms),
            app,
            style: transcript.style.clone(),
        };

        let mut entries = self.lock();
        entries.insert(0, entry.clone());
        entries.truncate(LIMIT);
        let _ = storage::write(&self.path, &*entries);
        entry
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
