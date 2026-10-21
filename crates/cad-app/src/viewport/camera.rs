//! Orbit camera: right-drag orbits, middle-drag (or Shift + right-drag)
//! pans, the wheel zooms, and Home or the toolbar button resets the view.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};

use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use super::PointerOverUi;
use crate::editor::EditorSystems;

/// Request to restore the initial camera view.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct ResetCamera;

/// Spherical camera state around a focus point, in Bevy world space (meters, Y-up).
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct OrbitCamera {
    pub focus: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub radius: f32,
}

impl Default for OrbitCamera {
    /// Isometric-style view looking at the origin from the +X/-Y/+Z CAD octant.
    fn default() -> Self {
        Self {
            focus: Vec3::new(0.0, 0.05, 0.0),
            yaw: FRAC_PI_4,
            pitch: 0.55,
            radius: 0.9,
        }
    }
}

impl OrbitCamera {
    const MIN_RADIUS: f32 = 0.005;
    const MAX_RADIUS: f32 = 200.0;
    const ORBIT_SPEED: f32 = 0.008;
    const ZOOM_STEP: f32 = 0.12;

    pub fn transform(&self) -> Transform {
        let offset = Vec3::new(
            self.pitch.cos() * self.yaw.sin(),
            self.pitch.sin(),
            self.pitch.cos() * self.yaw.cos(),
        ) * self.radius;
        Transform::from_translation(self.focus + offset).looking_at(self.focus, Vec3::Y)
    }

    fn orbit(&mut self, delta: Vec2) {
        self.yaw -= delta.x * Self::ORBIT_SPEED;
        self.pitch =
            (self.pitch + delta.y * Self::ORBIT_SPEED).clamp(-FRAC_PI_2 + 0.01, FRAC_PI_2 - 0.01);
    }

    /// Moves the focus so the scene follows the cursor.
    fn pan(&mut self, delta: Vec2, view: &Transform, fov: f32, viewport_height: f32) {
        let world_per_pixel = 2.0 * self.radius * (fov / 2.0).tan() / viewport_height.max(1.0);
        self.focus += (-view.right() * delta.x + view.up() * delta.y) * world_per_pixel;
    }

    fn zoom(&mut self, steps: f32) {
        self.radius = (self.radius * (1.0 - Self::ZOOM_STEP).powf(steps))
            .clamp(Self::MIN_RADIUS, Self::MAX_RADIUS);
    }
}

pub struct OrbitCameraPlugin;

impl Plugin for OrbitCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ResetCamera>()
            .add_systems(Startup, spawn_camera)
            .add_systems(
                Update,
                (
                    orbit_camera_input.in_set(EditorSystems::Input),
                    reset_camera,
                )
                    .chain(),
            );
    }
}

fn spawn_camera(mut commands: Commands) {
    let orbit = OrbitCamera::default();
    commands.spawn((
        Name::new("Viewport camera"),
        Camera3d::default(),
        Projection::from(PerspectiveProjection {
            // CAD parts are millimeter-scale; keep the near plane close.
            near: 0.001,
            far: 1000.0,
            ..default()
        }),
        orbit.transform(),
        orbit,
    ));
}

/// Which mouse drag is in progress. A drag only starts over the viewport,
/// but continues even if the cursor passes over a panel.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum Drag {
    #[default]
    None,
    Orbit,
    Pan,
}

/// Drag deltas come from the cursor position rather than raw device motion,
/// so panning tracks the cursor exactly regardless of mouse acceleration.
#[allow(clippy::too_many_arguments)]
fn orbit_camera_input(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    scroll: Res<AccumulatedMouseScroll>,
    over_ui: PointerOverUi,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut drag: Local<Drag>,
    mut last_cursor: Local<Option<Vec2>>,
    mut cameras: Query<(&mut OrbitCamera, &mut Transform, &Projection)>,
) {
    let Ok((mut orbit, mut transform, projection)) = cameras.single_mut() else {
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };
    let cursor = window.cursor_position();
    let delta = match (cursor, *last_cursor) {
        (Some(now), Some(before)) => now - before,
        _ => Vec2::ZERO,
    };
    *last_cursor = cursor;

    let pointer_over_ui = over_ui.get();
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    if !pointer_over_ui {
        if mouse.just_pressed(MouseButton::Middle)
            || (mouse.just_pressed(MouseButton::Right) && shift)
        {
            *drag = Drag::Pan;
        } else if mouse.just_pressed(MouseButton::Right) {
            *drag = Drag::Orbit;
        }
    }
    if !mouse.any_pressed([MouseButton::Right, MouseButton::Middle]) {
        *drag = Drag::None;
    }

    let mut next = *orbit;
    match *drag {
        Drag::Orbit => next.orbit(delta),
        Drag::Pan => {
            let fov = match projection {
                Projection::Perspective(p) => p.fov,
                _ => FRAC_PI_4,
            };
            next.pan(delta, &transform, fov, window.height());
        }
        Drag::None => {}
    }
    if !pointer_over_ui && scroll.delta.y != 0.0 {
        let steps = match scroll.unit {
            MouseScrollUnit::Line => scroll.delta.y,
            MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
        };
        next.zoom(steps);
    }

    if next != *orbit {
        *orbit = next;
        *transform = orbit.transform();
    }
}

fn reset_camera(
    mut requests: MessageReader<ResetCamera>,
    mut cameras: Query<(&mut OrbitCamera, &mut Transform)>,
) {
    if requests.read().count() == 0 {
        return;
    }
    for (mut orbit, mut transform) in &mut cameras {
        *orbit = OrbitCamera::default();
        *transform = orbit.transform();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_looks_at_focus_from_radius() {
        let orbit = OrbitCamera::default();
        let t = orbit.transform();
        assert!((t.translation.distance(orbit.focus) - orbit.radius).abs() < 1e-5);
        let to_focus = (orbit.focus - t.translation).normalize();
        assert!(t.forward().dot(to_focus) > 0.9999);
    }

    #[test]
    fn zoom_and_pitch_are_clamped() {
        let mut orbit = OrbitCamera::default();
        orbit.zoom(10_000.0);
        assert_eq!(orbit.radius, OrbitCamera::MIN_RADIUS);
        orbit.orbit(Vec2::new(0.0, 1.0e6));
        assert!(orbit.pitch < FRAC_PI_2);
    }
}
