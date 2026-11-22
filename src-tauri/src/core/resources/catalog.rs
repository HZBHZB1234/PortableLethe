//! Addressables catalog v2 reader (port of tools/parse_catalog.py header format).
use serde::Serialize;
use crate::{core::Hub, error::Result};

#[derive(Debug, Clone, Serialize)]
pub struct CatalogHit {
    pub key: String,
    pub internal_id: Option<String>,
    pub bundle_name: Option<String>,
    pub hash: Option<String>,
}

pub fn find(_hub: &Hub, _pattern: &str, _catalog_path: Option<&str>) -> Result<Vec<CatalogHit>> {
    Err(crate::error::Error::msg("catalog parser: phase 3"))
}
