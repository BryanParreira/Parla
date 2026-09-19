use crate::settings::Snippet;

/// Swaps spoken trigger phrases for their saved text. A dictation that is nothing but
/// a trigger becomes exactly the snippet, without the full stop the transcriber adds;
/// a trigger inside a longer sentence is replaced where it stands.
pub fn expand(text: &str, snippets: &[Snippet]) -> String {
    let spoken = normalize(text);
    if let Some(snippet) = snippets.iter().find(|s| normalize(&s.trigger) == spoken) {
        return snippet.text.clone();
    }

    let mut result = text.to_string();
    for snippet in snippets {
        result = replace_phrase(&result, &snippet.trigger, &snippet.text);
    }
    result
}

/// Lowercase words only, so "My email." and "my email" compare equal.
fn normalize(text: &str) -> String {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Replaces whole-word, case-insensitive occurrences of `phrase`. Works on chars rather
/// than a lowercased copy because lowercasing can change a string's byte length.
fn replace_phrase(text: &str, phrase: &str, replacement: &str) -> String {
    // One char in, one char out, so match positions line up with the original text.
    let fold = |c: char| c.to_lowercase().next().unwrap_or(c);
    let needle: Vec<char> = phrase.trim().chars().map(fold).collect();
    if needle.is_empty() {
        return text.to_string();
    }
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let lower: Vec<char> = chars.iter().map(|(_, c)| fold(*c)).collect();
    let boundary = |index: Option<&(usize, char)>| index.map_or(true, |(_, c)| !c.is_alphanumeric());

    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let end = i + needle.len();
        let matches = end <= chars.len()
            && lower[i..end] == needle[..]
            && boundary(i.checked_sub(1).and_then(|p| chars.get(p)))
            && boundary(chars.get(end));
        if matches {
            out.push_str(replacement);
            i = end;
        } else {
            out.push(chars[i].1);
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snippet(trigger: &str, text: &str) -> Snippet {
        Snippet { trigger: trigger.into(), text: text.into() }
    }

    #[test]
    fn a_lone_trigger_becomes_the_snippet_without_punctuation() {
        let snippets = [snippet("my email", "bryan@example.com")];
        assert_eq!(expand("My email.", &snippets), "bryan@example.com");
        assert_eq!(expand("my EMAIL", &snippets), "bryan@example.com");
    }

    #[test]
    fn a_trigger_inside_a_sentence_is_replaced_in_place() {
        let snippets = [snippet("my email", "bryan@example.com")];
        assert_eq!(
            expand("Send it to My Email, please.", &snippets),
            "Send it to bryan@example.com, please."
        );
    }

    #[test]
    fn only_whole_words_match() {
        let snippets = [snippet("sig", "— Bryan")];
        assert_eq!(expand("A signature and sig.", &snippets), "A signature and — Bryan.");
    }

    #[test]
    fn leaves_text_alone_without_a_match_and_survives_unicode() {
        let snippets = [snippet("İstanbul office", "Address")];
        assert_eq!(expand("Große Straße", &snippets), "Große Straße");
        assert_eq!(expand("Große İstanbul office", &snippets), "Große Address");
    }
}
