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
use crate::editor::{CadDocument, EditMode, EditorSystems, Selection};

/// Links a render entity to its document object.
#[derive(Component, Debug, Clone, Copy)]
pub struct CadObject {
    pub id: ObjectId,
}

/// Geometry the entity's current mesh was generated from.
#[derive(Component, Debug, Clone)]
struct RenderedGeometry(Geometry);

/// Parent of all CAD objects; converts the CAD Z-up frame to Bevy's Y-up.
#[derive(Component)]
struct CadRoot;

/// Explicit mapping from stable document IDs to runtime entities.
#[derive(Resource, Default, Debug)]
pub struct ObjectEntityMap(HashMap<ObjectId, Entity>);

impl ObjectEntityMap {
    pub fn get(&self, id: ObjectId) -> Option<Entity> {
        self.0.get(&id).copied()
    }
}

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
                    rendered.0 = feature.geometry().clone();
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
            RenderedGeometry(feature.geometry().clone()),
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
    // Pointer events bubble up the hierarchy and finally to the window, so
    // only react to the first trigger and look at what was actually hit.
    let target = click.original_event_target();
    if click.button != PointerButton::Primary || click.event_target() != target {
        return;
    }
    // In vertex/edge/face mode, clicks pick topology instead (see `topology`).
    if selection.mode() != EditMode::Object {
        return;
    }
    if let Ok(object) = objects.get(target) {
        selection.select(object.id);
    } else if windows.contains(target) {
        selection.clear();
    }
}

#[cfg(test)]
mod tests {
    //! Headless checks of the document -> entity pipeline (no window or GPU).

    use bevy::asset::AssetPlugin;
    use cad_core::{Command, PrimitiveKind, SphereParameters, Transform3D};

