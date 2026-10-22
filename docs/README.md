# Documentation

| Document | Read it when you want to… |
| --- | --- |
| [User guide](user-guide.md) | install, run, and use the editor |
| [Architecture](architecture.md) | understand how the crates, data flow, and frame schedule fit together |
| [Project file format](file-format.md) | read, write, or generate `.cad.json` files |
| [Development guide](development.md) | build, test, and extend the code (new primitive, new command) |
| [Design decisions](decisions.md) | know *why* things are the way they are |
| [Roadmap and limitations](roadmap.md) | see what is missing and what Phase 2 needs |

## Glossary

| Term | Meaning |
| --- | --- |
| **Document** | The authoritative CAD model (`cad_core::Document`): schema version, units, ordered features. |
| **Feature** | One named, placed object in the document: `id`, `name`, `geometry`, `transform`. |
| **Primitive** | A kind of parametric solid: box, sphere, or cylinder (`PrimitiveKind`). |
| **Geometry** | A primitive kind plus its typed parameters (`Geometry::Box(BoxParameters)`, …). |
| **Parameters** | The dimensions that define a primitive, in millimeters. The source of truth. |
| **ObjectId** | A stable UUID v4 that identifies a feature across edits, saves, and loads. |
| **Command** | A data value that describes one document mutation (`cad_core::Command`). |
| **Inverse** | The command that undoes an applied command; returned by every `apply`. |
| **TriMesh** | Renderer-agnostic triangle mesh generated from geometry (derived data). |
| **Editable mesh** | Geometry converted from a primitive into explicit vertices, edges, and faces (`Geometry::Mesh`). |
| **Topology ID** | `VertexId`, `EdgeId`, or `FaceId`: stable within one mesh and never reused. |
| **Edit mode** | What viewport clicks select: object, vertex, edge, or face. |
| **Render entity** | The Bevy entity that displays a feature. Derived from the document; never persisted. |
