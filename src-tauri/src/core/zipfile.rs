//! Minimal ZIP reader/writer in pure Rust (flate2), used for prebuilt release
//! extraction and client package export. The optional `zip-crate` feature can
//! replace this backend later without touching callers.
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use crate::error::{Error, Result};

fn u16le(b: &[u8], p: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(b.get(p..p+2).ok_or_else(|| Error::msg("zip truncated"))?.try_into().unwrap()))
}
fn u32le(b: &[u8], p: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(b.get(p..p+4).ok_or_else(|| Error::msg("zip truncated"))?.try_into().unwrap()))
}

fn safe_relative(name: &str) -> Result<PathBuf> {
    let n = name.replace('\\', "/");
    let mut out = PathBuf::new();
    for part in n.split('/') {
        match part {
            "" | "." => {},
            ".." => return Err(Error::msg("zip contains traversal path")),
            p => out.push(p),
        }
    }
    if out.as_os_str().is_empty() { Err(Error::msg("zip contains empty entry")) } else { Ok(out) }
}

struct Entry { name: String, method: u16, crc: u32, csize: u32, usize_: u32, offset: u32, is_dir: bool }

pub fn extract_zip(path: &Path, out_dir: &Path) -> Result<usize> {
    let data = std::fs::read(path)?;
    let eocd = (0..data.len().saturating_sub(21)).rev()
        .find(|&p| data.get(p..p+4) == Some(&[0x50, 0x4b, 0x05, 0x06]))
        .ok_or_else(|| Error::msg("zip EOCD not found"))?;
    let count = u16le(&data, eocd + 10)? as usize;
    let mut pos = u32le(&data, eocd + 16)? as usize;
    let mut entries = Vec::new();
    for _ in 0..count {
        if data.get(pos..pos+4) != Some(&[0x50, 0x4b, 0x01, 0x02]) { break; }
        let name_len = u16le(&data, pos + 28)? as usize;
        let extra_len = u16le(&data, pos + 30)? as usize;
        let comment_len = u16le(&data, pos + 32)? as usize;
        let name = String::from_utf8_lossy(data.get(pos+46..pos+46+name_len)
            .ok_or_else(|| Error::msg("zip central header truncated"))?).to_string();
        // Trailing slash is the reliable directory marker; external attributes
        // differ between zip producers (PowerShell vs. dotnet vs. Info-ZIP).
        let is_dir = name.ends_with('/') || u16le(&data, pos+38)? & 0x10 != 0;
        entries.push(Entry {
            name, method: u16le(&data, pos+10)?, crc: u32le(&data, pos+16)?,
            csize: u32le(&data, pos+20)?, usize_: u32le(&data, pos+24)?,
            offset: u32le(&data, pos+42)?, is_dir,
        });
        pos += 46 + name_len + extra_len + comment_len;
    }
    std::fs::create_dir_all(out_dir)?;
    let mut written = 0;
    for e in entries {
        let rel = safe_relative(&e.name)?;
        let target = out_dir.join(rel);
        if e.is_dir {
            std::fs::create_dir_all(&target)
                .map_err(|ioerr| Error::msg(format!("mkdir {}: {}", e.name, ioerr)))?;
            continue;
        }
        let p = e.offset as usize;
        if data.get(p..p+4) != Some(&[0x50, 0x4b, 0x03, 0x04]) { return Err(Error::msg("zip local header not found")); }
        let nlen = u16le(&data, p+26)? as usize;
        let elen = u16le(&data, p+28)? as usize;
        let start = p + 30 + nlen + elen;
        let compressed = data.get(start..start + e.csize as usize)
            .ok_or_else(|| Error::msg("zip data truncated"))?;
        let raw = match e.method {
            0 => compressed.to_vec(),
            8 => {
                let mut d = flate2::read::DeflateDecoder::new(compressed);
                let mut v = Vec::with_capacity(e.usize_ as usize);
                d.read_to_end(&mut v)?;
                v
            },
            m => return Err(Error::msg(format!("unsupported zip compression {}", m))),
        };
        if crc32fast::hash(&raw) != e.crc { return Err(Error::msg(format!("zip CRC mismatch: {}", e.name))); }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|ioerr| Error::msg(format!("mkdir parent of {}: {}", e.name, ioerr)))?;
        }
        // Archives produced by PowerShell Compress-Archive may contain duplicate
        // names (e.g. a directory record and a file, or repeated files). On
        // Windows std::fs::write then fails with AlreadyExists, aborting the
        // whole install. Replace the previous entry deterministically.
        if target.is_dir() { std::fs::remove_dir_all(&target)?; }
        else if target.exists() { std::fs::remove_file(&target)?; }
        std::fs::write(&target, &raw)
            .map_err(|ioerr| Error::msg(format!("write {}: {}", e.name, ioerr)))?;
        written += 1;
    }
    Ok(written)
}

