//! Static resource synchronisation.
//!
//! Three sources are supported:
//!   release  - standalone versioned zip published with this server (default)
//!   official - Project Moon CDN / catalog bundle (官服)
//!   lethe    - Lethe private-server CDN / catalog bundle (私服)
//!
//! Bundle and localize data are distinct channels upstream, but the server
//! consumes one merged tree. Both bundle providers map the `EN_*` TextAssets
//! from `static_s1_0` into `Localize/en`, which is the OpenLethe convention.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tauri::Emitter;
use crate::{core::{resources, util, zipfile}, error::{Error, Result}, model::*, state::{AppState, EV_SYNC_PROGRESS}};

fn locate_bundle(bundle_name: &str) -> Option<PathBuf> {
    // The 32-hex tail of the bundle filename is the Unity cache inner key;
    // ABRO.bundle_name is the outer key. Both are documented in Claude.md.
    let inner = bundle_name.rsplit('_').next()?.trim_end_matches(".bundle").to_string();
    if inner.len() != 32 { return None; }
    let root = PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_default())
        .join(r"Low\Unity\ProjectMoon_LimbusCompany");
    for outer in std::fs::read_dir(&root).ok()?.flatten() {
        let p = outer.path().join(&inner).join("__data");
        if p.is_file() { return Some(p); }
    }
    None
}

fn emit(state: &AppState, phase: &str, current: u64, total: u64, message: &str) {
    if let Some(h) = state.handle.lock().ok().and_then(|h| h.clone()) {
        let _ = h.emit(EV_SYNC_PROGRESS, serde_json::json!({
            "phase": phase, "current": current, "total": total, "message": message,
        }));
    }
}

fn catalog_path(state: &AppState, source: &str, override_path: Option<&str>) -> PathBuf {
    if let Some(p) = override_path.filter(|s| !s.is_empty()) {
        return crate::core::paths::expand_path(p, &state.hub.root);
    }
    if source == "lethe" {
        if let Some(d) = state.hub.config.lethe_dir.as_deref().filter(|s| !s.is_empty()) {
            return PathBuf::from(d).join(r"LimbusCompany_Data\StreamingAssets\aa\catalog.bin");
        }
    }
    if let Some(p) = state.hub.config.catalog_path.as_deref().filter(|s| !s.is_empty()) {
        return crate::core::paths::expand_path(p, &state.hub.root);
    }
    PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_default())
        .join(r"Low\ProjectMoon\LimbusCompany\com.unity.addressables\catalog_S1.bin")
}

fn settings_path(state: &AppState, source: &str) -> PathBuf {
    if source == "lethe" {
        if let Some(d) = state.hub.config.lethe_dir.as_deref().filter(|s| !s.is_empty()) {
            return PathBuf::from(d).join(r"LimbusCompany_Data\StreamingAssets\aa\settings.json");
        }
    }
    state.hub.config.settings_path.as_deref().map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files (x86)\Steam\steamapps\common\Limbus Company\LimbusCompany_Data\StreamingAssets\aa\settings.json"))
}

fn install_artifact(state: &AppState, source: &str, version: &str,
                    root: &Path, files: BTreeMap<String, String>) -> Result<SyncReport> {
    let obj = state.hub.resources_objects().join("staticdata").join(source).join(version);
    if obj.exists() { std::fs::remove_dir_all(&obj)?; }
    std::fs::create_dir_all(obj.parent().unwrap())?;
    std::fs::rename(root, &obj)?;
    resources::set_category(state, "StaticData", &obj.join("StaticData"))?;
    resources::set_category(state, "Localize", &obj.join("Localize"))?;
    Ok(SyncReport { category: "staticdata".into(), source: Some(source.into()), version: version.into(),
        added: files.len(), needs_restart: true, ..Default::default() })
}

pub async fn sync(state: &AppState, source: &str, override_path: Option<&str>) -> Result<SyncReport> {
    resources::ensure_layout(state)?;
    match source {
        "release" => sync_release(state).await,
        "official" | "lethe" => sync_bundle(state, source, override_path).await,
        other => Err(Error::msg(format!("unknown static source: {}", other))),
    }
}

