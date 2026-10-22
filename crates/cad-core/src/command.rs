//! Editing commands: the only way to mutate a [`Document`].
//!
//! Commands are plain data, independent of any UI. Applying a command either
//! succeeds completely or leaves the document untouched, and returns the
//! inverse command, which is the building block for a future undo/redo
//! history.

use crate::document::{Document, DocumentError};
use crate::feature::{Feature, ObjectId, Transform3D};
use crate::geometry::{EditableGeometry, Geometry, Tessellation};
use crate::mesh::{EdgeId, EditableMesh, FaceId, VertexId};
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
    /// Replaces a parametric primitive with an equivalent editable mesh
    /// (one-way). ID, name, and transform are preserved.
    ConvertToEditable {
        id: ObjectId,
    },
    /// Moves one vertex of an editable mesh to a local position (mm).
    MoveVertex {
        id: ObjectId,
        vertex: VertexId,
        position: [f64; 3],
    },
    /// Moves both endpoints of an edge by the same local offset (mm).
    TranslateEdge {
        id: ObjectId,
        edge: EdgeId,
        offset: [f64; 3],
    },
    /// Extrudes a planar convex face along its normal by `distance` (mm).
    ExtrudeFace {
        id: ObjectId,
        face: FaceId,
        distance: f64,
    },
}

/// What changed when a command was applied.
#[derive(Debug, Clone, PartialEq)]
pub struct Applied {
    /// Command that reverts this change.
    pub inverse: Command,
    /// Objects whose state changed (created, modified, or deleted).
    pub affected: Vec<ObjectId>,
    /// Face created by an extrusion (the new cap), for selection updates.
    pub new_face: Option<FaceId>,
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
                    new_face: None,
                })
            }
            Command::UpdateParameters { id, geometry } => {
                geometry.validate()?;
                let feature = doc.feature_mut(id)?;
                let previous = feature.geometry().clone();
                feature.set_geometry(geometry);
                Ok(Applied {
                    inverse: Command::UpdateParameters {
                        id,
                        geometry: previous,
                    },
                    affected: vec![id],
                    new_face: None,
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
                    new_face: None,
                })
            }
            Command::DeleteObject { id } => {
                let (index, feature) = doc.remove(id)?;
                Ok(Applied {
                    inverse: Command::InsertFeature { index, feature },
                    affected: vec![id],
                    new_face: None,
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
                    new_face: None,
                })
            }
            Command::InsertFeature { index, feature } => {
                let id = feature.id();
                doc.insert(index, feature)?;
                Ok(Applied {
                    inverse: Command::DeleteObject { id },
                    affected: vec![id],
                    new_face: None,
                })
            }
            Command::Batch(commands) => {
                // Work on a copy so a failure part-way leaves `doc` untouched.
                let mut scratch = doc.clone();
                let mut inverses = Vec::with_capacity(commands.len());
                let mut affected = Vec::new();
                let mut new_face = None;
                for command in commands {
                    let applied = command.apply(&mut scratch)?;
                    inverses.push(applied.inverse);
                    new_face = applied.new_face.or(new_face);
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
                    new_face,
                })
            }
            Command::ConvertToEditable { id } => {
                let feature = doc.feature_mut(id)?;
                let converted = feature
                    .geometry()
                    .to_editable(&Tessellation::EDITABLE)
                    .ok_or(DocumentError::AlreadyEditable(id))??;
                replace_geometry(doc, id, converted, None)
            }
            Command::MoveVertex {
                id,
                vertex,
                position,
            } => edit_mesh(doc, id, |mesh| {
                Ok((mesh.with_vertex_moved(vertex, position)?, None))
            }),
            Command::TranslateEdge { id, edge, offset } => edit_mesh(doc, id, |mesh| {
                Ok((mesh.with_edge_translated(edge, offset)?, None))
            }),
            Command::ExtrudeFace { id, face, distance } => edit_mesh(doc, id, |mesh| {
                let (mesh, cap) = mesh.with_face_extruded(face, distance)?;
                Ok((mesh, Some(cap)))
            }),
        }
    }
}

/// Applies a topology edit to an editable object, keeping its provenance.
/// The edit function validates and returns a new mesh, so the document is
/// only touched once the result is known to be valid.
fn edit_mesh(
    doc: &mut Document,
    id: ObjectId,
    edit: impl FnOnce(&EditableMesh) -> Result<(EditableMesh, Option<FaceId>), DocumentError>,
) -> Result<Applied, DocumentError> {
    let feature = doc.feature(id).ok_or(DocumentError::NotFound(id))?;
    let editable = feature
        .geometry()
        .as_editable()
        .ok_or(DocumentError::NotEditable(id))?;
    let (mesh, new_face) = edit(&editable.mesh)?;
    let geometry = Geometry::Mesh(EditableGeometry {
        mesh,
        source: editable.source.clone(),
    });
    replace_geometry(doc, id, geometry, new_face)
}

fn replace_geometry(
    doc: &mut Document,
    id: ObjectId,
    geometry: Geometry,
    new_face: Option<FaceId>,
) -> Result<Applied, DocumentError> {
    geometry.validate()?;
    let feature = doc.feature_mut(id)?;
    let previous = feature.geometry().clone();
    feature.set_geometry(geometry);
    Ok(Applied {
        inverse: Command::UpdateParameters {
            id,
            geometry: previous,
        },
        affected: vec![id],
        new_face,
    })
}
