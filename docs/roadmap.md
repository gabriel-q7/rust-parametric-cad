# Roadmap and limitations

## Current limitations (Phase 1)

| Area | Limitation | Notes |
| --- | --- | --- |
| Editing | No undo/redo | Inverse commands already exist; only a history stack and keybindings are missing. |
| Editing | Single selection | `Selection` holds one `ObjectId`. |
| Editing | No transform gizmo | Position and rotation can only be typed into fields. |
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
