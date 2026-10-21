//! Editor-wide state shared by the viewport, UI, and command handlers.

use bevy::prelude::*;
use cad_core::{Document, ObjectId};

/// The authoritative CAD document. Render entities are derived from it.
#[derive(Resource, Default, Deref, DerefMut)]
pub struct CadDocument(pub Document);

/// Currently selected object, by stable document ID.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection(Option<ObjectId>);

impl Selection {
    pub fn get(&self) -> Option<ObjectId> {
        self.0
    }

    pub fn is(&self, id: ObjectId) -> bool {
        self.0 == Some(id)
    }

    pub fn select(&mut self, id: ObjectId) {
        self.0 = Some(id);
    }

    pub fn clear(&mut self) {
        self.0 = None;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusLevel {
    Info,
    Success,
    Error,
}

/// Feedback line shown in the status bar.
#[derive(Resource, Debug, Clone)]
pub struct Status {
    pub message: String,
    pub level: StatusLevel,
}

impl Default for Status {
    fn default() -> Self {
        Self {
            message: "Ready".into(),
            level: StatusLevel::Info,
        }
    }
}

impl Status {
    pub fn info(&mut self, message: impl Into<String>) {
        self.set(StatusLevel::Info, message);
    }

    pub fn success(&mut self, message: impl Into<String>) {
        self.set(StatusLevel::Success, message);
    }

    pub fn error(&mut self, message: impl Into<String>) {
        self.set(StatusLevel::Error, message);
    }

    fn set(&mut self, level: StatusLevel, message: impl Into<String>) {
        self.level = level;
        self.message = message.into();
    }
}

/// System sets ordering one editor frame: UI and viewport input produce
/// messages, commands mutate the document, then derived state is synced.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum EditorSystems {
    Input,
    Commands,
    Sync,
}

pub struct EditorStatePlugin;

impl Plugin for EditorStatePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CadDocument>()
            .init_resource::<Selection>()
            .init_resource::<Status>()
            .configure_sets(
                Update,
                (
                    EditorSystems::Input,
                    EditorSystems::Commands,
                    EditorSystems::Sync,
                )
                    .chain(),
            );
    }
}
