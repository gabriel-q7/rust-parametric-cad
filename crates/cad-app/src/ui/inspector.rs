//! Properties inspector: shows the selected object's name, dimensions, and
//! placement as editable fields, and turns "Apply" into one atomic command.
//!
//! For editable meshes it also shows the topology tools: edit-mode buttons,
//! stepping through elements, and the panel for the selected vertex (move),
//! edge (translate), or face (extrude).

use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use bevy::text::EditableText;
use cad_core::{Command, EditableGeometry, Feature, Geometry, Transform3D};

use super::UiAction;
use super::widgets::{
    ACCENT_BG, BUTTON_BG, DANGER_BG, ERROR, SELECTED_BG, TEXT, TEXT_DIM, button, colored_button,
    format_number, label, number_field, text_field,
};
use crate::commands::{DocumentChanged, EditCommand};
use crate::editor::{CadDocument, EditMode, Selection, Status, TopologyElement};

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
    /// Local coordinate of the selected vertex.
    VertexPosition(usize),
    /// Offset to translate the selected edge by.
    EdgeOffset(usize),
    /// Distance to extrude the selected face by.
    ExtrudeDistance,
}

const AXES: [&str; 3] = ["X", "Y", "Z"];

impl InspectorField {
    fn label(self) -> String {
        match self {
            InspectorField::Name => "Name".into(),
            InspectorField::Parameter { label, .. } => label.into(),
            InspectorField::Position(i) => format!("Position {}", AXES[i]),
            InspectorField::Rotation(i) => format!("Rotation {}", AXES[i]),
            InspectorField::VertexPosition(i) => format!("Vertex {}", AXES[i]),
            InspectorField::EdgeOffset(i) => format!("Offset {}", AXES[i]),
            InspectorField::ExtrudeDistance => "Distance".into(),
        }
    }

