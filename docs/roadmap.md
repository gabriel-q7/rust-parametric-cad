# Roadmap and limitations

## Current limitations (Phase 1)

| Area | Limitation | Notes |
| --- | --- | --- |
| Editing | No undo/redo | Inverse commands already exist; only a history stack and keybindings are missing. |
| Editing | Single selection | `Selection` holds one `ObjectId`. |
| Editing | No transform gizmo | Position, rotation, vertices, and edge offsets are typed into fields. Bevy 0.19.1 ships `TransformGizmoPlugin` (see below). |
| Mesh editing | Conversion is one-way | By design ([D12](decisions.md#d12-one-way-conversion-with-provenance)); the inverse command could restore the primitive once undo exists. |
| Mesh editing | Only vertex move, edge translate, and face extrusion | No subdivision, bevel, collapse, loops, merge, or delete. |
| Mesh editing | Extrusion needs a planar, convex face and always uses the face normal | Faces made non-planar by vertex moves cannot be extruded. |
| Mesh editing | Self-intersection is not detected | For example, extruding a whole face inwards past the opposite side, or a vertex pushed through the solid. Topology stays valid; the geometry may overlap. |
| Mesh editing | Non-planar or concave faces render with fan triangulation | It can look wrong for strongly concave faces after vertex edits. |
| Mesh editing | Vertex and edge picking ignore occlusion (x-ray) | Hidden elements can be picked. Use Prev / Next or orbit. |
| Mesh editing | No multi-selection of elements | One vertex, edge, or face at a time. |
| Editing | No live preview | Changes are applied with Enter or Apply ([D9](decisions.md#d9-explicit-apply-instead-of-live-editing)). |
| Files | No native file dialog | The path is typed into the toolbar ([D8](decisions.md#d8-file-path-in-the-toolbar-instead-of-a-native-file-dialog)). |
| Files | No unsaved-changes prompt | New and Open replace the document immediately. |
| UI | Text fields have no placeholder text or in-field undo | Limits of Bevy 0.19's `EditableText`. |
| UI | The object list has no visible scrollbar | Mouse-wheel scrolling works. |
| UI | Only ASCII renders reliably in labels | The default font is a subset. |
| Rendering | Fixed tessellation quality (48 segments, 24 rings) | `Tessellation` can be configured in code, but the UI does not expose it. |
| Platforms | Tested only on Linux X11 with Vulkan | Windows, macOS, Wayland, and HiDPI have not been verified. |

## Suggested next steps

Listed roughly by value relative to effort:

0. **Interactive gizmo for objects, vertices, and edges.** Bevy 0.19.1 ships
   `bevy::gizmos::transform_gizmo::{TransformGizmoPlugin, TransformGizmoFocus,
   TransformGizmoState}`, with translate, rotate, and scale handles plus snapping.
   - For objects: focus it on the object entity, and send a
     `SetObjectTransform` command when the drag ends.
   - For a vertex or edge: focus it on a proxy entity at the element, and turn the
     drag delta into `MoveVertex` or `TranslateEdge`.
   - While `TransformGizmoState` is active, block orbit and picking.

   This needs no new dependency.
1. **Undo/redo.** Push `Applied::inverse` onto an undo stack in `apply_edit_commands`. When
   undoing, apply the inverse and push *its* inverse onto a redo stack. Clear both stacks on
   New and Open. Bind Ctrl+Z and Ctrl+Shift+Z.
2. **Dirty flag and confirmation.** Track whether the document has unsaved changes, and
   confirm before New and Open replace it.
3. **Native file dialog**, for example with `rfd`, behind the existing
   `commands/project.rs` boundary.
4. **Duplicate object** (Ctrl+D): a `CreatePrimitive` command with a new ID and the copied
   geometry.
5. **Frame selection** (F key): fit the camera to the selected object's bounds
   (`TriMesh::bounds`).
6. **More primitives**: cone, torus, wedge. See "Add a new primitive" in
   [development.md](development.md).

## Ecosystem crates (researched 2026-10-21, against Bevy 0.19.1)

| Option | Works with 0.19.1? | Use for |
| --- | --- | --- |
| Built-in `TransformGizmoPlugin` (`bevy_gizmos`) | yes, already compiled | Object, vertex, and edge drag handles (step 0 above) |
| Built-in `InfiniteGridPlugin` (`bevy_dev_tools` feature) | yes | Replacing the hand-drawn gizmo grid |
| `bevy_mod_outline` 0.13 | yes | Selection and hover outlines instead of a material swap |
| `bevy_editor_cam` 0.10 / `bevy_panorbit_camera` 0.35 | yes | Replacing the hand-written orbit camera if it becomes a burden; `bevy_editor_cam` targets CAD and editors |
| `transform-gizmo-bevy` 0.11 | yes | Only if the built-in gizmo is too limited (adds the `epaint` and `emath` crates from egui) |
| `bevy_egui` 0.42 + `bevy-inspector-egui` 0.37 | yes | A debug inspector behind a feature flag. Not for the main UI: it would be a second UI stack. |
| `bevy_transform_gizmo` (fslabs) | **no**: last release targets Bevy 0.14 with `bevy_mod_picking` | Superseded by the built-in gizmo |
| `bevy_editor_pls`, `bevy_infinite_grid` | **no** (Bevy 0.14 / 0.18) | Not needed |
| `WireframePlugin` (built-in) | yes, but native-only and depth-tested | Not suitable for the x-ray overlay or Wasm |

## Phase 2: WebAssembly

The groundwork already in place:

- `cad-core` does not use Bevy, threads, or the filesystem.
- File access is isolated in `cad-app/src/commands/project.rs`.
- Window setup is isolated in `app.rs`, separate from `CadEditorPlugins`.

What still needs to be done:

- [ ] Enable `uuid`'s `js` feature (or `getrandom`'s wasm backend) for `ObjectId::new` on
  `wasm32-unknown-unknown`.
- [ ] Choose Bevy features for the web (`webgl2` or `webgpu`), and decide whether to disable
  `bevy_gilrs`, `x11`, and `wayland` on wasm targets.
- [ ] Replace `commands/project.rs` on wasm with browser storage, downloads, and file
  uploads. Keep `to_json` / `from_json` unchanged.
- [ ] Configure the window to use a canvas selector in `app.rs` for the web build.
- [ ] Add a build tool (for example `trunk` or `wasm-bindgen-cli`) and a small HTML host
  page.
- [ ] Check that `EditableText` works with the browser's keyboard input and IME.
- [ ] Add CI jobs that run `cargo build --target wasm32-unknown-unknown -p cad-core` (and
  later `-p cad-app`).

## Out of scope until the foundation is stable

Sketches and constraints, extrude and revolve, boolean operations, B-rep topology,
STEP/IGES/STL export, assemblies, technical drawings, collaboration, and accounts.
