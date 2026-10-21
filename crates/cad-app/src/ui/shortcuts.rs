//! Keyboard shortcuts. Disabled while a text field has focus, except Enter
//! (apply the inspector), Escape (leave the field), and Ctrl combinations.

use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use bevy::text::EditableText;

use super::UiAction;
use super::inspector::InspectorField;

pub fn keyboard_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    mut focus: ResMut<InputFocus>,
    text_fields: Query<(), With<EditableText>>,
    inspector_fields: Query<(), With<InspectorField>>,
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
            && focused.is_some_and(|e| inspector_fields.contains(e))
        {
            actions.write(UiAction::ApplyProperties);
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
        selection.clear();
    }
    if keys.just_pressed(KeyCode::Home) {
        actions.write(UiAction::ResetCamera);
    }
}
