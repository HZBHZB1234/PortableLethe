//! Schema-driven config hosting: reads the server's language-neutral
//! config-schema.json, writes Hub-owned keys into a per-instance bootstrap
//! file atomically, protects secrets with DPAPI on Windows (doc 02 §4).
use std::path::PathBuf;
use crate::{core::{dpapi, util, Hub}, error::{Error, Result}, model::*, state::AppState};

fn field_from_json(v: &serde_json::Value) -> Option<ConfigField> {
    let key = v.get("key")?.as_str()?.to_string();
    let typ = v.get("type").and_then(|x| x.as_str())
        .or_else(|| v.get("kind").and_then(|x| x.as_str())).unwrap_or("string");
    let kind = match typ {
        "int" | "integer" | "double" | "number" => "number",
        "bool" | "boolean" => "boolean",
        "secret" | "password" => "secret",
        "path" => "path",
        _ => "string",
    };
    Some(ConfigField {
        key: key.clone(),
        label: v.get("label").and_then(|x| x.as_str()).unwrap_or(&key).to_string(),
        kind: kind.into(),
        owner: v.get("owner").and_then(|x| x.as_str()).unwrap_or("hub").into(),
        requires_restart: v.get("requiresRestart").or_else(|| v.get("requires_restart"))
            .and_then(|x| x.as_bool()).unwrap_or(true),
        secret: kind == "secret",
        default: v.get("default").cloned().unwrap_or(serde_json::Value::Null),
        description: v.get("description").and_then(|x| x.as_str()).unwrap_or("").into(),
    })
}

fn builtin_fields() -> Vec<ConfigField> {
    let rows = [
        ("ASPNETCORE_URLS", "string", "http://127.0.0.1:5180", true, "绑定地址与端口"),
        ("Auth:JwtSecret", "secret", "", true, "JWT 签名密钥（DPAPI 加密存储）"),
        ("Auth:JwtExpiresMinutes", "number", "1440", true, "JWT 有效期（分钟）"),
        ("Auth:RateLimitPerMinute", "number", "600", true, "每分钟请求限流"),
        ("MAX_REQUEST_BODY_BYTES", "number", "2097152", true, "请求体上限"),
        ("REQUEST_TIMEOUT_SECONDS", "number", "15", true, "请求超时（秒）"),
        ("FRONTEND_URL", "string", "", true, "CORS / 前端来源"),
        ("STATIC_DATA_DIR", "path", "static", true, "静态数据目录"),
        ("MOD_FILES_DIR", "path", "modfiles", true, "modfiles 目录"),
        ("ConnectionStrings:Local", "string", "Data Source=data/openlethe.db", true, "SQLite 数据库路径"),
        ("Plugins:Enabled", "boolean", "false", true, "DLL 插件开关"),
        ("Plugins:Dir", "path", "plugins", true, "DLL 插件目录"),
        ("HUB_INSTANCE_ID", "string", "", true, "实例 ID（Hub 拥有）"),
    ];
    rows.iter().map(|(key, kind, def, restart, desc)| ConfigField {
        key: (*key).into(), label: (*key).into(), kind: (*kind).into(), owner: "hub".into(),
        requires_restart: *restart, secret: *kind == "secret",
        default: if *kind == "number" { serde_json::from_str(def).unwrap_or(serde_json::Value::Null) }
                 else { serde_json::Value::String((*def).into()) },
        description: (*desc).into(),
    }).collect()
}

fn schema_path(hub: &Hub) -> Option<PathBuf> {
    let mut candidates = vec![hub.root.join("config-schema.json")];
    if let Some(parent) = hub.root.parent() {
        candidates.push(parent.join("PortableOpenLethe").join("config-schema.json"));
    }
    if let Ok(p) = std::env::var("OPENLETHE_SCHEMA") { candidates.push(PathBuf::from(p)); }
    candidates.into_iter().find(|p| p.is_file())
}

pub fn schema(hub: &Hub) -> ConfigSchema {
    if let Some(path) = schema_path(hub) {
        if let Ok(raw) = std::fs::read_to_string(&path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw) {
                let rows = json.get("keys").or_else(|| json.get("fields"))
                    .and_then(|x| x.as_array()).cloned().unwrap_or_default();
                let mut fields: Vec<ConfigField> = rows.iter().filter_map(field_from_json).collect();
                if !fields.iter().any(|f| f.key == "HUB_INSTANCE_ID") {
                    fields.push(builtin_fields().last().unwrap().clone());
                }
                if !fields.is_empty() {
                    return ConfigSchema { source: path.to_string_lossy().into_owned(), fields };
                }
            }
        }
    }
    ConfigSchema { source: "builtin-v1".into(), fields: builtin_fields() }
}

