//! Converting primitives to editable meshes and editing their topology.

use cad_core::{
    BoxParameters, Command, CylinderParameters, Document, DocumentError, EditableMesh, FaceId,
    Geometry, MeshError, ObjectId, SphereParameters, Transform3D, VertexId, from_json, to_json,
};

const PLACEMENT: Transform3D = Transform3D {
    translation: [10.0, 20.0, 30.0],
    rotation_deg: [0.0, 0.0, 15.0],
};

fn doc_with(geometry: Geometry) -> (Document, ObjectId) {
    let mut doc = Document::new();
    let id = doc
        .apply(Command::create("Part", geometry, PLACEMENT))
        .unwrap()
        .affected[0];
    (doc, id)
}

fn cube() -> Geometry {
    Geometry::Box(BoxParameters::new(100.0, 100.0, 100.0).unwrap())
}

fn converted_cube() -> (Document, ObjectId) {
    let (mut doc, id) = doc_with(cube());
    doc.apply(Command::ConvertToEditable { id }).unwrap();
    (doc, id)
}

fn mesh(doc: &Document, id: ObjectId) -> &EditableMesh {
    &doc.feature(id)
        .unwrap()
        .geometry()
        .as_editable()
        .expect("editable")
        .mesh
}

fn counts(mesh: &EditableMesh) -> (usize, usize, usize) {
    (
        mesh.vertices().len(),
        mesh.edges().len(),
        mesh.faces().len(),
    )
}

/// The face whose normal points along `direction`.
fn face_facing(mesh: &EditableMesh, direction: [f64; 3]) -> FaceId {
    mesh.faces()
        .iter()
        .map(|f| f.id)
        .find(|&f| {
            let n = mesh.face_normal(f).unwrap();
            (n[0] * direction[0] + n[1] * direction[1] + n[2] * direction[2]) > 0.999
        })
        .expect("face with that normal")
}

/// Asserts that a failed command left the document exactly as it was.
fn assert_rejected(doc: &mut Document, command: Command) -> DocumentError {
    let before = doc.clone();
    let err = doc.apply(command).unwrap_err();
    assert_eq!(*doc, before, "failed command must not modify the document");
    err
}

// ---------------------------------------------------------------- conversion

#[test]
fn converting_a_box_creates_expected_topology_and_keeps_identity() {
    let (doc, id) = converted_cube();
    let feature = doc.feature(id).unwrap();
    assert_eq!(feature.id(), id);
    assert_eq!(feature.name(), "Part");
    assert_eq!(*feature.transform(), PLACEMENT);
    assert_eq!(feature.geometry().label(), "Mesh");
    assert_eq!(feature.geometry().primitive_kind(), None);

    let editable = feature.geometry().as_editable().unwrap();
    assert_eq!(editable.source.as_deref(), Some(&cube()), "provenance kept");
    assert_eq!(counts(&editable.mesh), (8, 12, 6));
    editable.mesh.validate().unwrap();

    let tri = feature.geometry().tessellate(&Default::default());
    let (lo, hi) = tri.bounds().unwrap();
    assert_eq!((lo, hi), ([-50.0, -50.0, 0.0], [50.0, 50.0, 100.0]));
}

#[test]
fn spheres_and_cylinders_convert_to_closed_meshes() {
    let (mut doc, cylinder) = doc_with(Geometry::Cylinder(
        CylinderParameters::new(10.0, 40.0).unwrap(),
    ));
    doc.apply(Command::ConvertToEditable { id: cylinder })
        .unwrap();
    // 24 segments: 2 rings, 24 side quads + 2 caps.
    assert_eq!(counts(mesh(&doc, cylinder)), (48, 72, 26));

    let (mut doc, sphere) = doc_with(Geometry::Sphere(SphereParameters::new(10.0).unwrap()));
    doc.apply(Command::ConvertToEditable { id: sphere })
        .unwrap();
    let (v, e, f) = counts(mesh(&doc, sphere));
    assert_eq!((v, f), (2 + 11 * 24, 24 + 10 * 24 + 24));
    assert_eq!(v + f - e, 2, "Euler characteristic of a closed sphere");
}

#[test]
fn converting_twice_or_unknown_objects_fails_without_changes() {
    let (mut doc, id) = converted_cube();
    assert_eq!(
        assert_rejected(&mut doc, Command::ConvertToEditable { id }),
        DocumentError::AlreadyEditable(id)
    );
    let missing = ObjectId::new();
    assert_eq!(
        assert_rejected(&mut doc, Command::ConvertToEditable { id: missing }),
        DocumentError::NotFound(missing)
    );
}

