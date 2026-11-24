//! Detect a locally installed Limbus Company game and expose its resource
//! directories as a data-resource source. Nothing is copied here: the heavy
//! directories are joined into resource sets by `sets.rs`, so two copies of a
//! 40+ GB game never exist on the same disk.
use std::path::{Path, PathBuf};
use crate::{core::paths::expand_path, model::GameInstall};

fn existing_dir(p: PathBuf) -> Option<PathBuf> { if p.is_dir() { Some(p) } else { None } }
fn existing_file(p: PathBuf) -> Option<PathBuf> { if p.is_file() { Some(p) } else { None } }

#[cfg(windows)]
fn registry_steam_path() -> Option<PathBuf> {
    use std::os::windows::process::CommandExt;
    const NO_WINDOW: u32 = 0x0800_0000;
    for (hive, key) in [
        ("HKCU", r"Software\Valve\Steam"),
        ("HKLM", r"SOFTWARE\WOW6432Node\Valve\Steam"),
        ("HKLM", r"SOFTWARE\Valve\Steam"),
    ] {
        let out = std::process::Command::new("reg")
            .args(["query", &format!("{}\\{}", hive, key), "/v", "SteamPath"])
            .creation_flags(NO_WINDOW)
            .output().ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            if let Some(i) = line.find("REG_SZ") {
                let raw = line[i + 6..].trim().trim_matches('"');
                if !raw.is_empty() { return Some(PathBuf::from(raw)); }
            }
        }
    }
    None
}

#[cfg(not(windows))]
fn registry_steam_path() -> Option<PathBuf> { None }

fn steam_libraries(steam: &Path) -> Vec<PathBuf> {
    let mut out = vec![steam.to_path_buf()];
    let vdf = steam.join("steamapps").join("libraryfolders.vdf");
    let Ok(text) = std::fs::read_to_string(vdf) else { return out; };
    for line in text.lines() {
        let line = line.trim();
        if !line.starts_with("\"path\"") { continue; }
        let Some(start) = line.find('"') else { continue; };
        let rest = &line[start + 1..];
        let Some(end) = rest.find('"') else { continue; };
        let p = PathBuf::from(&rest[..end]);
        if p.is_dir() { out.push(p); }
    }
    out
}

fn game_candidates(override_dir: Option<&str>, hub_root: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    if let Some(s) = override_dir.filter(|s| !s.trim().is_empty()) {
        out.push(expand_path(s, hub_root));
    }
    if let Ok(s) = std::env::var("LIMBUS_GAME_DIR") {
        if !s.trim().is_empty() { out.push(PathBuf::from(s)); }
    }
    let mut steam_roots: Vec<PathBuf> = Vec::new();
    if let Some(p) = registry_steam_path() { steam_roots.push(p); }
    for p in [
        r"C:\Program Files (x86)\Steam",
        r"C:\Program Files\Steam",
        r"D:\Steam",
        r"E:\Steam",
    ] { steam_roots.push(PathBuf::from(p)); }
    for steam in &steam_roots {
        for lib in steam_libraries(steam) {
            out.push(lib.join("steamapps").join("common").join("Limbus Company"));
        }
    }
    // Epic/standalone style installs sometimes live directly under a drive root.
    for p in [
        r"C:\Limbus Company",
        r"D:\Limbus Company",
        r"E:\Limbus Company",
    ] { out.push(PathBuf::from(p)); }
    out.dedup();
    out
}

/// Scan for a usable local game. A directory counts as a game only when
/// `LimbusCompany_Data` exists, so a stale Steam entry cannot produce a source
/// card that fails on first use.
pub fn detect(override_dir: Option<&str>, hub_root: &Path) -> GameInstall {
    let game_dir = game_candidates(override_dir, hub_root).into_iter()
        .filter_map(existing_dir)
        .find(|p| p.join("LimbusCompany_Data").is_dir());
    let mut result = GameInstall::default();
    let Some(game_dir) = game_dir else { return result; };
    let data_dir = game_dir.join("LimbusCompany_Data");
    let streaming_dir = data_dir.join("StreamingAssets").join("aa");
    let save_dir = PathBuf::from(std::env::var("USERPROFILE").unwrap_or_default())
        .join(r"AppData\LocalLow\ProjectMoon\LimbusCompany");
    let cache_dir = PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_default())
        .join(r"Low\Unity\ProjectMoon_LimbusCompany");
    let catalog = save_dir.join("com.unity.addressables").join("catalog_S1.bin");
    let settings = streaming_dir.join("settings.json");
    let lang_dir = data_dir.join("lang");

    result.found = true;
    result.game_dir = Some(game_dir.to_string_lossy().into_owned());
    result.data_dir = Some(data_dir.to_string_lossy().into_owned());
    result.cache_dir = existing_dir(cache_dir).map(|p| p.to_string_lossy().into_owned());
    result.save_dir = existing_dir(save_dir).map(|p| p.to_string_lossy().into_owned());
    result.catalog = existing_file(catalog).map(|p| p.to_string_lossy().into_owned());
    result.settings = existing_file(settings).map(|p| p.to_string_lossy().into_owned());
    result.lang_dir = existing_dir(lang_dir).map(|p| p.to_string_lossy().into_owned());
    result.streaming_dir = existing_dir(streaming_dir).map(|p| p.to_string_lossy().into_owned());
    // A stable, readable proxy for "which game build is installed".
    result.version = result.catalog.as_ref().and_then(|p| std::fs::metadata(p).ok())
        .and_then(|m| m.modified().ok())
        .map(|t| {
            let dt: chrono::DateTime<chrono::Local> = t.into();
            format!("catalog {}", dt.format("%Y-%m-%d"))
        });
    result
}
