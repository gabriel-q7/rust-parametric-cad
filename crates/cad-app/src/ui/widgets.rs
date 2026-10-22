//! Shared look and small widget constructors.

use bevy::input_focus::tab_navigation::TabIndex;
use bevy::prelude::*;
use bevy::text::{EditableText, EditableTextFilter, TextCursorStyle};

use super::UiAction;

pub const TOOLBAR_BG: Color = Color::srgb(0.11, 0.12, 0.13);
pub const PANEL_BG: Color = Color::srgb(0.14, 0.15, 0.16);
pub const SECTION_BG: Color = Color::srgb(0.18, 0.19, 0.21);
pub const BUTTON_BG: Color = Color::srgb(0.24, 0.26, 0.29);
pub const ACCENT_BG: Color = Color::srgb(0.17, 0.42, 0.72);
pub const DANGER_BG: Color = Color::srgb(0.55, 0.18, 0.18);
pub const SELECTED_BG: Color = Color::srgb(0.55, 0.36, 0.10);
pub const FIELD_BG: Color = Color::srgb(0.08, 0.08, 0.09);
pub const FIELD_BORDER: Color = Color::srgb(0.32, 0.34, 0.38);
pub const TEXT: Color = Color::srgb(0.90, 0.91, 0.93);
pub const TEXT_DIM: Color = Color::srgb(0.60, 0.62, 0.66);
pub const SUCCESS: Color = Color::srgb(0.45, 0.82, 0.48);
pub const ERROR: Color = Color::srgb(1.0, 0.45, 0.42);

pub fn font(size: f32) -> TextFont {
    TextFont {
        font_size: FontSize::Px(size),
        ..default()
    }
}

/// Plain text label.
pub fn label(text: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (Text::new(text), font(size), TextColor(color))
}

/// Base color of a button; hover/press shades are derived from it.
#[derive(Component, Debug, Clone, Copy)]
pub struct ButtonColor(pub Color);

pub fn button(text: impl Into<String>, action: UiAction) -> impl Bundle {
    colored_button(text, action, BUTTON_BG)
}

pub fn colored_button(text: impl Into<String>, action: UiAction, color: Color) -> impl Bundle {
    (
        Button,
        action,
        ButtonColor(color),
        BackgroundColor(color),
        Node {
            padding: UiRect::axes(px(10), px(5)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(px(4)),
            ..default()
        },
        children![label(text, 14.0, TEXT)],
    )
}

/// Single-line editable text field. Callers add a marker component.
pub fn text_field(initial: impl AsRef<str>, tab_index: i32) -> impl Bundle {
    (
        EditableText::new(initial),
        TextCursorStyle::default(),
        font(14.0),
        TextColor(TEXT),
        TextLayout::no_wrap(),
        TabIndex(tab_index),
        BackgroundColor(FIELD_BG),
        BorderColor::all(FIELD_BORDER),
        Node {
            flex_grow: 1.0,
            min_width: px(60),
            border: UiRect::all(px(1)),
            padding: UiRect::axes(px(6), px(3)),
            border_radius: BorderRadius::all(px(3)),
            overflow: Overflow::clip_x(),
            ..default()
        },
    )
}

/// Text field that only accepts characters that can form a decimal number.
pub fn number_field(value: f64, tab_index: i32) -> impl Bundle {
    (
        text_field(format_number(value), tab_index),
        EditableTextFilter::new(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E')),
    )
}

/// Compact decimal text (`100`, `12.5`, `0.001`), rounded to 6 decimals so
/// floating-point noise such as `49.99999999999` displays as `50`.
pub fn format_number(value: f64) -> String {
    let rounded = (value * 1e6).round() / 1e6;
    // `+ 0.0` turns -0 into 0.
    format!("{}", rounded + 0.0)
}

type ButtonVisualsQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Interaction,
        &'static ButtonColor,
        &'static mut BackgroundColor,
    ),
    Or<(Changed<Interaction>, Changed<ButtonColor>)>,
>;

/// Hover/press feedback for every button.
pub fn button_visuals(mut buttons: ButtonVisualsQuery) {
    for (interaction, base, mut background) in &mut buttons {
        background.0 = match interaction {
            Interaction::Pressed => base.0.lighter(0.15),
            Interaction::Hovered => base.0.lighter(0.07),
            Interaction::None => base.0,
        };
    }
}

/// Highlights the border of the focused text field.
pub fn field_focus_visuals(
    focus: Res<bevy::input_focus::InputFocus>,
    mut fields: Query<(Entity, &mut BorderColor), With<EditableText>>,
) {
    if !focus.is_changed() {
        return;
    }
    for (entity, mut border) in &mut fields {
        let color = if focus.get() == Some(entity) {
            ACCENT_BG.lighter(0.15)
        } else {
            FIELD_BORDER
        };
        border.set_all(color);
    }
}
