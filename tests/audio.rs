use wdys::audio::{is_effectively_silent, to_whisper_input, AudioClip};

fn tone(seconds: f32) -> AudioClip {
    let n = (16_000.0 * seconds) as usize;
    AudioClip {
        samples: (0..n)
            .map(|i| 0.5 * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 16_000.0).sin())
            .collect(),
    }
}

#[test]
fn stereo_48k_becomes_mono_16k() {
    let clip = to_whisper_input(&vec![0.1; 96_000], 48_000, 2);
    assert_eq!(clip.samples.len(), 16_000);
}

#[test]
fn already_16k_mono_is_unchanged() {
    let input = vec![0.1, 0.2, -0.3, 0.4];
    assert_eq!(to_whisper_input(&input, 16_000, 1).samples, input);
}

#[test]
fn downmix_averages_channels() {
    assert_eq!(to_whisper_input(&[1.0, -1.0], 16_000, 2).samples, vec![0.0]);
}

#[test]
fn silent_clip_is_detected() {
    assert!(is_effectively_silent(&AudioClip { samples: vec![0.0; 16_000] }));
}

#[test]
fn one_second_of_tone_is_not_silent() {
    assert!(!is_effectively_silent(&tone(1.0)));
}

#[test]
fn very_short_clip_counts_as_silent() {
    assert!(is_effectively_silent(&tone(0.2)));
}
