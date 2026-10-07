//! Lays a dictated email out like a letter: the greeting on its own line, the body, then
//! the sign-off and the name on their own lines. Only a greeting said at the very start
//! and a sign-off said at the very end are moved; nothing is ever added.
//!
//! The Windows and Linux counterpart of `EmailLayout` in the Swift engine; the two are
//! kept in step so a dictation reads the same on every platform.

const GREETINGS: &[&str] = &[
    "good morning", "good afternoon", "good evening", "hi", "hello", "hey", "dear", "greetings",
    "olá", "ola", "oi", "bom dia", "boa tarde", "boa noite", "caro", "cara", "prezado", "prezada",
];

const SIGN_OFFS: &[&str] = &[
    "thanks so much", "thanks again", "thank you", "many thanks", "best regards",
    "kind regards", "warm regards", "all the best", "talk soon", "take care", "thanks",
    "best", "regards", "cheers", "sincerely", "obrigado", "obrigada", "abraços", "abraço",
    "atenciosamente", "cumprimentos", "beijos",
];

// Common words that start sentences, in English and Portuguese, so "Thanks, So" is
// never taken for a sign-off and a name.
const SENTENCE_STARTERS: &[&str] = &[
    "a", "about", "actually", "after", "all", "also", "an", "and", "any", "are", "as", "at",
    "because", "before", "both", "but", "by", "can", "could", "did", "do", "does", "each", "even",
    "every", "for", "from", "had", "has", "have", "he", "her", "here", "his", "how", "if", "in",
    "is", "it", "it's", "its", "just", "let's", "like", "maybe", "more", "most", "my", "no", "not",
    "now", "of", "on", "one", "only", "or", "our", "she", "should", "so", "some", "still", "that",
    "the", "their", "them", "then", "there", "these", "they", "this", "those", "to", "today",
    "tomorrow", "too", "was", "we", "we're", "well", "were", "what", "when", "where", "which",
    "while", "who", "why", "will", "with", "would", "yes", "yet", "you", "your", "yesterday",
    "o", "os", "as", "um", "uma", "e", "mas", "ou", "então", "eles", "elas", "ele", "ela", "eu",
    "nós", "isso", "isto", "este", "esta", "que", "quando", "onde", "porque", "como", "também",
    "se", "em", "no", "na", "nos", "nas", "para", "com", "de", "do", "da", "por", "depois", "antes",
    "só", "ainda", "não", "sim", "talvez", "agora", "hoje", "amanhã", "ontem", "muito", "mais", "já",
];

fn is_punctuation(c: char) -> bool {
    c.is_ascii_punctuation() || matches!(c, '¿' | '¡' | '…' | '“' | '”' | '‘' | '’' | '«' | '»')
}

fn trim_punctuation(text: &str) -> &str {
    text.trim_matches(is_punctuation)
}

fn bare(word: &str) -> String {
    word.to_lowercase().chars().filter(|c| c.is_alphabetic()).collect()
}

fn is_name(word: &str) -> bool {
    let letters = trim_punctuation(word);
    letters.chars().count() >= 2
        && letters.chars().next().is_some_and(char::is_uppercase)
        && letters.chars().all(|c| c.is_alphabetic() || c == '-' || c == '\'')
        && !SENTENCE_STARTERS.contains(&letters.to_lowercase().as_str())
        && letters != "I"
}

fn ends_clause(word: &str) -> bool {
    word.chars().last().is_some_and(|c| ",.!?;:".contains(c))
}

fn matches(words: &[String], at: usize, phrase: &[&str]) -> bool {
    at + phrase.len() <= words.len()
        && words[at..at + phrase.len()].iter().zip(phrase).all(|(word, part)| bare(word) == *part)
}

fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() || b.is_empty() {
        return a.len().max(b.len());
    }
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut current = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            let substitution = previous[j - 1] + usize::from(a[i - 1] != b[j - 1]);
            current[j] = (previous[j] + 1).min(current[j - 1] + 1).min(substitution);
        }
        previous = current;
    }
    previous[b.len()]
}

/// A signed name one letter off the user's own, like "Brian" for "Bryan", is theirs.
fn sign_name(word: &str, user_name: Option<&str>) -> String {
    let name = trim_punctuation(word);
    let Some(user) = user_name else { return name.to_string() };
    let (lower, user_lower) = (name.to_lowercase(), user.to_lowercase());
    if lower != user_lower
        && lower.chars().next() == user_lower.chars().next()
        && distance(&lower, &user_lower) <= 1
    {
        user.to_string()
    } else {
        name.to_string()
    }
}

