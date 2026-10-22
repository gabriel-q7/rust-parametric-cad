# Development guide

## Everyday commands

Run these from the repository root:

```bash
cargo run                                  # start the editor
cargo test                                 # all tests; no window or GPU needed
cargo test -p cad-core                     # core only (fast; does not build Bevy)
cargo fmt --all -- --check                 # formatting
cargo clippy --workspace --all-targets     # lints (should report 0 warnings)
```

Before you open a pull request, all four checks must pass: `fmt`, `clippy`, `test`, and a
manual `cargo run` smoke test.

### Build configuration

- **Bevy version.** Bevy is pinned to `=0.19.1` in the root `Cargo.toml`. Only use APIs from
  that release. If you are unsure whether an API exists, read the downloaded source in
  `~/.cargo/registry/src/*/bevy_*-0.19.1/`, including its `examples/`.
- **Bevy features.** `default-features = false` with `["3d", "ui"]`. Audio and 2D are left
  out.
- **Optimization.** Debug builds compile dependencies with `opt-level = 3` and the
  workspace crates with `opt-level = 1`, so the first build is slow but the editor runs
  smoothly in debug.
- **Lints.** `unsafe_code` is forbidden, and `clippy::all` is set to warn.
- **`cargo run`.** Both crates are default members. `cargo run` works because `cad-app` is
  the only binary in the workspace.

## Tests

| Location | What it covers |
| --- | --- |
| `crates/cad-core/src/**` (`#[cfg(test)]`) | units, validation, parameter constructors, tessellation (bounds and outward winding), naming |
| `crates/cad-core/tests/document_operations.rs` | the command API: create, update, rename, delete, batch atomicity, inverses, duplicate IDs |
| `crates/cad-core/tests/mesh_editing.rs` | conversion, vertex move, edge translate, extrusion (counts, connectivity, ±distance, invalid input, atomicity, inverses), mesh persistence and corrupted files |
| `crates/cad-core/src/mesh.rs` | mesh invariants, degenerate edits, face picking, triangulation |
| `crates/cad-core/tests/persistence.rs` | JSON round trips, field layout, malformed and unsupported files, validation on load, the example file |
| `crates/cad-app/src/commands/project.rs` | saving to and loading from real temporary files |
| `crates/cad-app/src/viewport/convert.rs` | mm → m, Z-up → Y-up, Euler order |
| `crates/cad-app/src/viewport/objects.rs` | **headless ECS** tests of the document → entity sync, including convert and extrude (same entity, cap selected) |
| `crates/cad-app/src/viewport/topology.rs` | screen-space pick helpers, local frame round trip |
| `crates/cad-app/src/editor.rs` | selection, mode, and element consistency |
| `crates/cad-app/src/ui/inspector.rs` | parsing property and topology fields into commands, error messages |
| `crates/cad-app/src/viewport/camera.rs` | orbit math and clamping |

### Headless app tests

`viewport/objects.rs` builds a real Bevy `App` without a window or renderer:

```rust
App::new()
    .add_plugins((MinimalPlugins, AssetPlugin::default()))
    .init_asset::<Mesh>()
    .init_asset::<StandardMaterial>()
    .add_plugins((EditorStatePlugin, CommandsPlugin, ObjectsPlugin));
```

It sends `EditCommand` or `ProjectCommand` messages with `world_mut().write_message(..)`,
calls `app.update()`, and then checks `ObjectEntityMap`, the components, and the
`Assets<Mesh>`. Use the same pattern to test new systems.

## Extending the code

### Add a new primitive (example: a cone)

Everything specific to a primitive lives in `cad-core`. The toolbar, the inspector, and the
persistence code pick up a new primitive automatically.

1. **Parameters**: in `parameters.rs`, add `ConeParameters` with private fields, a
   validating `new`, getters, `validate`, `descriptors`, and `Default`. Give each
   parameter a stable `key` (it becomes the JSON field name) and a `label`.
