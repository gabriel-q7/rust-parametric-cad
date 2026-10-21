# Project file format

Projects are UTF-8 JSON files, usually named `*.cad.json`. This page specifies
**schema version 1**, the only version this build reads and writes.

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
| `schema_version` | integer | yes | Must be `1`. |
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
   - Any value other than `1` → `UnsupportedSchemaVersion`.
2. **Typed parse.** The value is converted into a `Document`. Invalid JSON, a wrong type, a
   missing field, an unknown `type`, an unknown `units` value, or an invalid UUID →
   `Malformed`.
3. **Validation.** `Document::validate` checks the rules above: positive and bounded
   dimensions, finite transforms, non-empty names, and unique IDs. Any violation →
   `Invalid`. The error names the feature ID that failed.

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
- When version 2 exists, the reader should upgrade version 1 files step by step: parse the
  generic value, run `migrate_v1_to_v2`, then do the typed parse. The version check already
  happens before the typed parse, which is where such a migration step would go.
