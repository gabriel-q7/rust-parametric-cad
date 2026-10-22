# rust-parametric-cad

A small parametric 3D CAD editor for the desktop, written in Rust with
[Bevy](https://bevyengine.org/) 0.19.1. You create boxes, spheres, and cylinders, edit
their dimensions in millimeters, and save projects as JSON. The parameters are the source
of truth: meshes are always regenerated from them.

Status: **Phase 1 (desktop MVP)**. WebAssembly support is planned for Phase 2.

## Quick start

```bash
cargo run     # start the editor (the first build takes a few minutes)
cargo test    # core and app tests; no window or GPU needed
```

Requirements: stable Rust 1.89 or newer, and a GPU with Vulkan, Metal, or DX12. Linux also
needs a few system libraries; see the [user guide](docs/user-guide.md#prerequisites).

Basic controls: **left click** selects, **right drag** orbits, **middle drag** pans, the
**wheel** zooms, and **Home** resets the view. Edit values in the inspector and press
**Enter** to apply them. To try a sample project, type `examples/demo.cad.json` in the
toolbar's File field and click **Open**.

## Features

- A 3D viewport with an orbit camera, a ground grid, world axes, and lighting.
- Box, sphere, and cylinder primitives with typed, validated parameters.
- Selection in the viewport or the object list; rename and delete.
- A properties inspector with dimensions, position, and rotation, applied atomically.
  Invalid input is rejected with a clear message.
- **Direct mesh editing**: convert a primitive into an editable mesh, then pick
  vertices, edges, and faces in the viewport to move vertices, translate edges,
  and extrude planar faces. Each element has a stable topology ID.
- JSON projects with a schema version (now 2; version 1 files still open), units,
  and stable UUIDs, all validated on load.

## Repository layout

```
crates/cad-core   domain model, validation, commands, JSON, tessellation (no Bevy)
crates/cad-app    Bevy desktop editor (viewport, UI, file IO)
examples/         sample project
docs/             documentation
```

## Documentation

| | |
| --- | --- |
| [User guide](docs/user-guide.md) | installing, using the editor, controls, troubleshooting |
| [Architecture](docs/architecture.md) | crates, data model, commands, frame schedule, coordinate frames |
| [Project file format](docs/file-format.md) | the `.cad.json` specification (schema version 2) |
| [Development guide](docs/development.md) | build, tests, conventions, adding primitives and commands |
| [Design decisions](docs/decisions.md) | why things are built the way they are |
| [Roadmap and limitations](docs/roadmap.md) | known limitations, next steps, Phase 2 checklist |

## Contributing

Before you submit a change, run all four checks:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets
cargo test
cargo run      # quick manual smoke test
```

Read the [development guide](docs/development.md) before you add features.
