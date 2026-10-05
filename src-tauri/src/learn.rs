//! Learns the words the user keeps fixing. A while after a paste, the text in the field
//! is compared with what Parla typed; a word swapped for a close spelling, or for a
//! name, becomes a dictionary suggestion. Only the word pair is kept, never the field.

use std::{path::PathBuf, sync::Mutex};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::storage;

const MAX_SUGGESTIONS: usize = 30;
// Past this many words the comparison stops being cheap, and the dictation would be too
// long to have been fixed word by word anyway.
const MAX_PASTED_WORDS: usize = 400;
const MAX_FIELD_WORDS: usize = 4000;

struct Token {
    word: String,
    /// First word of a sentence, where a capital letter is grammar rather than spelling.
    sentence_start: bool,
}

fn tokens(text: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut sentence_start = true;
    for raw in text.split_whitespace() {
        let word = raw.trim_matches(|c: char| !c.is_alphanumeric());
        if !word.is_empty() {
            out.push(Token { word: word.to_string(), sentence_start });
        }
        sentence_start = raw.ends_with(['.', '!', '?']) || raw.ends_with(['\n']);
    }
    out
}

/// Pairs of (what Parla typed, what the user changed it to).
pub fn corrections(pasted: &str, field: &str) -> Vec<(String, String)> {
    let typed = tokens(pasted);
    let current = tokens(field);
    if typed.len() < 3 || typed.len() > MAX_PASTED_WORDS || current.len() > MAX_FIELD_WORDS {
        return Vec::new();
    }

    // Longest common subsequence over lowercased words finds the dictation inside the
    // field, whatever else the field holds.
    let a: Vec<String> = typed.iter().map(|t| t.word.to_lowercase()).collect();
    let b: Vec<String> = current.iter().map(|t| t.word.to_lowercase()).collect();
    let (n, m) = (a.len(), b.len());
    let mut table = vec![vec![0u16; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            table[i][j] = if a[i] == b[j] {
                table[i + 1][j + 1] + 1
            } else {
                table[i + 1][j].max(table[i][j + 1])
            };
        }
    }
    let mut matched = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if a[i] == b[j] {
            matched.push((i, j));
            i += 1;
            j += 1;
        } else if table[i + 1][j] >= table[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    // Too little of the dictation survives: the user rewrote it, or moved to another field.
    if matched.len() < 3 || matched.len() * 10 < n * 6 {
        return Vec::new();
    }

    let mut found = Vec::new();
    // Same word with different capitals: matched above, since matching ignores case.
    let recase = |i: usize, j: usize, found: &mut Vec<(String, String)>| {
        if typed[i].word != current[j].word && is_learnable(&typed[i].word, &current[j]) {
            found.push((typed[i].word.clone(), current[j].word.clone()));
        }
    };
    recase(matched[0].0, matched[0].1, &mut found);
    for pair in matched.windows(2) {
        let ((i0, j0), (i1, j1)) = (pair[0], pair[1]);
        let heard_len = i1 - i0 - 1;
        let fixed_len = j1 - j0 - 1;
        if heard_len == fixed_len && (1..=3).contains(&heard_len) {
            // Word for word: "Brian Pereira" fixed to "Bryan Parreira".
            for k in 1..=heard_len {
                if is_learnable(&typed[i0 + k].word, &current[j0 + k]) {
                    found.push((typed[i0 + k].word.clone(), current[j0 + k].word.clone()));
                }
            }
        } else if fixed_len == 1 && heard_len == 2 {
            // One word heard as two: "par la" fixed to "Parla".
            let heard = format!("{} {}", typed[i0 + 1].word, typed[i0 + 2].word);
            if is_learnable(&heard, &current[j0 + 1]) {
                found.push((heard, current[j0 + 1].word.clone()));
            }
        }
        recase(i1, j1, &mut found);
    }
    found
}

fn is_learnable(heard: &str, fixed: &Token) -> bool {
    let word = fixed.word.as_str();
    if word.chars().count() < 2 || !word.chars().any(char::is_alphabetic) {
        return false;
    }
    if heard.eq_ignore_ascii_case(word) {
        // Only a capital where grammar wouldn't put one says something about spelling:
        // "iphone" to "iPhone", or "parla" to "Parla" mid-sentence.
        let inner_capital = word.chars().skip(1).any(char::is_uppercase);
        return inner_capital || (!fixed.sentence_start && word.starts_with(char::is_uppercase));
    }
    // A name or a product spelling, or the same word spelled differently. A different
    // ordinary word ("big" for "large") is an edit, not a correction.
    let distinctive = word.chars().any(|c| c.is_uppercase() || c.is_ascii_digit()) && !fixed.sentence_start;
    distinctive || similarity(&heard.to_lowercase(), &word.to_lowercase()) >= 0.6
}

/// 1 for identical strings, 0 for nothing in common, from the edit distance.
fn similarity(a: &str, b: &str) -> f64 {
    let a: Vec<char> = a.chars().filter(|c| !c.is_whitespace()).collect();
    let b: Vec<char> = b.chars().collect();
    let longest = a.len().max(b.len());
    if longest == 0 {
        return 1.0;
    }
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut previous = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let substitution = previous + usize::from(ca != cb);
            previous = row[j + 1];
            row[j + 1] = substitution.min(row[j] + 1).min(row[j + 1] + 1);
        }
    }
    1.0 - row[b.len()] as f64 / longest as f64
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    /// The spelling the user typed, which would go in the dictionary.
    pub word: String,
    /// What Parla had typed instead.
    pub heard: String,
    /// How many times the user has made this fix.
    pub count: u32,
}

