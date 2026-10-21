//! Desktop file access for project documents.
//!
//! This is the only place that touches the filesystem; `cad_core` works on
//! JSON strings. A browser build can swap this module for another storage
//! backend without touching the core or the UI.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use cad_core::{Document, PersistenceError, from_json, to_json};

#[derive(Debug)]
pub enum ProjectFileError {
    Io {
        path: PathBuf,
        source: io::Error,
    },
    Format {
        path: PathBuf,
        source: PersistenceError,
    },
}

impl fmt::Display for ProjectFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProjectFileError::Io { path, source } => {
                write!(f, "{}: {source}", path.display())
            }
            ProjectFileError::Format { path, source } => {
                write!(f, "{}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for ProjectFileError {}

pub fn save(document: &Document, path: &Path) -> Result<(), ProjectFileError> {
    let json = to_json(document).map_err(|source| ProjectFileError::Format {
        path: path.to_owned(),
        source,
    })?;
    // Write to a sibling temp file first so a failed write never truncates
    // an existing project.
    let tmp = path.with_extension("json.tmp");
    let io_err = |source| ProjectFileError::Io {
        path: path.to_owned(),
        source,
    };
    fs::write(&tmp, json).map_err(io_err)?;
    fs::rename(&tmp, path).map_err(io_err)
}

pub fn load(path: &Path) -> Result<Document, ProjectFileError> {
    let json = fs::read_to_string(path).map_err(|source| ProjectFileError::Io {
        path: path.to_owned(),
        source,
    })?;
    from_json(&json).map_err(|source| ProjectFileError::Format {
        path: path.to_owned(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cad_core::{Command, PrimitiveKind, Transform3D};

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("cad-app-{}-{name}", std::process::id()))
    }

    #[test]
    fn save_then_load_round_trips() {
        let mut doc = Document::new();
        doc.apply(Command::create(
            "Part",
            PrimitiveKind::Cylinder.default_geometry(),
            Transform3D::from_translation([1.0, 2.0, 3.0]),
        ))
        .unwrap();
        let path = temp_path("roundtrip.cad.json");
        save(&doc, &path).unwrap();
        let loaded = load(&path).unwrap();
        fs::remove_file(&path).unwrap();
        assert_eq!(loaded, doc);
    }

    #[test]
    fn missing_and_invalid_files_are_errors() {
        assert!(matches!(
            load(&temp_path("does-not-exist.json")),
            Err(ProjectFileError::Io { .. })
        ));
        let path = temp_path("garbage.cad.json");
        fs::write(&path, "{ definitely not a project").unwrap();
        let result = load(&path);
        fs::remove_file(&path).unwrap();
        assert!(matches!(result, Err(ProjectFileError::Format { .. })));
    }
}
