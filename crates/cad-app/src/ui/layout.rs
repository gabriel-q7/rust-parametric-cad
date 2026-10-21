//! Static window layout: toolbar on top, viewport in the middle, sidebar on
//! the right, status bar at the bottom.

use bevy::input_focus::tab_navigation::TabGroup;
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use cad_core::PrimitiveKind;

use super::inspector::InspectorPanel;
use super::object_list::ObjectList;
use super::widgets::{
    ACCENT_BG, PANEL_BG, SECTION_BG, TEXT, TEXT_DIM, TOOLBAR_BG, button, colored_button, label,
    text_field,
};
use super::{DEFAULT_PROJECT_PATH, ProjectPathField, StatusText, UiAction};

const SIDEBAR_WIDTH: f32 = 320.0;

pub fn spawn_layout(mut commands: Commands) {
    commands
        .spawn((
            Name::new("UI root"),
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            // Transparent containers must not block clicks to the 3D viewport.
            Pickable::IGNORE,
        ))
        .with_children(|root| {
            spawn_toolbar(root);
            root.spawn((
                Node {
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Row,
                    min_height: px(0),
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .with_children(|middle| {
                spawn_viewport_overlay(middle);
                spawn_sidebar(middle);
            });
            spawn_status_bar(root);
        });
}

fn spawn_toolbar(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Name::new("Toolbar"),
        Node {
            height: px(44),
            padding: UiRect::axes(px(10), px(6)),
            column_gap: px(6),
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(TOOLBAR_BG),
        TabGroup::new(1),
    ))
    .with_children(|bar| {
        bar.spawn(label("Create:", 14.0, TEXT_DIM));
        for kind in PrimitiveKind::ALL {
            bar.spawn(colored_button(
                format!("+ {}", kind.label()),
                UiAction::Create(kind),
                ACCENT_BG,
            ));
        }
        bar.spawn(separator());
        bar.spawn(label("File:", 14.0, TEXT_DIM));
        bar.spawn(Node {
            width: px(260),
            ..default()
        })
        .with_child((text_field(DEFAULT_PROJECT_PATH, 0), ProjectPathField));
        bar.spawn(button("Open", UiAction::OpenProject));
        bar.spawn(button("Save", UiAction::SaveProject));
        bar.spawn(button("New", UiAction::NewProject));
        bar.spawn(separator());
        bar.spawn(button("Reset view", UiAction::ResetCamera));
    });
}

fn separator() -> impl Bundle {
    (
        Node {
            width: px(1),
            height: px(24),
            margin: UiRect::horizontal(px(6)),
            ..default()
        },
        BackgroundColor(SECTION_BG.lighter(0.1)),
    )
}

/// Transparent area over the 3D view with a controls cheat sheet.
fn spawn_viewport_overlay(middle: &mut ChildSpawnerCommands) {
    middle
        .spawn((
            Name::new("Viewport area"),
            Node {
                flex_grow: 1.0,
                justify_content: JustifyContent::FlexEnd,
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(px(10)),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|area| {
            area.spawn((
                label(
                    "Left click: select   Right drag: orbit   Middle / Shift+Right drag: pan   \
                     Wheel: zoom   Home: reset view\n\
                     Del: delete selected   Esc: deselect   Enter: apply properties   \
                     Ctrl+S / Ctrl+O: save / open",
                    12.0,
                    TEXT_DIM,
                ),
                Pickable::IGNORE,
            ));
        });
}

fn spawn_sidebar(middle: &mut ChildSpawnerCommands) {
    middle
        .spawn((
            Name::new("Sidebar"),
            Node {
                width: px(SIDEBAR_WIDTH),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(px(10)),
                row_gap: px(8),
                ..default()
            },
            BackgroundColor(PANEL_BG),
        ))
        .with_children(|side| {
            side.spawn(label("Objects", 16.0, TEXT));
            side.spawn((
                Name::new("Object list"),
                ObjectList,
                RelativeCursorPosition::default(),
                ScrollPosition::default(),
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(2),
                    padding: UiRect::all(px(4)),
                    height: px(220),
                    flex_shrink: 0.0,
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                BackgroundColor(SECTION_BG),
            ));
            side.spawn(label("Properties", 16.0, TEXT));
            side.spawn((
                Name::new("Inspector"),
                InspectorPanel,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(6),
                    padding: UiRect::all(px(8)),
                    ..default()
                },
                BackgroundColor(SECTION_BG),
                TabGroup::new(0),
            ));
        });
}

fn spawn_status_bar(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Name::new("Status bar"),
        Node {
            height: px(26),
            padding: UiRect::axes(px(10), px(4)),
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(TOOLBAR_BG),
    ))
    .with_children(|bar| {
        bar.spawn((label("Ready", 13.0, TEXT_DIM), StatusText));
    });
}