fn instance_config_path(state: &AppState, id: &str) -> PathBuf {
    state.hub.instance_dir(id).join("config.json")
}

pub fn raw_values(state: &AppState, id: &str) -> Result<JsonMap> {
    let p = instance_config_path(state, id);
    if p.exists() { util::read_json(&p) } else { Ok(JsonMap::new()) }
}

pub async fn get(state: &AppState, id: &str) -> Result<JsonMap> {
    let stored = raw_values(state, id)?;
    let mut out = JsonMap::new();
    for f in schema(&state.hub).fields {
        let raw = stored.get(&f.key).cloned().unwrap_or(f.default.clone());
        let value = if f.secret {
            match raw.as_str() {
                Some(s) => serde_json::Value::String(dpapi::mask(&dpapi::reveal(s).unwrap_or_else(|_| s.to_string()))),
                None => raw,
            }
        } else { raw };
        out.insert(f.key, value);
    }
    for (k, v) in stored { out.entry(k).or_insert(v); }
    Ok(out)
}

pub async fn set(state: &AppState, id: &str, values: &JsonMap) -> Result<JsonMap> {
    let rec = {
        let p = state.persisted.read().await;
        p.instances.iter().find(|i| i.id == id).cloned()
            .ok_or_else(|| Error::msg(format!("unknown instance {}", id)))?
    };
    let mut merged = raw_values(state, id)?;
    let secret_keys: Vec<String> = schema(&state.hub).fields.iter()
        .filter(|f| f.secret).map(|f| f.key.clone()).collect();
    for (k, v) in values {
        if secret_keys.contains(k) {
            if let Some(s) = v.as_str() {
                if dpapi::is_mask(s) { continue; } // keep the existing secret
                merged.insert(k.clone(), serde_json::Value::String(dpapi::seal(s)?));
                continue;
            }
        }
        merged.insert(k.clone(), v.clone());
    }
    merged.insert("HUB_INSTANCE_ID".into(), serde_json::Value::String(id.to_string()));
    let dir = state.hub.instance_dir(id);
    std::fs::create_dir_all(dir.join("data"))?;
    util::atomic_write_json(&instance_config_path(state, id), &merged, true)?;

    // Bootstrap file: absolute paths derived from the Hub layout, never written
    // into the server's panel-editable area. Static data resolves through this
    // instance's selected resource set (a junction root), so switching the set
    // only repoints that junction.
    let mut boot: JsonMap = merged.clone();
    boot.insert("MOD_FILES_DIR".into(), serde_json::Value::String(dir.join("modfiles").to_string_lossy().into_owned()));
    let res_root = crate::core::resources::sets::root_for(state, rec.resource_set.as_deref())
        .unwrap_or_else(|_| state.hub.resources_current());
    boot.insert("STATIC_DATA_DIR".into(), serde_json::Value::String(res_root.to_string_lossy().into_owned()));
    boot.insert("HUB_INSTANCE_ID".into(), serde_json::Value::String(id.to_string()));
    boot.insert("PORT".into(), serde_json::json!(rec.port));
    // Decrypt secrets so the server can use them.
    for k in &secret_keys {
        if let Some(serde_json::Value::String(s)) = boot.get(k).cloned() {
            boot.insert(k.clone(), serde_json::Value::String(dpapi::reveal(&s)?));
        }
    }
    for p in [dir.join("server.bootstrap.json"), dir.join("data").join("server.bootstrap.json")] {
        util::atomic_write_json(&p, &boot, true)?;
    }
    state.log("info", "config", &format!("wrote bootstrap for instance {}", id));
    get(state, id).await
}

/// Initial config for a freshly created instance.
pub fn bootstrap_defaults(state: &AppState, id: &str, port: u16) -> JsonMap {
    let mut out = JsonMap::new();
    for f in schema(&state.hub).fields {
        if f.owner == "hub" { out.insert(f.key.clone(), f.default.clone()); }
    }
    out.insert("ASPNETCORE_URLS".into(), serde_json::json!(format!("http://127.0.0.1:{}", port)));
    out.insert("HUB_INSTANCE_ID".into(), serde_json::Value::String(id.to_string()));
    out.insert("Auth:JwtSecret".into(), serde_json::Value::String(dpapi::seal(&uuid::Uuid::new_v4().simple().to_string())
        .unwrap_or_else(|_| uuid::Uuid::new_v4().simple().to_string())));
    out
}

