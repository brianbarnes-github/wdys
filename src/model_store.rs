use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::config::ModelSize;
use crate::error::ModelError;

pub trait Downloader {
    fn fetch(&self, url: &str, dest: &Path) -> Result<(), ModelError>;
}

pub trait Consent {
    fn allow_download(&self, size: ModelSize, approx_bytes: u64) -> bool;
}

fn file_name(size: ModelSize) -> &'static str {
    match size {
        ModelSize::BaseEn => "ggml-base.en.bin",
        ModelSize::SmallEn => "ggml-small.en.bin",
    }
}

pub fn model_path(dir: &Path, size: ModelSize) -> PathBuf {
    dir.join(file_name(size))
}

pub fn model_url(size: ModelSize) -> &'static str {
    match size {
        ModelSize::BaseEn => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin",
        ModelSize::SmallEn => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.en.bin",
    }
}

/// SHA-256 of the published files (Hugging Face LFS object ids for ggerganov/whisper.cpp).
pub fn model_sha256(size: ModelSize) -> &'static str {
    match size {
        ModelSize::BaseEn => "a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002",
        ModelSize::SmallEn => "c6138d6d58ecc8322097e0f987c32f1be8bb0a18532a3f88f734d1bbf9c41e5d",
    }
}

pub fn model_approx_bytes(size: ModelSize) -> u64 {
    match size {
        ModelSize::BaseEn => 147_964_211,
        ModelSize::SmallEn => 487_614_201,
    }
}

pub fn verify_checksum(path: &Path, expected_hex: &str) -> Result<(), ModelError> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    if format!("{:x}", hasher.finalize()).eq_ignore_ascii_case(expected_hex) {
        Ok(())
    } else {
        Err(ModelError::ChecksumMismatch { path: path.to_path_buf() })
    }
}

fn part_path(final_path: &Path) -> PathBuf {
    let mut name: OsString = final_path.as_os_str().to_owned();
    name.push(".part");
    PathBuf::from(name)
}

/// Returns the path to a verified model, downloading it (after consent) if needed.
/// A mismatched existing file is treated as missing but is only replaced (by rename) once a
/// verified download exists, so a hand-placed file is never destroyed; a bad download never
/// reaches the final path.
pub fn ensure_model(
    dir: &Path,
    size: ModelSize,
    downloader: &dyn Downloader,
    consent: &dyn Consent,
    expected_sha256: &str,
) -> Result<PathBuf, ModelError> {
    let final_path = model_path(dir, size);
    if final_path.exists() && verify_checksum(&final_path, expected_sha256).is_ok() {
        return Ok(final_path);
    }
    if !consent.allow_download(size, model_approx_bytes(size)) {
        return Err(ModelError::DownloadDeclined);
    }
    fs::create_dir_all(dir)?;
    let part = part_path(&final_path);
    let fetched = downloader
        .fetch(model_url(size), &part)
        .and_then(|()| verify_checksum(&part, expected_sha256));
    if let Err(e) = fetched {
        let _ = fs::remove_file(&part);
        return Err(e);
    }
    fs::rename(&part, &final_path)?;
    Ok(final_path)
}

/// Downloads over HTTPS with `ureq`.
pub struct UreqDownloader;

impl Downloader for UreqDownloader {
    fn fetch(&self, url: &str, dest: &Path) -> Result<(), ModelError> {
        let response = ureq::get(url).call().map_err(|e| ModelError::Download(e.to_string()))?;
        let mut file = File::create(dest)?;
        io::copy(&mut response.into_reader(), &mut file)?;
        Ok(())
    }
}
