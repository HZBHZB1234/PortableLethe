//! Shared primitives: atomic IO, hashing, HTTP with ETag/Range/retry, Windows junctions.
use std::path::{Path, PathBuf};
use std::time::Duration;
use sha2::{Digest, Sha256};
use crate::error::{Error, Result};

pub fn now_iso() -> String { chrono::Local::now().to_rfc3339() }

pub fn sanitize_name(s: &str) -> String {
    let mut out: String = s.trim().chars().map(|c| match c {
        '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => '_',
        _ => c,
    }).collect();
    while out.ends_with('.') || out.ends_with(' ') { out.pop(); }
    if out.is_empty() { out = "unnamed".into(); }
    out
}

pub fn sha256_bytes(data: &[u8]) -> String {
    let mut h = Sha256::new(); h.update(data); hex::encode(h.finalize())
}

pub fn sha256_file(path: &Path) -> Result<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 { break; }
        h.update(&buf[..n]);
    }
    Ok(hex::encode(h.finalize()))
}

/// Temp file in the same directory + rename => readers never see a partial file.
pub fn atomic_write(path: &Path, data: &[u8], backup: bool) -> Result<()> {
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
    let tmp = path.with_extension(format!("tmp-{}", uuid::Uuid::new_v4().simple()));
    std::fs::write(&tmp, data)?;
    if path.exists() && backup {
        let bak = path.with_extension(format!("{}.bak", path.extension().unwrap_or_default().to_string_lossy()));
        let _ = std::fs::copy(path, &bak);
    }
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            // Windows rename-over-existing can fail; replace explicitly.
            if path.exists() {
                std::fs::remove_file(path)?;
                std::fs::rename(&tmp, path)?;
                Ok(())
            } else { Err(e.into()) }
        }
    }
}

pub fn atomic_write_json<T: serde::Serialize>(path: &Path, value: &T, backup: bool) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    atomic_write(path, &bytes, backup)
}

pub fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
}

pub fn dir_size(path: &Path) -> u64 {
    walkdir::WalkDir::new(path).into_iter().filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok().map(|m| m.len())).sum()
}

pub fn http_client(proxy: Option<&str>) -> Result<reqwest::Client> {
    let mut b = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .user_agent(concat!("PortableLetheHub/", env!("CARGO_PKG_VERSION")));
    if let Some(p) = proxy.filter(|s| !s.is_empty()) {
        b = b.proxy(reqwest::Proxy::all(p)?);
    }
    Ok(b.build()?)
}

/// Build a GitHub download URL with the configured gh-proxy prefix (doc 04 §7).
pub fn proxied_github_url(url: &str, prefix: &str) -> String {
    if url.contains("github.com") || url.contains("objects.githubusercontent.com") {
        format!("{}{}", prefix.trim_end_matches('/'), url)
    } else { url.to_string() }
}

pub struct Fetched {
    pub bytes: Vec<u8>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub not_modified: bool,
}

pub async fn fetch(client: &reqwest::Client, url: &str, etag: Option<&str>) -> Result<Fetched> {
    let mut req = client.get(url);
    if let Some(e) = etag.filter(|s| !s.is_empty()) { req = req.header("If-None-Match", e); }
    let resp = req.send().await?.error_for_status()?;
    let not_modified = resp.status() == reqwest::StatusCode::NOT_MODIFIED;
    let etag = resp.headers().get(reqwest::header::ETAG)
        .and_then(|v| v.to_str().ok()).map(str::to_string);
    let last_modified = resp.headers().get(reqwest::header::LAST_MODIFIED)
        .and_then(|v| v.to_str().ok()).map(str::to_string);
    let bytes = if not_modified { Vec::new() } else { resp.bytes().await?.to_vec() };
    Ok(Fetched { bytes, etag, last_modified, not_modified })
}

/// Resumable download with SHA256 verification and exponential retry.
pub async fn download_file<F>(client: &reqwest::Client, url: &str, dest: &Path,
                              expected_sha256: Option<&str>, mut on_progress: F) -> Result<String>
