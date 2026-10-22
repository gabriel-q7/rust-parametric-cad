//! Editable polygon meshes with stable topology IDs.
//!
//! An [`EditableMesh`] is an indexed polygon mesh with explicit vertex, edge,
//! and face records. It is deliberately simpler than a half-edge or B-rep
//! structure: connectivity is derived on demand from face boundaries.
//!
//! Invariants checked by [`EditableMesh::validate`]:
//!
//! - Every ID is unique across vertices, edges, and faces, and is below the
//!   mesh's `next_id` counter. IDs are never reused, so removed elements
//!   cannot be confused with new ones.
//! - Positions are finite and within the supported coordinate range.
//! - Each edge joins two distinct existing vertices, has a non-zero length,
//!   and no two edges join the same pair of vertices.
//! - Each face has at least three distinct vertices, is not degenerate (has a
//!   non-zero area), and every consecutive pair of its boundary vertices is
//!   joined by an edge.
//! - The surface is a closed, consistently oriented 2-manifold: every edge
//!   is traversed exactly once in each direction by the face boundaries.
//!   Face boundaries are counter-clockwise when seen from outside.
//! - Every vertex belongs to at least one face.
//!
//! Positions are in document units (millimeters) in the object's local frame.
//! All editing operations are pure: they return a new, validated mesh and
//! leave `self` untouched.

use std::collections::{HashMap, HashSet};
use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::geometry::TriMesh;
use crate::validation::{MAX_LENGTH_MM, ValidationError, validate_coordinate};

/// Upper bound on vertices + edges + faces, to keep files and edits bounded.
pub const MAX_MESH_ELEMENTS: usize = 500_000;

/// Faces smaller than this area (mm²) are considered degenerate.
const MIN_FACE_AREA: f64 = 1e-9;

/// Edges shorter than this (mm) are considered degenerate.
const MIN_EDGE_LENGTH: f64 = 1e-9;

macro_rules! topology_id {
    ($(#[$doc:meta])* $name:ident, $prefix:literal) => {
        $(#[$doc])*
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub u32);

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!($prefix, "{}"), self.0)
            }
        }
    };
}

topology_id!(
    /// Stable vertex identifier within one mesh.
    VertexId,
    "v"
);
topology_id!(
    /// Stable edge identifier within one mesh.
    EdgeId,
    "e"
);
topology_id!(
    /// Stable face identifier within one mesh.
    FaceId,
    "f"
);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Vertex {
    pub id: VertexId,
    /// Local position in millimeters.
    pub position: [f64; 3],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub id: EdgeId,
    pub vertices: [VertexId; 2],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Face {
    pub id: FaceId,
    /// Boundary, counter-clockwise when seen from outside the solid.
    pub vertices: Vec<VertexId>,
}

/// Errors from mesh validation and mesh editing operations.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum MeshError {
    #[error("mesh has no faces")]
    Empty,
    #[error("mesh has more than {MAX_MESH_ELEMENTS} elements")]
    TooLarge,
    #[error("topology id {0} is used more than once")]
    DuplicateId(u32),
    #[error("topology id {0} is not below the mesh's next_id counter")]
    IdOutOfRange(u32),
    #[error("vertex {vertex}: {source}")]
    Position {
        vertex: VertexId,
        source: ValidationError,
    },
    #[error("no vertex with id {0}")]
    UnknownVertex(VertexId),
    #[error("no edge with id {0}")]
    UnknownEdge(EdgeId),
    #[error("no face with id {0}")]
    UnknownFace(FaceId),
    #[error("edge {0} must join two different existing vertices")]
    InvalidEdge(EdgeId),
    #[error("edge {0} duplicates another edge")]
    DuplicateEdge(EdgeId),
    #[error("face {0} must have at least three distinct vertices")]
    InvalidFace(FaceId),
    #[error("face {face}: no edge joins {a} and {b}")]
    MissingEdge {
        face: FaceId,
        a: VertexId,
        b: VertexId,
    },
    #[error("edge {0} is not shared by exactly two consistently oriented faces")]
    NonManifoldEdge(EdgeId),
    #[error("vertex {0} is not part of any face")]
    OrphanVertex(VertexId),
    #[error("edge {0} would have zero length")]
    DegenerateEdge(EdgeId),
    #[error("face {0} would become degenerate (zero area)")]
    DegenerateFace(FaceId),
    #[error("face {0} is not planar; only planar faces can be extruded")]
    NonPlanarFace(FaceId),
    #[error("face {0} is not convex; only convex faces can be extruded")]
    NonConvexFace(FaceId),
    #[error("extrusion distance must be a finite, non-zero number of at most {MAX_LENGTH_MM} mm")]
    InvalidDistance,
    #[error("offset: {0}")]
    InvalidOffset(ValidationError),
}

