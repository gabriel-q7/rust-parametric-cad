//! The editor's plugin group.

use bevy::app::{PluginGroup, PluginGroupBuilder};

use crate::commands::CommandsPlugin;
use crate::editor::EditorStatePlugin;
use crate::ui::EditorUiPlugin;
use crate::viewport::ViewportPlugin;

/// Everything the CAD editor adds on top of Bevy's `DefaultPlugins`.
pub struct CadEditorPlugins;

impl PluginGroup for CadEditorPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(EditorStatePlugin)
            .add(CommandsPlugin)
            .add(ViewportPlugin)
            .add(EditorUiPlugin)
    }
}
