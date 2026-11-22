//! Addressables catalog v2 reader (pure Rust port of tools/parse_catalog.py).
//! Only the key -> location -> bundle chain is needed; --chains recursion is
//! implemented with a depth cap. Never grep catalog.bin for plain text: long
//! keys are stored as reversed dynamic-string segment lists.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use serde::Serialize;
use crate::{core::{paths::expand_path, Hub}, error::{Error, Result}};

const MASK_DYN: u32 = 0x4000_0000;
const MASK_UNI: u32 = 0x8000_0000;
const MASK_OFF: u32 = 0x3FFF_FFFF;
const NONE: u32 = 0xFFFF_FFFF;

#[derive(Debug, Clone, Serialize)]
pub struct CatalogHit {
    pub key: String,
    pub internal_id: Option<String>,
    pub bundle_name: Option<String>,
    pub hash: Option<String>,
}

#[derive(Debug, Clone)]
enum Decoded { Str(String), Hash(String), Abro { hash: String, bundle_name: String }, Other }

pub struct Catalog {
    data: Vec<u8>,
    version: i32,
    keys_off: usize,
    cache: HashMap<(u32, Option<char>), Option<String>>,
}

impl Catalog {
    pub fn open(path: &Path) -> Result<Self> {
        let data = std::fs::read(path)?;
        if data.get(0..4) != Some(&[0x42, 0x89, 0xe3, 0x0d]) {
            return Err(Error::msg("bad catalog magic (expect 0x0DE38942)"));
        }
        let version = Self::i32(&data, 4)?;
        if !(1..=3).contains(&version) { return Err(Error::msg(format!("unsupported catalog version {}", version))); }
        let keys_off = Self::u32(&data, 8)? as usize;
        Ok(Self { data, version, keys_off, cache: HashMap::new() })
    }

    fn u32(d: &[u8], p: usize) -> Result<u32> {
        d.get(p..p + 4).ok_or_else(|| Error::msg("catalog read out of bounds"))
            .map(|s| u32::from_le_bytes(s.try_into().unwrap()))
    }
    fn i32(d: &[u8], p: usize) -> Result<i32> { Ok(Self::u32(d, p)? as i32) }

    fn basic_string(&self, off: u32, unicode: bool) -> Result<String> {
        let p = off as usize;
        if p == 0 { return Ok(String::new()); }
        let len = Self::i32(&self.data, p.wrapping_sub(4))?;
        if len < 0 { return Err(Error::msg("negative string length")); }
        let raw = self.data.get(p..p + len as usize).ok_or_else(|| Error::msg("string out of bounds"))?;
        if unicode {
            let units: Vec<u16> = raw.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
            Ok(String::from_utf16_lossy(&units))
        } else {
            Ok(raw.iter().map(|b| *b as char).collect()) // latin-1
        }
    }

    fn dynamic_string(&mut self, off: u32, sep: char) -> Result<String> {
        let mut parts = Vec::new();
        let mut pos = off as usize;
        for _ in 0..100_000 {
            let part_off = Self::u32(&self.data, pos)?;
            let next = Self::u32(&self.data, pos + 4)?;
            parts.push(self.encoded_string(part_off, None)?.unwrap_or_default());
            if next == NONE { break; }
            pos = next as usize;
        }
        if parts.len() == 1 { return Ok(parts.remove(0)); }
        if self.version > 1 { parts.reverse(); }
        Ok(parts.join(&sep.to_string()))
    }

    fn encoded_string(&mut self, enc: u32, sep: Option<char>) -> Result<Option<String>> {
        if enc == NONE { return Ok(None); }
        if let Some(v) = self.cache.get(&(enc, sep)) { return Ok(v.clone()); }
        let unicode = enc & MASK_UNI != 0;
        let dynamic = enc & MASK_DYN != 0 && sep.is_some();
        let off = enc & MASK_OFF;
        let value = if dynamic {
            Some(self.dynamic_string(off, sep.unwrap())?)
        } else { Some(self.basic_string(off, unicode)?) };
        self.cache.insert((enc, sep), value.clone());
        Ok(value)
    }

