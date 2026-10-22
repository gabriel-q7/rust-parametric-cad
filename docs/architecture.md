# Architecture

This page explains how the code is organized and how one edit travels from a click to the
screen. For the reasons behind these choices, see [decisions.md](decisions.md).

## Principles

1. **Parameters are the source of truth.** Meshes, entities, and UI state are derived from
   the document and can be rebuilt from it at any time.
2. **The core knows nothing about Bevy, windows, or files.** `cad-core` compiles and is
   tested without a GPU, a window, or filesystem access.
3. **One way to change the document.** All mutations go through `cad_core::Command`.
   Commands are validated and atomic.
4. **Incremental updates.** An edit touches only the entities of the objects it changed.

## Workspace layout

```
rust-parametric-cad/
├── Cargo.toml                 workspace; pins bevy = "=0.19.1"
├── examples/demo.cad.json     sample project (checked by a test)
├── docs/                      this documentation
└── crates/
    ├── cad-core/              library: domain model (no Bevy)
    │   ├── src/
    │   │   ├── lib.rs         re-exports the public API
    │   │   ├── units.rs       LengthUnit, mm <-> m
    │   │   ├── validation.rs  ValidationError and rule functions
    │   │   ├── parameters.rs  BoxParameters, SphereParameters, CylinderParameters
    │   │   ├── geometry.rs    Geometry, PrimitiveKind, conversion, tessellation -> TriMesh
    │   │   ├── mesh.rs        EditableMesh: topology, validation, edits, picking
    │   │   ├── feature.rs     ObjectId, Transform3D, Feature
    │   │   ├── document.rs    Document, DocumentError, CURRENT_SCHEMA_VERSION
    │   │   ├── command.rs     Command, Applied (inverse + affected ids)
    │   │   └── persistence.rs to_json / from_json, PersistenceError
    │   └── tests/             integration tests
    └── cad-app/               binary: Bevy desktop editor
        └── src/
            ├── main.rs        entry point
            ├── app.rs         window config, DefaultPlugins, demo document
            ├── plugins.rs     CadEditorPlugins (plugin group)
            ├── editor.rs      CadDocument, Selection, Status; EditorSystems sets
            ├── commands/      applying commands; project file IO
            ├── viewport/      camera, scene, core->Bevy conversion, entity sync, picking,
            │                  topology overlay + vertex/edge/face picking
            └── ui/            layout, toolbar actions, object list, inspector, shortcuts
```

Dependency direction: `cad-app → cad-core`. Never the other way.

## The core (`cad-core`)

### Data model

```mermaid
classDiagram
    Document "1" *-- "*" Feature : features (ordered)
    Feature *-- ObjectId
    Feature *-- Geometry
    Feature *-- Transform3D
    Geometry <|-- BoxParameters : Box
    Geometry <|-- SphereParameters : Sphere
    Geometry <|-- CylinderParameters : Cylinder
    class Document {
      schema_version: u32
      units: LengthUnit
      apply(Command) Result~Applied~
      validate()
    }
    class Feature {
      id: ObjectId
      name: String
      geometry: Geometry
      transform: Transform3D
    }
    class Transform3D {
      translation: [f64; 3]  mm
      rotation_deg: [f64; 3]
    }
```

How the model protects its invariants:

- The parameter structs, `Feature`, and `Document` have **private fields**. In code, values
  can only be built through validating constructors (`BoxParameters::new`, `Feature::new`,
  `Document::new`), so every such value is valid.
- Deserialization skips those constructors. That is why `from_json` always calls
  `Document::validate` afterwards.
- `Feature` setters are `pub(crate)`, so only commands can call them.

### Commands

```rust
pub enum Command {
    CreatePrimitive { id, name, geometry, transform },
    UpdateParameters { id, geometry },
    RenameObject { id, name },
    DeleteObject { id },
    SetObjectTransform { id, transform },
    InsertFeature { index, feature },   // inverse of DeleteObject
    Batch(Vec<Command>),                // all or nothing
    ConvertToEditable { id },           // one-way
    MoveVertex { id, vertex, position },
    TranslateEdge { id, edge, offset },
    ExtrudeFace { id, face, distance },
}
```

`Document::apply(command) -> Result<Applied, DocumentError>`:

- **Validation first.** Each command validates its input before it mutates anything. On
  `Err`, the document is unchanged.
- **Batch.** A batch runs on a clone of the document and replaces the original only if every
  command in it succeeds.
- **Result.** `Applied { inverse, affected }`. Applying `inverse` restores the previous
  state exactly, including the position of a deleted feature. `affected` lists the IDs of
  the objects that changed.
- **Topology commands.** These address elements by stable IDs (`ObjectId` plus
  `VertexId`, `EdgeId`, or `FaceId`), never by UI indices or entities.
  - On a parametric object they fail with `NotEditable`.
  - Their inverse is `UpdateParameters` with the previous geometry.
  - `Applied::new_face` reports the cap face created by an extrusion.