/// An editable polygon mesh. See the module docs for its invariants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditableMesh {
    vertices: Vec<Vertex>,
    edges: Vec<Edge>,
    faces: Vec<Face>,
    /// Next unused topology ID; shared by vertices, edges, and faces.
    next_id: u32,
}

impl EditableMesh {
    /// Builds a mesh from positions and polygons (indices into `positions`).
    ///
    /// Vertices get IDs `0..n` in order, edges are derived from the face
    /// boundaries in order of first appearance, and faces follow. The result
    /// is validated.
    pub fn from_polygons(
        positions: &[[f64; 3]],
        polygons: &[Vec<usize>],
    ) -> Result<Self, MeshError> {
        let mut next_id = 0u32;
        let mut take_id = || {
            let id = next_id;
            next_id += 1;
            id
        };
        let vertices: Vec<Vertex> = positions
            .iter()
            .map(|&position| Vertex {
                id: VertexId(take_id()),
                position,
            })
            .collect();
        let vid = |index: usize| {
            vertices
                .get(index)
                .map(|v| v.id)
                .ok_or(MeshError::UnknownVertex(VertexId(index as u32)))
        };

        let mut edges = Vec::new();
        let mut seen = HashSet::new();
        for polygon in polygons {
            for (a, b) in cyclic_pairs(polygon) {
                let (a, b) = (vid(*a)?, vid(*b)?);
                if seen.insert(edge_key(a, b)) {
                    edges.push(Edge {
                        id: EdgeId(take_id()),
                        vertices: [a, b],
                    });
                }
            }
        }
        let faces = polygons
            .iter()
            .map(|polygon| {
                Ok(Face {
                    id: FaceId(take_id()),
                    vertices: polygon.iter().map(|&i| vid(i)).collect::<Result<_, _>>()?,
                })
            })
            .collect::<Result<Vec<_>, MeshError>>()?;

        let mesh = Self {
            vertices,
            edges,
            faces,
            next_id,
        };
        mesh.validate()?;
        Ok(mesh)
    }

    pub fn vertices(&self) -> &[Vertex] {
        &self.vertices
    }

    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    pub fn faces(&self) -> &[Face] {
        &self.faces
    }

    pub fn vertex(&self, id: VertexId) -> Option<&Vertex> {
        self.vertices.iter().find(|v| v.id == id)
    }

    pub fn edge(&self, id: EdgeId) -> Option<&Edge> {
        self.edges.iter().find(|e| e.id == id)
    }

    pub fn face(&self, id: FaceId) -> Option<&Face> {
        self.faces.iter().find(|f| f.id == id)
    }

    /// Position of a vertex, if it exists.
    pub fn position(&self, id: VertexId) -> Option<[f64; 3]> {
        self.vertex(id).map(|v| v.position)
    }

    /// Length of an edge in millimeters.
    pub fn edge_length(&self, id: EdgeId) -> Option<f64> {
        let edge = self.edge(id)?;
        let [a, b] = edge.vertices.map(|v| self.position(v));
        Some(length(sub(b?, a?)))
    }

