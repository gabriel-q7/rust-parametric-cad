//! Render entities derived from document features, plus click selection.
//!
//! Each feature gets one entity tagged with [`CadObject`], tracked in
//! [`ObjectEntityMap`]. When [`DocumentChanged`] names an object, only that
//! entity is created, updated, or despawned; its mesh is regenerated only if
//! the parametric geometry actually changed.

use std::collections::HashMap;

use bevy::picking::events::{Click, Pointer};
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use cad_core::{Document, Feature, Geometry, ObjectId, Tessellation};

use super::convert::{CAD_TO_BEVY, mesh_from_trimesh, transform_from_cad};
use crate::commands::DocumentChanged;
use crate::editor::{CadDocument, EditorSystems, Selection};

/// Links a render entity to its document object.
#[derive(Component, Debug, Clone, Copy)]
pub struct CadObject {
    pub id: ObjectId,
}

/// Geometry the entity's current mesh was generated from.
#[derive(Component, Debug, Clone, Copy)]
struct RenderedGeometry(Geometry);

/// Parent of all CAD objects; converts the CAD Z-up frame to Bevy's Y-up.
#[derive(Component)]
struct CadRoot;

/// Explicit mapping from stable document IDs to runtime entities.
#[derive(Resource, Default, Debug)]
pub struct ObjectEntityMap(HashMap<ObjectId, Entity>);

#[derive(Resource)]
struct ObjectMaterials {
    normal: Handle<StandardMaterial>,
    selected: Handle<StandardMaterial>,
}

pub struct ObjectsPlugin;

impl Plugin for ObjectsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ObjectEntityMap>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (sync_objects, highlight_selection)
                    .chain()
                    .in_set(EditorSystems::Sync),
            )
            .add_observer(select_on_click);
    }
}

fn setup(mut commands: Commands, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.spawn((
        Name::new("CAD root"),
        CadRoot,
        Transform::from_rotation(CAD_TO_BEVY),
        Visibility::default(),
    ));
    commands.insert_resource(ObjectMaterials {
        normal: materials.add(StandardMaterial {
            base_color: Color::srgb(0.62, 0.68, 0.76),
            perceptual_roughness: 0.55,
            metallic: 0.1,
            ..default()
        }),
        selected: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.62, 0.18),
            perceptual_roughness: 0.45,
            metallic: 0.1,
            ..default()
        }),
    });
}

#[allow(clippy::too_many_arguments)]
fn sync_objects(
    mut changes: MessageReader<DocumentChanged>,
    doc: Res<CadDocument>,
    mut map: ResMut<ObjectEntityMap>,
    root: Single<Entity, With<CadRoot>>,
    materials: Res<ObjectMaterials>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut objects: Query<(&mut Transform, &mut Name, &Mesh3d, &mut RenderedGeometry)>,
    mut commands: Commands,
) {
    let mut ids = Vec::new();
    let mut reloaded = false;
    for change in changes.read() {
        reloaded |= change.reloaded;
        ids.extend_from_slice(&change.affected);
    }
    if reloaded {
        ids.extend(map.0.keys().copied());
        ids.extend(doc.features().iter().map(Feature::id));
    }
    ids.sort_unstable();
    ids.dedup();

    let quality = Tessellation::default();
    for id in ids {
        match (doc.feature(id), map.0.get(&id).copied()) {
            (Some(feature), Some(entity)) => {
                let Ok((mut transform, mut name, mesh, mut rendered)) = objects.get_mut(entity)
                else {
                    continue;
                };
                *transform = transform_from_cad(feature.transform(), doc.units());
                if name.as_str() != feature.name() {
                    name.set(feature.name().to_owned());
                }
                if rendered.0 != *feature.geometry() {
                    let tri = feature.geometry().tessellate(&quality);
                    if let Some(mut asset) = meshes.get_mut(&mesh.0) {
                        *asset = mesh_from_trimesh(&tri, doc.units());
                    }
                    rendered.0 = *feature.geometry();
                }
            }
            (Some(feature), None) => {
                let entity = spawn_object(
                    &mut commands,
                    *root,
                    feature,
                    &doc,
                    &quality,
                    &mut meshes,
                    &materials,
                );
                map.0.insert(id, entity);
            }
            (None, Some(entity)) => {
                commands.entity(entity).despawn();
                map.0.remove(&id);
            }
            (None, None) => {}
        }
    }
}

fn spawn_object(
    commands: &mut Commands,
    root: Entity,
    feature: &Feature,
    doc: &Document,
    quality: &Tessellation,
    meshes: &mut Assets<Mesh>,
    materials: &ObjectMaterials,
) -> Entity {
    let tri = feature.geometry().tessellate(quality);
    commands
        .spawn((
            Name::new(feature.name().to_owned()),
            CadObject { id: feature.id() },
            RenderedGeometry(*feature.geometry()),
            Mesh3d(meshes.add(mesh_from_trimesh(&tri, doc.units()))),
            MeshMaterial3d(materials.normal.clone()),
            transform_from_cad(feature.transform(), doc.units()),
            ChildOf(root),
        ))
        .id()
}

fn highlight_selection(
    selection: Res<Selection>,
    materials: Res<ObjectMaterials>,
    mut objects: Query<(&CadObject, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    for (object, mut material) in &mut objects {
        let wanted = if selection.is(object.id) {
            &materials.selected
        } else {
            &materials.normal
        };
        if material.0 != *wanted {
            material.0 = wanted.clone();
        }
    }
}

/// Left-click selects an object; clicking empty viewport space deselects.
/// Clicks on UI panels hit UI nodes instead and are ignored here.
fn select_on_click(
    click: On<Pointer<Click>>,
    objects: Query<&CadObject>,
    windows: Query<(), With<PrimaryWindow>>,
    mut selection: ResMut<Selection>,
) {
    if click.button != PointerButton::Primary {
        return;
    }
    let target = click.event_target();
    if let Ok(object) = objects.get(target) {
        selection.select(object.id);
    } else if windows.contains(target) {
        selection.clear();
    }
}
