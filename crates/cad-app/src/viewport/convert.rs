//! Boundary between `cad_core` data and Bevy render types.
//!
//! The document uses millimeters, `f64`, and a Z-up frame. Bevy uses meters,
//! `f32`, and a Y-up frame. Lengths are converted to meters here; the frame
//! change is a single rotation on the [`CAD_TO_BEVY`] root entity that all
//! CAD objects are parented to, so object transforms stay in CAD terms.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use cad_core::{LengthUnit, Transform3D, TriMesh};

/// Rotation taking CAD Z-up coordinates to Bevy Y-up coordinates:
/// CAD (x, y, z) maps to Bevy (x, z, -y).
pub const CAD_TO_BEVY: Quat = Quat::from_xyzw(-0.70710677, 0.0, 0.0, 0.70710677);

/// Direction of a CAD axis in Bevy world space.
pub fn cad_direction(v: [f32; 3]) -> Vec3 {
    CAD_TO_BEVY * Vec3::from_array(v)
}

/// Builds a Bevy mesh (meters, CAD frame) from a core triangle mesh.
pub fn mesh_from_trimesh(mesh: &TriMesh, unit: LengthUnit) -> Mesh {
    let positions: Vec<[f32; 3]> = mesh
        .positions
        .iter()
        .map(|p| p.map(|c| unit.to_meters(c) as f32))
        .collect();
    let normals: Vec<[f32; 3]> = mesh.normals.iter().map(|n| n.map(|c| c as f32)).collect();
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_indices(Indices::U32(mesh.indices.clone()))
}

/// Converts a document transform into a local transform under the CAD root.
pub fn transform_from_cad(transform: &Transform3D, unit: LengthUnit) -> Transform {
    let [x, y, z] = transform.translation.map(|c| unit.to_meters(c) as f32);
    let [rx, ry, rz] = transform.rotation_deg.map(|a| (a as f32).to_radians());
    // Extrinsic X, then Y, then Z == intrinsic Z-Y-X: R = Rz * Ry * Rx.
    Transform::from_xyz(x, y, z).with_rotation(Quat::from_euler(EulerRot::ZYX, rz, ry, rx))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cad_core::{BoxParameters, Geometry, Tessellation};
    use std::f32::consts::FRAC_PI_2;

    #[test]
    fn cad_to_bevy_maps_z_up_to_y_up() {
        assert!(CAD_TO_BEVY.abs_diff_eq(Quat::from_rotation_x(-FRAC_PI_2), 1e-6));
        assert!(cad_direction([0.0, 0.0, 1.0]).abs_diff_eq(Vec3::Y, 1e-6));
        assert!(cad_direction([0.0, 1.0, 0.0]).abs_diff_eq(Vec3::NEG_Z, 1e-6));
        assert!(cad_direction([1.0, 0.0, 0.0]).abs_diff_eq(Vec3::X, 1e-6));
    }

    #[test]
    fn mesh_is_converted_to_meters() {
        let tri = Geometry::Box(BoxParameters::new(100.0, 200.0, 50.0).unwrap())
            .tessellate(&Tessellation::default());
        let mesh = mesh_from_trimesh(&tri, LengthUnit::Millimeter);
        let aabb = mesh.compute_aabb().unwrap();
        assert!(Vec3::from(aabb.half_extents).abs_diff_eq(Vec3::new(0.05, 0.1, 0.025), 1e-6));
        assert_eq!(mesh.indices().unwrap().len(), tri.indices.len());
    }

    #[test]
    fn transform_is_converted_to_meters_and_radians() {
        let t = transform_from_cad(
            &Transform3D {
                translation: [1000.0, -500.0, 20.0],
                rotation_deg: [0.0, 0.0, 90.0],
            },
            LengthUnit::Millimeter,
        );
        assert!(t.translation.abs_diff_eq(Vec3::new(1.0, -0.5, 0.02), 1e-6));
        // 90 degrees about CAD Z turns +X into +Y.
        assert!((t.rotation * Vec3::X).abs_diff_eq(Vec3::Y, 1e-6));
    }
}
