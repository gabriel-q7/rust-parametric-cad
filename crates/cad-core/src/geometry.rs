//! Primitive geometry definitions and platform-independent tessellation.
//!
//! [`Geometry`] is the parametric source of truth. [`TriMesh`] is *derived*
//! data, generated on demand by [`Geometry::tessellate`], in document units
//! (millimeters) and in the document's Z-up frame. Renderers convert a
//! `TriMesh` into their own mesh type; a future B-rep kernel can replace the
//! tessellator without changing the document model.

use std::f64::consts::{PI, TAU};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::parameters::{BoxParameters, CylinderParameters, ParameterDescriptor, SphereParameters};
use crate::validation::ValidationError;

/// The kind of a primitive, without its parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimitiveKind {
    Box,
    Sphere,
    Cylinder,
}

impl PrimitiveKind {
    pub const ALL: [PrimitiveKind; 3] = [
        PrimitiveKind::Box,
        PrimitiveKind::Sphere,
        PrimitiveKind::Cylinder,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PrimitiveKind::Box => "Box",
            PrimitiveKind::Sphere => "Sphere",
            PrimitiveKind::Cylinder => "Cylinder",
        }
    }

    /// Geometry of this kind with default dimensions.
    pub fn default_geometry(self) -> Geometry {
        match self {
            PrimitiveKind::Box => Geometry::Box(BoxParameters::default()),
            PrimitiveKind::Sphere => Geometry::Sphere(SphereParameters::default()),
            PrimitiveKind::Cylinder => Geometry::Cylinder(CylinderParameters::default()),
        }
    }
}

impl fmt::Display for PrimitiveKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Parametric geometry of a feature.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Geometry {
    Box(BoxParameters),
    Sphere(SphereParameters),
    Cylinder(CylinderParameters),
}

/// Error returned when editing a parameter by key.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ParameterError {
    #[error("{kind} has no parameter named '{key}'")]
    UnknownParameter { kind: PrimitiveKind, key: String },
    #[error(transparent)]
    Invalid(#[from] ValidationError),
}

impl Geometry {
    pub fn kind(&self) -> PrimitiveKind {
        match self {
            Geometry::Box(_) => PrimitiveKind::Box,
            Geometry::Sphere(_) => PrimitiveKind::Sphere,
            Geometry::Cylinder(_) => PrimitiveKind::Cylinder,
        }
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        match self {
            Geometry::Box(p) => p.validate(),
            Geometry::Sphere(p) => p.validate(),
            Geometry::Cylinder(p) => p.validate(),
        }
    }

    /// Editable parameters of this geometry, in display order.
    pub fn parameters(&self) -> Vec<ParameterDescriptor> {
        match self {
            Geometry::Box(p) => p.descriptors(),
            Geometry::Sphere(p) => p.descriptors(),
            Geometry::Cylinder(p) => p.descriptors(),
        }
    }

    /// Returns a copy with the given parameters replaced. All values are
    /// validated; on error `self` is unaffected.
    pub fn with_parameters(&self, values: &[(&str, f64)]) -> Result<Geometry, ParameterError> {
        let mut current: Vec<ParameterDescriptor> = self.parameters();
        for &(key, value) in values {
            let slot = current.iter_mut().find(|d| d.key == key).ok_or_else(|| {
                ParameterError::UnknownParameter {
                    kind: self.kind(),
                    key: key.to_owned(),
                }
            })?;
            slot.value = value;
        }
        let get = |key: &str| {
            current
                .iter()
                .find(|d| d.key == key)
                .map(|d| d.value)
                .expect("descriptor keys are fixed per kind")
        };
        let geometry = match self {
            Geometry::Box(_) => Geometry::Box(BoxParameters::new(
                get("width"),
                get("depth"),
                get("height"),
            )?),
            Geometry::Sphere(_) => Geometry::Sphere(SphereParameters::new(get("radius"))?),
            Geometry::Cylinder(_) => {
                Geometry::Cylinder(CylinderParameters::new(get("radius"), get("height"))?)
            }
        };
        Ok(geometry)
    }

    /// Generates a triangle mesh with the given tessellation quality.
    pub fn tessellate(&self, quality: &Tessellation) -> TriMesh {
        match self {
            Geometry::Box(p) => tessellate_box(p),
            Geometry::Sphere(p) => tessellate_sphere(p, quality),
            Geometry::Cylinder(p) => tessellate_cylinder(p, quality),
        }
    }
}

/// Tessellation quality settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tessellation {
    /// Number of segments around a circle.
    pub radial_segments: u32,
    /// Number of latitude bands on a sphere.
    pub sphere_rings: u32,
}

impl Default for Tessellation {
    fn default() -> Self {
        Self {
            radial_segments: 48,
            sphere_rings: 24,
        }
    }
}

