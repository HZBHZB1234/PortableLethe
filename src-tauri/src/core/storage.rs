//! Content-addressed blob store + hardlink/reflink deployment + port allocation.
use std::path::{Path, PathBuf};
use crate::{core::util, error::{Error, Result}, state::AppState};

/// Store a file once under blobs/sha256/<hash>; return the hash.
pub fn ingest_blob(state: &AppState, path: &Path) -> Result<String> {
    let hash = util::sha256_file(path)?;
    let target = state.hub.blobs_dir().join(&hash);
    if !target.exists() {
        std::fs::create_dir_all(state.hub.blobs_dir())?;
        link_or_copy(state, path, &target)?;
    }
    Ok(hash)
}

/// Same-volume NTFS hardlink first; cross-volume falls back to a plain copy.
/// With `reflink-crate` enabled this is where a CoW clone would be attempted.
pub fn link_or_copy(_state: &AppState, src: &Path, dst: &Path) -> Result<()> {
    if let Some(parent) = dst.parent() { std::fs::create_dir_all(parent)?; }
    if dst.exists() {
        let _ = std::fs::remove_file(dst);
    }
    match std::fs::hard_link(src, dst) {
        Ok(()) => return Ok(()),
        Err(_) => {
            #[cfg(feature = "reflink-crate")]
            if reflink::reflink(src, dst).is_ok() { return Ok(()); }
            std::fs::copy(src, dst)?;
            Ok(())
        }
    }
}

/// Deploy an entire tree through the blob store (deduplicates identical files).
pub fn ingest_tree(state: &AppState, src: &Path, dst: &Path) -> Result<usize> {
    let mut count = 0;
    for item in walkdir::WalkDir::new(src) {
        let item = item?;
        let rel = item.path().strip_prefix(src)?;
        let out = dst.join(rel);
        if item.file_type().is_dir() {
            std::fs::create_dir_all(&out)?;
        } else if item.file_type().is_file() {
            let hash = ingest_blob(state, item.path())?;
            let blob = state.hub.blobs_dir().join(hash);
            link_or_copy(state, &blob, &out)?;
            count += 1;
        }
    }
    Ok(count)
}

/// Pick a free port from the configured range, considering existing instances.
pub fn allocate_port(state: &AppState) -> u16 {
    let cfg = &state.hub.config;
    let used: Vec<u16> = match state.persisted.try_read() {
        Ok(p) => p.instances.iter().map(|i| i.port).collect(),
        Err(_) => Vec::new(),
    };
    for port in cfg.port_range_start..=cfg.port_range_end {
        if used.contains(&port) { continue; }
        if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() { return port; }
    }
    // Fallback: OS-assigned ephemeral port (still inside no configured range).
    std::net::TcpListener::bind(("127.0.0.1", 0))
        .ok().and_then(|l| l.local_addr().ok()).map(|a| a.port()).unwrap_or(cfg.port_range_start)
}

/// Remove an instance's overlays and links without touching the global store.
pub fn remove_instance_layout(state: &AppState, id: &str) -> Result<()> {
    let dir = state.hub.instance_dir(id);
    for name in ["server", "wwwroot", "static", "modfiles", "modfiles-src", "localize-src", "plugins-src"] {
        let p = dir.join(name);
        if p.symlink_metadata().is_ok() { util::remove_link(&p)?; }
    }
    if dir.exists() { std::fs::remove_dir_all(&dir)?; }
    Ok(())
}

#[allow(dead_code)]
fn _unused(_: PathBuf, _: Error) {}
