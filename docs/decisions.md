# Design decisions

Short decision records. Each one gives the context, the decision, and its consequences. To
change a decision, add a new record that replaces the old one; do not edit history.

---

## D1. Two crates: a pure core and a Bevy app

**Context.** The editor must later run in the browser (Phase 2), and the CAD logic must be
testable without a window.

**Decision.** `cad-core` holds the domain model, validation, commands, persistence (as
strings), and tessellation. It has no dependency on Bevy or the OS. `cad-app` holds
everything specific to Bevy and to the desktop.

**Consequences.** Core tests run in milliseconds and never build Bevy. The cost is
conversion code at the boundary (`viewport/convert.rs`).

## D2. Pin Bevy to exactly 0.19.1

**Context.** Bevy changes its APIs in every release. When this project started, 0.20 had
just been released.

**Decision.** Use `bevy = "=0.19.1"` with `default-features = false, features = ["3d", "ui"]`.

**Consequences.** Every API used can be checked against one known source. 0.19 also
provides `EditableText`, which allows real text fields with no extra UI crate. Upgrading is
a deliberate task of its own.

## D3. Millimeters, `f64`, and Z-up in the document

**Context.** CAD users think in millimeters. Most CAD tools and exchange formats use Z-up.
Bevy uses meters, `f32`, and Y-up.

**Decision.** The document stores `f64` millimeters in a right-handed Z-up frame.
Conversion happens only when Bevy types are built. The change of frame is a single rotation
on the CAD root entity.

**Consequences.** The precision and conventions of the document do not depend on the
renderer. Gizmos that are drawn in world space must use `cad_direction`.

## D4. Typed parameters with private fields

**Context.** Invalid dimensions must never get into the model.

**Decision.** Each primitive has its own struct. Values can only be built through
validating constructors. Deserialized documents are always validated as a whole.

**Consequences.** Invalid states cannot be built in code. Loading has a single validation
step.

## D5. Commands in the core, with inverses

**Context.** Edits must be validated, predictable, and usable without the UI. Undo/redo
must stay easy to add later.

**Decision.** `cad_core::Command` is the only way to change a document. Applying a command
is atomic and returns the inverse command. `Batch` groups several commands into one atomic
change.

**Consequences.** The inspector's Apply becomes a single all-or-nothing change. Undo/redo
only needs a history stack (see the roadmap).

## D6. Tessellate in the core instead of using Bevy's mesh builders

**Context.** Bevy has `Cuboid`, `Sphere`, and `Cylinder` meshes. Using them would put
geometry generation inside the renderer.

**Decision.** `Geometry::tessellate` produces a `TriMesh` in millimeters. Bevy only
converts it.

**Consequences.** Geometry is tested headlessly (bounds and winding) and is portable. A
real kernel can replace the function later. The cost is about 150 lines of mesh code.

## D7. Incremental document-to-entity sync

**Context.** Editing one object should not rebuild the whole scene.

**Decision.** Command handlers send `DocumentChanged { affected, reloaded }`.
`sync_objects` updates only those IDs, through `ObjectEntityMap`. It re-tessellates only
when the geometry actually changed, and it overwrites the existing mesh asset in place.

**Consequences.** Entities keep their identity across edits. A full rebuild happens only on
New and Open.

## D8. File path in the toolbar instead of a native file dialog

**Context.** Native dialogs (for example the `rfd` crate) pull in GTK or portal dependencies
and behave differently on each platform.

**Decision.** The path is typed into a text field in the toolbar. All file access is in
`cad-app/src/commands/project.rs`. Saves write a temporary file and then rename it.

**Consequences.** Phase 1 has fewer dependencies, and the storage code can be swapped in
Phase 2. Usability is worse than with a dialog (see the roadmap).

## D9. Explicit Apply instead of live editing

**Context.** Applying values live, on every keystroke, has to deal with half-typed numbers
such as `-` or `1e`.

**Decision.** Edits are applied with Enter or the Apply button. All fields are validated
together.

**Consequences.** Validation and atomic application are simple. You do not see a preview
while typing.

## D10. Camera drags use cursor positions, not raw mouse motion

**Context.** Raw device motion (`AccumulatedMouseMotion`) depends on mouse acceleration and
input-device quirks. In testing, it made orbit and pan jump far too much.

**Decision.** Drag deltas are computed from `Window::cursor_position()` from one frame to
the next.

**Consequences.** Panning matches the cursor movement exactly. Dragging stops updating while
the cursor is outside the window.

## D11. Indexed polygon mesh, not half-edge or B-rep

**Context.** Direct editing needs stable vertex, edge, and face identities,
connectivity for validation, and extrusion. A half-edge structure or a B-rep
kernel is a large investment.

**Decision.** `EditableMesh` stores explicit vertex, edge, and face records with
`u32` IDs drawn from one per-mesh counter. Connectivity, meaning which faces
traverse which edges and in which direction, is derived when needed. The
invariant is a closed, consistently oriented 2-manifold with no zero-length
edges and no zero-area faces.

**Consequences.**

- Simple, serializable, and easy to validate.
- Every operation is O(n) in the mesh size, which is fine for primitive-sized
  meshes.
- A half-edge structure can be introduced behind the same API if mesh sizes or
  operations (loops, bevels) require it.

## D12. One-way conversion with provenance

**Context.** After arbitrary topology edits, the primitive parameters can no
longer be recovered reliably.

**Decision.** `ConvertToEditable` replaces the parametric geometry with a mesh
and keeps the original geometry in `source` as read-only provenance. The object's
ID, name, and transform are unchanged. There is no conversion back.

**Consequences.** The hybrid workflow stays honest: parametric objects stay
parametric until the user opts in. The inverse command (for a future undo) can
still restore the primitive exactly.

## D13. Pure mesh operations, generic geometry inverses

**Context.** Topology edits must be atomic and reversible.

**Decision.** Mesh operations return a new, validated mesh and never mutate in
place. The topology commands use `UpdateParameters { previous geometry }` as
their inverse.

**Consequences.** Atomicity holds by construction. Inverses are trivially
correct, at the cost of cloning the mesh for each edit, which is negligible at
this size.

## D14. X-ray topology overlay and screen-space picking

**Context.** Picking vertices and edges needs tolerance in pixels, and hidden
elements must remain reachable.

**Decision.**

- A gizmo group with `depth_bias = -1` draws the overlay on top of the scene.
- Vertices and edges are picked by projecting them to the screen and taking the
  nearest one within a pixel radius. Depth breaks ties.
- Faces are picked by a ray cast in the core.

**Consequences.** Works with any camera angle and needs no extra rendering
pipeline. Picking ignores occlusion, which matches what the overlay shows; Prev
/ Next remain as a precise fallback.

## D15. Schema version 2, reading 1 and 2

**Context.** Editable meshes add a new geometry type that older builds cannot
read.

**Decision.** Bump `CURRENT_SCHEMA_VERSION` to 2 and keep
`MIN_SUPPORTED_SCHEMA_VERSION` at 1. Version 1 is a strict subset of version 2,
so loading it only marks the document as version 2.

**Consequences.** Existing projects keep working. Saving always writes version 2,
which older builds reject with a clear "unsupported schema version" error.