- **Caller-chosen IDs.** `CreatePrimitive` carries its ID, so replaying it (for example for a
  redo) recreates the same object. `Command::create` generates a fresh ID for convenience.

### Editable meshes

`Geometry::Mesh(EditableGeometry { mesh, source })` sits next to the parametric
variants. There is no separate document system for meshes: a converted object
is still a `Feature` with the same ID, name, and transform.

```
EditableMesh
├── vertices: [Vertex { id: VertexId, position: [f64; 3] }]   local mm
├── edges:    [Edge   { id: EdgeId, vertices: [VertexId; 2] }]
├── faces:    [Face   { id: FaceId, vertices: Vec<VertexId> }] CCW from outside
└── next_id:  u32      one counter for all three; IDs are never reused
```

This is an indexed polygon mesh with explicit topology records, not a half-edge
or B-rep kernel. Connectivity, meaning which faces use which edges and in which
direction, is derived when needed. That is enough for validation and extrusion
at this size; see [D11](decisions.md#d11-indexed-polygon-mesh-not-half-edge-or-b-rep).

**Invariant.** The surface is a closed, consistently oriented 2-manifold:

- every edge is walked once in each direction by the face boundaries;
- there are no zero-length edges and no zero-area faces.

The full list is in [file-format.md](file-format.md#editable-mesh-type-mesh-version-2).

**Operations.** All of them are pure: they take `&self`, return a new mesh, and
validate it before returning.

| Operation | Effect |
| --- | --- |
| `Geometry::to_editable` | Primitive → mesh: a box is 8/12/6; cylinders and spheres use `Tessellation::EDITABLE` (24 segments, 12 rings) |
| `with_vertex_moved(v, p)` | Sets one position |
| `with_edge_translated(e, d)` | Adds `d` to both endpoints (length and direction preserved) |
| `with_face_extruded(f, d)` | Checks planar + convex. Adds `v_i' = v_i + d·n`, rising edges, rim edges, side quads `[v_i, v_i+1, v'_i+1, v'_i]`, and a cap. Removes `f` and returns the cap ID. |
| `triangulate` | Flat-shaded fan triangulation for rendering |
| `pick_face(origin, dir)` | Nearest ray hit (Möller–Trumbore over the fan triangles) |

Unaffected elements keep their IDs and data. New elements always get fresh
IDs, and removed IDs are never reused.

### Geometry pipeline

```
BoxParameters ──┐
SphereParameters├─▶ Geometry ──tessellate(&Tessellation)──▶ TriMesh (mm, f64, Z-up)
CylinderParams ─┘
```

`TriMesh` is an indexed triangle list with per-vertex normals and counter-clockwise front
faces. Tests check the bounding boxes and that every triangle faces outward. A future B-rep
kernel can replace `tessellate` without changing the document model.

`Geometry::parameters()` and `Geometry::with_parameters(&[(key, value)])` give generic,
key-based access to the parameters. The inspector uses them, so it needs no code specific to
a primitive.

## The app (`cad-app`)

### Plugins

`app::build_app` adds Bevy's `DefaultPlugins` (with the window configuration) and then
`CadEditorPlugins`:

| Plugin | Adds |
| --- | --- |
| `EditorStatePlugin` | Resources `CadDocument`, `Selection`, `Status`; orders the `EditorSystems` sets |
| `CommandsPlugin` | Messages `EditCommand`, `ProjectCommand`, `DocumentChanged`; the systems that apply them |
| `ViewportPlugin` | `MeshPickingPlugin`, scene (lights, grid, axes), orbit camera, object sync and selection |
| `EditorUiPlugin` | `TabNavigationPlugin`, UI layout, the systems that handle actions, the list, the inspector, and the status bar |

### Messages

| Message | Sent by | Handled by |
| --- | --- | --- |
| `UiAction` | buttons, keyboard shortcuts | `ui::handle_ui_actions` |
| `InspectorRequest::{Apply, Revert}` | `handle_ui_actions` | `inspector::apply_inspector`, `inspector::rebuild_inspector` |
| `EditCommand(Command)` | UI | `commands::apply_edit_commands` |
| `ProjectCommand::{New, Open, Save}` | UI | `commands::handle_project_commands` |
| `DocumentChanged { affected, reloaded }` | command handlers, and once at startup | object sync, object list, inspector |
| `ResetCamera` | UI | `camera::reset_camera` |

### Frame schedule

Each frame runs three ordered system sets in `Update`:

```
EditorSystems::Input     buttons/keys -> UiAction -> EditCommand / ProjectCommand
                         inspector Apply -> one Batch command
                         camera input (orbit / pan / zoom)
        │
EditorSystems::Commands  apply_edit_commands, handle_project_commands
                         -> mutate CadDocument, set Status / Selection,
                            write DocumentChanged
        │
EditorSystems::Sync      sync_objects, highlight_selection,
                         rebuild_object_list, rebuild_inspector,
                         status bar, button/field visuals
```

Because of this order, an edit appears on screen in the same frame it is made.

### Life of an edit

What happens when you change a sphere's radius to 90 and press Enter:

```mermaid
sequenceDiagram
    participant K as keyboard_shortcuts
    participant H as handle_ui_actions
    participant I as apply_inspector
    participant C as apply_edit_commands
    participant D as Document
    participant S as sync_objects
    K->>H: UiAction::ApplyProperties
    H->>I: InspectorRequest::Apply
    I->>I: parse fields, Geometry::with_parameters, validate
    alt invalid
        I-->>I: Status::error, no command sent
    else valid
        I->>C: EditCommand(Batch[UpdateParameters])
        C->>D: apply
        D-->>C: Applied { affected: [id] }
        C->>S: DocumentChanged { affected: [id] }
        S->>S: same entity, re-tessellate, overwrite Mesh asset in place
    end
```

### Mapping objects to entities

- `ObjectEntityMap: HashMap<ObjectId, Entity>` is the only link between document objects
  and Bevy entities. Entity IDs are never stored in the document.
- Each render entity has `CadObject { id }` and `RenderedGeometry(Geometry)`.
- For every affected ID, `sync_objects` compares the document with the map:

  | In document | In map | Action |
  | --- | --- | --- |
  | yes | no | spawn the entity (mesh, material, transform) as a child of the CAD root |
  | yes | yes | update the transform and name; re-tessellate only if the `RenderedGeometry` differs |
  | no | yes | despawn the entity and remove it from the map |

- When `reloaded` is true (after New or Open, and at startup), every ID from the document
  and from the map is synced.

### Coordinate frames and units

| | Document (`cad-core`) | Bevy world |
| --- | --- | --- |
| Length unit | millimeter, `f64` | meter, `f32` |
| Up axis | +Z | +Y |
| Angles | degrees | radians (quaternions) |

`viewport/convert.rs` is the only place where these conversions happen:

- `mesh_from_trimesh` converts the core mesh to meters and builds a Bevy `Mesh`.
- `transform_from_cad` converts a `Transform3D` to meters and radians, using
  `EulerRot::ZYX` so that `R = Rz·Ry·Rx`.
- `CAD_TO_BEVY` is a −90° rotation about X. It is set on a single **CAD root** entity, and
  every object entity is a child of that root. Object transforms therefore stay in CAD terms,
  and the change of frame happens exactly once. CAD (x, y, z) maps to Bevy (x, z, −y).

### Selection model

`Selection` holds three things:

- the selected object;
- an `EditMode` (`Object`, `Vertex`, `Edge`, or `Face`);
- an optional `TopologyElement` (a `VertexId`, `EdgeId`, or `FaceId`).

Rules it keeps:

- Selecting a different object resets the mode.
- Changing the mode drops an element of another kind.
- After each command, `fix_up_selection` clears an element that no longer exists
  and resets the mode if the object is not editable.
- After an extrusion, the new cap face is selected.

### Topology overlay and picking (`viewport/topology.rs`)

- **Overlay.** A dedicated gizmo group with `depth_bias = -1` draws the
  wireframe on top of the scene (x-ray), plus vertex markers in vertex mode and
  the selected element in yellow. A face is shown with its outline, a hatch,
  and a normal arrow. Marker sizes scale with camera distance.
- **Picking.** On a left click in a topology mode, the click is turned into a
  pick, and the object-click observer ignores it.
  - Vertices: nearest projected vertex within 12 px.
  - Edges: nearest projected segment within 8 px.
  - Ties within 2 px go to the element nearer the camera.
  - Faces: the cursor ray is transformed into the object's local millimeter frame
    and passed to `EditableMesh::pick_face`.
- **Occlusion.** Vertex and edge picking ignore occlusion, so they match the
  x-ray overlay.

### Input handling details

- **Is the pointer over the UI?** `PointerOverUi` checks whether Bevy's picking `HoverMap`
  contains any UI `Node`. Transparent layout containers have `Pickable::IGNORE` so they do
  not block the viewport.
- **Selection.** A global observer on `Pointer<Click>` handles selection. Picking events
  bubble up to parent entities and finally to the window, so the observer uses
  `original_event_target()` and only reacts to the first, un-bubbled trigger. A hit on a
  `CadObject` selects it, a hit on the window deselects, and a hit on UI is ignored.
- **Camera.** The orbit camera computes drag deltas from the window cursor position, not
  from raw mouse motion, so panning tracks the cursor exactly. A drag starts only over the
  viewport but continues over the panels.
- **Text fields** use Bevy's `EditableText` widget. Keyboard shortcuts check `InputFocus` so
  that typing in a field does not trigger them. The inspector clears `InputFocus` when it
  rebuilds, because the focused field is despawned and Bevy can reuse its entity ID.

### Persistence boundary

```
UI (path) ─▶ ProjectCommand ─▶ commands/project.rs ─▶ std::fs  (desktop only)
                                     │
                                     ▼
                     cad_core::{to_json, from_json}  (strings only)
```

To use different storage (for example browser storage in Phase 2), replace
`commands/project.rs`. Nothing else needs to change.
