//! Official-site modfiles mirror. ETag-based change detection, SHA256 integrity,
//! staging + atomic swap; copied into each instance's MOD_FILES_DIR for instant effect.
use std::collections::BTreeMap;
use tauri::Emitter;
use crate::{core::util, error::{Error, Result}, model::*, state::{AppState, EV_SYNC_PROGRESS}};

pub const FILES: &[(&str, &str)] = &[
    ("Lethe.dll", "https://api.lethelc.site/Lethe.dll"),
    ("ModularSkillScripts.dll", "https://api.lethelc.site/ModularSkillScripts.dll"),
    ("motions.dll", "https://api.lethelc.site/motions.dll"),
    ("limbus-manifest.txt", "https://files.lethelc.site/limbus-manifest.txt"),
    ("noticeMeta.json", "https://files.lethelc.site/noticeMeta.json"),
    ("news.json", "https://files.lethelc.site/news.json"),
    ("header-password", "https://files.lethelc.site/header-password"),
];

pub async fn sync(state: &AppState, force: bool) -> Result<SyncReport> {
    let client = util::http_client(state.hub.proxy_url().as_deref())?;
    let old = state.persisted.read().await.resources.categories.get("modfiles").cloned().unwrap_or_default();
    let staging = state.hub.resources_dir().join("staging").join("modfiles");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging)?;
    let mut files = BTreeMap::new();
    let mut etags = BTreeMap::new();
    let mut report = SyncReport { category: "modfiles".into(), ..Default::default() };
    let handle = state.handle.lock().ok().and_then(|h| h.clone());
    for (i, (name, url)) in FILES.iter().enumerate() {
        if let Some(h) = &handle {
            let _ = h.emit(EV_SYNC_PROGRESS, serde_json::json!({
                "phase": "modfiles", "current": i + 1, "total": FILES.len(), "message": name,
            }));
        }
        let cached_etag = old.etag.as_ref().and_then(|all| serde_json::from_str::<BTreeMap<String, String>>(all).ok())
            .and_then(|m| m.get(*name).cloned());
        let fetched = util::fetch(&client, url, if force { None } else { cached_etag.as_deref() }).await
            .map_err(|e| Error::msg(format!("{}: {}", url, e)))?;
        if fetched.not_modified {
            if let Some(h) = fetched.etag { etags.insert((*name).to_string(), h); }
            if let Some(hash) = old.files.get(*name) { files.insert((*name).to_string(), hash.clone()); }
            report.skipped += 1;
            continue;
        }
        let hash = util::sha256_bytes(&fetched.bytes);
        if let Some(prev) = old.files.get(*name) {
            if prev == &hash { report.skipped += 1; } else { report.changed += 1; }
        } else { report.added += 1; }
        std::fs::write(staging.join(name), &fetched.bytes)?;
        files.insert((*name).to_string(), hash);
        if let Some(e) = fetched.etag { etags.insert((*name).to_string(), e); }
    }
    let version = chrono::Local::now().format("%Y%m%d%H%M%S").to_string();
    let object_dir = state.hub.resources_objects().join("modfiles").join(&version);
    std::fs::create_dir_all(object_dir.parent().unwrap())?;
    let official = object_dir.join("modfiles");
    std::fs::rename(&staging, &official)?;
    // Only this category's junction changes; staticdata is unaffected.
    super::set_category(state, "modfiles", &official)?;
    // Immediate effect: copy into every instance's MOD_FILES_DIR.
    let ids: Vec<String> = state.persisted.read().await.instances.iter().map(|i| i.id.clone()).collect();
    for id in ids {
        let dst = state.hub.instance_dir(&id).join("modfiles");
        std::fs::create_dir_all(&dst)?;
        for (name, _) in FILES {
            let src = official.join(name);
            if src.is_file() { std::fs::copy(&src, dst.join(name))?; }
        }
    }
    {
        let mut p = state.persisted.write().await;
        p.resources.categories.insert("modfiles".into(), CategoryState {
            source: Some("official".into()), version: Some(version.clone()),
            etag: Some(serde_json::to_string(&etags).unwrap_or_default()),
            updated_at: Some(util::now_iso()), files, last_error: None,
        });
    }
    state.persist().await?;
    report.version = version;
    state.log("info", "resources", &format!("modfiles synced: +{} ~{} ={}", report.added, report.changed, report.skipped));
    Ok(report)
}
