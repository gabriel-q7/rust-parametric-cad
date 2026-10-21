//! Properties inspector: shows the selected object's name, dimensions, and
//! placement as editable fields, and turns "Apply" into one atomic command.

use bevy::prelude::*;
use bevy::text::EditableText;
use cad_core::{Command, Feature, ObjectId, Transform3D};

use super::UiAction;
use super::widgets::{
    ACCENT_BG, DANGER_BG, TEXT, TEXT_DIM, button, colored_button, label, number_field, text_field,
};
use crate::commands::{DocumentChanged, EditCommand};
use crate::editor::{CadDocument, Selection, Status};

#[derive(Component)]
pub struct InspectorPanel;

/// Identifies what an inspector text field edits.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub enum InspectorField {
    Name,
    Parameter {
        key: &'static str,
        label: &'static str,
    },
    Position(usize),
    Rotation(usize),
}

impl InspectorField {
    fn label(self) -> String {
        const AXES: [&str; 3] = ["X", "Y", "Z"];
        match self {
            InspectorField::Name => "Name".into(),
            InspectorField::Parameter { label, .. } => label.into(),
            InspectorField::Position(i) => format!("Position {}", AXES[i]),
            InspectorField::Rotation(i) => format!("Rotation {}", AXES[i]),
        }
    }
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorRequest {
    Apply,
    Revert,
}

/// Rebuilds the inspector when the selection changes, when the selected
/// object changes in the document, or when the user reverts their edits.
pub fn rebuild_inspector(
    mut changes: MessageReader<DocumentChanged>,
    mut requests: MessageReader<InspectorRequest>,
    selection: Res<Selection>,
    doc: Res<CadDocument>,
    panel: Single<Entity, With<InspectorPanel>>,
    mut shown: Local<Option<Option<ObjectId>>>,
    mut commands: Commands,
) {
    let selected = selection.get();
    let selected_changed = selected.is_some_and(|id| changes.read().any(|c| c.touches(id)));
    let reverted = requests.read().any(|r| *r == InspectorRequest::Revert);
    if *shown == Some(selected) && !selected_changed && !reverted {
        return;
    }
    *shown = Some(selected);

    let mut panel = commands.entity(*panel);
    panel.despawn_children();
    match selected.and_then(|id| doc.feature(id)) {
        Some(feature) => {
            panel.with_children(|p| spawn_fields(p, feature, doc.units().symbol()));
        }
        None => {
            panel.with_child(label(
                "Select an object in the viewport or the list.",
                13.0,
                TEXT_DIM,
            ));
        }
    }
}

fn spawn_fields(panel: &mut ChildSpawnerCommands, feature: &Feature, unit: &str) {
    let mut tab = 0;
    let mut next_tab = || {
        tab += 1;
        tab
    };
    let id = feature.id().to_string();
    panel.spawn(label(
        format!("{}  ·  id {}", feature.geometry().kind(), &id[..8]),
        12.0,
        TEXT_DIM,
    ));
    field_row(
        panel,
        "Name",
        (text_field(feature.name(), next_tab()), InspectorField::Name),
    );

    panel.spawn(section_title(format!("Dimensions ({unit})")));
    for param in feature.geometry().parameters() {
        field_row(
            panel,
            param.label,
            (
                number_field(param.value, next_tab()),
                InspectorField::Parameter {
                    key: param.key,
                    label: param.label,
                },
            ),
        );
    }

    let Transform3D {
        translation,
        rotation_deg,
    } = *feature.transform();
    panel.spawn(section_title(format!("Position ({unit})")));
    for (i, axis) in ["X", "Y", "Z"].into_iter().enumerate() {
        field_row(
            panel,
            axis,
            (
                number_field(translation[i], next_tab()),
                InspectorField::Position(i),
            ),
        );
    }
    panel.spawn(section_title("Rotation (degrees)".into()));
    for (i, axis) in ["X", "Y", "Z"].into_iter().enumerate() {
        field_row(
            panel,
            axis,
            (
                number_field(rotation_deg[i], next_tab()),
                InspectorField::Rotation(i),
            ),
        );
    }

    panel
        .spawn(Node {
            column_gap: px(6),
            margin: UiRect::top(px(6)),
            ..default()
        })
        .with_children(|row| {
            row.spawn(colored_button(
                "Apply",
                UiAction::ApplyProperties,
                ACCENT_BG,
            ));
            row.spawn(button("Revert", UiAction::RevertProperties));
            row.spawn(colored_button(
                "Delete",
                UiAction::DeleteSelected,
                DANGER_BG,
            ));
        });
}

fn section_title(text: String) -> impl Bundle {
    (
        label(text, 13.0, TEXT),
        Node {
            margin: UiRect::top(px(4)),
            ..default()
        },
    )
}

fn field_row(panel: &mut ChildSpawnerCommands, text: &str, field: impl Bundle) {
    panel
        .spawn(Node {
            align_items: AlignItems::Center,
            column_gap: px(8),
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                label(text, 13.0, TEXT_DIM),
                Node {
                    width: px(70),
                    flex_shrink: 0.0,
                    ..default()
                },
            ));
            row.spawn(field);
        });
}

