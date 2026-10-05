//! Spoken formatting: "new line", "comma", "question mark" and the like, said out loud
//! and typed as the characters they name. Also code casing ("camel case user id") and
//! numbered lists ("one. milk two. eggs" or "number one milk number two eggs").

#[derive(Clone, Copy)]
enum Action {
    Break(&'static str),
    Mark(char),
}

// Longer phrases come first where one contains another ("ponto e vírgula" holds
// "vírgula"), though matching already prefers the longest phrase at a position.
const COMMANDS: &[(&str, Action)] = &[
    ("new paragraph", Action::Break("\n\n")),
    ("novo parágrafo", Action::Break("\n\n")),
    ("new line", Action::Break("\n")),
    ("newline", Action::Break("\n")),
    ("nova linha", Action::Break("\n")),
    ("bullet point", Action::Break("\n- ")),
    ("new bullet", Action::Break("\n- ")),
    ("question mark", Action::Mark('?')),
    ("ponto de interrogação", Action::Mark('?')),
    ("exclamation mark", Action::Mark('!')),
    ("exclamation point", Action::Mark('!')),
    ("ponto de exclamação", Action::Mark('!')),
    ("full stop", Action::Mark('.')),
    ("ponto final", Action::Mark('.')),
    ("semicolon", Action::Mark(';')),
    ("ponto e vírgula", Action::Mark(';')),
    ("comma", Action::Mark(',')),
    ("vírgula", Action::Mark(',')),
];

/// Punctuation the transcriber puts around a spoken command, which the command replaces.
fn is_transcriber_mark(c: char) -> bool {
    matches!(c, ',' | '.' | ';' | ':')
}

pub fn apply(text: &str) -> String {
    let text = commands(text);
    text.split('\n').map(|line| numbered_list(&casing(line))).collect::<Vec<_>>().join("\n")
}

fn commands(text: &str) -> String {
    let fold = |c: char| c.to_lowercase().next().unwrap_or(c);
    let chars: Vec<char> = text.chars().collect();
    let lower: Vec<char> = chars.iter().map(|c| fold(*c)).collect();
    let is_word = |i: usize| chars.get(i).is_some_and(|c| c.is_alphanumeric());

    let mut out = String::with_capacity(text.len());
    let mut capitalize_next = false;
    let mut i = 0;
    while i < chars.len() {
        let found = (i == 0 || !is_word(i - 1))
            .then(|| {
                COMMANDS
                    .iter()
                    .filter_map(|(phrase, action)| {
                        let needle: Vec<char> = phrase.chars().collect();
                        let end = i + needle.len();
                        (end <= chars.len() && lower[i..end] == needle[..] && !is_word(end))
                            .then_some((end, *action))
                    })
                    .max_by_key(|(end, _)| *end)
            })
            .flatten();

        let Some((end, action)) = found else {
            let c = chars[i];
            if capitalize_next && c.is_alphabetic() {
                out.extend(c.to_uppercase());
                capitalize_next = false;
            } else {
                if !c.is_whitespace() {
                    capitalize_next = false;
                }
                out.push(c);
            }
            i += 1;
            continue;
        };

        // A mark said before anything else was said is just a word.
        if matches!(action, Action::Mark(_)) && out.trim().is_empty() {
            out.extend(&chars[i..end]);
            i = end;
            continue;
        }

        let kept = out.trim_end().len();
        out.truncate(kept);
        let mut next = end;
        while next < chars.len() && (chars[next].is_whitespace() || is_transcriber_mark(chars[next])) {
            next += 1;
        }
        match action {
            Action::Break(separator) => {
                // The pause before "new line" comes out as a comma; a full stop or a
                // colon the speaker meant stays.
                while out.ends_with([',', ';']) {
                    out.pop();
                }
                out.push_str(separator);
                capitalize_next = true;
            }
            Action::Mark(mark) => {
                while out.ends_with(is_transcriber_mark) {
                    out.pop();
                }
                out.push(mark);
                if next < chars.len() && chars[next] != '\n' {
                    out.push(' ');
                }
                capitalize_next = matches!(mark, '.' | '?' | '!');
            }
        }
        i = next;
    }
    out.trim_end_matches([' ', '\t']).to_string()
}

#[derive(Clone, Copy)]
enum Case {
    Camel,
    Pascal,
    Snake,
    Kebab,
    Constant,
}

const CASES: &[(&str, Case)] = &[
    ("camel", Case::Camel),
    ("pascal", Case::Pascal),
    ("snake", Case::Snake),
    ("kebab", Case::Kebab),
    ("constant", Case::Constant),
];

// An identifier is rarely longer than this. The name ends at the first punctuation
// mark, which is where the transcriber puts the pause after it, or after this many words.
const MAX_IDENTIFIER_WORDS: usize = 5;

fn bare(word: &str) -> String {
    word.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// "camel case user id" becomes "userId", "snake case max retries" becomes "max_retries".
fn casing(line: &str) -> String {
    let words: Vec<&str> = line.split(' ').collect();
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    let mut i = 0;
    while i < words.len() {
        let command = CASES.iter().find_map(|(name, case)| {
            let first = bare(words[i]);
            if first == format!("{name}case") {
                Some((*case, i + 1))
            } else if first == *name && words.get(i + 1).is_some_and(|w| bare(w) == "case") {
                Some((*case, i + 2))
            } else {
                None
            }
        });
        let Some((case, start)) = command else {
            out.push(words[i].to_string());
            i += 1;
            continue;
        };

        let mut parts: Vec<String> = Vec::new();
        let mut end = start;
        let mut trailing = String::new();
        while end < words.len() && parts.len() < MAX_IDENTIFIER_WORDS {
            let word = words[end];
            let part = bare(word);
            end += 1;
            if !part.is_empty() {
                parts.push(part);
            }
            let tail: String = word.chars().rev().take_while(|c| !c.is_alphanumeric()).collect();
            if !tail.is_empty() && !parts.is_empty() {
                trailing = tail.chars().rev().collect();
                break;
            }
        }
        if parts.is_empty() {
            out.push(words[i].to_string());
            i += 1;
            continue;
        }

        let capital = |part: &str| {
            let mut chars = part.chars();
            chars.next().map(|c| c.to_uppercase().chain(chars).collect::<String>()).unwrap_or_default()
        };
        let identifier = match case {
            Case::Camel => parts
                .iter()
                .enumerate()
                .map(|(n, part)| if n == 0 { part.clone() } else { capital(part) })
                .collect(),
            Case::Pascal => parts.iter().map(|part| capital(part)).collect(),
            Case::Snake => parts.join("_"),
            Case::Kebab => parts.join("-"),
            Case::Constant => parts.join("_").to_uppercase(),
        };
        out.push(identifier + &trailing);
        i = end;
    }
    out.join(" ")
}

fn number_word(word: &str) -> Option<u32> {
    const WORDS: [&str; 10] = ["one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten"];
    let word = bare(word);
    word.parse().ok().or_else(|| WORDS.iter().position(|w| *w == word).map(|n| n as u32 + 1))
}

/// Where a list item starts at `words[i]`: its number and the first word after the marker.
/// "number two", or a numeral with a stop after it ("2." "2)" "2,"). A bare "two" is
/// left alone, since counting out loud is far more common than dictating a list that way.
fn list_marker(words: &[&str], i: usize) -> Option<(u32, usize)> {
    let word = words[i];
    if bare(word) == "number" && !word.ends_with([',', '.', ':', ';']) {
        return words.get(i + 1).and_then(|next| number_word(next)).map(|n| (n, i + 2));
    }
    let digits = word.trim_end_matches(['.', ')', ',', ':']);
    (digits.len() < word.len() && (1..=2).contains(&digits.len()) && digits.chars().all(|c| c.is_ascii_digit()))
        .then(|| digits.parse().ok())
        .flatten()
        .map(|n| (n, i + 1))
}

/// "Groceries: number one milk, number two eggs" becomes a numbered list, one item per
/// line. It takes a run counting up from one with at least two items, so a lone
/// "number one" or "chapter 2." stays as it was said.
fn numbered_list(line: &str) -> String {
    let words: Vec<&str> = line.split(' ').filter(|w| !w.is_empty()).collect();
    let mut markers: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < words.len() {
        match list_marker(&words, i) {
            Some((n, next)) if n as usize == markers.len() + 1 => {
                markers.push((i, next));
                i = next;
            }
            _ => i += 1,
        }
    }
    if markers.len() < 2 {
        return line.to_string();
    }

    let clean = |slice: &[&str]| slice.join(" ").trim_end_matches([',', ';', '.', ' ']).to_string();
    let mut items = Vec::with_capacity(markers.len());
    for (n, &(_, start)) in markers.iter().enumerate() {
        let end = markers.get(n + 1).map_or(words.len(), |m| m.0);
        let item = clean(&words[start..end]);
        if item.is_empty() {
            return line.to_string();
        }
        let mut chars = item.chars();
        let first = chars.next().map(|c| c.to_uppercase().chain(chars).collect::<String>()).unwrap_or_default();
        items.push(format!("{}. {first}", n + 1));
    }

    let intro = clean(&words[..markers[0].0]);
    let mut out = String::new();
    if !intro.is_empty() {
        out.push_str(&intro);
        if !intro.ends_with([':', '?', '!']) {
            out.push(':');
        }
        out.push('\n');
    }
    out.push_str(&items.join("\n"));
    out
}

#[cfg(test)]
mod tests {
    use super::apply;

    #[test]
    fn breaks_lines_and_paragraphs() {
        assert_eq!(apply("Hi Sam, new line, thanks for the notes."), "Hi Sam\nThanks for the notes.");
        assert_eq!(apply("Done. New paragraph. Next up is pricing."), "Done.\n\nNext up is pricing.");
        assert_eq!(
            apply("Groceries: bullet point milk, bullet point eggs."),
            "Groceries:\n- Milk\n- Eggs."
        );
    }

    #[test]
    fn types_the_punctuation_it_names() {
        assert_eq!(apply("Are you coming question mark"), "Are you coming?");
        assert_eq!(apply("Wow, exclamation mark. That worked."), "Wow! That worked.");
        assert_eq!(apply("apples comma pears comma plums"), "apples, pears, plums");
        assert_eq!(apply("Está bem vírgula falamos amanhã ponto final"), "Está bem, falamos amanhã.");
        assert_eq!(apply("first ponto e vírgula second"), "first; second");
    }

    #[test]
    fn leaves_ordinary_words_alone() {
        assert_eq!(apply("Commander Data is online."), "Commander Data is online.");
        assert_eq!(apply("Comma."), "Comma.");
        assert_eq!(apply("The newlines look fine."), "The newlines look fine.");
    }

    #[test]
    fn cases_identifiers() {
        assert_eq!(apply("Rename it to camel case user name."), "Rename it to userName.");
        assert_eq!(apply("Set snake case max retries, to five."), "Set max_retries, to five.");
        assert_eq!(apply("pascal case dictation event"), "DictationEvent");
        assert_eq!(apply("The constant case api key comma please"), "The API_KEY, please");
        assert_eq!(apply("kebab-case main nav bar"), "main-nav-bar");
        assert_eq!(apply("I love a good case of camel."), "I love a good case of camel.");
    }

    #[test]
    fn numbers_lists() {
        assert_eq!(apply("Groceries number one milk number two eggs number three bread."), "Groceries:\n1. Milk\n2. Eggs\n3. Bread");
        assert_eq!(apply("Plan: 1. Draft it. 2. Send it."), "Plan:\n1. Draft it\n2. Send it");
        assert_eq!(apply("1) ship, 2) rest"), "1. Ship\n2. Rest");
    }

    #[test]
    fn leaves_ordinary_numbers_alone() {
        assert_eq!(apply("We are number one in town."), "We are number one in town.");
        assert_eq!(apply("One, two, three, go."), "One, two, three, go.");
        assert_eq!(apply("See chapter 2. It covers 3 cases."), "See chapter 2. It covers 3 cases.");
        assert_eq!(apply("Meet at 2. Then 1. Fine."), "Meet at 2. Then 1. Fine.");
    }
}
