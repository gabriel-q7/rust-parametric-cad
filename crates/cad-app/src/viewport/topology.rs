//! Topology overlay and viewport picking for editable meshes.
//!
//! When the selected object is an editable mesh, its edges are drawn as an
//! x-ray wireframe (always on top), and in vertex/edge/face mode a left click
//! picks the element under the cursor:
//!
//! - vertices: nearest projected vertex within [`VERTEX_PICK_RADIUS`] pixels;
//! - edges: nearest projected segment within [`EDGE_PICK_DISTANCE`] pixels;
//! - faces: nearest face hit by the cursor ray (`EditableMesh::pick_face`).
//!
//! Vertex and edge picking ignore occlusion, matching the x-ray overlay, so
//! hidden elements can be picked too. Ties prefer the element nearer to the
//! camera.

use bevy::color::palettes::css;
use bevy::math::Affine3A;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use cad_core::{EditableMesh, LengthUnit};

use super::PointerOverUi;
use super::objects::{CadObject, ObjectEntityMap};
use crate::editor::{CadDocument, EditMode, EditorSystems, Selection, TopologyElement};

/// Screen-space vertex picking radius, in logical pixels.
pub const VERTEX_PICK_RADIUS: f32 = 12.0;
/// Screen-space edge picking distance, in logical pixels.
pub const EDGE_PICK_DISTANCE: f32 = 8.0;

/// Gizmo group for the topology overlay, drawn on top of all geometry.
#[derive(Default, Reflect, GizmoConfigGroup)]
struct TopologyGizmos;

pub struct TopologyPlugin;

impl Plugin for TopologyPlugin {
    fn build(&self, app: &mut App) {
        app.init_gizmo_group::<TopologyGizmos>()
            .add_systems(Startup, configure_gizmos)
            .add_systems(
                Update,
                (
                    pick_topology.in_set(EditorSystems::Input),
                    draw_topology.after(EditorSystems::Sync),
                ),
            );
    }
}

fn configure_gizmos(mut store: ResMut<GizmoConfigStore>) {
    let (config, _) = store.config_mut::<TopologyGizmos>();
    // -1 draws in front of everything: an x-ray overlay.
    config.depth_bias = -1.0;
    config.line.width = 2.0;
}

/// Maps an object's local CAD coordinates (mm) to Bevy world space and back.
struct LocalFrame {
    to_world: Affine3A,
    unit: LengthUnit,
}

impl LocalFrame {
    fn new(object: &GlobalTransform, unit: LengthUnit) -> Self {
        Self {
            to_world: object.affine(),
            unit,
        }
    }

    fn world(&self, p: [f64; 3]) -> Vec3 {
        let meters = Vec3::from_array(p.map(|c| self.unit.to_meters(c) as f32));
        self.to_world.transform_point3(meters)
    }

    fn local(&self, p: Vec3) -> [f64; 3] {
        let meters = self.to_world.inverse().transform_point3(p);
        let per_meter = self.unit.to_meters(1.0);
        meters.to_array().map(|c| f64::from(c) / per_meter)
    }
}

/// Distance from `p` to the segment `a`-`b` (2D).
pub fn distance_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = if ab.length_squared() > 0.0 {
        ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
    } else {
        0.0
    };
    p.distance(a + ab * t)
}

/// Picks the candidate with the smallest screen distance within `max`,
/// preferring the one nearer to the camera when distances are similar.
/// Candidates are `(item, screen distance, depth)`.
pub fn pick_nearest<T>(candidates: impl IntoIterator<Item = (T, f32, f32)>, max: f32) -> Option<T> {
    candidates
        .into_iter()
        .filter(|(_, distance, _)| *distance <= max)
        .min_by(|(_, da, za), (_, db, zb)| {
            // Bucket screen distance to 2 px so near-coincident projections
            // (front and back of a mesh) are decided by depth.
            let bucket = |d: f32| (d / 2.0).floor();
            bucket(*da).total_cmp(&bucket(*db)).then(za.total_cmp(zb))
        })
        .map(|(item, _, _)| item)
}

