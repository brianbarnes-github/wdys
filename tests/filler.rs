use wdys::filler::clean_transcript;

#[test]
fn removes_basic_fillers() {
    assert_eq!(
        clean_transcript("So, um, I think uh we should go"),
        "So, I think we should go"
    );
}

#[test]
fn leading_filler_recapitalizes() {
    assert_eq!(clean_transcript("Um, hello there"), "Hello there");
}

#[test]
fn keeps_words_containing_fillers() {
    let text = "The umbrella is human, Ahmed made an error";
    assert_eq!(clean_transcript(text), text);
}

#[test]
fn collapses_short_stutters() {
    assert_eq!(clean_transcript("I I I want the the thing"), "I want the thing");
}

#[test]
fn leaves_longer_repeats_alone() {
    // "that that" is legitimate English ("I know that that is true") and is 4 letters,
    // above the 3-letter stutter threshold. Emphatic "no, no" is also kept.
    assert_eq!(clean_transcript("I know that that is true"), "I know that that is true");
    assert_eq!(clean_transcript("no, no, wait"), "no, no, wait");
}

#[test]
fn strips_whisper_markers() {
    assert_eq!(clean_transcript("[BLANK_AUDIO]"), "");
    assert_eq!(clean_transcript("hello [ Silence ]"), "hello");
    assert_eq!(clean_transcript("(music) hello"), "hello");
}

#[test]
fn filler_only_becomes_empty() {
    assert_eq!(clean_transcript("Um. Uh, ahh."), "");
}

#[test]
fn keeps_sentence_end_when_trailing_filler_dropped() {
    assert_eq!(clean_transcript("I think so, um."), "I think so.");
}

#[test]
fn recapitalizes_after_dropped_filler_following_sentence_end() {
    assert_eq!(clean_transcript("Okay. Um, let's go"), "Okay. Let's go");
}

#[test]
fn keeps_meaningful_hyphenated_uh_huh() {
    assert_eq!(clean_transcript("uh-huh, sure"), "uh-huh, sure");
}