    use super::*;
    use crate::commands::{CommandsPlugin, EditCommand, ProjectCommand};
    use crate::editor::EditorStatePlugin;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .add_plugins((EditorStatePlugin, CommandsPlugin, ObjectsPlugin));
        app.update();
        app
    }

    fn send(app: &mut App, command: Command) {
        app.world_mut().write_message(EditCommand(command));
        app.update();
    }

    fn entity_of(app: &App, id: ObjectId) -> Option<Entity> {
        app.world()
            .resource::<ObjectEntityMap>()
            .0
            .get(&id)
            .copied()
    }

    fn mesh_vertex_count(app: &App, entity: Entity) -> usize {
        let handle = &app.world().get::<Mesh3d>(entity).unwrap().0;
        app.world()
            .resource::<Assets<Mesh>>()
            .get(handle)
            .unwrap()
            .count_vertices()
    }

    #[test]
    fn create_update_delete_keep_entities_in_sync() {
        let mut app = app();
        let create = Command::create(
            "Ball",
            PrimitiveKind::Sphere.default_geometry(),
            Transform3D::default(),
        );
        let Command::CreatePrimitive { id, .. } = create else {
            unreachable!()
        };
        send(&mut app, create);
        let entity = entity_of(&app, id).expect("entity spawned for new object");
        assert_eq!(app.world().get::<CadObject>(entity).unwrap().id, id);
        assert!(
            app.world().resource::<Selection>().is(id),
            "new object is selected"
        );

        // Parameter edit: same entity, regenerated mesh.
        let before = app.world().get::<Mesh3d>(entity).unwrap().0.clone();
        send(
            &mut app,
            Command::UpdateParameters {
                id,
                geometry: Geometry::Sphere(SphereParameters::new(5.0).unwrap()),
            },
        );
        assert_eq!(entity_of(&app, id), Some(entity));
        assert_eq!(app.world().get::<Mesh3d>(entity).unwrap().0, before);
        let rendered = app
            .world()
            .get::<RenderedGeometry>(entity)
            .unwrap()
            .0
            .clone();
        assert_eq!(
            rendered,
            Geometry::Sphere(SphereParameters::new(5.0).unwrap())
        );
        assert!(mesh_vertex_count(&app, entity) > 0);

        // Move: transform converted to meters.
        send(
            &mut app,
            Command::SetObjectTransform {
                id,
                transform: Transform3D::from_translation([250.0, 0.0, 0.0]),
            },
        );
        let translation = app.world().get::<Transform>(entity).unwrap().translation;
        assert!(translation.abs_diff_eq(Vec3::new(0.25, 0.0, 0.0), 1e-6));

        send(&mut app, Command::DeleteObject { id });
        assert_eq!(entity_of(&app, id), None);
        assert!(app.world().get_entity(entity).is_err(), "entity despawned");
        assert_eq!(app.world().resource::<Selection>().get(), None);
    }

    #[test]
    fn invalid_command_changes_nothing_and_reports_error() {
        let mut app = app();
        let create = Command::create(
            "Box",
            PrimitiveKind::Box.default_geometry(),
            Transform3D::default(),
        );
        let Command::CreatePrimitive { id, .. } = create else {
            unreachable!()
        };
        send(&mut app, create);
        let doc_before = app.world().resource::<CadDocument>().0.clone();

        send(
            &mut app,
            Command::RenameObject {
                id,
                name: "   ".into(),
            },
        );
        assert_eq!(app.world().resource::<CadDocument>().0, doc_before);
        let status = app.world().resource::<crate::editor::Status>();
        assert_eq!(status.level, crate::editor::StatusLevel::Error);
    }

    #[test]
    fn new_project_clears_all_entities() {
        let mut app = app();
        for kind in PrimitiveKind::ALL {
            send(
                &mut app,
                Command::create("X", kind.default_geometry(), Transform3D::default()),
            );
        }
        assert_eq!(app.world().resource::<ObjectEntityMap>().0.len(), 3);
        app.world_mut().write_message(ProjectCommand::New);
        app.update();
        assert!(app.world().resource::<ObjectEntityMap>().0.is_empty());
        let mut objects = app.world_mut().query::<&CadObject>();
        assert_eq!(objects.iter(app.world()).count(), 0);
    }

    #[test]
    fn convert_and_extrude_update_the_same_entity_and_select_the_cap() {
        use crate::editor::{EditMode, TopologyElement};

        let mut app = app();
        let create = Command::create(
            "Block",
            PrimitiveKind::Box.default_geometry(),
            Transform3D::from_translation([100.0, 0.0, 0.0]),
        );
        let Command::CreatePrimitive { id, .. } = create else {
            unreachable!()
        };
        send(&mut app, create);
        let entity = entity_of(&app, id).unwrap();
        assert_eq!(mesh_vertex_count(&app, entity), 24, "parametric box");

        send(&mut app, Command::ConvertToEditable { id });
        assert_eq!(entity_of(&app, id), Some(entity), "identity preserved");
        assert_eq!(mesh_vertex_count(&app, entity), 24, "6 flat quads");
        let doc = &app.world().resource::<CadDocument>().0;
        let top = doc
            .feature(id)
            .unwrap()
            .geometry()
            .as_editable()
            .unwrap()
            .mesh
            .faces()[1]
            .id;

        app.world_mut()
            .resource_mut::<Selection>()
            .select_element(TopologyElement::Face(top));
        send(
            &mut app,
            Command::ExtrudeFace {
                id,
                face: top,
                distance: 30.0,
            },
        );
        assert_eq!(entity_of(&app, id), Some(entity));
        // 9 quads + 1 cap, flat-shaded: 10 faces x 4 vertices.
        assert_eq!(mesh_vertex_count(&app, entity), 40);
        let translation = app.world().get::<Transform>(entity).unwrap().translation;
        assert!(
            translation.abs_diff_eq(Vec3::new(0.1, 0.0, 0.0), 1e-6),
            "transform kept"
        );

        let selection = *app.world().resource::<Selection>();
        assert_eq!(selection.mode(), EditMode::Face);
        let Some(TopologyElement::Face(cap)) = selection.element() else {
            panic!("cap face should be selected, got {selection:?}");
        };
        assert_ne!(cap, top, "the extruded face was replaced by a new cap");

        // A failing extrusion keeps the document and reports an error.
        let before = app.world().resource::<CadDocument>().0.clone();
        send(
            &mut app,
            Command::ExtrudeFace {
                id,
                face: top,
                distance: 5.0,
            },
        );
        assert_eq!(app.world().resource::<CadDocument>().0, before);
        assert_eq!(
            app.world().resource::<crate::editor::Status>().level,
            crate::editor::StatusLevel::Error
        );
    }
}