/// With `require_both`, the text is only laid out when it was spoken as a whole email,
/// greeting first and sign-off last. That is how an email is recognised by what was
/// said rather than by the app it is going into.
pub fn apply(text: &str, user_name: Option<&str>, require_both: bool) -> String {
    let mut words: Vec<String> = text.split(' ').filter(|w| !w.is_empty()).map(String::from).collect();
    if words.len() < 2 || text.contains('\n') {
        return text.to_string();
    }

    // Greeting: up to a comma soon after the phrase ("Dear team,", "Hello there,"), or
    // else the phrase and up to two names.
    let mut greeting = None;
    let phrase = GREETINGS
        .iter()
        .map(|g| g.split(' ').collect::<Vec<_>>())
        .find(|phrase| matches(&words, 0, phrase));
    if let Some(phrase) = phrase {
        let mut end = phrase.len();
        let comma = (phrase.len() - 1..words.len().min(phrase.len() + 3)).find(|&i| words[i].ends_with(','));
        if let Some(comma) = comma {
            end = comma + 1;
        } else if !ends_clause(&words[end - 1]) {
            while end < words.len() && end < phrase.len() + 2 && is_name(&words[end]) {
                end += 1;
                if ends_clause(&words[end - 1]) {
                    break;
                }
            }
        }
        if end < words.len() {
            let said = words[..end].join(" ");
            greeting = Some(capitalized(trim_punctuation(&said)) + ",");
            words.drain(..end);
            words[0] = capitalized(&words[0]);
        }
    }

    // Sign-off: the phrase, then at most two names, and nothing after. The transcriber
    // often hears a closing "Bryan, thanks" as "Bryan thinks", which is taken the same way.
    let mut sign_off = None;
    let mut name = None;
    let lowest = words.len().saturating_sub(5);
    for start in (lowest..words.len()).rev() {
        let boundary = start == 0 || ends_clause(&words[start - 1]);
        if !boundary {
            continue;
        }
        let phrase = SIGN_OFFS
            .iter()
            .map(|s| s.split(' ').collect::<Vec<_>>())
            .find(|phrase| matches(&words, start, phrase));
        if let Some(phrase) = phrase {
            let rest = &words[start + phrase.len()..];
            if rest.len() > 2 || !rest.iter().all(|w| is_name(w)) {
                continue;
            }
            let said = words[start..start + phrase.len()].join(" ");
            sign_off = Some(capitalized(trim_punctuation(&said)));
            if let Some((first, others)) = rest.split_first() {
                let mut parts = vec![sign_name(first, user_name)];
                parts.extend(others.iter().map(|w| trim_punctuation(w).to_string()));
                name = Some(parts.join(" "));
            }
            words.truncate(start);
            break;
        }
        if words.len() - start == 2
            && is_name(&words[start])
            && ["thanks", "thinks"].contains(&bare(&words[start + 1]).as_str())
        {
            sign_off = Some("Thanks".to_string());
            name = Some(sign_name(&words[start], user_name));
            words.truncate(start);
            break;
        }
    }

    // A quick "Hey, got a sec? Thanks" is a chat line, not a letter: a recognised email
    // also signs off with a name or says enough to be one.
    let whole_email =
        greeting.is_some() && sign_off.is_some() && (name.is_some() || words.len() >= 15);
    let laid_out = if require_both { whole_email } else { greeting.is_some() || sign_off.is_some() };
    if !laid_out {
        return text.to_string();
    }
    let mut body = words.join(" ").trim().to_string();
    if body.ends_with(',') || body.ends_with(';') {
        body.pop();
    }
    let mut parts = Vec::new();
    if let Some(greeting) = greeting {
        parts.push(greeting);
    }
    if !body.is_empty() {
        parts.push(body);
    }
    if let Some(sign_off) = sign_off {
        parts.push(match name {
            Some(name) => format!("{sign_off},\n{name}"),
            None => sign_off,
        });
    }
    parts.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn whole(text: &str) -> String {
        apply(text, Some("Bryan"), true)
    }

    #[test]
    fn lays_out_a_spoken_email() {
        assert_eq!(
            whole("Hi Sarah, I wanted to follow up on the proposal. Can you send me the numbers by Friday? Thanks, Brian."),
            "Hi Sarah,\n\nI wanted to follow up on the proposal. Can you send me the numbers by Friday?\n\nThanks,\nBryan"
        );
        assert_eq!(
            whole("Hello. I was thinking about getting the meeting done for next Monday. Best regards, Bryan"),
            "Hello,\n\nI was thinking about getting the meeting done for next Monday.\n\nBest regards,\nBryan"
        );
        // Apple Intelligence's actual cleanup of a spoken email.
        assert_eq!(
            whole("Hi Sarah, so I wanted to follow up on the proposal we talked about yesterday, can you send me the latest numbers by Friday? Thanks, Brian."),
            "Hi Sarah,\n\nSo I wanted to follow up on the proposal we talked about yesterday, can you send me the latest numbers by Friday?\n\nThanks,\nBryan"
        );
        assert_eq!(
            whole("Olá Maria, tudo bem? Queria confirmar a reunião de amanhã. Obrigado, Bryan"),
            "Olá Maria,\n\nTudo bem? Queria confirmar a reunião de amanhã.\n\nObrigado,\nBryan"
        );
    }

    #[test]
    fn signs_off_without_a_name_only_when_long_enough() {
        assert_eq!(
            whole("Dear team, the release is moved to next week because QA found two blocking bugs in the payment flow that we need to fix first. Cheers"),
            "Dear team,\n\nThe release is moved to next week because QA found two blocking bugs in the payment flow that we need to fix first.\n\nCheers"
        );
        assert_eq!(whole("Hi John, quick update on the launch. Thanks"), "Hi John, quick update on the launch. Thanks");
    }

    #[test]
    fn leaves_chat_lines_and_plain_text_alone() {
        for text in [
            "Hey, can you send me that file? Thanks.",
            "I think the meeting went well. Thanks for asking.",
            "Hello hello hello hello.",
            "Hi",
            "Hi there,\nalready laid out. Thanks, Bryan",
        ] {
            assert_eq!(whole(text), text);
        }
    }

    #[test]
    fn email_apps_take_either_end() {
        assert_eq!(apply("Hi Sarah, see you at three.", None, false), "Hi Sarah,\n\nSee you at three.");
        assert_eq!(apply("See you at three. Bryan thinks", None, false), "See you at three.\n\nThanks,\nBryan");
    }
}
