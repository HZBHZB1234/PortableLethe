//! StaticData/Localize extraction via unityfs 0.1.4 (byte-identical to UnityPy
//! in the spike: 2185/2185 files). Extension `.txt` is normalised to `.json`.
use std::path::{Path, PathBuf};
use tauri::Emitter;
use crate::{core::{resources::catalog, util}, error::{Error, Result}, model::*, state::{AppState, EV_SYNC_PROGRESS}};

fn locate_bundle(bundle_name: &str) -> Option<PathBuf> {
    let inner = bundle_name.rsplit('_').next()?.trim_end_matches(".bundle").to_string();
    if inner.len() != 32 { return None; }
    let root = PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_default())
        .join(r"Low\Unity\ProjectMoon_LimbusCompany");
    let walk = |dir: &Path| -> Option<PathBuf> {
        for outer in std::fs::read_dir(dir).ok()?.flatten() {
            let p = outer.path().join(&inner).join("__data");
            if p.is_file() { return Some(p); }
        }
        None
    };
    if let Some(p) = walk(&root) { return Some(p); }
    // D: junction fallback is already covered by the junction itself.
    None
}

async fn obtain_bundle(state: &AppState, name: &str, token: &str, dest: &Path) -> Result<PathBuf> {
    if let Some(p) = locate_bundle(name) { return Ok(p); }
    let url = format!("https://download.limbuscompanycdn.org/{}/{}", token, name);
    let client = util::http_client(state.hub.proxy_url().as_deref())?;
    let handle = state.handle.lock().ok().and_then(|h| h.clone());
    let u = url.clone();
    util::download_file(&client, &url, dest, None, move |done, total| {
        if let Some(h) = &handle {
            let _ = h.emit(EV_SYNC_PROGRESS, serde_json::json!({
                "phase": "staticdata-download", "current": done, "total": total, "message": u,
            }));
        }
    }).await?;
    Ok(dest.to_path_buf())
}

fn catalog_path(state: &AppState, override_path: Option<&str>) -> PathBuf {
    if let Some(p) = override_path.filter(|s| !s.is_empty()) {
        return crate::core::paths::expand_path(p, &state.hub.root);
    }
    if let Some(p) = state.hub.config.catalog_path.as_deref().filter(|s| !s.is_empty()) {
        return crate::core::paths::expand_path(p, &state.hub.root);
    }
    PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_default())
        .join(r"Low\ProjectMoon\LimbusCompany\com.unity.addressables\catalog_S1.bin")
}

pub async fn sync(state: &AppState, source_path: Option<&str>) -> Result<SyncReport> {
    let mut report = SyncReport { category: "staticdata".into(), ..Default::default() };
    let cat_path = catalog_path(state, source_path);
    if !cat_path.is_file() { return Err(Error::msg(format!("catalog not found: {}", cat_path.display()))); }
    let hits = catalog::find(&state.hub, "static_s1_0", Some(&cat_path.to_string_lossy()))?;
    let hit = hits.iter().find(|h| h.bundle_name.as_deref().map(|b: &str| b.contains("static_s1_0")).unwrap_or(false))
        .ok_or_else(|| Error::msg("static_s1_0 bundle not found in catalog"))?;
    // ABRO.bundle_name is the bare 32-hex cache-directory name; the real
    // `<name>_<32hex>.bundle` lives in the catalog key / internalId.
    let bundle_name = hit.key.clone();
    if !bundle_name.ends_with(".bundle") {
        return Err(Error::msg(format!("unexpected static bundle key: {}", bundle_name)));
    }
    let version = bundle_name.rsplit('_').next().unwrap_or("unknown").trim_end_matches(".bundle").to_string();
    {
        let p = state.persisted.read().await;
        if !source_path.map(|_| false).unwrap_or(false)
            && p.resources.categories.get("staticdata").and_then(|c| c.version.as_ref()) == Some(&version) {
            report.version = version; report.skipped = 1; return Ok(report);
        }
    }
    let settings = state.hub.config.settings_path.as_deref()
        .map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Program Files (x86)\Steam\steamapps\common\Limbus Company\LimbusCompany_Data\StreamingAssets\aa\settings.json"));
    let token = catalog::extract_stoken(&settings).unwrap_or_default();
    let staging = state.hub.resources_dir().join("staging").join(format!("staticdata-{}", version));
    let extracted = staging.join("_unityfs");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&extracted)?;
    let bundle_file = staging.join(&bundle_name);
    let bundle_path = obtain_bundle(state, &bundle_name, &token, &bundle_file).await?;
    let raw_bytes = std::fs::read(&bundle_path)?;
    if !unityfs::is_unity_bundle(&raw_bytes) { return Err(Error::msg("not a Unity bundle")); }
    let paths = unityfs::extract_unity_assets(&raw_bytes, &extracted)
        .map_err(|e| Error::msg(format!("unityfs extraction failed: {}", e)))?;
    let out_root = staging.join("root");
    let mut files = std::collections::BTreeMap::new();
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
        report.added += 1;
    }
    if report.added < 100 { return Err(Error::msg("suspiciously few static data files extracted")); }
    // Atomic swap: staging -> official/<version> -> current junction.
    let official = state.hub.resources_dir().join("official").join(&version);
    if official.exists() { std::fs::remove_dir_all(&official)?; }
    std::fs::create_dir_all(official.parent().unwrap())?;
    std::fs::rename(&out_root, &official)?;
    util::create_junction(&official, &state.hub.resources_current())?;
    let _ = std::fs::remove_dir_all(&staging);
    {
        let mut p = state.persisted.write().await;
        p.resources.current = version.clone();
        p.resources.categories.insert("staticdata".into(), CategoryState {
            version: Some(version.clone()), etag: None, updated_at: Some(util::now_iso()),
            files, last_error: None,
        });
    }
    state.persist().await?;
    report.version = version;
    report.needs_restart = true;
    state.log("info", "resources", &format!("staticdata synced: {} files", report.added));
    Ok(report)
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
        std::fs::copy(p, target)?;
        count += 1;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unityfs_static_bundle_if_present() {
        let p = std::env::var("PL_TEST_BUNDLE").unwrap_or_default();
        if p.is_empty() || !Path::new(&p).is_file() { return; }
        let out = std::env::temp_dir().join(format!("pl-unityfs-test-{}", uuid::Uuid::new_v4().simple()));
        let n = extract_bundle(Path::new(&p), &out).unwrap();
        assert_eq!(n, 2185, "expected 2185 TextAssets (UnityPy parity)");
        // extract_bundle always writes `<stem>.json`; the 42 `.txt` outputs of
        // unityfs are normalised here by construction.
        let _ = std::fs::remove_dir_all(&out);
    }
}