    /// Unit normal of a face (Newell's method), if the face exists and is
    /// not degenerate.
    pub fn face_normal(&self, id: FaceId) -> Option<[f64; 3]> {
        let points = self.face_positions(self.face(id)?)?;
        normalize(newell(&points))
    }

    /// Checks whether a face can be extruded (planar and convex).
    pub fn check_extrudable(&self, id: FaceId) -> Result<(), MeshError> {
        let face = self.face(id).ok_or(MeshError::UnknownFace(id))?;
        let points = self
            .face_positions(face)
            .ok_or(MeshError::InvalidFace(id))?;
        let normal = normalize(newell(&points)).ok_or(MeshError::DegenerateFace(id))?;
        let centroid = centroid(&points);
        let extent = points
            .iter()
            .map(|p| length(sub(*p, centroid)))
            .fold(0.0, f64::max);
        let tolerance = 1e-6 * extent.max(1.0);
        if points
            .iter()
            .any(|p| dot(sub(*p, centroid), normal).abs() > tolerance)
        {
            return Err(MeshError::NonPlanarFace(id));
        }
        let n = points.len();
        for i in 0..n {
            let (a, b, c) = (points[i], points[(i + 1) % n], points[(i + 2) % n]);
            let turn = dot(cross(sub(b, a), sub(c, b)), normal);
            if turn < -tolerance * tolerance {
                return Err(MeshError::NonConvexFace(id));
            }
        }
        Ok(())
    }

    /// Validates every invariant listed in the module docs.
    pub fn validate(&self) -> Result<(), MeshError> {
        if self.faces.is_empty() {
            return Err(MeshError::Empty);
        }
        if self.vertices.len() + self.edges.len() + self.faces.len() > MAX_MESH_ELEMENTS {
            return Err(MeshError::TooLarge);
        }

        let mut ids = HashSet::new();
        let all_ids = self
            .vertices
            .iter()
            .map(|v| v.id.0)
            .chain(self.edges.iter().map(|e| e.id.0))
            .chain(self.faces.iter().map(|f| f.id.0));
        for id in all_ids {
            if id >= self.next_id {
                return Err(MeshError::IdOutOfRange(id));
            }
            if !ids.insert(id) {
                return Err(MeshError::DuplicateId(id));
            }
        }

        const AXES: [&str; 3] = ["x", "y", "z"];
        let mut positions = HashMap::with_capacity(self.vertices.len());
        for vertex in &self.vertices {
            for (axis, value) in AXES.into_iter().zip(vertex.position) {
                validate_coordinate(axis, value).map_err(|source| MeshError::Position {
                    vertex: vertex.id,
                    source,
                })?;
            }
            positions.insert(vertex.id, vertex.position);
        }

        let mut edge_keys = HashMap::with_capacity(self.edges.len());
        for edge in &self.edges {
            let [a, b] = edge.vertices;
            if a == b || !positions.contains_key(&a) || !positions.contains_key(&b) {
                return Err(MeshError::InvalidEdge(edge.id));
            }
            if edge_keys.insert(edge_key(a, b), edge.id).is_some() {
                return Err(MeshError::DuplicateEdge(edge.id));
            }
            if length(sub(positions[&a], positions[&b])) < MIN_EDGE_LENGTH {
                return Err(MeshError::DegenerateEdge(edge.id));
            }
        }

        let mut directed: HashMap<(VertexId, VertexId), u32> = HashMap::new();
        let mut used_vertices = HashSet::new();
        for face in &self.faces {
            let distinct: HashSet<_> = face.vertices.iter().collect();
            if face.vertices.len() < 3
                || distinct.len() != face.vertices.len()
                || face.vertices.iter().any(|v| !positions.contains_key(v))
            {
                return Err(MeshError::InvalidFace(face.id));
            }
            for (&a, &b) in cyclic_pairs(&face.vertices) {
                if !edge_keys.contains_key(&edge_key(a, b)) {
                    return Err(MeshError::MissingEdge {
                        face: face.id,
                        a,
                        b,
                    });
                }
                *directed.entry((a, b)).or_default() += 1;
            }
            used_vertices.extend(face.vertices.iter().copied());
            let points: Vec<_> = face.vertices.iter().map(|v| positions[v]).collect();
            if length(newell(&points)) / 2.0 < MIN_FACE_AREA {
                return Err(MeshError::DegenerateFace(face.id));
            }
        }

        for edge in &self.edges {
            let [a, b] = edge.vertices;
            let forward = directed.get(&(a, b)).copied().unwrap_or(0);
            let backward = directed.get(&(b, a)).copied().unwrap_or(0);
            if forward != 1 || backward != 1 {
                return Err(MeshError::NonManifoldEdge(edge.id));
            }
        }
        if let Some(orphan) = self
            .vertices
            .iter()
            .find(|v| !used_vertices.contains(&v.id))
        {
            return Err(MeshError::OrphanVertex(orphan.id));
        }
        Ok(())
    }