#[derive(Default, Serialize, Deserialize)]
struct Stored {
    suggestions: Vec<Suggestion>,
    /// Words the user said no to, so they are never offered again.
    dismissed: Vec<String>,
}

pub struct SuggestionStore {
    path: PathBuf,
    stored: Mutex<Stored>,
}

impl SuggestionStore {
    pub fn load(app: &AppHandle) -> Self {
        let path = storage::path(app, "suggestions.json");
        let stored = Mutex::new(storage::read(&path));
        Self { path, stored }
    }

    pub fn list(&self) -> Vec<Suggestion> {
        self.lock().suggestions.clone()
    }

    /// Records fixes, skipping words already in the dictionary. Returns whether anything
    /// new was learned.
    pub fn record(&self, fixes: &[(String, String)], dictionary: &[String]) -> bool {
        let mut stored = self.lock();
        let mut changed = false;
        for (heard, word) in fixes {
            let known = |kept: &String| kept.eq_ignore_ascii_case(word);
            if dictionary.iter().any(known) || stored.dismissed.iter().any(known) {
                continue;
            }
            match stored.suggestions.iter_mut().find(|s| s.word.eq_ignore_ascii_case(word)) {
                Some(existing) => {
                    existing.count += 1;
                    existing.heard = heard.clone();
                }
                None => stored.suggestions.insert(
                    0,
                    Suggestion { word: word.clone(), heard: heard.clone(), count: 1 },
                ),
            }
            changed = true;
        }
        stored.suggestions.sort_by(|a, b| b.count.cmp(&a.count));
        stored.suggestions.truncate(MAX_SUGGESTIONS);
        if changed {
            let _ = storage::write(&self.path, &*stored);
        }
        changed
    }

    /// Drops a suggestion. A refused one is remembered so it isn't offered again; an
    /// accepted one is in the dictionary now, which already keeps it from coming back.
    pub fn resolve(&self, word: &str, accepted: bool) -> Result<(), String> {
        let mut stored = self.lock();
        stored.suggestions.retain(|s| !s.word.eq_ignore_ascii_case(word));
        if !accepted && !stored.dismissed.iter().any(|kept| kept.eq_ignore_ascii_case(word)) {
            stored.dismissed.push(word.to_string());
        }
        storage::write(&self.path, &*stored)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Stored> {
        self.stored.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::corrections;

    fn pairs(pasted: &str, field: &str) -> Vec<(String, String)> {
        corrections(pasted, field)
    }

    #[test]
    fn learns_a_respelled_name_inside_a_longer_field() {
        assert_eq!(
            pairs(
                "Please send the contract to Brian Pereira by Friday.",
                "Hi team,\nPlease send the contract to Bryan Parreira by Friday.\nThanks"
            ),
            vec![("Brian".into(), "Bryan".into()), ("Pereira".into(), "Parreira".into())]
        );
    }

    #[test]
    fn learns_a_split_word_and_an_inner_capital() {
        assert_eq!(
            pairs("I switched to par la on my iphone today.", "I switched to Parla on my iPhone today."),
            vec![("par la".into(), "Parla".into()), ("iphone".into(), "iPhone".into())]
        );
    }

    #[test]
    fn ignores_rewording_grammar_capitals_and_other_fields() {
        // A different ordinary word is an edit, not a misheard one.
        assert!(pairs("That is a big problem for us.", "That is a large problem for us.").is_empty());
        // Capitalising the first word of a sentence is grammar.
        assert!(pairs("ok. so we ship today.", "ok. So we ship today.").is_empty());
        // The field no longer holds the dictation.
        assert!(pairs("Send the report on Monday morning.", "Totally unrelated text here.").is_empty());
    }
}