#[test]
fn conversion_can_be_reverted_with_its_inverse() {
    let (mut doc, id) = doc_with(cube());
    let original = doc.clone();
    let inverse = doc
        .apply(Command::ConvertToEditable { id })
        .unwrap()
        .inverse;
    doc.apply(inverse).unwrap();
    assert_eq!(doc, original);
}

#[test]
fn topology_edits_require_conversion_first() {
    let (mut doc, id) = doc_with(cube());
    let err = assert_rejected(
        &mut doc,
        Command::MoveVertex {
            id,
            vertex: VertexId(0),
            position: [0.0; 3],
        },
    );
    assert_eq!(err, DocumentError::NotEditable(id));
}

// ----------------------------------------------------------- vertex editing

#[test]
fn moving_a_vertex_changes_only_that_vertex() {
    let (mut doc, id) = converted_cube();
    let before = mesh(&doc, id).clone();
    let target = before.vertices()[7].id;

    let applied = doc
        .apply(Command::MoveVertex {
            id,
            vertex: target,
            position: [70.0, 60.0, 130.0],
        })
        .unwrap();
    assert_eq!(applied.affected, vec![id]);

    let after = mesh(&doc, id);
    assert_eq!(after.position(target), Some([70.0, 60.0, 130.0]));
    assert_eq!(counts(after), counts(&before));
    for (old, new) in before.vertices().iter().zip(after.vertices()) {
        assert_eq!(old.id, new.id, "vertex ids are stable");
        if old.id != target {
            assert_eq!(old.position, new.position);
        }
    }
    assert_eq!(before.edges(), after.edges());
    assert_eq!(before.faces(), after.faces());
    assert_eq!(*doc.feature(id).unwrap().transform(), PLACEMENT);
}

#[test]
fn moving_an_unknown_vertex_is_rejected() {
    let (mut doc, id) = converted_cube();
    let err = assert_rejected(
        &mut doc,
        Command::MoveVertex {
            id,
            vertex: VertexId(999),
            position: [0.0; 3],
        },
    );
    assert_eq!(
        err,
        DocumentError::Mesh(MeshError::UnknownVertex(VertexId(999)))
    );
}

#[test]
fn moving_a_vertex_to_non_finite_or_degenerate_positions_is_rejected() {
    let (mut doc, id) = converted_cube();
    let vertex = mesh(&doc, id).vertices()[0].id;
    for position in [
        [f64::NAN, 0.0, 0.0],
        [0.0, f64::INFINITY, 0.0],
        [0.0, 0.0, 1.0e9],
    ] {
        let err = assert_rejected(
            &mut doc,
            Command::MoveVertex {
                id,
                vertex,
                position,
            },
        );
        assert!(
            matches!(err, DocumentError::Mesh(MeshError::Position { .. })),
            "{err:?}"
        );
    }
    // Vertex 0 onto vertex 1 collapses the edge between them.
    let onto = mesh(&doc, id).vertices()[1].position;
    let err = assert_rejected(
        &mut doc,
        Command::MoveVertex {
            id,
            vertex,
            position: onto,
        },
    );
    assert!(matches!(
        err,
        DocumentError::Mesh(MeshError::DegenerateEdge(_))
    ));
}

// ------------------------------------------------------------- edge editing

#[test]
fn translating_an_edge_moves_both_endpoints_and_keeps_its_length() {
    let (mut doc, id) = converted_cube();
    let before = mesh(&doc, id).clone();
    let edge = before.edges()[3].clone();
    let length = before.edge_length(edge.id).unwrap();
    let offset = [5.0, -7.5, 12.25];

    doc.apply(Command::TranslateEdge {
        id,
        edge: edge.id,
        offset,
    })
    .unwrap();

    let after = mesh(&doc, id);
    assert!((after.edge_length(edge.id).unwrap() - length).abs() < 1e-9);
    for v in before.vertices() {
        let moved = after.position(v.id).unwrap();
        let expected = if edge.vertices.contains(&v.id) {
            [
                v.position[0] + offset[0],
                v.position[1] + offset[1],
                v.position[2] + offset[2],
            ]
        } else {
            v.position
        };
        assert_eq!(moved, expected, "{}", v.id);
    }
    // Orientation is unchanged: same endpoint order, same direction vector.
    assert_eq!(after.edge(edge.id).unwrap().vertices, edge.vertices);
}