/// Default bootstrap source: a versioned zip published in our GitHub Release.
async fn sync_release(state: &AppState) -> Result<SyncReport> {
    let cfg = &state.hub.config;
    let (url, version_hint) = if let Some(u) = cfg.static_zip_url.as_deref().filter(|s| !s.is_empty()) {
        (u.to_string(), "custom".to_string())
    } else {
        let candidates = util::proxy_candidates(&cfg.static_release_api, &state.hub.github_proxy_mode());
        let client = util::http_client(state.hub.proxy_url().as_deref())?;
        let body = util::fetch_first(&client, &candidates, None).await?.bytes;
        let json: serde_json::Value = serde_json::from_slice(&body)?;
        let assets = json.get("assets").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        let asset = assets.iter().find(|a| a.get("name").and_then(|n| n.as_str())
            .map(|n| n.starts_with("PortableLetheStatic-")).unwrap_or(false))
            .ok_or_else(|| Error::msg("no PortableLetheStatic asset in release"))?;
        let name = asset.get("name").and_then(|n| n.as_str()).unwrap_or("static.zip");
        let hint = name.trim_start_matches("PortableLetheStatic-v").trim_end_matches(".zip").to_string();
        (asset.get("browser_download_url").and_then(|v| v.as_str())
            .ok_or_else(|| Error::msg("static asset has no download URL"))?.to_string(), hint)
    };

    let staging = state.hub.resources_dir().join("staging").join(format!("static-release-{}", uuid::Uuid::new_v4().simple()));
    let _ = std::fs::remove_dir_all(&staging);
    let extracted = staging.join("extracted");
    std::fs::create_dir_all(&extracted)?;
    emit(state, "static-download", 0, 0, &url);
    let candidates = util::proxy_candidates(&url, &state.hub.github_proxy_mode());
    let client = util::http_client(state.hub.proxy_url().as_deref())?;
    let mut last = String::new();
    for candidate in &candidates {
        let _ = std::fs::remove_file(staging.join("static.part"));
        let c = candidate.clone();
        let h0 = state.handle.lock().ok().and_then(|h| h.clone());
        match util::download_file(&client, candidate, &staging.join("static.zip"), None, move |done, total| {
            if let Some(h) = &h0 {
                let _ = h.emit(EV_SYNC_PROGRESS, serde_json::json!({
                    "phase": "static-download", "current": done, "total": total, "message": c,
                }));
            }
        }).await {
            Ok(_) => { last.clear(); break; }
            Err(e) => last = format!("{}: {}", candidate, e),
        }
    }
    if !last.is_empty() { return Err(Error::msg(last)); }
    emit(state, "static-extract", 0, 0, "");
    zipfile::extract_zip(&staging.join("static.zip"), &extracted)?;
    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(extracted.join("static-manifest.json"))
            .map_err(|e| Error::msg(format!("static zip missing manifest: {}", e)))?,
    )?;
    let version = manifest.get("static_version").and_then(|v| v.as_str())
        .unwrap_or(&version_hint).to_string();
    let files = manifest.get("files").and_then(|v| v.as_object())
        .map(|o| o.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string())).collect())
        .unwrap_or_default();
    if !extracted.join("StaticData/static-data").is_dir() || !extracted.join("Localize/en").is_dir() {
        return Err(Error::msg("static zip has invalid layout"));
    }
    let report = install_artifact(state, "release", &version, &extracted, files)?;
    let _ = std::fs::remove_dir_all(&staging);
    persist_source(state, "release", &version, &report).await?;
    Ok(report)
}

