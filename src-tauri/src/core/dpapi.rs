//! Windows DPAPI protection for Hub-owned secrets (pure FFI, no C#/Python).
//! Values are stored as `dpapi:<hex>` in the instance config and decrypted on read.
use crate::error::{Error, Result};

const MASK: &str = "••••••••";

#[cfg(windows)]
#[repr(C)]
struct DataBlob { cb_data: u32, pb_data: *mut u8 }

#[cfg(windows)]
extern "system" {
    #[link(name = "crypt32")]
    fn CryptProtectData(
        data_in: *const DataBlob, descr: *const u16, entropy: *const DataBlob,
        reserved: *mut std::ffi::c_void, prompt: *mut std::ffi::c_void,
        flags: u32, data_out: *mut DataBlob,
    ) -> i32;
    #[link(name = "crypt32")]
    fn CryptUnprotectData(
        data_in: *const DataBlob, descr: *mut *mut u16, entropy: *const DataBlob,
        reserved: *mut std::ffi::c_void, prompt: *mut std::ffi::c_void,
        flags: u32, data_out: *mut DataBlob,
    ) -> i32;
    #[link(name = "kernel32")]
    fn LocalFree(hmem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
}

#[cfg(windows)]
fn protect(raw: &[u8]) -> Result<Vec<u8>> {
    use std::os::windows::ffi::OsStrExt;
    let descr: Vec<u16> = std::ffi::OsStr::new("PortableLethe Hub")
        .encode_wide().chain(Some(0)).collect();
    let input = DataBlob { cb_data: raw.len() as u32, pb_data: raw.as_ptr() as *mut u8 };
    let mut output = DataBlob { cb_data: 0, pb_data: std::ptr::null_mut() };
    let ok = unsafe { CryptProtectData(&input, descr.as_ptr(), std::ptr::null(), std::ptr::null_mut(), std::ptr::null_mut(), 0, &mut output) };
    if ok == 0 { return Err(Error::msg("DPAPI CryptProtectData failed")); }
    let bytes = unsafe { std::slice::from_raw_parts(output.pb_data, output.cb_data as usize).to_vec() };
    unsafe { LocalFree(output.pb_data as *mut std::ffi::c_void); }
    Ok(bytes)
}

#[cfg(windows)]
fn unprotect(raw: &[u8]) -> Result<Vec<u8>> {
    let input = DataBlob { cb_data: raw.len() as u32, pb_data: raw.as_ptr() as *mut u8 };
    let mut output = DataBlob { cb_data: 0, pb_data: std::ptr::null_mut() };
    let ok = unsafe { CryptUnprotectData(&input, std::ptr::null_mut(), std::ptr::null(), std::ptr::null_mut(), std::ptr::null_mut(), 0, &mut output) };
    if ok == 0 { return Err(Error::msg("DPAPI CryptUnprotectData failed")); }
    let bytes = unsafe { std::slice::from_raw_parts(output.pb_data, output.cb_data as usize).to_vec() };
    unsafe { LocalFree(output.pb_data as *mut std::ffi::c_void); }
    Ok(bytes)
}

/// Store a secret (opaque to disk readers) or pass through on non-Windows.
pub fn seal(plain: &str) -> Result<String> {
    if plain.is_empty() || plain.starts_with("dpapi:") || plain == MASK { return Ok(plain.to_string()); }
    #[cfg(windows)]
    { return Ok(format!("dpapi:{}", hex::encode(protect(plain.as_bytes())?))); }
    #[cfg(not(windows))]
    { Ok(plain.to_string()) }
}

/// Reverse `seal`, tolerating legacy plaintext values.
pub fn reveal(stored: &str) -> Result<String> {
    if let Some(hexpart) = stored.strip_prefix("dpapi:") {
        #[cfg(windows)]
        { let bytes = hex::decode(hexpart)?; return Ok(String::from_utf8_lossy(&unprotect(&bytes)?).into_owned()); }
        #[cfg(not(windows))]
        { let _ = hexpart; return Err(Error::msg("DPAPI value cannot be read on this platform")); }
    }
    Ok(stored.to_string())
}

pub fn mask(stored: &str) -> String {
    if stored.is_empty() { return String::new(); }
    if stored.starts_with("dpapi:") || stored == MASK { MASK.to_string() }
    else { format!("{}…{}", &stored[..1], &stored[stored.len()-1..]) }
}

pub fn is_mask(v: &str) -> bool { v == MASK }
