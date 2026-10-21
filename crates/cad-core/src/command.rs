//! Editing commands: the only way to mutate a [`Document`].
//!
//! Commands are plain data, independent of any UI. Applying a command either
//! succeeds completely or leaves the document untouched, and returns the
//! inverse command, which is the building block for a future undo/redo
//! history.

use crate::document::{Document, DocumentError};
use crate::feature::{Feature, ObjectId, Transform3D};
use crate::geometry::Geometry;
use crate::validation::normalize_name;

/// A document mutation.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Appends a new primitive. The ID is chosen by the caller so that
    /// replaying the command (e.g. redo) recreates the same object.
    CreatePrimitive {
        id: ObjectId,
        name: String,
        geometry: Geometry,
        transform: Transform3D,
    },
    /// Replaces the parametric geometry of an object; its ID is preserved.
    UpdateParameters {
        id: ObjectId,
        geometry: Geometry,
    },
    RenameObject {
        id: ObjectId,
        name: String,
    },
    DeleteObject {
        id: ObjectId,
    },
    SetObjectTransform {
        id: ObjectId,
        transform: Transform3D,
    },
    /// Re-inserts a whole feature at a position (the inverse of a delete).
    InsertFeature {
        index: usize,
        feature: Feature,
    },
    /// Applies several commands atomically: all of them or none.
    Batch(Vec<Command>),
}

/// What changed when a command was applied.
#[derive(Debug, Clone, PartialEq)]
pub struct Applied {
    /// Command that reverts this change.
    pub inverse: Command,
    /// Objects whose state changed (created, modified, or deleted).
    pub affected: Vec<ObjectId>,
}

impl Command {
    /// Convenience constructor that generates a fresh ID.
    pub fn create(name: &str, geometry: Geometry, transform: Transform3D) -> Command {
        Command::CreatePrimitive {
            id: ObjectId::new(),
            name: name.to_owned(),
            geometry,
            transform,
        }
    }

    pub(crate) fn apply(self, doc: &mut Document) -> Result<Applied, DocumentError> {
        match self {
            Command::CreatePrimitive {
                id,
                name,
                geometry,
                transform,
            } => {
                let feature = Feature::new(id, &name, geometry, transform)?;
                doc.insert(doc.len(), feature)?;
                Ok(Applied {
                    inverse: Command::DeleteObject { id },
                    affected: vec![id],
                })
            }
            Command::UpdateParameters { id, geometry } => {
                geometry.validate()?;
                let feature = doc.feature_mut(id)?;
                let previous = *feature.geometry();
                feature.set_geometry(geometry);
                Ok(Applied {
                    inverse: Command::UpdateParameters {
                        id,
                        geometry: previous,
                    },
                    affected: vec![id],
                })
            }
            Command::RenameObject { id, name } => {
                let name = normalize_name(&name)?;
                let feature = doc.feature_mut(id)?;
                let previous = feature.name().to_owned();
                feature.set_name(name);
                Ok(Applied {
                    inverse: Command::RenameObject { id, name: previous },
                    affected: vec![id],
                })
            }
            Command::DeleteObject { id } => {
                let (index, feature) = doc.remove(id)?;
                Ok(Applied {
                    inverse: Command::InsertFeature { index, feature },
                    affected: vec![id],
                })
            }
            Command::SetObjectTransform { id, transform } => {
                transform.validate()?;
                let feature = doc.feature_mut(id)?;
                let previous = *feature.transform();
                feature.set_transform(transform);
                Ok(Applied {
                    inverse: Command::SetObjectTransform {
                        id,
                        transform: previous,
                    },
                    affected: vec![id],
                })
            }
            Command::InsertFeature { index, feature } => {
                let id = feature.id();
                doc.insert(index, feature)?;
                Ok(Applied {
                    inverse: Command::DeleteObject { id },
                    affected: vec![id],
                })
            }
            Command::Batch(commands) => {
                // Work on a copy so a failure part-way leaves `doc` untouched.
                let mut scratch = doc.clone();
                let mut inverses = Vec::with_capacity(commands.len());
                let mut affected = Vec::new();
                for command in commands {
                    let applied = command.apply(&mut scratch)?;
                    inverses.push(applied.inverse);
                    for id in applied.affected {
                        if !affected.contains(&id) {
                            affected.push(id);
                        }
                    }
                }
                *doc = scratch;
                inverses.reverse();
                Ok(Applied {
                    inverse: Command::Batch(inverses),
                    affected,
                })
            }
        }
    }
}
