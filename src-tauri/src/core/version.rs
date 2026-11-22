//! Version management: prebuilt release download, SHA256 verify, blob-backed
//! install, junction switch and rollback (doc 02 §3).
use std::path::{Path, PathBuf};
use tauri::Emitter;
use crate::{core::{storage, util, zipfile}, error::{Error, Result}, model::VersionInfo, state::{AppState, EV_DOWNLOAD_PROGRESS}};

fn asset_from_release(json: &serde_json::Value) -> Result<(String, String)> {
    let tag = json.get("tag_name").and_then(|v| v.as_str()).unwrap_or("latest").to_string();
    let assets = json.get("assets").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let asset = assets.iter().find(|a| a.get("name").and_then(|n| n.as_str())
        .map(|n| n.ends_with(".zip") && n.contains("win-x64")).unwrap_or(false))
        .or_else(|| assets.iter().find(|a| a.get("name").and_then(|n| n.as_str()).map(|n| n.ends_with(".zip")).unwrap_or(false)))
        .ok_or_else(|| Error::msg("release has no zip asset"))?;
    let url = asset.get("browser_download_url").and_then(|v| v.as_str())
        .ok_or_else(|| Error::msg("asset has no download URL"))?.to_string();
    Ok((tag, url))
}

async fn resolve_download(state: &AppState, url: Option<&str>, version: Option<&str>) -> Result<(String, String, Option<String>)> {
    if let Some(u) = url.filter(|s| !s.is_empty()) {
        let name = version.map(str::to_string).unwrap_or_else(|| {
            Path::new(u).file_name().map(|n| n.to_string_lossy().trim_end_matches(".zip").to_string())
                .unwrap_or_else(|| "manual".into())
        });
        return Ok((name, util::proxied_github_url(u, &state.hub.config.gh_proxy_prefix), state.hub.config.expected_sha256.clone()));
    }
    if let Some(u) = state.hub.config.zip_url.as_deref().filter(|s| !s.is_empty()) {
        let name = version.map(str::to_string).unwrap_or_else(|| "prebuilt".into());
        return Ok((name, util::proxied_github_url(u, &state.hub.config.gh_proxy_prefix), state.hub.config.expected_sha256.clone()));
    }
    let api = util::proxied_github_url(&state.hub.config.release_api, &state.hub.config.gh_proxy_prefix);
    let client = util::http_client(state.hub.proxy_url().as_deref())?;
    let body = util::fetch(&client, &api, None).await?.bytes;
    let json: serde_json::Value = serde_json::from_slice(&body)?;
    let (tag, asset_url) = asset_from_release(&json)?;
    Ok((version.map(str::to_string).unwrap_or(tag),
        util::proxied_github_url(&asset_url, &state.hub.config.gh_proxy_prefix),
        state.hub.config.expected_sha256.clone()))
}

pub async fn list(state: &AppState) -> Result<Vec<VersionInfo>> {
    let p = state.persisted.read().await;
    let mut out = p.versions.clone();
    for v in &mut out { v.is_current = Some(&v.name) == p.current_version.as_ref(); }
    drop(p);
    // Discover manually placed versions as well.
    if let Ok(rd) = std::fs::read_dir(state.hub.versions_dir()) {
        for e in rd.flatten() {
            if !e.path().is_dir() || e.file_name().to_string_lossy().starts_with('.') { continue; }
            let name = e.file_name().to_string_lossy().into_owned();
            if out.iter().any(|v| v.name == name) { continue; }
            out.push(VersionInfo {
                name: name.clone(), path: e.path().to_string_lossy().into_owned(),
                installed_at: util::now_iso(), size_bytes: util::dir_size(&e.path()),
                is_current: state.persisted.read().await.current_version.as_deref() == Some(name.as_str()),
                source: "filesystem".into(),
            });
        }
    }
    Ok(out)
}

