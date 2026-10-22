//! End-to-end document editing through the public command API.

use cad_core::{
    BoxParameters, Command, CylinderParameters, Document, DocumentError, Geometry, ObjectId,
    PrimitiveKind, SphereParameters, Transform3D, ValidationError,
};

fn create(doc: &mut Document, kind: PrimitiveKind) -> ObjectId {
    let name = doc.suggest_name(kind);
    let command = Command::create(&name, kind.default_geometry(), Transform3D::default());
    let applied = doc.apply(command).expect("valid create");
    applied.affected[0]
}

#[test]
fn creates_each_supported_primitive() {
    let mut doc = Document::new();
    for kind in PrimitiveKind::ALL {
        let id = create(&mut doc, kind);
        let feature = doc.feature(id).unwrap();
        assert_eq!(feature.geometry().primitive_kind(), Some(kind));
        assert_eq!(feature.name(), format!("{} 1", kind.label()));
    }
    assert_eq!(doc.len(), 3);
    doc.validate().unwrap();
}

#[test]
fn rejects_creating_a_primitive_with_invalid_dimensions() {
    // Invalid parameters cannot even be constructed through the public API.
    assert!(BoxParameters::new(10.0, 0.0, 10.0).is_err());
    assert!(SphereParameters::new(-1.0).is_err());
    assert!(CylinderParameters::new(f64::NAN, 1.0).is_err());

    let mut doc = Document::new();
    let result = doc.apply(Command::create(
        "",
        PrimitiveKind::Box.default_geometry(),
        Transform3D::default(),
    ));
    assert!(matches!(
        result,
        Err(DocumentError::Invalid(ValidationError::EmptyName))
    ));
    assert!(doc.is_empty());
}

#[test]
fn updating_parameters_preserves_identity_and_other_state() {
    let mut doc = Document::new();
    let id = create(&mut doc, PrimitiveKind::Cylinder);
    doc.apply(Command::SetObjectTransform {
        id,
        transform: Transform3D::from_translation([5.0, 6.0, 7.0]),
    })
    .unwrap();

    let new_geometry = Geometry::Cylinder(CylinderParameters::new(12.5, 80.0).unwrap());
    let applied = doc
        .apply(Command::UpdateParameters {
            id,
            geometry: new_geometry.clone(),
        })
        .unwrap();

    assert_eq!(applied.affected, vec![id]);
    let feature = doc.feature(id).unwrap();
    assert_eq!(feature.id(), id);
    assert_eq!(*feature.geometry(), new_geometry);
    assert_eq!(feature.name(), "Cylinder 1");
    assert_eq!(feature.transform().translation, [5.0, 6.0, 7.0]);
}

#[test]
fn invalid_parameter_update_leaves_document_unchanged() {
    let mut doc = Document::new();
    let id = create(&mut doc, PrimitiveKind::Box);
    let before = doc.clone();

    // Edits arrive from the UI as key/value pairs; validation happens first.
    let current = doc.feature(id).unwrap().geometry().clone();
    assert!(current.with_parameters(&[("width", 0.0)]).is_err());
    assert!(current.with_parameters(&[("height", -4.0)]).is_err());

    // Even a hand-built invalid geometry is rejected by the command.
    let bogus: Geometry =
        serde_json::from_str(r#"{"type":"box","width":-1,"depth":1,"height":1}"#).unwrap();
    assert!(
        doc.apply(Command::UpdateParameters {
            id,
            geometry: bogus
        })
        .is_err()
    );
    assert_eq!(doc, before);
}

#[test]
fn rename_and_delete() {
    let mut doc = Document::new();
    let id = create(&mut doc, PrimitiveKind::Sphere);

    doc.apply(Command::RenameObject {
        id,
        name: "  Ball bearing ".into(),
    })
    .unwrap();
    assert_eq!(doc.feature(id).unwrap().name(), "Ball bearing");

    assert!(
        doc.apply(Command::RenameObject {
            id,
            name: "   ".into()
        })
        .is_err()
    );
    assert_eq!(doc.feature(id).unwrap().name(), "Ball bearing");

    doc.apply(Command::DeleteObject { id }).unwrap();
    assert!(!doc.contains(id));
    assert_eq!(
        doc.apply(Command::DeleteObject { id }),
        Err(DocumentError::NotFound(id))
    );
}

#[test]
fn batch_is_atomic() {
    let mut doc = Document::new();
    let id = create(&mut doc, PrimitiveKind::Box);
    let before = doc.clone();

    let result = doc.apply(Command::Batch(vec![
        Command::RenameObject {
            id,
            name: "Renamed".into(),
        },
        Command::SetObjectTransform {
            id,
            transform: Transform3D::from_translation([f64::INFINITY, 0.0, 0.0]),
        },
    ]));
    assert!(result.is_err());
    assert_eq!(
        doc, before,
        "first command of a failed batch must not stick"
    );
}

#[test]
fn inverse_commands_restore_previous_state() {
    let mut doc = Document::new();
    let keep = create(&mut doc, PrimitiveKind::Box);
    let id = create(&mut doc, PrimitiveKind::Sphere);
    create(&mut doc, PrimitiveKind::Cylinder);
    let original = doc.clone();

    let edits = vec![
        Command::Batch(vec![
            Command::RenameObject {
                id,
                name: "Edited".into(),
            },
            Command::UpdateParameters {
                id,
                geometry: Geometry::Sphere(SphereParameters::new(3.0).unwrap()),
            },
            Command::SetObjectTransform {
                id,
                transform: Transform3D {
                    translation: [1.0, 2.0, 3.0],
                    rotation_deg: [0.0, 0.0, 45.0],
                },
            },
        ]),
        Command::DeleteObject { id },
        Command::DeleteObject { id: keep },
    ];
    let mut inverses = Vec::new();
    for edit in edits {
        inverses.push(doc.apply(edit).unwrap().inverse);
    }
    assert_eq!(doc.len(), 1);

    for inverse in inverses.into_iter().rev() {
        doc.apply(inverse).unwrap();
    }
    assert_eq!(doc, original, "order and content must be restored exactly");
}

#[test]
fn duplicate_ids_are_rejected() {
    let mut doc = Document::new();
    let id = ObjectId::new();
    let create = || Command::CreatePrimitive {
        id,
        name: "A".into(),
        geometry: PrimitiveKind::Box.default_geometry(),
        transform: Transform3D::default(),
    };
    doc.apply(create()).unwrap();
    assert_eq!(doc.apply(create()), Err(DocumentError::DuplicateId(id)));
}
