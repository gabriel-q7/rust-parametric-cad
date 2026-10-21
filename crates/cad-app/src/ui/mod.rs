//! Bevy UI editor shell: toolbar, object list, properties inspector, and
//! status bar. UI systems only read state and emit messages; document
//! changes happen in `crate::commands`.

mod inspector;
mod layout;
mod object_list;
mod shortcuts;
mod widgets;

use std::path::PathBuf;

use bevy::input_focus::tab_navigation::TabNavigationPlugin;
use bevy::prelude::*;
use bevy::text::EditableText;
use cad_core::{Command, Document, ObjectId, PrimitiveKind, Transform3D};

use crate::commands::{EditCommand, ProjectCommand};
use crate::editor::{CadDocument, EditorSystems, Selection, Status, StatusLevel};
use crate::viewport::ResetCamera;
use inspector::InspectorRequest;

/// Default project path, relative to the working directory.
pub const DEFAULT_PROJECT_PATH: &str = "project.cad.json";

/// What a button does when pressed. Also sent as a message.
#[derive(Component, Message, Debug, Clone, Copy, PartialEq)]
pub enum UiAction {
    Create(PrimitiveKind),
    NewProject,
    OpenProject,
    SaveProject,
    ResetCamera,
    Select(ObjectId),
    ApplyProperties,
    RevertProperties,
    DeleteSelected,
}

/// Marks the toolbar text field holding the project file path.
#[derive(Component)]
pub struct ProjectPathField;

/// Marks the status bar text.
#[derive(Component)]
struct StatusText;

pub struct EditorUiPlugin;

impl Plugin for EditorUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TabNavigationPlugin)
            .add_message::<UiAction>()
            .add_message::<InspectorRequest>()
            .add_systems(Startup, layout::spawn_layout)
            .add_systems(
                Update,
                (
                    (
                        collect_button_presses,
                        shortcuts::keyboard_shortcuts,
                        handle_ui_actions,
                        inspector::apply_inspector,
                    )
                        .chain()
                        .in_set(EditorSystems::Input),
                    (
                        object_list::rebuild_object_list,
                        object_list::scroll_object_list,
                        inspector::rebuild_inspector,
                        update_status_bar,
                        widgets::button_visuals,
                        widgets::field_focus_visuals,
                    )
                        .in_set(EditorSystems::Sync),
                ),
            );
    }
}

fn collect_button_presses(
    buttons: Query<(&Interaction, &UiAction), Changed<Interaction>>,
    mut actions: MessageWriter<UiAction>,
) {
    for (interaction, action) in &buttons {
        if *interaction == Interaction::Pressed {
            actions.write(*action);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_ui_actions(
    mut actions: MessageReader<UiAction>,
    doc: Res<CadDocument>,
    mut selection: ResMut<Selection>,
    path_field: Query<&EditableText, With<ProjectPathField>>,
    mut edits: MessageWriter<EditCommand>,
    mut project: MessageWriter<ProjectCommand>,
    mut camera: MessageWriter<ResetCamera>,
    mut inspector: MessageWriter<InspectorRequest>,
    mut status: ResMut<Status>,
) {
    let project_path = || {
        let text = path_field
            .single()
            .map(|field| field.value().to_string())
            .unwrap_or_default();
        let trimmed = text.trim();
        PathBuf::from(if trimmed.is_empty() {
            DEFAULT_PROJECT_PATH
        } else {
            trimmed
        })
    };
    for action in actions.read() {
        match *action {
            UiAction::Create(kind) => {
                edits.write(EditCommand(Command::create(
                    &doc.suggest_name(kind),
                    kind.default_geometry(),
                    Transform3D::from_translation(free_slot(&doc)),
                )));
            }
            UiAction::NewProject => {
                project.write(ProjectCommand::New);
            }
            UiAction::OpenProject => {
                project.write(ProjectCommand::Open(project_path()));
            }
            UiAction::SaveProject => {
                project.write(ProjectCommand::Save(project_path()));
            }
            UiAction::ResetCamera => {
                camera.write(ResetCamera);
            }
            UiAction::Select(id) => selection.select(id),
            UiAction::ApplyProperties => {
                inspector.write(InspectorRequest::Apply);
            }
            UiAction::RevertProperties => {
                inspector.write(InspectorRequest::Revert);
            }
            UiAction::DeleteSelected => match selection.get() {
                Some(id) => {
                    edits.write(EditCommand(Command::DeleteObject { id }));
                }
                None => status.info("Nothing selected to delete"),
            },
        }
    }
}

/// Picks a spot on a 150 mm grid not occupied by another object's origin,
/// so new primitives don't spawn inside existing ones.
fn free_slot(doc: &Document) -> [f64; 3] {
    const SPACING: f64 = 150.0;
    const COLUMNS: i32 = 5;
    (0..)
        .map(|n: i32| {
            let column = n % COLUMNS - COLUMNS / 2;
            let row = n / COLUMNS;
            [f64::from(column) * SPACING, f64::from(row) * SPACING, 0.0]
        })
        .find(|slot| {
            doc.features().iter().all(|f| {
                let t = f.transform().translation;
                (t[0] - slot[0]).hypot(t[1] - slot[1]) > SPACING / 2.0
            })
        })
        .expect("the grid is unbounded")
}

fn update_status_bar(
    status: Res<Status>,
    mut text: Query<(&mut Text, &mut TextColor), With<StatusText>>,
) {
    if !status.is_changed() {
        return;
    }
    let color = match status.level {
        StatusLevel::Info => widgets::TEXT_DIM,
        StatusLevel::Success => widgets::SUCCESS,
        StatusLevel::Error => widgets::ERROR,
    };
    for (mut text, mut text_color) in &mut text {
        text.0.clone_from(&status.message);
        text_color.0 = color;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_slot_avoids_occupied_positions() {
        let mut doc = Document::new();
        let first = free_slot(&doc);
        assert_eq!(first, [-300.0, 0.0, 0.0]);
        doc.apply(Command::create(
            "A",
            PrimitiveKind::Box.default_geometry(),
            Transform3D::from_translation(first),
        ))
        .unwrap();
        assert_eq!(free_slot(&doc), [-150.0, 0.0, 0.0]);
    }
}
