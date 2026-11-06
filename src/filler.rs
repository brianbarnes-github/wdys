const FILLERS: &[&str] = &[
    "um", "umm", "uh", "uhh", "ah", "ahh", "er", "erm", "hm", "hmm",
];
const STUTTER_MAX_LEN: usize = 3;
const SENTENCE_END: &[char] = &['.', '!', '?'];

struct Token<'a> {
    core: &'a str,
    trail: &'a str,
}

/// Splits a whitespace-delimited token into its word core and trailing punctuation.
fn split_token(token: &str) -> Token<'_> {
    let start = token.char_indices().find(|(_, c)| c.is_alphanumeric());
    let end = token
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_alphanumeric())
        .map(|(i, c)| i + c.len_utf8());
    match (start, end) {
        (Some((s, _)), Some(e)) => Token {
            core: &token[s..e],
            trail: &token[e..],
        },
        _ => Token {
            core: "",
            trail: "",
        },
    }
}

fn is_filler(core: &str) -> bool {
    FILLERS.contains(&core.to_lowercase().as_str())
}

fn ends_sentence(token: &str) -> bool {
    token.ends_with(SENTENCE_END)
}

/// Removes `[...]` and `(...)` spans such as `[BLANK_AUDIO]` or `(music)`.
fn strip_markers(raw: &str) -> String {
    let mut depth = 0u32;
    raw.chars()
        .filter(|&c| match c {
            '[' | '(' => {
                depth += 1;
                false
            }
            ']' | ')' if depth > 0 => {
                depth -= 1;
                false
            }
            _ => depth == 0,
        })
        .collect()
}

fn capitalize_first(token: &str) -> String {
    let mut done = false;
    token
        .chars()
        .map(|c| {
            if !done && c.is_alphabetic() {
                done = true;
                c.to_uppercase().collect::<String>()
            } else {
                c.to_string()
            }
        })
        .collect()
}

/// Keeps a dropped filler's sentence-ending punctuation by moving it onto the previous word.
fn attach_terminal(out: &mut [String], terminal: char) {
    if let Some(last) = out.last_mut() {
        if ends_sentence(last) {
            return;
        }
        if last.ends_with(',') {
            last.pop();
        }
        last.push(terminal);
    }
}

fn is_stutter_of(previous: &str, current_core: &str) -> bool {
    let prev = split_token(previous);
    prev.trail.is_empty()
        && !prev.core.is_empty()
        && prev.core.chars().count() <= STUTTER_MAX_LEN
        && prev.core.to_lowercase() == current_core.to_lowercase()
}

/// Removes filler words, short stutters and Whisper markers from a transcript.
/// Returns an empty string when nothing meaningful remains.
pub fn clean_transcript(raw: &str) -> String {
    let stripped = strip_markers(raw);
    let mut out: Vec<String> = Vec::new();
    let mut capitalize_next = false;

    for word in stripped.split_whitespace() {
        let token = split_token(word);
        if is_filler(token.core) {
            capitalize_next = out.last().is_none_or(|last| ends_sentence(last));
            if let (Some(terminal), false) = (
                token.trail.chars().find(|c| SENTENCE_END.contains(c)),
                out.is_empty(),
            ) {
                attach_terminal(&mut out, terminal);
            }
            continue;
        }
        if out
            .last()
            .is_some_and(|prev| is_stutter_of(prev, token.core))
        {
            out.pop();
        }
        out.push(if capitalize_next {
            capitalize_first(word)
        } else {
            word.to_string()
        });
        capitalize_next = false;
    }
    out.join(" ")
}