/// Platform-independent indexed triangle mesh, counter-clockwise front faces,
/// in millimeters in the document's Z-up frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TriMesh {
    pub positions: Vec<[f64; 3]>,
    pub normals: Vec<[f64; 3]>,
    pub indices: Vec<u32>,
}

impl TriMesh {
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Axis-aligned bounds `(min, max)`, or `None` for an empty mesh.
    pub fn bounds(&self) -> Option<([f64; 3], [f64; 3])> {
        let first = *self.positions.first()?;
        Some(
            self.positions
                .iter()
                .fold((first, first), |(mut lo, mut hi), p| {
                    for i in 0..3 {
                        lo[i] = lo[i].min(p[i]);
                        hi[i] = hi[i].max(p[i]);
                    }
                    (lo, hi)
                }),
        )
    }

    fn push_vertex(&mut self, position: [f64; 3], normal: [f64; 3]) -> u32 {
        let index = u32::try_from(self.positions.len()).expect("mesh exceeds u32 vertices");
        self.positions.push(position);
        self.normals.push(normal);
        index
    }

    fn push_triangle(&mut self, a: u32, b: u32, c: u32) {
        self.indices.extend_from_slice(&[a, b, c]);
    }
}

fn tessellate_box(p: &BoxParameters) -> TriMesh {
    let (hx, hy, h) = (p.width() / 2.0, p.depth() / 2.0, p.height());
    let center = [0.0, 0.0, h / 2.0];
    let half = [hx, hy, h / 2.0];
    // (normal axis, sign, u axis, v axis) with u x v = normal.
    let faces: [([f64; 3], [f64; 3], [f64; 3]); 6] = [
        ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        ([-1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0]),
        ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]),
    ];
    let mut mesh = TriMesh::default();
    for (n, u, v) in faces {
        let corner = |su: f64, sv: f64| {
            std::array::from_fn(|i| center[i] + (n[i] + su * u[i] + sv * v[i]) * half[i])
        };
        let a = mesh.push_vertex(corner(-1.0, -1.0), n);
        let b = mesh.push_vertex(corner(1.0, -1.0), n);
        let c = mesh.push_vertex(corner(1.0, 1.0), n);
        let d = mesh.push_vertex(corner(-1.0, 1.0), n);
        mesh.push_triangle(a, b, c);
        mesh.push_triangle(a, c, d);
    }
    mesh
}

fn tessellate_sphere(p: &SphereParameters, quality: &Tessellation) -> TriMesh {
    let r = p.radius();
    let segments = quality.radial_segments.max(3);
    let rings = quality.sphere_rings.max(2);
    let mut mesh = TriMesh::default();
    // Grid of (rings + 1) x (segments + 1) vertices; theta is the polar angle
    // from +Z, phi the azimuth from +X towards +Y.
    for i in 0..=rings {
        let theta = PI * f64::from(i) / f64::from(rings);
        for j in 0..=segments {
            let phi = TAU * f64::from(j) / f64::from(segments);
            let n = [
                theta.sin() * phi.cos(),
                theta.sin() * phi.sin(),
                theta.cos(),
            ];
            mesh.push_vertex([r * n[0], r * n[1], r + r * n[2]], n);
        }
    }
    let row = segments + 1;
    for i in 0..rings {
        for j in 0..segments {
            let a = i * row + j;
            let b = (i + 1) * row + j;
            let c = b + 1;
            let d = a + 1;
            // Skip the degenerate triangles that touch a pole.
            if i != rings - 1 {
                mesh.push_triangle(a, b, c);
            }
            if i != 0 {
                mesh.push_triangle(a, c, d);
            }
        }
    }
    mesh
}