    /// Whether this field belongs to the topology panel (applied with the
    /// topology action) rather than to the object properties.
    pub fn is_topology(self) -> bool {
        matches!(
            self,
            InspectorField::VertexPosition(_)
                | InspectorField::EdgeOffset(_)
                | InspectorField::ExtrudeDistance
        )
    }
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorRequest {
    Apply,
    Revert,
    /// Apply the topology panel (move vertex, translate edge, extrude face).
    Topology,
}

/// Rebuilds the inspector when the selection (object, mode, or element)
/// changes, when the selected object changes in the document, or when the
/// user reverts their edits.
#[allow(clippy::too_many_arguments)]
pub fn rebuild_inspector(
    mut changes: MessageReader<DocumentChanged>,
    mut requests: MessageReader<InspectorRequest>,
    selection: Res<Selection>,
    doc: Res<CadDocument>,
    panel: Single<Entity, With<InspectorPanel>>,
    mut shown: Local<Option<Selection>>,
    mut focus: ResMut<InputFocus>,
    mut commands: Commands,
) {
    let selected = selection.get();
    let selected_changed = selected.is_some_and(|id| changes.read().any(|c| c.touches(id)));
    let reverted = requests.read().any(|r| *r == InspectorRequest::Revert);
    if *shown == Some(*selection) && !selected_changed && !reverted {
        return;
    }
    *shown = Some(*selection);
    // The focused field is about to be despawned, and its entity id may be
    // reused; a stale focus would keep keyboard shortcuts disabled.
    focus.clear();

    let mut panel = commands.entity(*panel);
    panel.despawn_children();
    match selected.and_then(|id| doc.feature(id)) {
        Some(feature) => {
            panel.with_children(|p| spawn_fields(p, feature, &selection, doc.units().symbol()));
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

fn spawn_fields(
    panel: &mut ChildSpawnerCommands,
    feature: &Feature,
    selection: &Selection,
    unit: &str,
) {
    let mut tab = 0;
    let mut next_tab = || {
        tab += 1;
        tab
    };
    let id = feature.id().to_string();
    panel.spawn(label(
        format!("{}  |  id {}", feature.geometry().label(), &id[..8]),
        12.0,
        TEXT_DIM,
    ));
    if let Some(editable) = feature.geometry().as_editable() {
        panel.spawn(label(describe_mesh(editable), 12.0, TEXT_DIM));
    }
    field_row(
        panel,
        "Name",
        (text_field(feature.name(), next_tab()), InspectorField::Name),
    );

    let parameters = feature.geometry().parameters();
    if !parameters.is_empty() {
        panel.spawn(section_title(format!("Dimensions ({unit})")));
    }
    for param in parameters {
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
    xyz_row(
        panel,
        &format!("Position ({unit})"),
        translation,
        InspectorField::Position,
        &mut next_tab,
    );
    xyz_row(
        panel,
        "Rotation (deg)",
        rotation_deg,
        InspectorField::Rotation,
        &mut next_tab,
    );

    panel.spawn(button_row()).with_children(|row| {
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

    match feature.geometry().as_editable() {
        None => {
            panel.spawn(section_title("Direct editing".into()));
            panel.spawn(label(
                "Convert to an editable mesh to move vertices, translate edges, \
                 and extrude faces. Conversion is one-way.",
                12.0,
                TEXT_DIM,
            ));
            panel.spawn(button_row()).with_child(button(
                "Convert to editable mesh",
                UiAction::ConvertToEditable,
            ));
        }
        Some(editable) => spawn_topology_tools(panel, editable, selection, unit, &mut next_tab),
    }
}

fn spawn_topology_tools(
    panel: &mut ChildSpawnerCommands,
    editable: &EditableGeometry,
    selection: &Selection,
    unit: &str,
    next_tab: &mut impl FnMut() -> i32,
) {
    let mesh = &editable.mesh;
    let mode = selection.mode();
    panel.spawn(section_title("Topology editing (keys 1-4)".into()));
    panel.spawn(button_row()).with_children(|row| {
        for m in EditMode::ALL {
            let color = if m == mode { SELECTED_BG } else { BUTTON_BG };
            row.spawn(colored_button(m.label(), UiAction::SetEditMode(m), color));
        }
    });
    if mode == EditMode::Object {
        panel.spawn(label(
            "Pick a mode, then click an element in the viewport.",
            12.0,
            TEXT_DIM,
        ));
        return;
    }

    let noun = mode.label().to_lowercase();
    let (position, total) = element_position(mesh, mode, selection.element());
    panel.spawn(button_row()).with_children(|row| {
        row.spawn(button("< Prev", UiAction::StepElement(-1)));
        row.spawn(button("Next >", UiAction::StepElement(1)));
        let status = match position {
            Some(i) => format!("{} of {total}", i + 1),
            None => format!("none of {total}"),
        };
        row.spawn(label(status, 12.0, TEXT_DIM));
    });

    match selection.element() {
        None => {
            panel.spawn(label(
                format!(
                    "Click {} {noun} in the viewport, or use Prev / Next.",
                    if mode == EditMode::Edge { "an" } else { "a" }
                ),
                12.0,
                TEXT_DIM,
            ));
        }
        Some(TopologyElement::Vertex(v)) => {
            let Some(p) = mesh.position(v) else { return };
            panel.spawn(label(format!("Vertex {v}: local coordinates"), 12.0, TEXT));
            xyz_row(
                panel,
                &format!("Position ({unit})"),
                p,
                InspectorField::VertexPosition,
                next_tab,
            );
            panel.spawn(button_row()).with_child(colored_button(
                "Move vertex",
                UiAction::ApplyTopology,
                ACCENT_BG,
            ));
        }
        Some(TopologyElement::Edge(e)) => {
            let (Some(edge), Some(length)) = (mesh.edge(e), mesh.edge_length(e)) else {
                return;
            };
            let [a, b] = edge.vertices;
            panel.spawn(label(
                format!(
                    "Edge {e}: {a} -> {b}, length {} {unit}",
                    format_number(length)
                ),
                12.0,
                TEXT,
            ));
            xyz_row(
                panel,
                &format!("Offset ({unit})"),
                [0.0; 3],
                InspectorField::EdgeOffset,
                next_tab,
            );
            panel.spawn(button_row()).with_child(colored_button(
                "Translate edge",
                UiAction::ApplyTopology,
                ACCENT_BG,
            ));
        }
        Some(TopologyElement::Face(f)) => {
            let Some(face) = mesh.face(f) else { return };
            let normal = mesh
                .face_normal(f)
                .map(|n| n.map(format_number).join(", "))
                .unwrap_or_else(|| "-".into());
            panel.spawn(label(
                format!(
                    "Face {f}: {} vertices, normal ({normal})",
                    face.vertices.len()
                ),
                12.0,
                TEXT,
            ));
            if let Err(err) = mesh.check_extrudable(f) {
                panel.spawn(label(format!("Cannot extrude: {err}"), 12.0, ERROR));
            }
            field_row(
                panel,
                &format!("Distance ({unit})"),
                (
                    number_field(10.0, next_tab()),
                    InspectorField::ExtrudeDistance,
                ),
            );
            panel.spawn(label(
                "Positive extrudes outwards, negative inwards.",
                12.0,
                TEXT_DIM,
            ));
            panel.spawn(button_row()).with_child(colored_button(
                "Extrude face",
                UiAction::ApplyTopology,
                ACCENT_BG,
            ));
        }
    }
}

/// "8 vertices, 12 edges, 6 faces; from Box (width 100, ...)".
fn describe_mesh(editable: &EditableGeometry) -> String {
    let mesh = &editable.mesh;
    let counts = format!(
        "{} vertices, {} edges, {} faces",
        mesh.vertices().len(),
        mesh.edges().len(),
        mesh.faces().len()
    );
    match editable.source.as_deref() {
        Some(source) => {
            let params: Vec<String> = source
                .parameters()
                .iter()
                .map(|p| format!("{} {}", p.key, format_number(p.value)))
                .collect();
            format!(
                "{counts}\nconverted from {} ({})",
                source.label(),
                params.join(", ")
            )
        }
        None => counts,
    }
}

/// Index of the selected element among the mode's elements, and the total.
fn element_position(
    mesh: &cad_core::EditableMesh,
    mode: EditMode,
    element: Option<TopologyElement>,
) -> (Option<usize>, usize) {
    let all = super::elements_of(mesh, mode);
    (
        element.and_then(|e| all.iter().position(|x| *x == e)),
        all.len(),
    )
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

fn button_row() -> Node {
    Node {
        column_gap: px(6),
        margin: UiRect::top(px(4)),
        align_items: AlignItems::Center,
        flex_wrap: FlexWrap::Wrap,
        row_gap: px(4),
        ..default()
    }
}

fn row_label(text: &str) -> impl Bundle {
    (
        label(text, 13.0, TEXT_DIM),
        Node {
            width: px(70),
            flex_shrink: 0.0,
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
            row.spawn(row_label(text));
            row.spawn(field);
        });
}

/// One row with a label and X/Y/Z number fields.
fn xyz_row(
    panel: &mut ChildSpawnerCommands,
    text: &str,
    values: [f64; 3],
    field: fn(usize) -> InspectorField,
    next_tab: &mut impl FnMut() -> i32,
) {
    let tabs = [next_tab(), next_tab(), next_tab()];
    panel
        .spawn(Node {
            align_items: AlignItems::Center,
            column_gap: px(4),
            ..default()
        })
        .with_children(|row| {
            row.spawn(row_label(text));
            for i in 0..3 {
                row.spawn((number_field(values[i], tabs[i]), field(i)));
            }
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
    let mut apply = false;
    let mut topology = false;
    for request in requests.read() {
        apply |= *request == InspectorRequest::Apply;
        topology |= *request == InspectorRequest::Topology;
    }
    if !apply && !topology {
        return;
    }
    let Some(feature) = selection.get().and_then(|id| doc.feature(id)) else {
        status.info("Nothing selected");
        return;
    };
    let values = || {
        fields
            .iter()
            .map(|(field, text)| (*field, text.value().to_string()))
    };
    let result = if topology {
        match selection.element() {
            Some(element) => build_topology_command(feature, element, values()),
            None => Err(format!(
                "Select a {} first",
                selection.mode().label().to_lowercase()
            )),
        }
    } else {
        build_update(feature, values())
    };
    match result {
        Ok(None) => status.info("No changes to apply"),
        Ok(Some(command)) => {
            edits.write(EditCommand(command));
        }
        Err(message) => status.error(message),
    }
}

/// Turns raw property field text into a command, or a user-facing error.
fn build_update(
    feature: &Feature,
    values: impl IntoIterator<Item = (InspectorField, String)>,
) -> Result<Option<Command>, String> {
    let id = feature.id();
    let mut name = feature.name().to_owned();
    let mut parameters = Vec::new();
    let mut transform = *feature.transform();
    for (field, text) in values {
        if field.is_topology() {
            continue;
        }
        if field == InspectorField::Name {
            name = text;
            continue;
        }
        let value = parse_field(field, &text)?;
        match field {
            InspectorField::Parameter { key, .. } => parameters.push((key, value)),
            InspectorField::Position(i) => transform.translation[i] = value,
            InspectorField::Rotation(i) => transform.rotation_deg[i] = value,
            _ => unreachable!("name and topology fields handled above"),
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

/// Turns the topology panel's fields into a move/translate/extrude command.
/// Geometric validation (degenerate faces, planarity, ...) is left to the
/// core, whose errors are shown in the status bar.
fn build_topology_command(
    feature: &Feature,
    element: TopologyElement,
    values: impl IntoIterator<Item = (InspectorField, String)>,
) -> Result<Option<Command>, String> {
    let id = feature.id();
    let Geometry::Mesh(editable) = feature.geometry() else {
        return Err("Convert the object to an editable mesh first".into());
    };
    let mesh = &editable.mesh;
    let mut xyz = [None; 3];
    let mut distance = None;
    for (field, text) in values {
        match field {
            InspectorField::VertexPosition(i) | InspectorField::EdgeOffset(i) => {
                xyz[i] = Some(parse_field(field, &text)?);
            }
            InspectorField::ExtrudeDistance => distance = Some(parse_field(field, &text)?),
            _ => {}
        }
    }
    match element {
        TopologyElement::Vertex(vertex) => {
            let current = mesh
                .position(vertex)
                .ok_or_else(|| format!("Vertex {vertex} no longer exists"))?;
            let position = [0, 1, 2].map(|i| xyz[i].unwrap_or(current[i]));
            Ok((position != current).then_some(Command::MoveVertex {
                id,
                vertex,
                position,
            }))
        }
        TopologyElement::Edge(edge) => {
            let offset = xyz.map(|v| v.unwrap_or(0.0));
            Ok((offset != [0.0; 3]).then_some(Command::TranslateEdge { id, edge, offset }))
        }
        TopologyElement::Face(face) => {
            let distance = distance.ok_or("Enter an extrusion distance")?;
            Ok(Some(Command::ExtrudeFace { id, face, distance }))
        }
    }
}

fn parse_field(field: InspectorField, text: &str) -> Result<f64, String> {
    parse_number(text).map_err(|()| format!("{}: '{}' is not a number", field.label(), text.trim()))
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
    use cad_core::{BoxParameters, FaceId, ObjectId, Tessellation, VertexId};

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
    }

    fn mesh_feature() -> Feature {
        let geometry = Geometry::Box(BoxParameters::new(10.0, 20.0, 30.0).unwrap())
            .to_editable(&Tessellation::EDITABLE)
            .unwrap()
            .unwrap();
        Feature::new(ObjectId::new(), "Mesh 1", geometry, Transform3D::default()).unwrap()
    }

    #[test]
    fn topology_fields_are_ignored_by_property_apply() {
        let f = mesh_feature();
        let result = build_update(&f, [(InspectorField::VertexPosition(0), "999".into())]);
        assert_eq!(result, Ok(None));
    }

    #[test]
    fn vertex_fields_build_a_move_command() {
        let f = mesh_feature();
        let vertex = VertexId(0);
        let element = TopologyElement::Vertex(vertex);
        let unchanged = build_topology_command(
            &f,
            element,
            [(InspectorField::VertexPosition(0), "-5".into())],
        );
        assert_eq!(unchanged, Ok(None), "same position, no command");
        let command = build_topology_command(
            &f,
            element,
            [(InspectorField::VertexPosition(2), "4.5".into())],
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            command,
            Command::MoveVertex {
                id: f.id(),
                vertex,
                position: [-5.0, -10.0, 4.5],
            }
        );
    }

    #[test]
    fn edge_and_face_fields_build_commands_or_errors() {
        let f = mesh_feature();
        let edge = f.geometry().as_editable().unwrap().mesh.edges()[0].id;
        let element = TopologyElement::Edge(edge);
        assert_eq!(
            build_topology_command(&f, element, [(InspectorField::EdgeOffset(1), "0".into())]),
            Ok(None)
        );
        assert_eq!(
            build_topology_command(&f, element, [(InspectorField::EdgeOffset(1), "x".into())]),
            Err("Offset Y: 'x' is not a number".into())
        );
        let face = TopologyElement::Face(FaceId(20));
        assert_eq!(
            build_topology_command(&f, face, [(InspectorField::ExtrudeDistance, "-3".into())]),
            Ok(Some(Command::ExtrudeFace {
                id: f.id(),
                face: FaceId(20),
                distance: -3.0
            }))
        );
    }

    #[test]
    fn topology_commands_require_an_editable_object() {
        let err =
            build_topology_command(&feature(), TopologyElement::Face(FaceId(1)), []).unwrap_err();
        assert!(err.contains("editable mesh"), "{err}");
    }
}
