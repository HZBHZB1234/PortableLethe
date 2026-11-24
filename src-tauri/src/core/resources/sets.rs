//! Selectable data-resource sets. Each set is a small real directory whose
//! children are junctions into a versioned object (or straight into the local
//! game installation). An instance binds to a set with one more junction, so
//! switching data versions is O(1) and never duplicates large files.
use std::path::{Path, PathBuf};
use crate::{core::{paths::expand_path, resources::{self, game}, util}, error::{Error, Result}, model::{ResourceSet, SyncReport}, state::AppState};

const KINDS: &[&str] = &["release", "official", "lethe", "game", "local"];

pub fn list(state: &AppState) -> Vec<ResourceSet> {
    let p = match state.persisted.try_read() { Ok(p) => p, Err(_) => return Vec::new() };
    let mut out = p.resource_sets.clone();
    for s in &mut out { s.is_default = p.default_resource_set.as_deref() == Some(s.id.as_str()); }
    out
}

pub fn find(state: &AppState, id: &str) -> Option<ResourceSet> {
    list(state).into_iter().find(|s| s.id == id)
}

pub fn root_for(state: &AppState, id: Option<&str>) -> Result<PathBuf> {
    let p = state.persisted.try_read().map_err(|e| Error::msg(e.to_string()))?;
    let chosen = id.filter(|s| !s.is_empty()).or(p.default_resource_set.as_deref())
        .ok_or_else(|| Error::msg("no data resource set selected"))?;
    let set = p.resource_sets.iter().find(|s| s.id == chosen)
        .ok_or_else(|| Error::msg(format!("unknown resource set: {}", chosen)))?;
    Ok(PathBuf::from(&set.path))
}

fn newest_modfiles(state: &AppState) -> Option<PathBuf> {
    let root = state.hub.resources_objects().join("modfiles");
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&root).ok().into_iter().flatten()
        .filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()).collect();
    dirs.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
    dirs.last().map(|p| p.join("modfiles")).filter(|p| p.is_dir())
        .or_else(|| {
            let p = state.hub.resources_active().join("modfiles");
            if p.is_dir() { Some(p) } else { None }
        })
}

fn link_optional(target: &Path, link: &Path, extra: &mut std::collections::BTreeMap<String, String>, label: &str) {
    if !target.is_dir() { return; }
    if util::create_junction(target, link).is_ok() {
        extra.insert(label.to_string(), target.to_string_lossy().into_owned());
    }
}

async fn publish(state: &AppState, set: &ResourceSet) -> Result<()> {
    {
        let mut p = state.persisted.write().await;
        p.resource_sets.retain(|s| s.id != set.id);
        p.resource_sets.push(set.clone());
        if p.default_resource_set.is_none() { p.default_resource_set = Some(set.id.clone()); }
    }
    state.persist().await
}