    fn offset_array(&self, enc: u32) -> Result<Vec<u32>> {
        if enc == NONE { return Ok(Vec::new()); }
        let p = enc as usize;
        let len = Self::i32(&self.data, p.wrapping_sub(4))?;
        if len < 0 { return Ok(Vec::new()); }
        let n = (len as usize / 4).min(1_000_000);
        (0..n).map(|i| Self::u32(&self.data, p + 4 * i)).collect()
    }

    fn serialized_type(&mut self, off: u32) -> Result<(Option<String>, Option<String>)> {
        if off == NONE { return Ok((None, None)); }
        let p = off as usize;
        let a = Self::u32(&self.data, p)?;
        let c = Self::u32(&self.data, p + 4)?;
        Ok((self.encoded_string(a, Some('.'))?, self.encoded_string(c, Some('.'))?))
    }

    fn decode_v2(&mut self, off: u32) -> Result<Decoded> {
        if off == NONE { return Ok(Decoded::Other); }
        let p = off as usize;
        let type_off = Self::u32(&self.data, p)?;
        let obj_off = Self::u32(&self.data, p + 4)?;
        let (_, cls) = self.serialized_type(type_off)?;
        match cls.as_deref() {
            Some("System.String") => {
                if obj_off == NONE { return Ok(Decoded::Str(String::new())); }
                let q = obj_off as usize;
                let s_off = Self::u32(&self.data, q)?;
                let sep_b = *self.data.get(q + 4).unwrap_or(&0);
                let sep = if sep_b == 0 { None } else { Some(sep_b as char) };
                Ok(Decoded::Str(self.encoded_string(s_off, sep)?.unwrap_or_default()))
            },
            Some("UnityEngine.Hash128") => {
                if obj_off == NONE { return Ok(Decoded::Hash("0".repeat(32))); }
                let q = obj_off as usize;
                let mut s = String::new();
                for i in 0..4 { s.push_str(&format!("{:08x}", Self::u32(&self.data, q + 4 * i)?)); }
                Ok(Decoded::Hash(s))
            },
            Some(c) if c.ends_with("AssetBundleRequestOptions") => {
                if obj_off == NONE { return Ok(Decoded::Other); }
                self.read_abro(obj_off).map_err(|_| Error::msg("bad ABRO"))
            },
            _ => Ok(Decoded::Other),
        }
    }

    fn read_abro(&mut self, off: u32) -> Result<Decoded> {
        let p = off as usize;
        let hash_off = Self::u32(&self.data, p)?;
        let name_off = Self::u32(&self.data, p + 4)?;
        let mut hash = String::new();
        for i in 0..4 { hash.push_str(&format!("{:08x}", Self::u32(&self.data, hash_off as usize + 4 * i)?)); }
        let bundle_name = self.encoded_string(name_off, Some('_'))?.unwrap_or_default();
        Ok(Decoded::Abro { hash, bundle_name })
    }

    fn location(&mut self, off: u32, depth: u32) -> Result<(Option<String>, Option<String>, Option<String>, Option<String>)> {
        if depth > 5 { return Ok((None, None, None, None)); }
        let p = off as usize;
        let primary = self.encoded_string(Self::u32(&self.data, p)?, Some('/'))?;
        let internal = self.encoded_string(Self::u32(&self.data, p + 4)?, Some('/'))?;
        let data_off = Self::u32(&self.data, p + 20)?;
        let data = self.decode_v2(data_off)?;
        let (bundle, hash) = match data {
            Decoded::Abro { hash, bundle_name } => (Some(bundle_name), Some(hash)),
            _ => (None, None),
        };
        let mut bundle = bundle;
        let deps = self.offset_array(Self::u32(&self.data, p + 12)?)?;
        for dep in deps {
            let (_, _, b, _) = self.location(dep, depth + 1)?;
            if bundle.is_none() { bundle = b; }
        }
        Ok((primary, internal, bundle, hash))
    }

