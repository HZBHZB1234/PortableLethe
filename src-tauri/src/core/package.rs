//! Client package export: prebuilt launcher + BepInEx baseline + configs +
//! modfiles. Never includes game binaries, Addressables bundles or ZylarNex.
use std::path::{Path, PathBuf};
use crate::{core::{paths::expand_path, util, zipfile}, error::{Error, Result}, model::*, state::AppState};

fn is_banned(rel: &str) -> bool {
    let s = rel.replace('\\', "/").to_lowercase();
    s.starts_with("limbuscompany_data/") || s.contains("zylarnex")
        || s.ends_with(".bundle") || s.contains("gameassembly.dll")
        || s.contains("unityplayer.dll") || s.contains("steam_api")
}

pub async fn export(state: &AppState, instance_id: &str, output: Option<&str>) -> Result<ClientPkgResult> {
    let rec = {
        let p = state.persisted.read().await;
        p.instances.iter().find(|i| i.id == instance_id).cloned()
            .ok_or_else(|| Error::msg("unknown instance"))?
    };
    let staging = state.hub.root.join("clientpkg").join(format!(".staging-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&staging)?;
    let mut count = 0usize;
    // BepInEx baseline (prebuilt profile) + launcher.
    for key in ["launcher_source", "bepinex_source"] {
        let src = match key {
            "launcher_source" => state.hub.config.launcher_source.as_deref(),
            _ => state.hub.config.bepinex_source.as_deref(),
        };
        if let Some(s) = src.filter(|s| !s.is_empty()) {
            let dir = expand_path(s, &state.hub.root);
            if dir.is_dir() {
                for item in walkdir::WalkDir::new(&dir) {
                    let item = item?;
                    if !item.file_type().is_file() { continue; }
                    let rel = item.path().strip_prefix(&dir)?.to_string_lossy().replace('\\', "/");
                    if is_banned(&rel) { continue; }
                    let dst = staging.join(&rel);
                    if let Some(parent) = dst.parent() { std::fs::create_dir_all(parent)?; }
                    std::fs::copy(item.path(), &dst)?;
                    count += 1;
                }
            }
        }
    }
    // Modfiles from the instance's effective MOD_FILES_DIR.
    let moddir = state.hub.instance_dir(instance_id).join("modfiles");
    std::fs::create_dir_all(staging.join("BepInEx").join("plugins"))?;
    if moddir.is_dir() {
        for item in std::fs::read_dir(&moddir)?.flatten() {
            if item.path().is_file() {
                let name = item.file_name();
                let name = name.to_string_lossy();
                if is_banned(&name) { continue; }
                std::fs::copy(item.path(), staging.join("BepInEx").join("plugins").join(name.as_ref()))?;
                count += 1;
            }
        }
    }
    // Lethe.cfg with the target server address.
    std::fs::create_dir_all(staging.join("BepInEx").join("config"))?;
    let cfg = format!("[Lethe]\nServerRoot = http://127.0.0.1:{}\nIgnoreLauncherToken = true\nCachePath = ./BepInEx/cache\n", rec.port);
    std::fs::write(staging.join("BepInEx").join("config").join("Lethe.cfg"), cfg)?;
    let ini = format!("[lethe]\nServerUrl=http://127.0.0.1:{}\nProfileId={}\n", rec.port, rec.id);
    std::fs::write(staging.join("lethe-launcher.ini"), ini)?;
    let readme = "Portable Lethe client package\n\n1. Unzip into a clean Limbus Company game directory.\n2. Launch via PortableLetheLauncher (LimbusCompany.exe).\n3. The launcher reads lethe-launcher.ini for the server URL/profile.\n\nThis package contains no game binaries, Addressables bundles or ZylarNex.\n";
    std::fs::write(staging.join("README.txt"), readme)?;
    count += 3;

    let bytes = zipfile::zip_dir(&staging, "")?;
    let out = output.filter(|s| !s.is_empty()).map(PathBuf::from)
        .unwrap_or_else(|| state.hub.clientpkg_dir().join(format!("PortableLetheClient-{}.zip", rec.version.unwrap_or_else(|| "latest".into()))));
    if let Some(parent) = out.parent() { std::fs::create_dir_all(parent)?; }
    util::atomic_write(&out, &bytes, false)?;
    let _ = std::fs::remove_dir_all(&staging);
    let sha = util::sha256_bytes(&bytes);
    state.log("info", "package", &format!("exported {} ({} bytes)", out.display(), bytes.len()));
    Ok(ClientPkgResult { path: out.to_string_lossy().into_owned(), sha256: sha, size_bytes: bytes.len() as u64, file_count: count })
}

#[allow(dead_code)]
fn _p(_: &Path) {}