/// Create a resource set from any supported source and return its DTO.
pub async fn create(state: &AppState, name: &str, source: &str, source_path: Option<&str>) -> Result<ResourceSet> {
    if !KINDS.contains(&source) { return Err(Error::msg(format!("unknown resource source: {}", source))); }
    // Local sources already exist on disk; remote channels need syncing. The
    // game source exposes the install via junctions but still needs the JSON
    // tables, so it bootstraps from the bundled release snapshot.
    let direct = source == "local" || source == "game";
    if !direct || source == "game" {
        let sync_src = if source == "game" { "release" } else { source };
        resources::sync(state, "staticdata", sync_src, source_path, false).await?;
    }

    let (effective_source, version) = if direct {
        (source.to_string(), "local".to_string())
    } else {
        let p = state.persisted.read().await;
        let cat = p.resources.categories.get("staticdata");
        (source.to_string(), cat.and_then(|c| c.version.clone()).unwrap_or_else(|| "local".into()))
    };
    let object = if source == "game" {
        let install = game::detect(source_path, &state.hub.root);
        if !install.found { return Err(Error::msg("local game installation not found")); }
        PathBuf::from(install.game_dir.unwrap())
    } else if source == "local" {
        let raw = source_path.ok_or_else(|| Error::msg("local resource source needs a directory"))?;
        expand_path(raw, &state.hub.root)
    } else {
        state.hub.resources_objects().join("staticdata").join(&effective_source).join(&version)
    };
    // For a raw game install the static tables live in the Unity cache bundle,
    // not as loose StaticData/Localize folders. Fall back to the synced release
    // object for those two categories, and expose the game dir via `extra`.
    let static_dir = if object.join("StaticData").is_dir() {
        object.join("StaticData")
    } else {
        state.hub.resources_objects().join("staticdata").join("release")
            .join(state.persisted.try_read().ok()
                .and_then(|p| p.resources.categories.get("staticdata").and_then(|c| c.version.clone()))
                .unwrap_or_else(|| "0.1.1".into()))
            .join("StaticData")
    };
    let localize_dir = if object.join("Localize").is_dir() {
        object.join("Localize")
    } else {
        static_dir.parent().unwrap_or(&object).join("Localize")
    };
    if !static_dir.is_dir() {
        return Err(Error::msg(format!("resource source has no StaticData directory: {}", static_dir.display())));
    }
    let mod_source = newest_modfiles(state);

    let base = util::sanitize_name(name);
    let id = if find(state, &base).is_some() {
        format!("{}-{}", base, uuid::Uuid::new_v4().simple().to_string()[..6].to_string())
    } else { base };
    let root = state.hub.resource_set_dir(&id);
    if root.exists() { std::fs::remove_dir_all(&root)?; }
    std::fs::create_dir_all(&root)?;
    util::create_junction(&static_dir, &root.join("StaticData"))?;
    if localize_dir.is_dir() { util::create_junction(&localize_dir, &root.join("Localize"))?; }
    if let Some(mods) = &mod_source { util::create_junction(mods, &root.join("modfiles"))?; }

    let mut extra = std::collections::BTreeMap::new();
    let mut source_label = effective_source.clone();
    if source == "game" {
        let install = game::detect(source_path, &state.hub.root);
        if !install.found { return Err(Error::msg("local game installation not found")); }
        if let Some(d) = install.game_dir.as_deref() { source_label = format!("本机游戏 · {}", d); }
        link_optional(Path::new(install.cache_dir.as_deref().unwrap_or("")), &root.join("Bundles"), &mut extra, "Bundles");
        link_optional(Path::new(install.lang_dir.as_deref().unwrap_or("")), &root.join("Lang"), &mut extra, "Lang");
        link_optional(Path::new(install.streaming_dir.as_deref().unwrap_or("")), &root.join("StreamingAssets"), &mut extra, "StreamingAssets");
        link_optional(Path::new(install.game_dir.as_deref().unwrap_or("")), &root.join("Game"), &mut extra, "Game");
        link_optional(Path::new(install.save_dir.as_deref().unwrap_or("")), &root.join("Save"), &mut extra, "Save");
    } else if let Some(p) = source_path.filter(|s| !s.is_empty()) {
        source_label = format!("{} · {}", effective_source, p);
    }

    let (size_bytes, file_count) = util::dir_stats_follow(&static_dir);
    let set = ResourceSet {
        id: id.clone(), name: name.to_string(), kind: source.to_string(), source: source_label,
        version, path: root.to_string_lossy().into_owned(),
        static_dir: root.join("StaticData").to_string_lossy().into_owned(),
        localize_dir: root.join("Localize").to_string_lossy().into_owned(),
        modfiles_dir: root.join("modfiles").to_string_lossy().into_owned(),
        extra, created_at: util::now_iso(), size_bytes, file_count, is_default: false,
    };
    publish(state, &set).await?;
    state.log("info", "resources", &format!("created resource set {} ({}/{}) with {} junctions",
        name, source, set.version, set.extra.len() + 3));
    Ok(set)
}

pub async fn delete(state: &AppState, id: &str) -> Result<bool> {
    let used = {
        let p = state.persisted.read().await;
        p.instances.iter().any(|i| i.resource_set.as_deref() == Some(id))
    };
    if used { return Err(Error::msg("该资源集正被实例使用，请先切换实例的数据资源")); }
    let set = find(state, id).ok_or_else(|| Error::msg("resource set not found"))?;
    let root = PathBuf::from(&set.path);
    if root.exists() { std::fs::remove_dir_all(&root)?; }
    {
        let mut p = state.persisted.write().await;
        p.resource_sets.retain(|s| s.id != id);
        if p.default_resource_set.as_deref() == Some(id) {
            p.default_resource_set = p.resource_sets.first().map(|s| s.id.clone());
        }
    }
    state.persist().await?;
    Ok(true)
}

pub async fn set_default(state: &AppState, id: &str) -> Result<bool> {
    if find(state, id).is_none() { return Err(Error::msg("resource set not found")); }
    {
        let mut p = state.persisted.write().await;
        p.default_resource_set = Some(id.to_string());
    }
    state.persist().await?;
    Ok(true)
}

/// Register the pre-existing global `resources/current` tree as the first set.
/// This migrates state written by older Hub builds without moving any files.
pub async fn ensure_legacy(state: &AppState) -> Result<()> {
    if !list(state).is_empty() { return Ok(()); }
    let root = state.hub.resources_current();
    if !root.join("StaticData").is_dir() { return Ok(()); }
    let (size, files) = util::dir_stats_follow(&root);
    let set = ResourceSet {
        id: "default".into(), name: "默认资源集".into(), kind: "local".into(),
        source: "内置资源（升级前布局）".into(), version: "legacy".into(),
        path: root.to_string_lossy().into_owned(),
        static_dir: root.join("StaticData").to_string_lossy().into_owned(),
        localize_dir: root.join("Localize").to_string_lossy().into_owned(),
        modfiles_dir: root.join("modfiles").to_string_lossy().into_owned(),
        extra: Default::default(), created_at: util::now_iso(),
        size_bytes: size, file_count: files, is_default: true,
    };
    {
        let mut p = state.persisted.write().await;
        for rec in &mut p.instances {
            if rec.resource_set.is_none() { rec.resource_set = Some("default".into()); }
        }
    }
    publish(state, &set).await
}

#[allow(dead_code)]
fn _report_type(_: &SyncReport) {}
