# Project file format

Projects are UTF-8 JSON files, usually named `*.cad.json`. This page specifies
**schema version 2**. This build writes version 2 and reads versions 1 and 2.

| Version | Adds |
| --- | --- |
| 1 | Parametric primitives: `box`, `sphere`, `cylinder` |
| 2 | Editable meshes: `"type": "mesh"` |

Every version 1 file is also a valid version 2 file. On load it is accepted as
is and marked as version 2, so the next save writes `"schema_version": 2`. Older
builds cannot read version 2 files.

The implementation is in [`crates/cad-core/src/persistence.rs`](../crates/cad-core/src/persistence.rs).
The tests in [`crates/cad-core/tests/persistence.rs`](../crates/cad-core/tests/persistence.rs)
cover every rule on this page.

## Example

```json
{
  "schema_version": 1,
  "units": "mm",
  "features": [
    {
      "id": "0b6f6c1e-3f5a-4c0e-9a51-2f1d7c9e4a10",
      "name": "Base plate",
      "geometry": { "type": "box", "width": 300.0, "depth": 200.0, "height": 12.0 },
      "transform": { "translation": [0.0, 0.0, 0.0], "rotation_deg": [0.0, 0.0, 0.0] }
    },
    {
      "id": "c4e8f0a1-7d2b-4b6a-9c3e-8f1a2d5b7e33",
      "name": "Arm",
      "geometry": { "type": "cylinder", "radius": 10.0, "height": 180.0 },
      "transform": { "translation": [-100.0, 0.0, 150.0], "rotation_deg": [0.0, 90.0, 0.0] }
    }
  ]
}
```

For a complete file, see [`examples/demo.cad.json`](../examples/demo.cad.json).

## Top level

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `schema_version` | integer | yes | `1` or `2`. Written as `2`. |
| `units` | string | yes | Length unit. Only `"mm"` is supported. |
| `features` | array of [feature](#feature) | yes | Objects in creation order. The order is preserved. |

## Feature

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `id` | string (UUID) | yes | Stable identifier, hyphenated UUID form. Unique within the document. |
| `name` | string | yes | Display name. Must not be empty after trimming; at most 128 characters. |
| `geometry` | [geometry](#geometry) | yes | Primitive type and parameters. |
| `transform` | [transform](#transform) | no | Placement. If omitted, the identity transform is used. |

## Geometry

`geometry` is an object tagged by `type`. The parameters sit next to `type`.

| `type` | Parameters (millimeters) | Local shape |
| --- | --- | --- |
| `"box"` | `width` (along X), `depth` (along Y), `height` (along Z) | Centered in X/Y, from Z = 0 to Z = height |
| `"sphere"` | `radius` | Centered in X/Y, center at Z = radius |
| `"cylinder"` | `radius`, `height` | Axis along +Z, centered in X/Y, from Z = 0 to Z = height |

Every parameter is required and must be a finite number with `0 < value ≤ 1,000,000`.

### Editable mesh (`"type": "mesh"`, version 2)

```json
"geometry": {
  "type": "mesh",
  "mesh": {
    "vertices": [ { "id": 0, "position": [-50.0, -50.0, 0.0] }, ... ],
    "edges":    [ { "id": 8, "vertices": [0, 2] }, ... ],
    "faces":    [ { "id": 20, "vertices": [0, 2, 3, 1] }, ... ],
    "next_id": 26
  },
  "source": { "type": "box", "width": 100.0, "depth": 100.0, "height": 100.0 }
}
```

| Field | Description |
| --- | --- |
| `mesh.vertices[]` | `id` (integer) and `position` (`[x, y, z]` in mm, the object's local frame). |
| `mesh.edges[]` | `id` and `vertices`: the IDs of its two endpoint vertices. |
| `mesh.faces[]` | `id` and `vertices`: the boundary vertex IDs, counter-clockwise when seen from outside. |
| `mesh.next_id` | The next unused topology ID. New elements take IDs from here, and IDs are never reused. |
| `source` | Optional, read-only provenance: the parametric primitive the mesh was converted from. It is not updated by later edits. |

Topology IDs are unique across vertices, edges, and faces of the same mesh. They
are only meaningful within that mesh.

A mesh must satisfy all of the following:

- Every ID is unique and less than `next_id`.
- Positions are finite with `|value| ≤ 1,000,000`.
- Each edge joins two different existing vertices, has a non-zero length, and is
  the only edge between those two vertices.
- Each face has at least 3 distinct vertices and a non-zero area. Every pair of
  consecutive boundary vertices (including last → first) is joined by an edge.
- The surface is closed and consistently oriented: every edge is walked exactly
  once in each direction by the face boundaries.
- Every vertex belongs to at least one face.
- `source`, if present, is a valid `box`, `sphere`, or `cylinder`.

Faces do not have to be planar. Only extrusion requires planar, convex faces.

## Transform

| Field | Type | Description |
| --- | --- | --- |
| `translation` | `[x, y, z]` numbers | Position in millimeters. Each component must be finite with `|value| ≤ 1,000,000`. |
| `rotation_deg` | `[rx, ry, rz]` numbers | Rotation in degrees. Each component must be finite. |

Coordinate system: right-handed, **Z up**.

The full transform of a point `p` is `translation + Rz · Ry · Rx · p`. In words: the shape is
rotated about the fixed X axis, then about the fixed Y axis, then about the fixed Z axis,
and then moved by `translation`. There is no scale, so dimensions always come from the
parameters.

## How files are loaded

`from_json` checks a file in three steps and stops at the first error:

1. **Version check.** The JSON is parsed as a generic value and `schema_version` is read.
   - Missing, or not a non-negative integer → `MissingSchemaVersion`.
   - Any value outside `1..=2` → `UnsupportedSchemaVersion`.
2. **Typed parse.** The value is converted into a `Document`. Invalid JSON, a wrong type, a
   missing field, an unknown `type`, an unknown `units` value, or an invalid UUID →
   `Malformed`.
3. **Validation.** `Document::validate` checks the rules above: positive and bounded
   dimensions, finite transforms, non-empty names, unique IDs, and the mesh
   invariants. Any violation → `Invalid`. The error names the feature ID that failed.
4. **Upgrade.** Version 1 documents are marked as version 2. No data changes
   are needed.

Unknown extra fields are ignored.

## Writing files

`to_json` writes pretty-printed JSON with every field present, including `transform`.
Numbers are written as JSON floats (`100.0`). The desktop app writes to `<path>.json.tmp`
first and then renames that file over the target path.

## Versioning policy

- `schema_version` changes whenever the meaning or the required structure of the format
  changes.
- Adding optional fields that older readers can safely ignore does not require a new
  version.
- If a future version changes existing data rather than only adding to it, add a
  migration step between the version check and the typed parse in
  `persistence::from_json`. Migrate one version at a time
  (`migrate_v2_to_v3`, …), working on the generic JSON value.