2. **Geometry**: in `geometry.rs`:
   - add `PrimitiveKind::Cone`, extend `ALL`, `label`, and `default_geometry`;
   - add `Geometry::Cone(ConeParameters)` (its JSON tag is `"cone"`);
   - extend the `match` arms in `kind`, `validate`, `parameters`, `with_parameters`, and
     `tessellate`;
   - write `tessellate_cone`. Follow the existing convention: centered in X/Y, resting on
     Z = 0, counter-clockwise faces, outward normals.
3. **Tests**: add a mesh test using `assert_well_formed`, a bounds check, and constructor
   tests for zero and negative values.
4. **Docs**: add the new type to [file-format.md](file-format.md).
5. **Schema**: adding a new `type` value does not change existing files. Older builds will
   reject files that contain it as `Malformed`. Decide whether that justifies bumping
   `CURRENT_SCHEMA_VERSION` (see the versioning policy in the file format document).

You do not need to touch `cad-app`. The compiler shows any remaining `match` arms that need
updating.

### Add a new command

1. Add a variant to `cad_core::Command`.
2. In `Command::apply`:
   - validate **before** mutating;
   - return `Applied { inverse, affected }`;
   - make sure applying `inverse` restores the exact previous state.
3. Add an inverse-round-trip test to `tests/document_operations.rs`.
4. In `cad-app/src/commands/mod.rs`, extend `verb` and `subject_name` so the status bar shows
   a message.
5. Send it from the UI with `EditCommand(Command::YourCommand { .. })`. Do not mutate
   `CadDocument` directly.

### Add a mesh operation (example: scale a face)

1. Write a pure method on `EditableMesh` in `mesh.rs` that takes `&self` and
   returns `Result<EditableMesh, MeshError>`:
   - clone the mesh;
   - change the clone;
   - allocate new element IDs only with `take_id`;
   - end with `mesh.validate()?`.
2. Add a `Command` variant and implement it with the `edit_mesh` helper in
   `command.rs`. That gives you `NotEditable` handling, provenance preservation,
   and the inverse for free.
3. Add tests to `tests/mesh_editing.rs`:
   - the expected counts;
   - that untouched IDs are unchanged;
   - rejected input leaves the document unchanged (`assert_rejected`);
   - applying the inverse restores the original.
4. In the app, add an inspector field and handle it in
   `inspector::build_topology_command`. Add a status verb in `commands/mod.rs`.

### Add a toolbar button or keyboard shortcut

1. Add a variant to `ui::UiAction` and handle it in `handle_ui_actions`.
2. Spawn the button with `widgets::button(label, UiAction::…)` in `ui/layout.rs`, or emit the
   action from `ui/shortcuts.rs`.
3. Keyboard shortcuts must respect text focus. Follow the early return in
   `keyboard_shortcuts`.

## Conventions

- **Units.** Core lengths are `f64` millimeters. Convert to meters only in
  `viewport/convert.rs`.
- **Errors.** `cad-core` uses `thiserror` enums, and error messages are written for users
  because they appear in the status bar.
- **Identity.** Refer to objects by `ObjectId` everywhere outside `viewport/objects.rs`. Do
  not persist or pass around `Entity` values for document objects.
- **Systems.** Put each new system in the right `EditorSystems` set:
  - **Input** produces messages;
  - **Commands** mutates the document;
  - **Sync** derives state from it.
- **UI containers.** Give transparent layout `Node`s the `Pickable::IGNORE` component, or
  they block clicks to the viewport.
- **Formatting and comments.** Use `rustfmt` defaults. Comments explain *why*, not *what*.

## Debugging tips

- **Logs.** Bevy logs to stderr. Set `RUST_LOG=info,cad_app=debug` for more detail.
- **Missing glyphs.** The default font is a FiraMono subset. Some symbols (for example `·`)
  are missing and render as boxes, so stick to ASCII in UI strings.
- **Clicks not reaching the viewport.** Some UI node is blocking the click. Check for a
  missing `Pickable::IGNORE`.
- **A selection that clears itself.** Remember that pointer events bubble up to the window;
  see "Input handling details" in [architecture.md](architecture.md).
