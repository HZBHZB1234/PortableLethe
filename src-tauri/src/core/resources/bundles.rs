//! Differential Addressables bundle sync (doc 04 §6).
//!
//! The official catalog and the private-server catalog share the vast majority
//! of bundles. Only bundles present in the official catalog but missing from
//! the private one need to be fetched into the shared Unity cache; conversely,
//! private-only bundles may remain in the cache (--keep-other semantics) so
//! switching servers never forces a re-download.
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::{
    core::{paths::expand_path, resources::catalog::{self, Catalog}, util},
    error::{Error, Result},
    model::SyncReport,
    state::AppState,
};

const OFFICIAL_CDN: &str = "https://download.limbuscompanycdn.org";
const HEADER_PASSWORD: &str = "this_is_header_value";

fn private_catalog(hub: &crate::core::Hub, source_path: Option<&str>) -> PathBuf {
    if let Some(p) = source_path.filter(|s| !s.is_empty()) { return expand_path(p, &hub.root); }
    let game = hub.config.game_dir.as_deref().unwrap_or("");
    Path::new(game).join(r"LimbusCompany_Data\StreamingAssets\aa\catalog.bin")
}

fn full_bundle_name(hit: &catalog::CatalogHit) -> Option<(String, String)> {
    let id = hit.internal_id.as_ref()?;
    let file = id.rsplit('/').next()?.to_string();
    if !file.ends_with(".bundle") { return None; }
    let inner = file.trim_end_matches(".bundle").rsplit('_').next()?.to_string();
    if inner.len() != 32 || !inner.chars().all(|c: char| c.is_ascii_hexdigit()) { return None; }
    Some((file, inner))
}

pub async fn sync(state: &AppState, source_path: Option<&str>) -> Result<SyncReport> {
    let mut report = SyncReport { category: "bundles".into(), ..Default::default() };
    let official_path = catalog::find_path(&state.hub, None)?;
    let private_path = private_catalog(&state.hub, source_path);
    if !official_path.is_file() { return Err(Error::msg(format!("official catalog missing: {}", official_path.display()))); }
    if !private_path.is_file() { return Err(Error::msg(format!("private catalog missing: {}", private_path.display()))); }

    let mut official = Catalog::open(&official_path)?;
    let mut private = Catalog::open(&private_path)?;
    let official_hits = official.find(".*")?;
    let private_hits = private.find(".*")?;
    let have: BTreeSet<String> = private_hits.iter()
        .filter_map(full_bundle_name).map(|(name, _)| name).collect();
    state.log("info", "bundles", &format!("catalogs: official={} private={} private-only kept", official_hits.len(), have.len()));

    let settings = state.hub.config.settings_path.as_deref()
        .map(|p| expand_path(p, &state.hub.root))
        .unwrap_or_else(|| Path::new(state.hub.config.game_dir.as_deref().unwrap_or(""))
            .join(r"LimbusCompany_Data\StreamingAssets\aa\settings.json"));
    let stoken = if settings.is_file() {
        catalog::extract_stoken(&settings)?
    } else {
        state.hub.config.catalog_path.as_deref()
            .and_then(|_| Some("s1".to_string()))
            .ok_or_else(|| Error::msg("settings.json with s-token not found"))?
    };

    let cache_root = std::env::var("LOCALAPPDATA").unwrap_or_default();
    let cache_root = Path::new(&cache_root)
        .join(r"Low\Unity\ProjectMoon_LimbusCompany");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .default_headers({
            let mut h = reqwest::header::HeaderMap::new();
            h.insert("X-Requested-With", reqwest::header::HeaderValue::from_static(HEADER_PASSWORD));
            h
        }).build()?;

    for hit in official_hits.iter() {
        let Some((file, inner)) = full_bundle_name(hit) else { continue };
        if have.contains(&file) { continue; }
        // Cache layout: <CACHE_ROOT>/<ABRO.bundle_name>/<32hex-from-filename>/__data.
        // ABRO.bundle_name is the outer directory, not the content hash field.
        let outer = hit.bundle_name.clone().unwrap_or_else(|| "unknown".into());
        let dest = cache_root.join(&outer).join(&inner).join("__data");
        if dest.is_file() { report.skipped += 1; continue; }
        let url = format!("{}/{}/{}", OFFICIAL_CDN, stoken, file);
        match util::download_file(&client, &url, &dest, None, |_, _| {}).await {
            Ok(_) => report.added += 1,
            Err(e) => {
                report.errors.push(format!("{}: {}", file, e));
                state.log("warn", "bundles", &format!("failed {}: {}", file, e));
            }
        }
    }
    report.needs_restart = report.added > 0;
    state.log("info", "bundles", &format!("added={} unchanged={} failed={}", report.added, report.skipped, report.errors.len()));
    Ok(report)
}

