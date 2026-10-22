//! JSON (de)serialization of documents.
//!
//! This module works on strings only. Reading and writing files is the
//! caller's responsibility, which keeps the core free of OS APIs.

use serde_json::Value;
use thiserror::Error;

use crate::document::{CURRENT_SCHEMA_VERSION, Document, MIN_SUPPORTED_SCHEMA_VERSION};
use crate::validation::ValidationError;

/// Errors produced while loading or saving a document.
#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("malformed project file: {0}")]
    Malformed(#[from] serde_json::Error),
    #[error("project file is missing a numeric 'schema_version'")]
    MissingSchemaVersion,
    #[error("unsupported schema version {found} (this build reads versions {oldest} to {newest})")]
    UnsupportedSchemaVersion {
        found: u64,
        oldest: u32,
        newest: u32,
    },
    #[error("invalid project data: {0}")]
    Invalid(#[from] ValidationError),
}

/// Serializes a document to pretty-printed JSON.
pub fn to_json(document: &Document) -> Result<String, PersistenceError> {
    Ok(serde_json::to_string_pretty(document)?)
}

/// Parses and validates a document.
///
/// The schema version is checked before the typed parse so that files from
/// newer versions produce a clear error rather than a confusing field error.
pub fn from_json(json: &str) -> Result<Document, PersistenceError> {
    let value: Value = serde_json::from_str(json)?;
    let version = value
        .get("schema_version")
        .and_then(Value::as_u64)
        .ok_or(PersistenceError::MissingSchemaVersion)?;
    let supported = u64::from(MIN_SUPPORTED_SCHEMA_VERSION)..=u64::from(CURRENT_SCHEMA_VERSION);
    if !supported.contains(&version) {
        return Err(PersistenceError::UnsupportedSchemaVersion {
            found: version,
            oldest: MIN_SUPPORTED_SCHEMA_VERSION,
            newest: CURRENT_SCHEMA_VERSION,
        });
    }
    // Every older supported version is a subset of the current one, so no
    // migration step is needed yet; the document is simply marked current.
    let mut document: Document = serde_json::from_value(value)?;
    document.validate()?;
    document.upgrade_schema();
    Ok(document)
}