    pub fn find(&mut self, pattern: &str) -> Result<Vec<CatalogHit>> {
        let re = regex::RegexBuilder::new(pattern).case_insensitive(true).build().ok();
        let pairs = self.offset_array(self.keys_off as u32)?;
        let mut hits = Vec::new();
        for pair in pairs.chunks(2) {
            if pair.len() < 2 { break; }
            let key = match self.decode_v2(pair[0])? { Decoded::Str(s) => s, _ => continue };
            let matched = match &re { Some(r) => r.is_match(&key), None => key.to_lowercase().contains(&pattern.to_lowercase()) };
            if !matched { continue; }
            let mut internal = None;
            let mut bundle = None;
            let mut hash = None;
            for loc in self.offset_array(pair[1])? {
                if let Ok((_, i, b, h)) = self.location(loc, 0) {
                    if internal.is_none() { internal = i; }
                    if bundle.is_none() { bundle = b; }
                    if hash.is_none() { hash = h; }
                }
            }
            let bundle = bundle.or_else(|| internal.as_deref().and_then(bundle_name_from_internal_id));
            hits.push(CatalogHit { key, internal_id: internal, bundle_name: bundle, hash });
        }
        Ok(hits)
    }
}

pub fn bundle_name_from_internal_id(id: &str) -> Option<String> {
    let mut s = id.rsplit('/').next().unwrap_or(id).to_string();
    if let Some(rest) = s.strip_suffix(".bundle") { s = rest.to_string(); }
    // Bundle names end with `_<32hex>`; strip it so the logical name matches
    // the catalog's dynamic-string decoded ABRO name.
    if let Some((prefix, hash)) = s.rsplit_once('_') {
        if hash.len() == 32 && hash.chars().all(|c| c.is_ascii_hexdigit()) {
            s = prefix.to_string();
        }
    }
    Some(s)
}

/// Extract the CDN s-token from StreamingAssets/aa/settings.json.
pub fn extract_stoken(settings_json: &Path) -> Result<String> {
    let text = std::fs::read_to_string(settings_json)?;
    let re = regex::Regex::new(r#"download\.limbuscompanycdn\.org/([^/"']+)"#).map_err(|e| Error::msg(e.to_string()))?;
    re.captures(&text).and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
        .ok_or_else(|| Error::msg("s-token not found in settings.json"))
}

fn default_catalog_path(hub: &Hub) -> PathBuf {
    if let Some(p) = hub.config.catalog_path.as_deref().filter(|s| !s.is_empty()) {
        return expand_path(p, &hub.root);
    }
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
    PathBuf::from(local).join(r"Low\ProjectMoon\LimbusCompany\com.unity.addressables\catalog_S1.bin")
}

pub fn find(hub: &Hub, pattern: &str, catalog_path: Option<&str>) -> Result<Vec<CatalogHit>> {
    let path = catalog_path.filter(|s| !s.is_empty()).map(|s| expand_path(s, &hub.root))
        .unwrap_or_else(|| default_catalog_path(hub));
    if !path.is_file() { return Err(Error::msg(format!("catalog not found: {}", path.display()))); }
    let mut cat = Catalog::open(&path)?;
    let mut hits = cat.find(pattern)?;
    if hits.len() > 2000 { hits.truncate(2000); }
    Ok(hits)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn internal_id_basename() {
        assert_eq!(bundle_name_from_internal_id("https://cdn/x/static_s1_0_assets_all_7061ce4fb64bbc9d5eb4d8a982a71418.bundle").unwrap(),
                   "static_s1_0_assets_all");
    }
    #[test]
    fn real_catalog_if_present() {
        let p = std::env::var("PL_TEST_CATALOG").unwrap_or_default();
        if p.is_empty() || !Path::new(&p).is_file() { return; }
        let mut cat = Catalog::open(Path::new(&p)).unwrap();
        let hits = cat.find("static_s1_0").unwrap();
        for h in hits.iter().take(5) {
            println!("HIT key={} internal={:?} bundle={:?} hash={:?}", h.key, h.internal_id, h.bundle_name, h.hash);
        }
        assert!(!hits.is_empty(), "static_s1_0 must be found in catalog");
        assert!(hits.iter().any(|h| h.key.contains("static_s1_0_assets_all_")
            && h.key.ends_with(".bundle")));
        assert!(hits[0].hash.as_ref().map(|h| h.len() == 32).unwrap_or(false));
    }
}
