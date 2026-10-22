//! Platform-independent core of the parametric CAD application.
//!
//! This crate owns the document model, typed primitive parameters,
//! validation, unit conventions, editing commands, JSON persistence, and
//! tessellation into a renderer-agnostic [`TriMesh`]. It does not depend on
//! Bevy or on operating-system APIs, so it can be tested headlessly and
//! compiled for other targets (e.g. WebAssembly) later.

pub mod command;
pub mod document;
pub mod feature;
pub mod geometry;
pub mod mesh;
pub mod parameters;
pub mod persistence;
pub mod units;
pub mod validation;

pub use command::{Applied, Command};
pub use document::{CURRENT_SCHEMA_VERSION, Document, DocumentError, MIN_SUPPORTED_SCHEMA_VERSION};
pub use feature::{Feature, ObjectId, Transform3D};
pub use geometry::{
    EditableGeometry, Geometry, ParameterError, PrimitiveKind, Tessellation, TriMesh,
};
pub use mesh::{EdgeId, EditableMesh, FaceId, MeshError, VertexId};
pub use parameters::{BoxParameters, CylinderParameters, ParameterDescriptor, SphereParameters};
pub use persistence::{PersistenceError, from_json, to_json};
pub use units::LengthUnit;
pub use validation::ValidationError;
