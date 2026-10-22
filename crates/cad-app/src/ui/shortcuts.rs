//! Keyboard shortcuts. Disabled while a text field has focus, except Enter
//! (apply the inspector), Escape (leave the field), and Ctrl combinations.
//!
//! Escape steps back one level: element, then edit mode, then object.

use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use bevy::text::EditableText;

use super::UiAction;
use super::inspector::InspectorField;
use crate::editor::EditMode;

pub fn keyboard_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    mut focus: ResMut<InputFocus>,
    text_fields: Query<(), With<EditableText>>,
    inspector_fields: Query<&InspectorField>,
    mut actions: MessageWriter<UiAction>,
    mut selection: ResMut<crate::editor::Selection>,
) {
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    if ctrl && keys.just_pressed(KeyCode::KeyS) {
        actions.write(UiAction::SaveProject);
    }
    if ctrl && keys.just_pressed(KeyCode::KeyO) {
        actions.write(UiAction::OpenProject);
    }

    let focused = focus.get();
    if focused.is_some_and(|e| text_fields.contains(e)) {
        if keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter])
            && let Some(field) = focused.and_then(|e| inspector_fields.get(e).ok())
        {
            actions.write(if field.is_topology() {
                UiAction::ApplyTopology
            } else {
                UiAction::ApplyProperties
            });
        }
        if keys.just_pressed(KeyCode::Escape) {
            focus.clear();
        }
        return;
    }

    if keys.just_pressed(KeyCode::Delete) {
        actions.write(UiAction::DeleteSelected);
    }
    if keys.just_pressed(KeyCode::Escape) {
        if selection.element().is_some() {
            selection.clear_element();
        } else if selection.mode() != EditMode::Object {
            selection.set_mode(EditMode::Object);
        } else {
            selection.clear();
        }
    }
    for (key, mode) in [
        (KeyCode::Digit1, EditMode::Object),
        (KeyCode::Digit2, EditMode::Vertex),
        (KeyCode::Digit3, EditMode::Edge),
        (KeyCode::Digit4, EditMode::Face),
    ] {
        if keys.just_pressed(key) {
            actions.write(UiAction::SetEditMode(mode));
        }
    }
    if keys.just_pressed(KeyCode::Home) {
        actions.write(UiAction::ResetCamera);
    }
}