#[test]
fn translating_an_invalid_edge_or_by_non_finite_offsets_is_atomic() {
    let (mut doc, id) = converted_cube();
    let edge = mesh(&doc, id).edges()[0].id;
    let unknown = cad_core::EdgeId(4242);
    assert_eq!(
        assert_rejected(
            &mut doc,
            Command::TranslateEdge {
                id,
                edge: unknown,
                offset: [1.0, 0.0, 0.0]
            }
        ),
        DocumentError::Mesh(MeshError::UnknownEdge(unknown))
    );
    let err = assert_rejected(
        &mut doc,
        Command::TranslateEdge {
            id,
            edge,
            offset: [0.0, f64::NAN, 0.0],
        },
    );
    assert!(matches!(
        err,
        DocumentError::Mesh(MeshError::InvalidOffset(_))
    ));
    // A vertex id is not an edge id, even if the number exists in the mesh.
    let vertex_number = mesh(&doc, id).vertices()[0].id.0;
    assert!(matches!(
        assert_rejected(
            &mut doc,
            Command::TranslateEdge {
                id,
                edge: cad_core::EdgeId(vertex_number),
                offset: [1.0, 0.0, 0.0]
            }
        ),
        DocumentError::Mesh(MeshError::UnknownEdge(_))
    ));
}

// ----------------------------------------------------------- face extrusion

#[test]
fn extruding_a_face_adds_the_expected_topology() {
    for distance in [50.0, -30.0] {
        let (mut doc, id) = converted_cube();
        let before = mesh(&doc, id).clone();
        let top = face_facing(&before, [0.0, 0.0, 1.0]);

        let applied = doc
            .apply(Command::ExtrudeFace {
                id,
                face: top,
                distance,
            })
            .unwrap();
        let after = mesh(&doc, id);
        after.validate().unwrap();

        // Face replaced by 4 sides + 1 cap; 4 new vertices; 4 rising + 4 rim edges.
        assert_eq!(counts(after), (12, 20, 10), "distance {distance}");
        assert!(after.face(top).is_none(), "extruded face is replaced");

        let cap = applied.new_face.expect("cap face reported");
        let cap_face = after.face(cap).unwrap();
        assert_eq!(cap_face.vertices.len(), 4);
        for v in &cap_face.vertices {
            assert_eq!(after.position(*v).unwrap()[2], 100.0 + distance);
        }
        let n = after.face_normal(cap).unwrap();
        assert!((n[2] - 1.0).abs() < 1e-12, "cap keeps the face orientation");

        // All untouched topology keeps its ids and data.
        for v in before.vertices() {
            assert_eq!(after.vertex(v.id), Some(v));
        }
        for e in before.edges() {
            assert_eq!(after.edge(e.id), Some(e));
        }
        for f in before.faces().iter().filter(|f| f.id != top) {
            assert_eq!(after.face(f.id), Some(f));
        }
        // New ids are fresh.
        assert!(cap.0 >= 26, "ids are never reused");
    }
}

#[test]
fn extruded_faces_can_be_extruded_again() {
    let (mut doc, id) = converted_cube();
    let top = face_facing(mesh(&doc, id), [0.0, 0.0, 1.0]);
    let cap = doc
        .apply(Command::ExtrudeFace {
            id,
            face: top,
            distance: 10.0,
        })
        .unwrap()
        .new_face
        .unwrap();
    let side = face_facing(mesh(&doc, id), [1.0, 0.0, 0.0]);
    doc.apply(Command::ExtrudeFace {
        id,
        face: cap,
        distance: 10.0,
    })
    .unwrap();
    doc.apply(Command::ExtrudeFace {
        id,
        face: side,
        distance: 5.0,
    })
    .unwrap();
    mesh(&doc, id).validate().unwrap();
}

#[test]
fn extrusion_rejects_zero_and_non_finite_distances() {
    let (mut doc, id) = converted_cube();
    let top = face_facing(mesh(&doc, id), [0.0, 0.0, 1.0]);
    for distance in [0.0, -0.0, f64::NAN, f64::INFINITY, 1.0e12] {
        let err = assert_rejected(
            &mut doc,
            Command::ExtrudeFace {
                id,
                face: top,
                distance,
            },
        );
        assert_eq!(err, DocumentError::Mesh(MeshError::InvalidDistance));
    }
}