where F: FnMut(u64, u64) + Send {
    if let Some(parent) = dest.parent() { std::fs::create_dir_all(parent)?; }
    let part = dest.with_extension("part");
    let mut last_err = String::new();
    for attempt in 0..3 {
        if attempt > 0 { tokio::time::sleep(Duration::from_millis(300 * (1 << attempt))).await; }
        let existing = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
        let mut req = client.get(url);
        if existing > 0 { req = req.header(reqwest::header::RANGE, format!("bytes={}-", existing)); }
        let res = req.send().await;
        let resp = match res {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => { last_err = format!("HTTP {}", r.status()); continue; }
            Err(e) => { last_err = e.to_string(); continue; }
        };
        let total = resp.content_length().unwrap_or(0) + if resp.status() == reqwest::StatusCode::PARTIAL_CONTENT { existing } else { 0 };
        let append = resp.status() == reqwest::StatusCode::PARTIAL_CONTENT && existing > 0;
        let result: Result<()> = async {
            use futures_util::StreamExt;
            let mut file = if append {
                use std::io::Write;
                let mut f = std::fs::OpenOptions::new().append(true).open(&part)?;
                f.flush()?; f
            } else { std::fs::File::create(&part)? };
            use std::io::Write as _;
            let mut stream = resp.bytes_stream();
            let mut written = if append { existing } else { 0 };
            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                file.write_all(&chunk)?;
                written += chunk.len() as u64;
                on_progress(written, total);
            }
            file.flush()?;
            Ok(())
        }.await;
        if let Err(e) = result { last_err = e.to_string(); continue; }
        let digest = sha256_file(&part)?;
        if let Some(expected) = expected_sha256 {
            if !expected.is_empty() && !digest.eq_ignore_ascii_case(expected) {
                last_err = format!("SHA256 mismatch: got {} expected {}", digest, expected);
                let _ = std::fs::remove_file(&part);
                continue;
            }
        }
        std::fs::rename(&part, dest)?;
        return Ok(digest);
    }
    Err(Error::msg(format!("download failed after 3 attempts ({}): {}", url, last_err)))
}

#[cfg(windows)]
pub fn create_junction(target: &Path, link: &Path) -> Result<()> {
    use std::os::windows::process::CommandExt;
    if let Some(parent) = link.parent() { std::fs::create_dir_all(parent)?; }
    if link.exists() || link.symlink_metadata().is_ok() { remove_link(link)?; }
    let t = std::path::absolute(target)?;
    let l = std::path::absolute(link)?;
    let out = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J", &l.to_string_lossy(), &t.to_string_lossy()])
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .output()?;
    if !out.status.success() {
        return Err(Error::msg(format!("mklink /J failed: {}", String::from_utf8_lossy(&out.stderr))));
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn create_junction(target: &Path, link: &Path) -> Result<()> {
    if let Some(parent) = link.parent() { std::fs::create_dir_all(parent)?; }
    if link.symlink_metadata().is_ok() { remove_link(link)?; }
    std::os::unix::fs::symlink(target, link)?;
    Ok(())
}

pub fn remove_link(link: &Path) -> Result<()> {
    if link.symlink_metadata().is_err() { return Ok(()); }
    std::fs::remove_dir(link).or_else(|_| std::fs::remove_file(link))?;
    Ok(())
}

#[cfg(windows)]
pub fn is_junction(path: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    std::fs::symlink_metadata(path).map(|m| m.file_attributes() & 0x400 != 0).unwrap_or(false)
}

#[cfg(not(windows))]
pub fn is_junction(path: &Path) -> bool {
    std::fs::symlink_metadata(path).map(|m| m.file_type().is_symlink()).unwrap_or(false)
}

/// Copy a directory tree (used when hardlinks cannot cross volumes).
pub fn copy_tree(src: &Path, dst: &Path) -> Result<u64> {
    let mut total = 0u64;
    for entry in walkdir::WalkDir::new(src) {
        let entry = entry?;
        let rel = entry.path().strip_prefix(src)?;
        let out = dst.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&out)?;
        } else if entry.file_type().is_file() {
            if let Some(p) = out.parent() { std::fs::create_dir_all(p)?; }
            std::fs::copy(entry.path(), &out)?;
            total += entry.metadata()?.len();
        }
    }
    Ok(total)
}
