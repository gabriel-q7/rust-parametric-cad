//! 3D viewport: camera, grid/axes, lighting, and render entities derived
//! from the CAD document.

mod camera;
mod convert;
mod objects;
mod scene;
mod topology;

use bevy::ecs::system::SystemParam;
use bevy::picking::hover::HoverMap;
use bevy::picking::mesh_picking::MeshPickingPlugin;
use bevy::picking::pointer::PointerId;
use bevy::prelude::*;

pub use camera::ResetCamera;

pub struct ViewportPlugin;

impl Plugin for ViewportPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            MeshPickingPlugin,
            scene::ScenePlugin,
            camera::OrbitCameraPlugin,
            objects::ObjectsPlugin,
            topology::TopologyPlugin,
        ));
    }
}

/// Answers "is the mouse over a UI panel?" so viewport controls don't react
/// to clicks, drags, or scrolling meant for the UI.
#[derive(SystemParam)]
pub struct PointerOverUi<'w, 's> {
    hover_map: Res<'w, HoverMap>,
    nodes: Query<'w, 's, (), With<Node>>,
}

impl PointerOverUi<'_, '_> {
    pub fn get(&self) -> bool {
        self.hover_map
            .get(&PointerId::Mouse)
            .is_some_and(|hits| hits.keys().any(|&entity| self.nodes.contains(entity)))
    }
}
