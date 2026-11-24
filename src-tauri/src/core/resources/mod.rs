pub mod catalog;
pub mod staticdata;
pub mod modfiles;
pub mod bundles;
pub mod game;
pub mod sets;

use std::path::Path;
use crate::{core::util, error::{Error, Result}, model::SyncReport, state::AppState};

/// Ensure `resources/current` is a junction to the stable `active` directory,
/// with one junction per resource category. Migrates the old layout where
/// `current` pointed directly at a single version tree.
pub fn ensure_layout(state: &AppState) -> Result<()> {
    let active = state.hub.resources_active();
    std::fs::create_dir_all(&active)?;
    std::fs::create_dir_all(state.hub.resources_objects())?;
    let current = state.hub.resources_current();

    if util::is_junction(&current) {
        if let Ok(old) = std::fs::canonicalize(&current) {
            for name in ["StaticData", "Localize", "modfiles"] {
                let src = old.join(name);
                let link = active.join(name);
                if src.is_dir() && link.symlink_metadata().is_err() {
                    util::create_junction(&src, &link)?;
                }
            }
        }
        util::remove_link(&current)?;
    }
    if !active.is_dir() { std::fs::create_dir_all(&active)?; }
    util::create_junction(&active, &current)?;

    // Recover missing category junctions from the newest object directory.
    for name in ["StaticData", "Localize", "modfiles"] {
        let link = active.join(name);
        if link.symlink_metadata().is_ok() { continue; }
        let root = if name == "modfiles" {
            state.hub.resources_objects().join("modfiles")
        } else {
            state.hub.resources_objects().join("staticdata")
        };
        let mut candidates: Vec<_> = std::fs::read_dir(&root).ok().into_iter().flatten()
            .filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()).collect();
        candidates.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
        if let Some(latest) = candidates.last() {
            let src = if name == "modfiles" { latest.join("modfiles") } else { latest.join(name) };
            if src.is_dir() { util::create_junction(&src, &link)?; }
        }
    }
    Ok(())
}

/// Point one category in `active` at a new object directory.
pub fn set_category(state: &AppState, name: &str, source_dir: &Path) -> Result<()> {
    if !["StaticData", "Localize", "modfiles"].contains(&name) {
        return Err(Error::msg(format!("unknown resource category: {}", name)));
    }
    util::create_junction(source_dir, &state.hub.resources_active().join(name))
}

pub async fn sync(state: &AppState, category: &str, source: &str,
                  source_path: Option<&str>, force: bool) -> Result<SyncReport> {
    ensure_layout(state)?;
    match category {
        "all" => {
            let mut mods = modfiles::sync(state, force).await?;
            let stat = staticdata::sync(state, source, source_path).await?;
            mods.needs_restart |= stat.needs_restart;
            mods.version = stat.version;
            Ok(mods)
        },
        "modfiles" => modfiles::sync(state, force).await,
        "staticdata" => staticdata::sync(state, source, source_path).await,
        "bundles" => bundles::sync(state, source_path).await,
        other => Err(Error::msg(format!("unknown resource category: {}", other))),
    }
}
