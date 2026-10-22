//! Editor-wide state shared by the viewport, UI, and command handlers.

use bevy::prelude::*;
use cad_core::{Document, EdgeId, EditableMesh, FaceId, ObjectId, VertexId};

/// The authoritative CAD document. Render entities are derived from it.
#[derive(Resource, Default, Deref, DerefMut)]
pub struct CadDocument(pub Document);

/// What clicks in the viewport select.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditMode {
    /// Whole objects.
    #[default]
    Object,
    /// Vertices of the selected editable mesh.
    Vertex,
    /// Edges of the selected editable mesh.
    Edge,
    /// Faces of the selected editable mesh.
    Face,
}

impl EditMode {
    pub const ALL: [EditMode; 4] = [
        EditMode::Object,
        EditMode::Vertex,
        EditMode::Edge,
        EditMode::Face,
    ];

    pub fn label(self) -> &'static str {
        match self {
            EditMode::Object => "Object",
            EditMode::Vertex => "Vertex",
            EditMode::Edge => "Edge",
            EditMode::Face => "Face",
        }
    }
}

/// A selected topology element of the selected object, by stable mesh ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopologyElement {
    Vertex(VertexId),
    Edge(EdgeId),
    Face(FaceId),
}

impl TopologyElement {
    pub fn mode(self) -> EditMode {
        match self {
            TopologyElement::Vertex(_) => EditMode::Vertex,
            TopologyElement::Edge(_) => EditMode::Edge,
            TopologyElement::Face(_) => EditMode::Face,
        }
    }

    /// Whether the element still exists in `mesh`.
    pub fn exists_in(self, mesh: &EditableMesh) -> bool {
        match self {
            TopologyElement::Vertex(id) => mesh.vertex(id).is_some(),
            TopologyElement::Edge(id) => mesh.edge(id).is_some(),
            TopologyElement::Face(id) => mesh.face(id).is_some(),
        }
    }
}

/// Current selection: an object (by stable document ID), the edit mode, and
/// optionally one topology element of that object.
///
/// Invariants: topology modes and elements only exist while an object is
/// selected, and an element always matches the current mode.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    object: Option<ObjectId>,
    mode: EditMode,
    element: Option<TopologyElement>,
}

impl Selection {
    pub fn get(&self) -> Option<ObjectId> {
        self.object
    }

    pub fn is(&self, id: ObjectId) -> bool {
        self.object == Some(id)
    }

    pub fn mode(&self) -> EditMode {
        self.mode
    }

    pub fn element(&self) -> Option<TopologyElement> {
        self.element
    }

    /// Selects an object. Selecting a different object resets the mode.
    pub fn select(&mut self, id: ObjectId) {
        if self.object != Some(id) {
            *self = Self {
                object: Some(id),
                ..Self::default()
            };
        }
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Switches the edit mode, dropping an element of another kind.
    pub fn set_mode(&mut self, mode: EditMode) {
        if self.object.is_none() {
            return;
        }
        self.mode = mode;
        if self.element.is_some_and(|e| e.mode() != mode) {
            self.element = None;
        }
    }

    /// Selects a topology element (and its mode) on the selected object.
    pub fn select_element(&mut self, element: TopologyElement) {
        if self.object.is_some() {
            self.mode = element.mode();
            self.element = Some(element);
        }
    }

    pub fn clear_element(&mut self) {
        self.element = None;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_keeps_mode_and_element_consistent() {
        let mut selection = Selection::default();
        selection.set_mode(EditMode::Vertex);
        assert_eq!(selection.mode(), EditMode::Object, "no object, no mode");

        let id = ObjectId::new();
        selection.select(id);
        selection.select_element(TopologyElement::Edge(EdgeId(3)));
        assert_eq!(selection.mode(), EditMode::Edge);

        selection.set_mode(EditMode::Face);
        assert_eq!(selection.element(), None, "edge dropped in face mode");

        selection.select(id);
        assert_eq!(selection.mode(), EditMode::Face, "re-selecting keeps mode");
        selection.select(ObjectId::new());
        assert_eq!(selection.mode(), EditMode::Object, "new object resets mode");
    }
}