pub async fn install(state: &AppState, url: Option<&str>, version: Option<&str>, sha256: Option<&str>) -> Result<VersionInfo> {
    let (raw_name, url, expected) = resolve_download(state, url, version).await?;
    let name = util::sanitize_name(&raw_name);
    let staging = state.hub.staging_dir().join(format!("{}.tmp-{}", name, uuid::Uuid::new_v4().simple()));
    let extracted = staging.join("extracted");
    std::fs::create_dir_all(&extracted)?;
    state.log("info", "version", &format!("downloading {} -> {}", url, staging.display()));
    let client = util::http_client(state.hub.proxy_url().as_deref())?;
    let zip_path = staging.join("release.zip");
    let handle = state.handle.lock().ok().and_then(|h| h.clone());
    let url_for_event = url.clone();
    let sha = util::download_file(&client, &url, &zip_path, sha256.or(expected.as_deref()), move |done, total| {
        if let Some(h) = &handle {
            let _ = h.emit(EV_DOWNLOAD_PROGRESS, serde_json::json!({
                "url": url_for_event, "downloaded": done, "total": total,
            }));
        }
    }).await?;
    let count = zipfile::extract_zip(&zip_path, &extracted)?;
    if count == 0 { return Err(Error::msg("release zip contained no files")); }
    if !find_exe(&extracted).is_some() {
        // Some releases wrap the payload in a single directory; find_exe recurses.
        return Err(Error::msg("release zip has no executable"));
    }
    let target = state.hub.current_version_path(&name);
    if target.exists() { std::fs::remove_dir_all(&target)?; }
    let files = storage::ingest_tree(state, &extracted, &target)?;
    let info = VersionInfo {
        name: name.clone(), path: target.to_string_lossy().into_owned(),
        installed_at: util::now_iso(), size_bytes: util::dir_size(&target),
        is_current: false, source: format!("download:{}", sha),
    };
    {
        let mut p = state.persisted.write().await;
        p.versions.retain(|v| v.name != name);
        p.versions.push(info.clone());
        if p.current_version.is_none() { p.current_version = Some(name.clone()); }
        prune_versions(state, &mut p).await;
    }
    state.persist().await?;
    let _ = std::fs::remove_dir_all(&staging);
    state.log("info", "version", &format!("installed {} ({} files)", name, files));
    Ok(info)
}

async fn prune_versions(state: &AppState, p: &mut crate::core::PersistedState) {
    let keep = state.hub.config.keep_versions.max(1);
    let mut old: Vec<String> = p.versions.iter()
        .filter(|v| Some(&v.name) != p.current_version.as_ref() && Some(&v.name) != p.previous_version.as_ref())
        .map(|v| v.name.clone()).collect();
    while p.versions.len() > keep && !old.is_empty() {
        let name = old.remove(0);
        let _ = std::fs::remove_dir_all(state.hub.current_version_path(&name));
        p.versions.retain(|v| v.name != name);
    }
}

fn find_exe(root: &Path) -> Option<PathBuf> {
    let mut all = Vec::new();
    for e in walkdir::WalkDir::new(root).follow_links(true).max_depth(4).into_iter().flatten() {
        if e.file_type().is_file() && e.path().extension().map(|x| x.eq_ignore_ascii_case("exe")).unwrap_or(false) {
            all.push(e.path().to_path_buf());
        }
    }
    all.sort_by_key(|p| {
        let n = p.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
        let rank = if n.contains("portableopenlethe") { 0 } else if n.contains("openlethe") { 1 }
                   else if n.contains("lethe") { 2 } else { 3 };
        (rank, p.components().count())
    });
    all.into_iter().next()
}

pub async fn switch(state: &AppState, version: &str) -> Result<VersionInfo> {
    let name = util::sanitize_name(version);
    let target = state.hub.current_version_path(&name);
    if !target.is_dir() { return Err(Error::msg(format!("version {} is not installed", name))); }
    crate::core::process::stop_all(state).await;
    util::create_junction(&target, &state.hub.root.join("current"))?;
    #[cfg(windows)]
    {
        // Also place a junction at the conventional location for tools that expect it.
        let link = state.hub.versions_dir().join("current");
        util::create_junction(&target, &link)?;
    }
    let info = {
        let mut p = state.persisted.write().await;
        p.previous_version = p.current_version.clone();
        p.current_version = Some(name.clone());
        for v in &mut p.versions { v.is_current = v.name == name; }
        for rec in &mut p.instances { rec.version = Some(name.clone()); }
        p.versions.iter().find(|v| v.name == name).cloned().unwrap_or(VersionInfo {
            name: name.clone(), path: target.to_string_lossy().into_owned(),
            installed_at: util::now_iso(), size_bytes: util::dir_size(&target), is_current: true, source: "filesystem".into(),
        })
    };
    // Re-point every instance's server junction.
    let ids: Vec<String> = state.persisted.read().await.instances.iter().map(|i| i.id.clone()).collect();
    for id in ids { util::create_junction(&target, &state.hub.instance_dir(&id).join("server"))?; }
    state.persist().await?;
    state.log("info", "version", &format!("switched current version to {}", name));
    Ok(info)
}

pub async fn rollback(state: &AppState) -> Result<VersionInfo> {
    let prev = state.persisted.read().await.previous_version.clone()
        .ok_or_else(|| Error::msg("no previous version to roll back to"))?;
    switch(state, &prev).await
}