#[allow(clippy::too_many_arguments)]
fn pick_topology(
    mouse: Res<ButtonInput<MouseButton>>,
    over_ui: PointerOverUi,
    window: Single<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    objects: Query<&GlobalTransform, With<CadObject>>,
    map: Res<ObjectEntityMap>,
    doc: Res<CadDocument>,
    mut selection: ResMut<Selection>,
) {
    let mode = selection.mode();
    if mode == EditMode::Object || !mouse.just_pressed(MouseButton::Left) || over_ui.get() {
        return;
    }
    let Some(id) = selection.get() else { return };
    let Some(editable) = doc.feature(id).and_then(|f| f.geometry().as_editable()) else {
        return;
    };
    let (Some(cursor), Ok((camera, camera_transform))) =
        (window.cursor_position(), cameras.single())
    else {
        return;
    };
    let Some(object) = map.get(id).and_then(|e| objects.get(e).ok()) else {
        return;
    };
    let frame = LocalFrame::new(object, doc.units());
    let mesh = &editable.mesh;
    let eye = camera_transform.translation();
    let project = |p: [f64; 3]| {
        let world = frame.world(p);
        camera
            .world_to_viewport(camera_transform, world)
            .ok()
            .map(|screen| (screen, world.distance(eye)))
    };

    let picked = match mode {
        EditMode::Object => None,
        EditMode::Vertex => pick_nearest(
            mesh.vertices().iter().filter_map(|v| {
                let (screen, depth) = project(v.position)?;
                Some((
                    TopologyElement::Vertex(v.id),
                    screen.distance(cursor),
                    depth,
                ))
            }),
            VERTEX_PICK_RADIUS,
        ),
        EditMode::Edge => pick_nearest(
            mesh.edges().iter().filter_map(|e| {
                let [a, b] = e.vertices.map(|v| mesh.position(v).and_then(project));
                let ((sa, za), (sb, zb)) = (a?, b?);
                let distance = distance_to_segment(cursor, sa, sb);
                Some((TopologyElement::Edge(e.id), distance, (za + zb) / 2.0))
            }),
            EDGE_PICK_DISTANCE,
        ),
        EditMode::Face => camera
            .viewport_to_world(camera_transform, cursor)
            .ok()
            .and_then(|ray| {
                let origin = frame.local(ray.origin);
                let ahead = frame.local(ray.origin + *ray.direction);
                let direction = [0, 1, 2].map(|i| ahead[i] - origin[i]);
                mesh.pick_face(origin, direction)
            })
            .map(|(face, _)| TopologyElement::Face(face)),
    };
    match picked {
        Some(element) => selection.select_element(element),
        None => selection.clear_element(),
    }
}

fn draw_topology(
    selection: Res<Selection>,
    doc: Res<CadDocument>,
    map: Res<ObjectEntityMap>,
    objects: Query<&GlobalTransform, With<CadObject>>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    mut gizmos: Gizmos<TopologyGizmos>,
) {
    let Some(id) = selection.get() else { return };
    let Some(editable) = doc.feature(id).and_then(|f| f.geometry().as_editable()) else {
        return;
    };
    let (Some(object), Ok(camera)) = (
        map.get(id).and_then(|e| objects.get(e).ok()),
        cameras.single(),
    ) else {
        return;
    };
    let frame = LocalFrame::new(object, doc.units());
    let mesh = &editable.mesh;
    let mode = selection.mode();
    // Marker sizes scale with viewing distance so they stay readable.
    let scale = camera
        .translation()
        .distance(object.translation())
        .max(0.01);

    let edge_color = if mode == EditMode::Object {
        Color::srgba(0.4, 0.85, 1.0, 0.25)
    } else {
        Color::srgba(0.4, 0.85, 1.0, 0.6)
    };
    for edge in mesh.edges() {
        if let [Some(a), Some(b)] = edge.vertices.map(|v| mesh.position(v)) {
            gizmos.line(frame.world(a), frame.world(b), edge_color);
        }
    }
    if mode == EditMode::Vertex {
        for vertex in mesh.vertices() {
            gizmos.sphere(
                Isometry3d::from_translation(frame.world(vertex.position)),
                scale * 0.004,
                Color::srgba(0.9, 0.95, 1.0, 0.8),
            );
        }
    }

    let highlight = css::YELLOW;
    match selection.element() {
        Some(TopologyElement::Vertex(v)) => {
            if let Some(p) = mesh.position(v) {
                gizmos.sphere(
                    Isometry3d::from_translation(frame.world(p)),
                    scale * 0.009,
                    highlight,
                );
            }
        }
        Some(TopologyElement::Edge(e)) => {
            if let Some(edge) = mesh.edge(e)
                && let [Some(a), Some(b)] = edge.vertices.map(|v| mesh.position(v))
            {
                let (a, b) = (frame.world(a), frame.world(b));
                gizmos.line(a, b, highlight);
                gizmos.sphere(Isometry3d::from_translation(a), scale * 0.005, highlight);
                gizmos.sphere(Isometry3d::from_translation(b), scale * 0.005, highlight);
            }
        }
        Some(TopologyElement::Face(f)) => draw_face(&mut gizmos, &frame, mesh, f, scale, highlight),
        None => {}
    }
}