#[test]
fn extrusion_rejects_unknown_non_planar_and_non_convex_faces() {
    let (mut doc, id) = converted_cube();
    assert!(matches!(
        assert_rejected(
            &mut doc,
            Command::ExtrudeFace {
                id,
                face: FaceId(777),
                distance: 1.0
            }
        ),
        DocumentError::Mesh(MeshError::UnknownFace(_))
    ));

    let top = face_facing(mesh(&doc, id), [0.0, 0.0, 1.0]);
    let corner = top_corner(mesh(&doc, id), top, [50.0, 50.0, 100.0]);

    // Lift one corner: the top face is no longer planar.
    let mut warped = doc.clone();
    warped
        .apply(Command::MoveVertex {
            id,
            vertex: corner,
            position: [50.0, 50.0, 120.0],
        })
        .unwrap();
    assert_eq!(
        assert_rejected(
            &mut warped,
            Command::ExtrudeFace {
                id,
                face: top,
                distance: 5.0
            }
        ),
        DocumentError::Mesh(MeshError::NonPlanarFace(top))
    );

    // Push one corner inwards within the plane: the top face becomes concave.
    let mut dented = doc.clone();
    dented
        .apply(Command::MoveVertex {
            id,
            vertex: corner,
            position: [-20.0, -20.0, 100.0],
        })
        .unwrap();
    assert_eq!(
        assert_rejected(
            &mut dented,
            Command::ExtrudeFace {
                id,
                face: top,
                distance: 5.0
            }
        ),
        DocumentError::Mesh(MeshError::NonConvexFace(top))
    );
}

fn top_corner(mesh: &EditableMesh, face: FaceId, at: [f64; 3]) -> VertexId {
    *mesh
        .face(face)
        .unwrap()
        .vertices
        .iter()
        .find(|&&v| mesh.position(v) == Some(at))
        .unwrap()
}

#[test]
fn extrusion_can_be_reverted_with_its_inverse() {
    let (mut doc, id) = converted_cube();
    let before = doc.clone();
    let top = face_facing(mesh(&doc, id), [0.0, 0.0, 1.0]);
    let inverse = doc
        .apply(Command::ExtrudeFace {
            id,
            face: top,
            distance: 25.0,
        })
        .unwrap()
        .inverse;
    doc.apply(inverse).unwrap();
    assert_eq!(doc, before);
}

// -------------------------------------------------------------- persistence

#[test]
fn editable_meshes_round_trip_with_stable_ids() {
    let (mut doc, id) = converted_cube();
    let top = face_facing(mesh(&doc, id), [0.0, 0.0, 1.0]);
    doc.apply(Command::ExtrudeFace {
        id,
        face: top,
        distance: 40.0,
    })
    .unwrap();
    let edge = mesh(&doc, id).edges()[2].id;
    doc.apply(Command::TranslateEdge {
        id,
        edge,
        offset: [0.0, 0.0, -3.5],
    })
    .unwrap();

    let json = to_json(&doc).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    let geometry = &value["features"][0]["geometry"];
    assert_eq!(geometry["type"], "mesh");
    assert_eq!(geometry["source"]["type"], "box");
    assert_eq!(geometry["mesh"]["vertices"][0]["id"], 0);

    let loaded = from_json(&json).unwrap();
    assert_eq!(
        loaded, doc,
        "ids, next_id, positions, and transform survive"
    );
    mesh(&loaded, id).validate().unwrap();
    assert_eq!(*loaded.feature(id).unwrap().transform(), PLACEMENT);
}

#[test]
fn loading_rejects_corrupted_meshes() {
    let (doc, _) = converted_cube();
    let json = to_json(&doc).unwrap();
    let corrupt = |f: &dyn Fn(&mut serde_json::Value)| {
        let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
        f(&mut value["features"][0]["geometry"]["mesh"]);
        from_json(&value.to_string())
    };

    // An open surface (one face removed).
    let open = corrupt(&|m| {
        m["faces"].as_array_mut().unwrap().pop();
    });
    // Two vertices with the same id.
    let duplicate = corrupt(&|m| m["vertices"][1]["id"] = m["vertices"][0]["id"].clone());
    // An edge to a vertex that does not exist.
    let dangling = corrupt(&|m| m["edges"][0]["vertices"][0] = 9999.into());
    // An id at or above the counter.
    let counter = corrupt(&|m| m["next_id"] = 3.into());

    for result in [open, duplicate, dangling, counter] {
        assert!(
            matches!(result, Err(cad_core::PersistenceError::Invalid(_))),
            "{result:?}"
        );
    }
}
