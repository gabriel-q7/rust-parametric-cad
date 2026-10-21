//! Application assembly: window/platform setup plus the editor plugins.
//!
//! Window configuration is kept here, separate from editor logic, so another
//! platform (e.g. a browser canvas in Phase 2) can reuse [`CadEditorPlugins`]
//! with different window settings.

use bevy::prelude::*;
use cad_core::{Command, Document, Geometry, PrimitiveKind, SphereParameters, Transform3D};

use crate::editor::CadDocument;
use crate::plugins::CadEditorPlugins;

pub fn build_app() -> App {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Rust Parametric CAD".into(),
            resolution: (1440, 900).into(),
            ..default()
        }),
        ..default()
    }))
    .insert_resource(CadDocument(demo_document()))
    .add_plugins(CadEditorPlugins);
    app
}

/// Starter scene with one primitive of each kind so the viewport is not empty.
fn demo_document() -> Document {
    let mut doc = Document::new();
    let items = [
        (PrimitiveKind::Box.default_geometry(), [-150.0, 0.0, 0.0]),
        (
            Geometry::Sphere(SphereParameters::new(60.0).expect("valid radius")),
            [0.0, 0.0, 0.0],
        ),
        (
            PrimitiveKind::Cylinder.default_geometry(),
            [150.0, 0.0, 0.0],
        ),
    ];
    for (geometry, translation) in items {
        let name = doc.suggest_name(geometry.kind());
        doc.apply(Command::create(
            &name,
            geometry,
            Transform3D::from_translation(translation),
        ))
        .expect("demo document is valid");
    }
    doc
}