/// Add a file entry to a ZIP writer. `sink` holds accumulated local records.
pub fn add_file(sink: &mut Vec<u8>, name: &str, data: &[u8], compress: bool) -> Result<(u32, u32, u32, u16)> {
    let name_bytes = name.as_bytes();
    let method = if compress && data.len() > 32 { 8u16 } else { 0u16 };
    let payload = if method == 8 {
        let mut enc = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(data)?;
        enc.finish()?
    } else { data.to_vec() };
    let crc = crc32fast::hash(data);
    let offset = sink.len() as u32;
    sink.extend_from_slice(&0x04034b50u32.to_le_bytes());
    sink.extend_from_slice(&20u16.to_le_bytes());   // version needed
    sink.extend_from_slice(&0u16.to_le_bytes());    // flags
    sink.extend_from_slice(&method.to_le_bytes());
    sink.extend_from_slice(&0u16.to_le_bytes());    // time
    sink.extend_from_slice(&0u16.to_le_bytes());    // date
    sink.extend_from_slice(&crc.to_le_bytes());
    sink.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    sink.extend_from_slice(&(data.len() as u32).to_le_bytes());
    sink.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
    sink.extend_from_slice(&0u16.to_le_bytes());
    sink.extend_from_slice(name_bytes);
    sink.extend_from_slice(&payload);
    Ok((crc, payload.len() as u32, offset, method))
}

pub fn finish_zip(sink: &mut Vec<u8>, entries: &[(String, u32, u32, u32, u32, u16)]) -> Result<()> {
    let central_start = sink.len() as u32;
    for (name, crc, csize, usize_, offset, method) in entries {
        let nb = name.as_bytes();
        sink.extend_from_slice(&0x02014b50u32.to_le_bytes());
        sink.extend_from_slice(&20u16.to_le_bytes());
        sink.extend_from_slice(&20u16.to_le_bytes());
        sink.extend_from_slice(&0u16.to_le_bytes());
        sink.extend_from_slice(&method.to_le_bytes());
        sink.extend_from_slice(&0u16.to_le_bytes());
        sink.extend_from_slice(&0u16.to_le_bytes());
        sink.extend_from_slice(&crc.to_le_bytes());
        sink.extend_from_slice(&csize.to_le_bytes());
        sink.extend_from_slice(&usize_.to_le_bytes());
        sink.extend_from_slice(&(nb.len() as u16).to_le_bytes());
        sink.extend_from_slice(&0u16.to_le_bytes());
        sink.extend_from_slice(&0u16.to_le_bytes());
        sink.extend_from_slice(&0u16.to_le_bytes());
        sink.extend_from_slice(&0u16.to_le_bytes());
        sink.extend_from_slice(&0u32.to_le_bytes());
        sink.extend_from_slice(&offset.to_le_bytes());
        sink.extend_from_slice(nb);
    }
    let central_size = sink.len() as u32 - central_start;
    sink.extend_from_slice(&0x06054b50u32.to_le_bytes());
    sink.extend_from_slice(&0u16.to_le_bytes());
    sink.extend_from_slice(&0u16.to_le_bytes());
    sink.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    sink.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    sink.extend_from_slice(&central_size.to_le_bytes());
    sink.extend_from_slice(&central_start.to_le_bytes());
    sink.extend_from_slice(&0u16.to_le_bytes());
    Ok(())
}

/// Convenience: zip a whole directory with an optional prefix.
pub fn zip_dir(src: &Path, prefix: &str) -> Result<Vec<u8>> {
    let mut sink = Vec::new();
    let mut entries = Vec::new();
    for item in walkdir::WalkDir::new(src) {
        let item = item?;
        if !item.file_type().is_file() { continue; }
        let rel = item.path().strip_prefix(src)?.to_string_lossy().replace('\\', "/");
        let name = if prefix.is_empty() { rel } else { format!("{}/{}", prefix.trim_end_matches('/'), rel) };
        let data = std::fs::read(item.path())?;
        let (crc, csize, offset, method) = add_file(&mut sink, &name, &data, true)?;
        entries.push((name, crc, csize, data.len() as u32, offset, method));
    }
    finish_zip(&mut sink, &entries)?;
    Ok(sink)
}

#[allow(dead_code)]
fn _check_component(_: Component<'_>) {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extracts_release_zip_if_present() {
        let zip = match std::env::var("PL_TEST_ZIP") { Ok(v) if !v.is_empty() => v, _ => return };
        let out = PathBuf::from("C:/Users/tester/zip-test-out");
        let _ = std::fs::remove_dir_all(&out);
        let n = extract_zip(Path::new(&zip), &out).unwrap();
        assert!(n > 100, "only {} files extracted", n);
    }
}
