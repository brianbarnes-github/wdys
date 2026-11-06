use std::cell::Cell;
use std::fs;
use std::path::Path;

use wdys::config::ModelSize;
use wdys::error::ModelError;
use wdys::model_store::{
    ensure_model, model_path, model_sha256, model_url, verify_checksum, Consent, Downloader,
};

const GOOD: &[u8] = b"fake model bytes";
const GOOD_SHA: &str = "355ac2cb838a71f81eda48f4fad7903af0c5e4276a86b8fd3dd845d173f58372";

struct FakeDownloader {
    bytes: &'static [u8],
    fail_after_write: bool,
    calls: Cell<u32>,
}

impl FakeDownloader {
    fn new(bytes: &'static [u8]) -> Self {
        Self {
            bytes,
            fail_after_write: false,
            calls: Cell::new(0),
        }
    }
}

impl Downloader for FakeDownloader {
    fn fetch(&self, _url: &str, dest: &Path) -> Result<(), ModelError> {
        self.calls.set(self.calls.get() + 1);
        fs::write(dest, self.bytes)?;
        if self.fail_after_write {
            return Err(ModelError::Download("connection reset".into()));
        }
        Ok(())
    }
}

struct FakeConsent {
    answer: bool,
    asked: Cell<u32>,
}

impl FakeConsent {
    fn new(answer: bool) -> Self {
        Self {
            answer,
            asked: Cell::new(0),
        }
    }
}

impl Consent for FakeConsent {
    fn allow_download(&self, _size: ModelSize, _approx_bytes: u64) -> bool {
        self.asked.set(self.asked.get() + 1);
        self.answer
    }
}

fn file_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn model_files_are_named_per_size_and_pinned() {
    let dir = Path::new("/models");
    assert_eq!(
        model_path(dir, ModelSize::BaseEn),
        dir.join("ggml-base.en.bin")
    );
    assert_eq!(
        model_path(dir, ModelSize::SmallEn),
        dir.join("ggml-small.en.bin")
    );
    for size in [ModelSize::BaseEn, ModelSize::SmallEn] {
        assert_eq!(model_sha256(size).len(), 64);
        assert!(model_url(size).starts_with("https://huggingface.co/ggerganov/whisper.cpp/"));
    }
}

#[test]
fn verify_checksum_accepts_match_and_rejects_mismatch() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("m.bin");
    fs::write(&path, GOOD).unwrap();
    assert!(verify_checksum(&path, GOOD_SHA).is_ok());
    assert!(matches!(
        verify_checksum(&path, &"0".repeat(64)),
        Err(ModelError::ChecksumMismatch { .. })
    ));
}

#[test]
fn existing_valid_model_needs_no_download_or_prompt() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(model_path(dir.path(), ModelSize::BaseEn), GOOD).unwrap();
    let (dl, consent) = (FakeDownloader::new(GOOD), FakeConsent::new(true));
    let path = ensure_model(dir.path(), ModelSize::BaseEn, &dl, &consent, GOOD_SHA).unwrap();
    assert_eq!(path, model_path(dir.path(), ModelSize::BaseEn));
    assert_eq!((dl.calls.get(), consent.asked.get()), (0, 0));
}

#[test]
fn missing_model_downloads_after_consent() {
    let dir = tempfile::tempdir().unwrap();
    let (dl, consent) = (FakeDownloader::new(GOOD), FakeConsent::new(true));
    let path = ensure_model(dir.path(), ModelSize::BaseEn, &dl, &consent, GOOD_SHA).unwrap();
    assert_eq!(fs::read(&path).unwrap(), GOOD);
    assert_eq!(file_names(dir.path()), vec!["ggml-base.en.bin"]);
    assert_eq!(consent.asked.get(), 1);
}

#[test]
fn declined_consent_is_error_and_downloads_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (dl, consent) = (FakeDownloader::new(GOOD), FakeConsent::new(false));
    let result = ensure_model(dir.path(), ModelSize::BaseEn, &dl, &consent, GOOD_SHA);
    assert!(matches!(result, Err(ModelError::DownloadDeclined)));
    assert_eq!(dl.calls.get(), 0);
    assert!(file_names(dir.path()).is_empty());
}

#[test]
fn checksum_mismatch_deletes_part_file_and_errors() {
    let dir = tempfile::tempdir().unwrap();
    let (dl, consent) = (FakeDownloader::new(b"tampered"), FakeConsent::new(true));
    let result = ensure_model(dir.path(), ModelSize::BaseEn, &dl, &consent, GOOD_SHA);
    assert!(matches!(result, Err(ModelError::ChecksumMismatch { .. })));
    assert!(file_names(dir.path()).is_empty());
}

#[test]
fn corrupt_existing_model_is_redownloaded() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(model_path(dir.path(), ModelSize::BaseEn), b"corrupt").unwrap();
    let (dl, consent) = (FakeDownloader::new(GOOD), FakeConsent::new(true));
    let path = ensure_model(dir.path(), ModelSize::BaseEn, &dl, &consent, GOOD_SHA).unwrap();
    assert_eq!(fs::read(path).unwrap(), GOOD);
    assert_eq!(consent.asked.get(), 1);
}

#[test]
fn downloader_failure_leaves_no_files() {
    let dir = tempfile::tempdir().unwrap();
    let dl = FakeDownloader {
        fail_after_write: true,
        ..FakeDownloader::new(GOOD)
    };
    let result = ensure_model(
        dir.path(),
        ModelSize::BaseEn,
        &dl,
        &FakeConsent::new(true),
        GOOD_SHA,
    );
    assert!(matches!(result, Err(ModelError::Download(_))));
    assert!(file_names(dir.path()).is_empty());
}

#[test]
fn mismatched_existing_model_is_kept_when_download_is_declined() {
    let dir = tempfile::tempdir().unwrap();
    let existing = model_path(dir.path(), ModelSize::BaseEn);
    fs::write(&existing, b"user placed this by hand").unwrap();
    let result = ensure_model(
        dir.path(),
        ModelSize::BaseEn,
        &FakeDownloader::new(GOOD),
        &FakeConsent::new(false),
        GOOD_SHA,
    );
    assert!(matches!(result, Err(ModelError::DownloadDeclined)));
    assert_eq!(fs::read(&existing).unwrap(), b"user placed this by hand");
}

#[test]
fn mismatched_existing_model_is_kept_when_download_fails_verification() {
    let dir = tempfile::tempdir().unwrap();
    let existing = model_path(dir.path(), ModelSize::BaseEn);
    fs::write(&existing, b"user placed this by hand").unwrap();
    let result = ensure_model(
        dir.path(),
        ModelSize::BaseEn,
        &FakeDownloader::new(b"tampered"),
        &FakeConsent::new(true),
        GOOD_SHA,
    );
    assert!(matches!(result, Err(ModelError::ChecksumMismatch { .. })));
    assert_eq!(fs::read(&existing).unwrap(), b"user placed this by hand");
}