/// Validates the inspector fields and sends one atomic batch command.
/// Nothing is sent if any field is invalid, so the document stays intact.
pub fn apply_inspector(
    mut requests: MessageReader<InspectorRequest>,
    selection: Res<Selection>,
    doc: Res<CadDocument>,
    fields: Query<(&InspectorField, &EditableText)>,
    mut edits: MessageWriter<EditCommand>,
    mut status: ResMut<Status>,
) {
    if !requests.read().any(|r| *r == InspectorRequest::Apply) {
        return;
    }
    let Some(feature) = selection.get().and_then(|id| doc.feature(id)) else {
        status.info("Nothing selected");
        return;
    };
    let values = fields
        .iter()
        .map(|(field, text)| (*field, text.value().to_string()));
    match build_update(feature, values) {
        Ok(None) => status.info("No changes to apply"),
        Ok(Some(command)) => {
            edits.write(EditCommand(command));
        }
        Err(message) => status.error(message),
    }
}

/// Turns raw field text into a command, or a user-facing error message.
fn build_update(
    feature: &Feature,
    values: impl IntoIterator<Item = (InspectorField, String)>,
) -> Result<Option<Command>, String> {
    let id = feature.id();
    let mut name = feature.name().to_owned();
    let mut parameters = Vec::new();
    let mut transform = *feature.transform();
    for (field, text) in values {
        if field == InspectorField::Name {
            name = text;
            continue;
        }
        let value = parse_number(&text)
            .map_err(|()| format!("{}: '{}' is not a number", field.label(), text.trim()))?;
        match field {
            InspectorField::Name => unreachable!("handled above"),
            InspectorField::Parameter { key, .. } => parameters.push((key, value)),
            InspectorField::Position(i) => transform.translation[i] = value,
            InspectorField::Rotation(i) => transform.rotation_deg[i] = value,
        }
    }

    let geometry = feature
        .geometry()
        .with_parameters(&parameters)
        .map_err(|err| format!("Invalid dimension: {err}"))?;
    transform
        .validate()
        .map_err(|err| format!("Invalid placement: {err}"))?;

    let mut commands = Vec::new();
    if name.trim() != feature.name() {
        commands.push(Command::RenameObject { id, name });
    }
    if geometry != *feature.geometry() {
        commands.push(Command::UpdateParameters { id, geometry });
    }
    if transform != *feature.transform() {
        commands.push(Command::SetObjectTransform { id, transform });
    }
    Ok((!commands.is_empty()).then_some(Command::Batch(commands)))
}

fn parse_number(text: &str) -> Result<f64, ()> {
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cad_core::{BoxParameters, Geometry, PrimitiveKind};

    fn feature() -> Feature {
        Feature::new(
            ObjectId::new(),
            "Box 1",
            Geometry::Box(BoxParameters::new(10.0, 20.0, 30.0).unwrap()),
            Transform3D::default(),
        )
        .unwrap()
    }

    fn width(text: &str) -> (InspectorField, String) {
        (
            InspectorField::Parameter {
                key: "width",
                label: "Width",
            },
            text.into(),
        )
    }

    #[test]
    fn unchanged_fields_produce_no_command() {
        let f = feature();
        let result = build_update(&f, [(InspectorField::Name, "Box 1".into()), width("10")]);
        assert_eq!(result, Ok(None));
    }

    #[test]
    fn changed_fields_produce_one_batch() {
        let f = feature();
        let result = build_update(
            &f,
            [
                (InspectorField::Name, "Lid".into()),
                width(" 12.5 "),
                (InspectorField::Position(2), "-4".into()),
            ],
        )
        .unwrap()
        .unwrap();
        let Command::Batch(commands) = result else {
            panic!("expected a batch");
        };
        assert_eq!(commands.len(), 3);
        assert!(commands.contains(&Command::UpdateParameters {
            id: f.id(),
            geometry: Geometry::Box(BoxParameters::new(12.5, 20.0, 30.0).unwrap()),
        }));
    }

    #[test]
    fn invalid_input_is_reported_without_a_command() {
        let f = feature();
        let err = build_update(&f, [width("abc")]).unwrap_err();
        assert_eq!(err, "Width: 'abc' is not a number");
        let err = build_update(&f, [width("0")]).unwrap_err();
        assert!(
            err.starts_with("Invalid dimension: width must be greater than 0"),
            "{err}"
        );
        let err = build_update(&f, [width("-3")]).unwrap_err();
        assert!(err.contains("greater than 0"), "{err}");
        assert!(build_update(&f, [width("inf")]).is_err());
        let _ = PrimitiveKind::Box;
    }
}
