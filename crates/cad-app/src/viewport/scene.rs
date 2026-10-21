//! Static scene dressing: background, lights, ground grid, and world axes.
//!
//! The grid lies on the CAD XY plane (Bevy's XZ plane). Axis colors follow
//! the CAD convention: X red, Y green, Z (up) blue.

use std::f32::consts::FRAC_PI_2;

use bevy::color::palettes::css;
use bevy::prelude::*;

use super::convert::cad_direction;

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(Color::srgb(0.17, 0.18, 0.20)))
            .insert_resource(GlobalAmbientLight {
                color: Color::WHITE,
                brightness: 350.0,
                ..default()
            })
            .add_systems(Startup, spawn_lights)
            .add_systems(Update, draw_grid_and_axes);
    }
}

fn spawn_lights(mut commands: Commands) {
    commands.spawn((
        Name::new("Key light"),
        DirectionalLight {
            illuminance: 9_000.0,
            ..default()
        },
        Transform::from_xyz(2.0, 4.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        Name::new("Fill light"),
        DirectionalLight {
            illuminance: 2_500.0,
            ..default()
        },
        Transform::from_xyz(-3.0, 1.5, -2.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

/// Grid spacing in meters: 10 mm minor cells near the origin, 100 mm major
/// cells over a wider area.
const MINOR_CELL: f32 = 0.01;
const MINOR_EXTENT: f32 = 0.6;
const MAJOR_CELL: f32 = 0.1;
const MAJOR_EXTENT: f32 = 3.0;
const AXIS_LENGTH: f32 = 0.25;

fn draw_grid_and_axes(mut gizmos: Gizmos) {
    // Gizmo grids are drawn in their local XY plane; rotate it onto Bevy's XZ.
    let ground = Isometry3d::from_rotation(Quat::from_rotation_x(FRAC_PI_2));
    let cells = |extent: f32, size: f32| UVec2::splat((extent / size).round() as u32);
    gizmos.grid(
        ground,
        cells(MINOR_EXTENT, MINOR_CELL),
        Vec2::splat(MINOR_CELL),
        Color::srgba(1.0, 1.0, 1.0, 0.04),
    );
    gizmos.grid(
        ground,
        cells(MAJOR_EXTENT, MAJOR_CELL),
        Vec2::splat(MAJOR_CELL),
        Color::srgba(1.0, 1.0, 1.0, 0.14),
    );
    for (axis, color) in [
        ([1.0, 0.0, 0.0], css::RED),
        ([0.0, 1.0, 0.0], css::LIME),
        ([0.0, 0.0, 1.0], css::DODGER_BLUE),
    ] {
        gizmos.line(Vec3::ZERO, cad_direction(axis) * AXIS_LENGTH, color);
    }
}
