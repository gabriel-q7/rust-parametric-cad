//! The authoritative CAD document.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::command::{Applied, Command};
use crate::feature::{Feature, ObjectId};
use crate::geometry::PrimitiveKind;
use crate::units::LengthUnit;
use crate::validation::ValidationError;

/// Schema version written by this build and the only one it can read.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Errors produced when mutating a document.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum DocumentError {
    #[error("no object with id {0}")]
    NotFound(ObjectId),
    #[error("an object with id {0} already exists")]
    DuplicateId(ObjectId),
    #[error(transparent)]
    Invalid(#[from] ValidationError),
}

/// A parametric CAD document: an ordered list of features plus metadata.
///
/// The document is the single source of truth. Meshes and other render data
/// are derived from it and never stored here. All mutations go through
/// [`Document::apply`], which validates before changing anything.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    schema_version: u32,
    units: LengthUnit,
    features: Vec<Feature>,
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

impl Document {
    /// An empty document using the current schema and millimeters.
    pub fn new() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            units: LengthUnit::Millimeter,
            features: Vec::new(),
        }
    }

    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub fn units(&self) -> LengthUnit {
        self.units
    }

    /// Features in creation order.
    pub fn features(&self) -> &[Feature] {
        &self.features
    }

    pub fn feature(&self, id: ObjectId) -> Option<&Feature> {
        self.features.iter().find(|f| f.id() == id)
    }

    pub fn contains(&self, id: ObjectId) -> bool {
        self.feature(id).is_some()
    }

    pub fn len(&self) -> usize {
        self.features.len()
    }

    pub fn is_empty(&self) -> bool {
        self.features.is_empty()
    }

    /// Applies a command. On error the document is left unchanged.
    pub fn apply(&mut self, command: Command) -> Result<Applied, DocumentError> {
        command.apply(self)
    }

    /// Checks every document invariant. Used after deserialization.
    pub fn validate(&self) -> Result<(), ValidationError> {
        let mut seen = HashSet::with_capacity(self.features.len());
        for feature in &self.features {
            if !seen.insert(feature.id()) {
                return Err(ValidationError::DuplicateId(feature.id()));
            }
            feature.validate()?;
        }
        Ok(())
    }

    /// Suggests an unused default name such as `"Box 3"`.
    pub fn suggest_name(&self, kind: PrimitiveKind) -> String {
        (1..)
            .map(|n| format!("{} {n}", kind.label()))
            .find(|name| self.features.iter().all(|f| f.name() != name))
            .expect("an unused name always exists")
    }

    // ---- crate-private mutation primitives used by commands ----

    pub(crate) fn index_of(&self, id: ObjectId) -> Result<usize, DocumentError> {
        self.features
            .iter()
            .position(|f| f.id() == id)
            .ok_or(DocumentError::NotFound(id))
    }

    pub(crate) fn feature_mut(&mut self, id: ObjectId) -> Result<&mut Feature, DocumentError> {
        let index = self.index_of(id)?;
        Ok(&mut self.features[index])
    }

    /// Inserts a validated feature at `index` (clamped to the end).
    pub(crate) fn insert(&mut self, index: usize, feature: Feature) -> Result<(), DocumentError> {
        if self.contains(feature.id()) {
            return Err(DocumentError::DuplicateId(feature.id()));
        }
        feature.validate()?;
        let index = index.min(self.features.len());
        self.features.insert(index, feature);
        Ok(())
    }

    pub(crate) fn remove(&mut self, id: ObjectId) -> Result<(usize, Feature), DocumentError> {
        let index = self.index_of(id)?;
        Ok((index, self.features.remove(index)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feature::Transform3D;

    #[test]
    fn new_document_is_empty_and_valid() {
        let doc = Document::new();
        assert!(doc.is_empty());
        assert_eq!(doc.schema_version(), CURRENT_SCHEMA_VERSION);
        assert_eq!(doc.units(), LengthUnit::Millimeter);
        doc.validate().unwrap();
    }

    #[test]
    fn suggests_first_unused_name() {
        let mut doc = Document::new();
        assert_eq!(doc.suggest_name(PrimitiveKind::Box), "Box 1");
        doc.apply(Command::create(
            "Box 1",
            PrimitiveKind::Box.default_geometry(),
            Transform3D::default(),
        ))
        .unwrap();
        assert_eq!(doc.suggest_name(PrimitiveKind::Box), "Box 2");
        assert_eq!(doc.suggest_name(PrimitiveKind::Sphere), "Sphere 1");
    }
}