fn draw_face(
    gizmos: &mut Gizmos<TopologyGizmos>,
    frame: &LocalFrame,
    mesh: &EditableMesh,
    face: cad_core::FaceId,
    scale: f32,
    color: impl Into<Color> + Copy,
) {
    let Some(face_data) = mesh.face(face) else {
        return;
    };
    let points: Vec<[f64; 3]> = face_data
        .vertices
        .iter()
        .filter_map(|&v| mesh.position(v))
        .collect();
    let world: Vec<Vec3> = points.iter().map(|&p| frame.world(p)).collect();
    gizmos.linestrip(world.iter().copied().chain(world.first().copied()), color);
    // Thin cross-hatch from the centroid makes the selected face obvious.
    let n = points.len() as f64;
    let centroid = points
        .iter()
        .fold([0.0; 3], |acc, p| {
            [acc[0] + p[0], acc[1] + p[1], acc[2] + p[2]]
        })
        .map(|c| c / n);
    let center = frame.world(centroid);
    for corner in &world {
        gizmos.line(center, center.lerp(*corner, 0.6), color);
    }
    if let Some(normal) = mesh.face_normal(face) {
        let tip_local = [0, 1, 2].map(|i| centroid[i] + normal[i]);
        let direction = (frame.world(tip_local) - center).normalize_or_zero();
        gizmos.arrow(center, center + direction * scale * 0.12, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_distance_handles_interior_and_endpoints() {
        let (a, b) = (Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0));
        assert_eq!(distance_to_segment(Vec2::new(5.0, 3.0), a, b), 3.0);
        assert_eq!(distance_to_segment(Vec2::new(-4.0, 3.0), a, b), 5.0);
        assert_eq!(distance_to_segment(Vec2::new(1.0, 1.0), a, a), 2f32.sqrt());
    }

    #[test]
    fn nearest_pick_prefers_closer_on_screen_then_closer_to_camera() {
        let picked = pick_nearest([("far", 5.0, 1.0), ("near", 9.0, 0.5)], 12.0);
        assert_eq!(picked, Some("far"), "screen distance wins");
        let picked = pick_nearest([("back", 4.2, 2.0), ("front", 4.9, 1.0)], 12.0);
        assert_eq!(picked, Some("front"), "same 2px bucket: depth wins");
        assert_eq!(pick_nearest([("x", 20.0, 1.0)], 12.0), None);
    }

    #[test]
    fn local_frame_round_trips_millimeters() {
        let object = GlobalTransform::from(
            Transform::from_xyz(1.0, 2.0, 3.0).with_rotation(Quat::from_rotation_x(-1.0)),
        );
        let frame = LocalFrame::new(&object, LengthUnit::Millimeter);
        let p = [100.0, -50.0, 25.0];
        let back = frame.local(frame.world(p));
        for i in 0..3 {
            assert!((back[i] - p[i]).abs() < 1e-3, "{back:?}");
        }
    }
}
