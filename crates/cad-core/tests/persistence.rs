//! JSON round trips and rejection of malformed or unsupported files.

use cad_core::{
    BoxParameters, CURRENT_SCHEMA_VERSION, Command, Document, Geometry, PersistenceError,
    PrimitiveKind, Transform3D, ValidationError, from_json, to_json,
};

fn sample_document() -> Document {
    let mut doc = Document::new();
    for (i, kind) in PrimitiveKind::ALL.into_iter().enumerate() {
        doc.apply(Command::create(
            &doc.suggest_name(kind),
            kind.default_geometry(),
            Transform3D {
                translation: [i as f64 * 150.0, -20.5, 0.0],
                rotation_deg: [0.0, 0.0, 15.0 * i as f64],
            },
        ))
        .unwrap();
    }
    doc
}

#[test]
fn round_trip_preserves_ids_parameters_names_and_transforms() {
    let doc = sample_document();
    let json = to_json(&doc).unwrap();
    let loaded = from_json(&json).unwrap();
    assert_eq!(loaded, doc);

    for (a, b) in doc.features().iter().zip(loaded.features()) {
        assert_eq!(a.id(), b.id());
        assert_eq!(a.geometry(), b.geometry());
    }
}

#[test]
fn json_contains_the_documented_fields() {
    let mut doc = Document::new();
    doc.apply(Command::create(
        "Plate",
        Geometry::Box(BoxParameters::new(200.0, 100.0, 5.0).unwrap()),
        Transform3D::default(),
    ))
    .unwrap();
    let value: serde_json::Value = serde_json::from_str(&to_json(&doc).unwrap()).unwrap();

    assert_eq!(value["schema_version"], CURRENT_SCHEMA_VERSION);
    assert_eq!(value["units"], "mm");
    let feature = &value["features"][0];
    assert!(
        feature["id"].as_str().unwrap().len() == 36,
        "UUID string id"
    );
    assert_eq!(feature["name"], "Plate");
    assert_eq!(
        feature["geometry"],
        serde_json::json!({ "type": "box", "width": 200.0, "depth": 100.0, "height": 5.0 })
    );
    assert_eq!(
        feature["transform"],
        serde_json::json!({ "translation": [0.0, 0.0, 0.0], "rotation_deg": [0.0, 0.0, 0.0] })
    );
}

#[test]
fn loads_a_hand_written_file() {
    let json = r#"{
        "schema_version": 1,
        "units": "mm",
        "features": [
            {
                "id": "6f1c2a3e-8a7b-4c1d-9e2f-0a1b2c3d4e5f",
                "name": "Shaft",
                "geometry": { "type": "cylinder", "radius": 6, "height": 140 }
            }
        ]
    }"#;
    let doc = from_json(json).unwrap();
    assert_eq!(doc.len(), 1);
    let shaft = &doc.features()[0];
    assert_eq!(
        shaft.id().to_string(),
        "6f1c2a3e-8a7b-4c1d-9e2f-0a1b2c3d4e5f"
    );
    assert_eq!(shaft.geometry().kind(), PrimitiveKind::Cylinder);
    assert_eq!(
        *shaft.transform(),
        Transform3D::default(),
        "transform defaults"
    );
}

#[test]
fn malformed_json_is_reported() {
    for input in ["", "{", "not json", "[1, 2, 3]", "{\"schema_version\": 1,}"] {
        let err = from_json(input).unwrap_err();
        assert!(
            matches!(
                err,
                PersistenceError::Malformed(_) | PersistenceError::MissingSchemaVersion
            ),
            "{input:?} -> {err:?}"
        );
    }
}

#[test]
fn structurally_wrong_documents_are_malformed() {
    let cases = [
        // Unknown primitive type.
        r#"{"schema_version":1,"units":"mm","features":[{"id":"6f1c2a3e-8a7b-4c1d-9e2f-0a1b2c3d4e5f","name":"X","geometry":{"type":"torus","radius":1}}]}"#,
        // Unsupported unit.
        r#"{"schema_version":1,"units":"inch","features":[]}"#,
        // Bad id.
        r#"{"schema_version":1,"units":"mm","features":[{"id":"42","name":"X","geometry":{"type":"sphere","radius":1}}]}"#,
        // Missing parameter.
        r#"{"schema_version":1,"units":"mm","features":[{"id":"6f1c2a3e-8a7b-4c1d-9e2f-0a1b2c3d4e5f","name":"X","geometry":{"type":"box","width":1,"depth":1}}]}"#,
    ];
    for json in cases {
        assert!(
            matches!(from_json(json), Err(PersistenceError::Malformed(_))),
            "{json}"
        );
    }
}

#[test]
fn unsupported_or_missing_schema_versions_are_rejected() {
    let future = r#"{"schema_version": 99, "units": "mm", "features": []}"#;
    assert!(matches!(
        from_json(future),
        Err(PersistenceError::UnsupportedSchemaVersion { found: 99, .. })
    ));
    let missing = r#"{"units": "mm", "features": []}"#;
    assert!(matches!(
        from_json(missing),
        Err(PersistenceError::MissingSchemaVersion)
    ));
}

#[test]
fn loaded_documents_must_pass_validation() {
    let id = "6f1c2a3e-8a7b-4c1d-9e2f-0a1b2c3d4e5f";
    let negative = format!(
        r#"{{"schema_version":1,"units":"mm","features":[{{"id":"{id}","name":"S","geometry":{{"type":"sphere","radius":-3}}}}]}}"#
    );
    let zero = format!(
        r#"{{"schema_version":1,"units":"mm","features":[{{"id":"{id}","name":"B","geometry":{{"type":"box","width":0,"depth":1,"height":1}}}}]}}"#
    );
    let blank_name = format!(
        r#"{{"schema_version":1,"units":"mm","features":[{{"id":"{id}","name":"  ","geometry":{{"type":"sphere","radius":3}}}}]}}"#
    );
    let duplicate = format!(
        r#"{{"schema_version":1,"units":"mm","features":[
            {{"id":"{id}","name":"A","geometry":{{"type":"sphere","radius":3}}}},
            {{"id":"{id}","name":"B","geometry":{{"type":"sphere","radius":3}}}}]}}"#
    );
    for json in [&negative, &zero, &blank_name] {
        assert!(
            matches!(
                from_json(json),
                Err(PersistenceError::Invalid(ValidationError::InFeature { .. }))
            ),
            "{json}"
        );
    }
    assert!(matches!(
        from_json(&duplicate),
        Err(PersistenceError::Invalid(ValidationError::DuplicateId(_)))
    ));
}

#[test]
fn errors_have_readable_messages() {
    let err = from_json(r#"{"schema_version": 2, "units": "mm", "features": []}"#).unwrap_err();
    assert_eq!(
        err.to_string(),
        "unsupported schema version 2 (this build supports version 1)"
    );
}
