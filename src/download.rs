//! Port of `download_if_necessary` from `pocket_tts/utils/utils.py`: `hf://` paths go through the
//! Hugging Face cache, shared with the Python package, and `http(s)://` URLs through
//! `~/.cache/pocket_tts`.

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// A connection that never opens must not hang the worker.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// Ceiling on the whole transfer. Generous enough for a multi-gigabyte model on a slow link,
/// but finite: the blocking client has no per-read timeout, so a stalled body is caught here.
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(60 * 60);
/// Models are the largest download, so this is generous, but still a ceiling: an endless or
/// hostile response cannot fill the disk or memory.
const MAX_DOWNLOAD_BYTES: u64 = 4 * 1024 * 1024 * 1024;

pub fn download_if_necessary(file_path: &str) -> Result<PathBuf> {
    if file_path.starts_with("http://") || file_path.starts_with("https://") {
        download_url(file_path)
    } else if let Some(rest) = file_path.strip_prefix("hf://") {
        let (repo_id, filename, revision) = split_hf_path(rest)?;
        download_hf(repo_id, &filename, revision)
            .with_context(|| format!("cannot download {file_path}"))
    } else {
        Ok(PathBuf::from(file_path))
    }
}

/// `owner/name/path/in/repo[@revision]` into its parts.
fn split_hf_path(path: &str) -> Result<(&str, String, Option<&str>)> {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() < 3 {
        bail!("`hf://{path}` should look like hf://<owner>/<repo>/<file>[@revision]")
    }
    let repo_end = parts[0].len() + 1 + parts[1].len();
    let repo_id = &path[..repo_end];
    let filename = &path[repo_end + 1..];
    match filename.split_once('@') {
        Some((filename, revision)) => Ok((repo_id, filename.to_string(), Some(revision))),
        None => Ok((repo_id, filename.to_string(), None)),
    }
}

/// The client reads `HF_TOKEN`, `HF_ENDPOINT`, `HF_HOME` and the token `hf auth login` stores.
fn download_hf(repo_id: &str, filename: &str, revision: Option<&str>) -> Result<PathBuf> {
    let client = hf_hub::HFClientSync::new()?;
    let (owner, name) = hf_hub::split_id(repo_id);
    let path = client
        .model(owner, name)
        .download_file()
        .filename(filename)
        .maybe_revision(revision.map(str::to_string))
        .send()?;
    Ok(path)
}

pub fn cache_directory() -> Result<PathBuf> {
    let home = std::env::home_dir().context("cannot locate the home directory")?;
    let dir = home.join(".cache").join("pocket_tts");
    std::fs::create_dir_all(&dir).with_context(|| format!("cannot create {}", dir.display()))?;
    Ok(dir)
}

fn download_url(url: &str) -> Result<PathBuf> {
    let url_path = url.split(['?', '#']).next().unwrap_or(url);
    let url_path = url_path.splitn(4, '/').nth(3).unwrap_or("");
    let suffix = Path::new(url_path)
        .extension()
        .map(|ext| format!(".{}", ext.to_string_lossy()))
        .unwrap_or_default();
    let digest = Sha256::digest(url.as_bytes());
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    let cached_file = cache_directory()?.join(format!("{hex}{suffix}"));
    if !cached_file.exists() {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(TRANSFER_TIMEOUT)
            .build()
            .context("cannot create the HTTP client")?;
        let response = client
            .get(url)
            .send()
            .and_then(|r| r.error_for_status())
            .with_context(|| format!("cannot download {url}"))?;
        if let Some(length) = response.content_length() {
            if length > MAX_DOWNLOAD_BYTES {
                bail!(
                    "{url} is {length} bytes, larger than the {MAX_DOWNLOAD_BYTES}-byte limit"
                );
            }
        }
        // Stream to a partial file instead of buffering the whole response, and stop one byte
        // past the limit so an unbounded response is refused without being kept.
        let partial = cached_file.with_extension("partial");
        let mut file = std::fs::File::create(&partial)
            .with_context(|| format!("cannot create {}", partial.display()))?;
        let written = std::io::copy(&mut response.take(MAX_DOWNLOAD_BYTES + 1), &mut file)
            .with_context(|| format!("cannot download {url}"))?;
        if written > MAX_DOWNLOAD_BYTES {
            drop(file);
            let _ = std::fs::remove_file(&partial);
            bail!("{url} is larger than the {MAX_DOWNLOAD_BYTES}-byte limit");
        }
        file.sync_all()?;
        drop(file);
        std::fs::rename(&partial, &cached_file)?;
    }
    Ok(cached_file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hf_paths_split_like_python() {
        assert_eq!(
            split_hf_path("kyutai/pocket-tts/languages/english/model.safetensors@abc").unwrap(),
            (
                "kyutai/pocket-tts",
                "languages/english/model.safetensors".to_string(),
                Some("abc")
            )
        );
        assert_eq!(
            split_hf_path("user/repo/config.yaml").unwrap(),
            ("user/repo", "config.yaml".to_string(), None)
        );
        assert!(split_hf_path("user/repo").is_err());
    }
}
