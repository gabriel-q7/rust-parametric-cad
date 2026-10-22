//! Scrollable list of document objects; clicking a row selects it.

use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use cad_core::Feature;

use super::UiAction;
use super::widgets::{BUTTON_BG, ButtonColor, SELECTED_BG, TEXT, TEXT_DIM, font, label};
use crate::commands::DocumentChanged;
use crate::editor::{CadDocument, Selection};

#[derive(Component)]
pub struct ObjectList;

/// Marks a panel that scrolls vertically with the mouse wheel.
#[derive(Component)]
pub struct WheelScroll;

pub fn rebuild_object_list(
    mut changes: MessageReader<DocumentChanged>,
    selection: Res<Selection>,
    doc: Res<CadDocument>,
    list: Single<Entity, With<ObjectList>>,
    mut commands: Commands,
) {
    let document_changed = changes.read().count() > 0;
    if !document_changed && !selection.is_changed() {
        return;
    }
    commands
        .entity(*list)
        .despawn_children()
        .with_children(|list| {
            if doc.is_empty() {
                list.spawn(label(
                    "No objects. Use the toolbar to create one.",
                    13.0,
                    TEXT_DIM,
                ));
            }
            for feature in doc.features() {
                list.spawn(object_row(feature, selection.is(feature.id())));
            }
        });
}

fn object_row(feature: &Feature, selected: bool) -> impl Bundle {
    let color = if selected { SELECTED_BG } else { BUTTON_BG };
    (
        Button,
        UiAction::Select(feature.id()),
        ButtonColor(color),
        BackgroundColor(color),
        Node {
            padding: UiRect::axes(px(8), px(4)),
            justify_content: JustifyContent::SpaceBetween,
            flex_shrink: 0.0,
            border_radius: BorderRadius::all(px(3)),
            ..default()
        },
        children![
            (Text::new(feature.name()), font(14.0), TextColor(TEXT)),
            (
                Text::new(feature.geometry().label()),
                font(12.0),
                TextColor(TEXT_DIM)
            ),
        ],
    )
}

pub fn scroll_panels(
    scroll: Res<AccumulatedMouseScroll>,
    mut lists: Query<(&RelativeCursorPosition, &mut ScrollPosition), With<WheelScroll>>,
) {
    if scroll.delta.y == 0.0 {
        return;
    }
    let pixels = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y * 24.0,
        MouseScrollUnit::Pixel => scroll.delta.y,
    };
    for (cursor, mut position) in &mut lists {
        if cursor.cursor_over() {
            position.y = (position.y - pixels).max(0.0);
        }
    }
}