    /// Returns a copy with one vertex moved to a new local position.
    pub fn with_vertex_moved(
        &self,
        id: VertexId,
        position: [f64; 3],
    ) -> Result<EditableMesh, MeshError> {
        let mut mesh = self.clone();
        let vertex = mesh
            .vertices
            .iter_mut()
            .find(|v| v.id == id)
            .ok_or(MeshError::UnknownVertex(id))?;
        vertex.position = position;
        mesh.validate()?;
        Ok(mesh)
    }

    /// Returns a copy with both endpoints of an edge moved by `offset`.
    /// The edge keeps its length and orientation.
    pub fn with_edge_translated(
        &self,
        id: EdgeId,
        offset: [f64; 3],
    ) -> Result<EditableMesh, MeshError> {
        for (axis, value) in ["x", "y", "z"].into_iter().zip(offset) {
            validate_coordinate(axis, value).map_err(MeshError::InvalidOffset)?;
        }
        let endpoints = self.edge(id).ok_or(MeshError::UnknownEdge(id))?.vertices;
        let mut mesh = self.clone();
        for vertex in mesh
            .vertices
            .iter_mut()
            .filter(|v| endpoints.contains(&v.id))
        {
            vertex.position = add(vertex.position, offset);
        }
        mesh.validate()?;
        Ok(mesh)
    }

    /// Extrudes a planar, convex face along its normal by `distance` (mm,
    /// negative pushes inwards). The original face is replaced by side faces
    /// and a new cap face, whose ID is returned.
    ///
    /// For each boundary vertex `v_i` a new vertex `v_i + distance * n` is
    /// created. Side face `i` is `[v_i, v_i+1, v'_i+1, v'_i]`, which keeps the
    /// surface closed and consistently oriented.
    pub fn with_face_extruded(
        &self,
        id: FaceId,
        distance: f64,
    ) -> Result<(EditableMesh, FaceId), MeshError> {
        if !distance.is_finite() || distance == 0.0 || distance.abs() > MAX_LENGTH_MM {
            return Err(MeshError::InvalidDistance);
        }
        self.check_extrudable(id)?;
        let normal = self.face_normal(id).ok_or(MeshError::DegenerateFace(id))?;
        let offset = normal.map(|c| c * distance);

        let mut mesh = self.clone();
        let index = mesh
            .faces
            .iter()
            .position(|f| f.id == id)
            .ok_or(MeshError::UnknownFace(id))?;
        let boundary = mesh.faces.remove(index).vertices;
        let n = boundary.len();

        let lifted: Vec<VertexId> = boundary
            .iter()
            .map(|&v| {
                let position = add(self.position(v).expect("validated face"), offset);
                let id = VertexId(mesh.take_id());
                mesh.vertices.push(Vertex { id, position });
                id
            })
            .collect();
        for i in 0..n {
            let rise = EdgeId(mesh.take_id());
            mesh.edges.push(Edge {
                id: rise,
                vertices: [boundary[i], lifted[i]],
            });
        }
        for i in 0..n {
            let rim = EdgeId(mesh.take_id());
            mesh.edges.push(Edge {
                id: rim,
                vertices: [lifted[i], lifted[(i + 1) % n]],
            });
        }
        for i in 0..n {
            let j = (i + 1) % n;
            let side = FaceId(mesh.take_id());
            mesh.faces.push(Face {
                id: side,
                vertices: vec![boundary[i], boundary[j], lifted[j], lifted[i]],
            });
        }
        let cap = FaceId(mesh.take_id());
        mesh.faces.push(Face {
            id: cap,
            vertices: lifted,
        });

        mesh.validate()?;
        Ok((mesh, cap))
    }

