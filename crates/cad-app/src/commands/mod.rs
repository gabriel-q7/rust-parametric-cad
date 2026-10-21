//! Command dispatch: the bridge between UI input and the CAD document.
//!
//! UI and viewport systems never mutate [`CadDocument`] directly. They send
//! an [`EditCommand`] (wrapping a `cad_core::Command`) or a
//! [`ProjectCommand`]; the systems here apply them, report the result in the
//! status bar, and announce what changed with [`DocumentChanged`] so derived
//! state (meshes, lists, inspector) can update incrementally.

mod project;

use std::path::PathBuf;

use bevy::prelude::*;
use cad_core::{Command, Document, ObjectId};

use crate::editor::{CadDocument, EditorSystems, Selection, Status};

/// Request to apply a document command.
#[derive(Message, Debug, Clone)]
pub struct EditCommand(pub Command);

/// Request to replace, load, or save the whole project.
#[derive(Message, Debug, Clone)]
pub enum ProjectCommand {
    New,
    Open(PathBuf),
    Save(PathBuf),
}

/// Announces which objects changed in the document.
#[derive(Message, Debug, Clone, Default)]
pub struct DocumentChanged {
    pub affected: Vec<ObjectId>,
    /// The whole document was replaced (new/open); every object may differ.
    pub reloaded: bool,
}

impl DocumentChanged {
    pub fn reloaded() -> Self {
        Self {
            affected: Vec::new(),
            reloaded: true,
        }
    }

    pub fn touches(&self, id: ObjectId) -> bool {
        self.reloaded || self.affected.contains(&id)
    }
}

pub struct CommandsPlugin;

impl Plugin for CommandsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<EditCommand>()
            .add_message::<ProjectCommand>()
            .add_message::<DocumentChanged>()
            .add_systems(Startup, announce_initial_document)
            .add_systems(
                Update,
                (apply_edit_commands, handle_project_commands)
                    .chain()
                    .in_set(EditorSystems::Commands),
            );
    }
}

/// The document resource exists before the first frame; let derived state
/// build itself from it.
fn announce_initial_document(mut changed: MessageWriter<DocumentChanged>) {
    changed.write(DocumentChanged::reloaded());
}

fn apply_edit_commands(
    mut commands: MessageReader<EditCommand>,
    mut doc: ResMut<CadDocument>,
    mut selection: ResMut<Selection>,
    mut status: ResMut<Status>,
    mut changed: MessageWriter<DocumentChanged>,
) {
    for EditCommand(command) in commands.read() {
        let subject = subject_name(command, &doc);
        let verb = verb(command);
        let created = match command {
            Command::CreatePrimitive { id, .. } => Some(*id),
            _ => None,
        };
        match doc.apply(command.clone()) {
            Ok(applied) => {
                if let Some(id) = created {
                    selection.select(id);
                }
                if selection.get().is_some_and(|id| !doc.contains(id)) {
                    selection.clear();
                }
                status.success(format!("{verb} {subject}"));
                changed.write(DocumentChanged {
                    affected: applied.affected,
                    reloaded: false,
                });
            }
            Err(err) => status.error(format!("Could not update {subject}: {err}")),
        }
    }
}

fn handle_project_commands(
    mut requests: MessageReader<ProjectCommand>,
    mut doc: ResMut<CadDocument>,
    mut selection: ResMut<Selection>,
    mut status: ResMut<Status>,
    mut changed: MessageWriter<DocumentChanged>,
) {
    for request in requests.read() {
        match request {
            ProjectCommand::New => {
                doc.0 = Document::new();
                selection.clear();
                changed.write(DocumentChanged::reloaded());
                status.info("New empty project");
            }
            ProjectCommand::Save(path) => match project::save(&doc, path) {
                Ok(()) => status.success(format!(
                    "Saved {} object(s) to {}",
                    doc.len(),
                    path.display()
                )),
                Err(err) => status.error(err.to_string()),
            },
            ProjectCommand::Open(path) => match project::load(path) {
                Ok(loaded) => {
                    let count = loaded.len();
                    doc.0 = loaded;
                    selection.clear();
                    changed.write(DocumentChanged::reloaded());
                    status.success(format!("Opened {} ({count} object(s))", path.display()));
                }
                // The current document stays untouched on failure.
                Err(err) => status.error(err.to_string()),
            },
        }
    }
}

fn verb(command: &Command) -> &'static str {
    match command {
        Command::CreatePrimitive { .. } => "Created",
        Command::UpdateParameters { .. } | Command::Batch(_) => "Updated",
        Command::RenameObject { .. } => "Renamed",
        Command::DeleteObject { .. } => "Deleted",
        Command::SetObjectTransform { .. } => "Moved",
        Command::InsertFeature { .. } => "Restored",
    }
}

/// Human-readable name of the object a command targets, looked up before
/// the command runs.
fn subject_name(command: &Command, doc: &Document) -> String {
    let id = match command {
        Command::CreatePrimitive { name, .. } => return format!("'{}'", name.trim()),
        Command::InsertFeature { feature, .. } => return format!("'{}'", feature.name()),
        Command::Batch(commands) => match commands.first() {
            Some(first) => return subject_name(first, doc),
            None => return "nothing".into(),
        },
        Command::UpdateParameters { id, .. }
        | Command::RenameObject { id, .. }
        | Command::DeleteObject { id }
        | Command::SetObjectTransform { id, .. } => *id,
    };
    doc.feature(id)
        .map_or_else(|| format!("object {id}"), |f| format!("'{}'", f.name()))
}