fn tessellate_cylinder(p: &CylinderParameters, quality: &Tessellation) -> TriMesh {
    let (r, h) = (p.radius(), p.height());
    let segments = quality.radial_segments.max(3);
    let mut mesh = TriMesh::default();
    let angle = |j: u32| TAU * f64::from(j) / f64::from(segments);

    // Side wall with smooth normals.
    let side_start = u32::try_from(mesh.positions.len()).expect("small mesh");
    for j in 0..=segments {
        let (s, c) = angle(j).sin_cos();
        let n = [c, s, 0.0];
        mesh.push_vertex([r * c, r * s, 0.0], n);
        mesh.push_vertex([r * c, r * s, h], n);
    }
    for j in 0..segments {
        let a = side_start + 2 * j;
        let b = a + 2;
        let c = b + 1;
        let d = a + 1;
        mesh.push_triangle(a, b, c);
        mesh.push_triangle(a, c, d);
    }

    // Flat caps.
    for (z, nz) in [(h, 1.0), (0.0, -1.0)] {
        let n = [0.0, 0.0, nz];
        let center = mesh.push_vertex([0.0, 0.0, z], n);
        let ring_start = center + 1;
        for j in 0..=segments {
            let (s, c) = angle(j).sin_cos();
            mesh.push_vertex([r * c, r * s, z], n);
        }
        for j in 0..segments {
            let (a, b) = (ring_start + j, ring_start + j + 1);
            if nz > 0.0 {
                mesh.push_triangle(center, a, b);
            } else {
                mesh.push_triangle(center, b, a);
            }
        }
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
        [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
    }

    fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    }

    fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }

    fn assert_close(actual: [f64; 3], expected: [f64; 3]) {
        for i in 0..3 {
            assert!(
                (actual[i] - expected[i]).abs() < 1e-9,
                "{actual:?} != {expected:?}"
            );
        }
    }

    /// Every triangle must be non-degenerate and wound counter-clockwise
    /// with respect to its vertex normals (i.e. facing outwards).
    fn assert_well_formed(mesh: &TriMesh) {
        assert_eq!(mesh.positions.len(), mesh.normals.len());
        assert_eq!(mesh.indices.len() % 3, 0);
        assert!(mesh.triangle_count() > 0);
        for tri in mesh.indices.chunks_exact(3) {
            let [a, b, c] = [tri[0], tri[1], tri[2]].map(|i| mesh.positions[i as usize]);
            let face = cross(sub(b, a), sub(c, a));
            assert!(dot(face, face) > 0.0, "degenerate triangle {tri:?}");
            let n = tri.iter().fold([0.0; 3], |acc, &i| {
                let n = mesh.normals[i as usize];
                [acc[0] + n[0], acc[1] + n[1], acc[2] + n[2]]
            });
            assert!(dot(face, n) > 0.0, "triangle {tri:?} faces inwards");
        }
    }

    #[test]
    fn box_mesh_matches_dimensions() {
        let mesh = Geometry::Box(BoxParameters::new(10.0, 20.0, 30.0).unwrap())
            .tessellate(&Tessellation::default());
        assert_well_formed(&mesh);
        assert_eq!(mesh.triangle_count(), 12);
        let (lo, hi) = mesh.bounds().unwrap();
        assert_close(lo, [-5.0, -10.0, 0.0]);
        assert_close(hi, [5.0, 10.0, 30.0]);
    }

    #[test]
    fn sphere_mesh_matches_radius() {
        let mesh = Geometry::Sphere(SphereParameters::new(7.0).unwrap())
            .tessellate(&Tessellation::default());
        assert_well_formed(&mesh);
        let (lo, hi) = mesh.bounds().unwrap();
        assert_close(lo, [-7.0, -7.0, 0.0]);
        assert_close(hi, [7.0, 7.0, 14.0]);
        for p in &mesh.positions {
            let d = sub(*p, [0.0, 0.0, 7.0]);
            assert!((dot(d, d).sqrt() - 7.0).abs() < 1e-9);
        }
    }

    #[test]
    fn cylinder_mesh_matches_dimensions() {
        let mesh = Geometry::Cylinder(CylinderParameters::new(4.0, 9.0).unwrap())
            .tessellate(&Tessellation::default());
        assert_well_formed(&mesh);
        let (lo, hi) = mesh.bounds().unwrap();
        assert_close(lo, [-4.0, -4.0, 0.0]);
        assert_close(hi, [4.0, 4.0, 9.0]);
    }

    #[test]
    fn coarse_tessellation_is_clamped_to_a_valid_minimum() {
        let quality = Tessellation {
            radial_segments: 0,
            sphere_rings: 0,
        };
        assert_well_formed(&Geometry::Sphere(SphereParameters::default()).tessellate(&quality));
        assert_well_formed(&Geometry::Cylinder(CylinderParameters::default()).tessellate(&quality));
    }

    #[test]
    fn with_parameters_updates_by_key_and_validates() {
        let geometry = Geometry::Box(BoxParameters::new(1.0, 2.0, 3.0).unwrap());
        let updated = geometry.with_parameters(&[("depth", 8.0)]).unwrap();
        assert_eq!(
            updated,
            Geometry::Box(BoxParameters::new(1.0, 8.0, 3.0).unwrap())
        );
        assert!(matches!(
            geometry.with_parameters(&[("depth", 0.0)]),
            Err(ParameterError::Invalid(_))
        ));
        assert!(matches!(
            geometry.with_parameters(&[("radius", 1.0)]),
            Err(ParameterError::UnknownParameter { .. })
        ));
    }

    #[test]
    fn geometry_serializes_with_type_tag() {
        let json =
            serde_json::to_value(Geometry::Sphere(SphereParameters::new(2.5).unwrap())).unwrap();
        assert_eq!(json, serde_json::json!({ "type": "sphere", "radius": 2.5 }));
    }
}