    /// Flat-shaded triangle mesh for rendering (fan triangulation per face).
    pub fn triangulate(&self) -> TriMesh {
        let positions: HashMap<VertexId, [f64; 3]> =
            self.vertices.iter().map(|v| (v.id, v.position)).collect();
        let mut out = TriMesh::default();
        for face in &self.faces {
            let points: Vec<[f64; 3]> = face.vertices.iter().map(|v| positions[v]).collect();
            let normal = normalize(newell(&points)).unwrap_or([0.0, 0.0, 1.0]);
            let base = out.positions.len() as u32;
            for p in &points {
                out.positions.push(*p);
                out.normals.push(normal);
            }
            for i in 1..points.len() as u32 - 1 {
                out.indices
                    .extend_from_slice(&[base, base + i, base + i + 1]);
            }
        }
        out
    }

    /// Nearest face hit by a ray in local coordinates, with the ray
    /// parameter `t` of the hit (`origin + t * direction`).
    pub fn pick_face(&self, origin: [f64; 3], direction: [f64; 3]) -> Option<(FaceId, f64)> {
        let mut best: Option<(FaceId, f64)> = None;
        for face in &self.faces {
            let Some(points) = self.face_positions(face) else {
                continue;
            };
            for i in 1..points.len() - 1 {
                if let Some(t) =
                    ray_triangle(origin, direction, points[0], points[i], points[i + 1])
                    && best.is_none_or(|(_, best_t)| t < best_t)
                {
                    best = Some((face.id, t));
                }
            }
        }
        best
    }

    fn face_positions(&self, face: &Face) -> Option<Vec<[f64; 3]>> {
        face.vertices.iter().map(|&v| self.position(v)).collect()
    }