/// Bundle source: catalog + s-token + CDN. Both official and Lethe require
/// the X-Requested-With header documented in tools/prepare_update.py.
async fn sync_bundle(state: &AppState, source: &str, override_path: Option<&str>) -> Result<SyncReport> {
    let cat_path = catalog_path(state, source, override_path);
    if !cat_path.is_file() { return Err(Error::msg(format!("catalog not found: {}", cat_path.display()))); }
    let hits = crate::core::resources::catalog::find(&state.hub, "static_s1_0", Some(&cat_path.to_string_lossy()))?;
    let hit = hits.iter().find(|h| h.bundle_name.as_deref().map(|b| b.contains("static_s1_0")).unwrap_or(false))
        .ok_or_else(|| Error::msg("static_s1_0 bundle not found in catalog"))?;
    let bundle_name = hit.key.clone();
    let version = bundle_name.rsplit('_').next().unwrap_or("unknown").trim_end_matches(".bundle").to_string();
    {
        let p = state.persisted.read().await;
        if p.resources.categories.get("staticdata").and_then(|c| c.version.as_ref()) == Some(&version)
            && p.resources.categories.get("staticdata").and_then(|c| c.source.as_deref()) == Some(source) {
            return Ok(SyncReport { category: "staticdata".into(), source: Some(source.into()),
                version: version.clone(), skipped: 1, ..Default::default() });
        }
    }

    let token = crate::core::resources::catalog::extract_stoken(&settings_path(state, source)).unwrap_or_default();
    if token.is_empty() { return Err(Error::msg("CDN token not found in settings.json")); }
    let staging = state.hub.resources_dir().join("staging").join(format!("static-{}-{}", source, version));
    let _ = std::fs::remove_dir_all(&staging);
    let extracted = staging.join("_unityfs");
    std::fs::create_dir_all(&extracted)?;
    let bundle_file = staging.join(&bundle_name);
    let url = if source == "lethe" {
        state.hub.config.lethe_cdn.replace("{token}", &token).replace("{name}", &bundle_name)
    } else {
        format!("https://download.limbuscompanycdn.org/{}/{}", token, bundle_name)
    };
    let bundle_path = if let Some(p) = locate_bundle(&bundle_name) { p } else {
        emit(state, "static-download", 0, 0, &url);
        let client = util::http_client_with_header(state.hub.proxy_url().as_deref(),
            Some("X-Requested-With"), Some(&state.hub.config.cdn_requested_with))?;
        let c = url.clone();
        let h0 = state.handle.lock().ok().and_then(|h| h.clone());
        util::download_file(&client, &url, &bundle_file, None, move |done, total| {
            if let Some(h) = &h0 {
                let _ = h.emit(EV_SYNC_PROGRESS, serde_json::json!({
                    "phase": "static-download", "current": done, "total": total, "message": c,
                }));
            }
        }).await?;
        bundle_file.clone()
    };

    emit(state, "static-extract", 0, 0, "");
    let bytes = std::fs::read(&bundle_path)?;
    if !unityfs::is_unity_bundle(&bytes) { return Err(Error::msg("not a Unity bundle")); }
    let paths = unityfs::extract_unity_assets(&bytes, &extracted)
        .map_err(|e| Error::msg(format!("unityfs extraction failed: {}", e)))?;
    let out_root = staging.join("root");
    let mut files = BTreeMap::new();
    for p in paths {
        if !p.is_file() { continue; }
        let stem = p.file_stem().unwrap_or_default().to_string_lossy().into_owned();
        let target = if stem.starts_with("EN_") {
            out_root.join("Localize").join("en").join(format!("{}.json", stem))
        } else {
            out_root.join("StaticData").join("static-data").join(format!("{}.json", stem))
        };
        if let Some(parent) = target.parent() { std::fs::create_dir_all(parent)?; }
        std::fs::copy(&p, &target)?;
        files.insert(target.strip_prefix(&out_root)?.to_string_lossy().replace('\\', "/"), util::sha256_file(&target)?);
    }
    if files.len() < 100 { return Err(Error::msg("suspiciously few static data files extracted")); }
    let mut report = install_artifact(state, source, &version, &out_root, files)?;
    report.source = Some(source.into());
    persist_source(state, source, &version, &report).await?;
    let _ = std::fs::remove_dir_all(&staging);
    Ok(report)
}

async fn persist_source(state: &AppState, source: &str, version: &str, report: &SyncReport) -> Result<()> {
    let mut p = state.persisted.write().await;
    p.resources.current = version.to_string();
    p.resources.categories.insert("staticdata".into(), CategoryState {
        source: Some(source.into()), version: Some(version.into()),
        etag: None, updated_at: Some(util::now_iso()), files: BTreeMap::new(), last_error: None,
    });
    drop(p);
    state.persist().await?;
    state.log("info", "resources", &format!("staticdata synced from {}: {} ({} files)",
        source, version, report.added));
    Ok(())
}

pub fn extract_bundle(bundle: &Path, out_dir: &Path) -> Result<usize> {
    let bytes = std::fs::read(bundle)?;
    if !unityfs::is_unity_bundle(&bytes) { return Err(Error::msg("not a Unity bundle")); }
    let paths = unityfs::extract_unity_assets(&bytes, out_dir)
        .map_err(|e| Error::msg(format!("unityfs: {}", e)))?;
    let mut count = 0;
    for p in &paths {
        if !p.is_file() { continue; }
        let stem = p.file_stem().unwrap_or_default().to_string_lossy().into_owned();
        let target = out_dir.join(format!("{}.json", stem));
        if p.as_path() != target.as_path() { std::fs::copy(p, &target)?; }
        count += 1;
    }
    Ok(count)
}