    fn take_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

fn cyclic_pairs<T>(items: &[T]) -> impl Iterator<Item = (&T, &T)> {
    items.iter().zip(items.iter().cycle().skip(1))
}

fn edge_key(a: VertexId, b: VertexId) -> (VertexId, VertexId) {
    if a < b { (a, b) } else { (b, a) }
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn length(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

fn normalize(a: [f64; 3]) -> Option<[f64; 3]> {
    let len = length(a);
    (len > 1e-12).then(|| a.map(|c| c / len))
}

fn centroid(points: &[[f64; 3]]) -> [f64; 3] {
    let sum = points.iter().fold([0.0; 3], |acc, p| add(acc, *p));
    sum.map(|c| c / points.len() as f64)
}

/// Newell's polygon normal; its length is twice the polygon's area.
fn newell(points: &[[f64; 3]]) -> [f64; 3] {
    cyclic_pairs(points).fold([0.0; 3], |n, (a, b)| {
        [
            n[0] + (a[1] - b[1]) * (a[2] + b[2]),
            n[1] + (a[2] - b[2]) * (a[0] + b[0]),
            n[2] + (a[0] - b[0]) * (a[1] + b[1]),
        ]
    })
}

/// Möller–Trumbore ray/triangle intersection (two-sided).
fn ray_triangle(
    origin: [f64; 3],
    direction: [f64; 3],
    a: [f64; 3],
    b: [f64; 3],
    c: [f64; 3],
) -> Option<f64> {
    let (e1, e2) = (sub(b, a), sub(c, a));
    let p = cross(direction, e2);
    let det = dot(e1, p);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = sub(origin, a);
    let u = dot(s, p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = cross(s, e1);
    let v = dot(direction, q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = dot(e2, q) * inv;
    (t > 1e-9).then_some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unit-ish tetrahedron with outward, counter-clockwise faces.
    fn tetrahedron() -> EditableMesh {
        EditableMesh::from_polygons(
            &[
                [0.0, 0.0, 0.0],
                [10.0, 0.0, 0.0],
                [0.0, 10.0, 0.0],
                [0.0, 0.0, 10.0],
            ],
            &[vec![0, 2, 1], vec![0, 1, 3], vec![1, 2, 3], vec![0, 3, 2]],
        )
        .unwrap()
    }

    #[test]
    fn from_polygons_assigns_sequential_ids() {
        let mesh = tetrahedron();
        assert_eq!(mesh.vertices().len(), 4);
        assert_eq!(mesh.edges().len(), 6);
        assert_eq!(mesh.faces().len(), 4);
        assert_eq!(mesh.vertices()[0].id, VertexId(0));
        assert_eq!(mesh.edges()[0].id, EdgeId(4));
        assert_eq!(mesh.faces()[0].id, FaceId(10));
        assert_eq!(mesh.next_id, 14);
    }

    #[test]
    fn rejects_open_or_inconsistently_oriented_surfaces() {
        let positions = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        assert!(matches!(
            EditableMesh::from_polygons(&positions, &[vec![0, 1, 2]]),
            Err(MeshError::NonManifoldEdge(_))
        ));
        let flipped = EditableMesh::from_polygons(
            &[
                [0.0, 0.0, 0.0],
                [10.0, 0.0, 0.0],
                [0.0, 10.0, 0.0],
                [0.0, 0.0, 10.0],
            ],
            &[vec![0, 1, 2], vec![0, 1, 3], vec![1, 2, 3], vec![0, 3, 2]],
        );
        assert!(matches!(flipped, Err(MeshError::NonManifoldEdge(_))));
    }

    #[test]
    fn rejects_degenerate_faces_after_a_move() {
        let mesh = tetrahedron();
        // Collapse vertex 3 onto vertex 0: faces touching both become lines.
        assert!(matches!(
            mesh.with_vertex_moved(VertexId(3), [0.0, 0.0, 0.0]),
            Err(MeshError::DegenerateEdge(_))
        ));
        // Put the apex on the edge v0-v1: face (v0, v1, v3) becomes a line.
        assert!(matches!(
            mesh.with_vertex_moved(VertexId(3), [5.0, 0.0, 0.0]),
            Err(MeshError::DegenerateFace(_))
        ));
    }

    #[test]
    fn pick_face_returns_nearest_hit() {
        let mesh = tetrahedron();
        // Ray straight down onto the bottom face (z = 0, outward normal -Z).
        let (face, t) = mesh.pick_face([2.0, 2.0, 50.0], [0.0, 0.0, -1.0]).unwrap();
        // The slanted face (1, 2, 3) is hit first, above the bottom face.
        assert_eq!(mesh.face(face).unwrap().vertices.len(), 3);
        assert!(t < 50.0);
        assert!(
            mesh.pick_face([50.0, 50.0, 50.0], [0.0, 0.0, 1.0])
                .is_none()
        );
    }

    #[test]
    fn triangulation_has_flat_normals() {
        let tri = tetrahedron().triangulate();
        assert_eq!(tri.triangle_count(), 4);
        assert_eq!(tri.positions.len(), 12);
        let bottom = tri.normals[0];
        assert!((bottom[2] + 1.0).abs() < 1e-12, "{bottom:?}");
    }

    #[test]
    fn ids_display_with_prefixes() {
        assert_eq!(VertexId(3).to_string(), "v3");
        assert_eq!(EdgeId(4).to_string(), "e4");
        assert_eq!(FaceId(5).to_string(), "f5");
    }
}
