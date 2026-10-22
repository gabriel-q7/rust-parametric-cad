# User guide

## Install and run

### Prerequisites

- A stable Rust toolchain, Rust 1.89 or newer (edition 2024). Install it with [rustup](https://rustup.rs/).
- A GPU and driver that support Vulkan, Metal, or DX12.
- Linux only: the system libraries Bevy needs. For example, on Debian or Ubuntu:

  ```bash
  sudo apt install pkg-config libudev-dev libwayland-dev libxkbcommon-dev
  ```

  Bevy's [Linux dependencies page](https://github.com/bevyengine/bevy/blob/main/docs/linux_dependencies.md)
  lists the packages for other distributions. Audio is disabled, so ALSA is not needed.

### Start the editor

```bash
git clone <repository-url>
cd rust-parametric-cad
cargo run
```

The first build compiles Bevy and takes several minutes; later builds take a few seconds.
The window opens with a demo scene: one box, one sphere, and one cylinder.

## The window

```
+-----------------------------------------------------------------+
| Create: [+ Box] [+ Sphere] [+ Cylinder]  File: [path] Open Save  |  1 toolbar
+------------------------------------------+----------------------+
|                                          | Objects              |  3 object list
|              3D viewport                 |                      |
|                                          | Properties           |  4 inspector
|                                          |                      |
|  controls cheat sheet                    | [Apply][Revert][Del] |
+------------------------------------------+----------------------+
| status message                                                  |  5 status bar
+-----------------------------------------------------------------+
            2 viewport
```

1. **Toolbar**: create primitives, set the project path, Open / Save / New, reset the view.
2. **Viewport**: the 3D scene with a ground grid (10 mm minor cells, 100 mm major cells)
   and the world axes: X red, Y green, Z blue (up).
3. **Object list**: every object in the document, in creation order. The selected one is
   highlighted in orange. Scroll it with the mouse wheel.
4. **Inspector**: the selected object's name, dimensions, position, and rotation.
5. **Status bar**: the result of the last action. Gray means information, green means
   success, red means error.

## Tasks

### Create an object

Click **+ Box**, **+ Sphere**, or **+ Cylinder**. The new object:

- gets default dimensions (box 100 × 100 × 100 mm, sphere radius 50 mm,
  cylinder radius 40 mm and height 120 mm);
- gets the next free name, such as `Box 2`;
- is placed on a free spot of a 150 mm grid so it does not overlap existing objects;
- becomes the selection.

### Select an object

- Left-click it in the viewport, or click its row in the object list.
- To deselect, click empty space in the viewport or press **Esc**.

### Change dimensions, position, or rotation

1. Select the object.
2. Click a field in the inspector and type a new value. **Tab** moves to the next field.
3. Press **Enter** or click **Apply**.

All fields are checked together. If every value is valid, they are applied as a single
change, the mesh is regenerated, and the status bar shows `Updated '<name>'`. If any value
is invalid, nothing changes and the status bar explains the problem, for example:

```
Invalid dimension: radius must be greater than 0 mm (got -5)
Width: 'abc' is not a number
```

Validation rules:

| Field | Rule |
| --- | --- |
| Dimensions | a finite number greater than 0 and at most 1,000,000 mm (1 km) |
| Position | a finite number between −1,000,000 and 1,000,000 mm |
| Rotation | any finite number of degrees |
| Name | not empty after trimming spaces; at most 128 characters |

Click **Revert** to throw away edits you have not applied yet.

How placement works:

- Each primitive is centered on its own X/Y origin and rests on its own Z = 0. A box with
  position Z = 0 therefore sits on the grid.
- The rotation is applied first, about the fixed X axis, then Y, then Z. The translation is
  applied after it.

### Rename an object

Edit the **Name** field and apply. Leading and trailing spaces are removed.

### Delete an object

Select it, then press **Delete** or click **Delete** in the inspector.

### Edit geometry directly (editable meshes)

Parametric objects are edited through their dimensions. To move individual
vertices, translate edges, or extrude faces, first convert the object to an
**editable mesh**.

1. Select a box, sphere, or cylinder.
2. Click **Convert to editable mesh** in the inspector (under "Direct editing").
   - The object keeps its ID, name, position, and rotation.
   - Its dimensions are no longer editable. The inspector shows them as read-only
     history ("converted from Box (width 100, ...)").
   - **Conversion is one-way.** A box becomes 8 vertices, 12 edges, and 6 faces.
     Cylinders and spheres are converted with 24 segments (and 12 rings for a
     sphere).
3. Choose what to select with the mode buttons in the inspector, or with the keys
   **1** (Object), **2** (Vertex), **3** (Edge), and **4** (Face).
4. Left-click an element in the viewport. The selected element is highlighted in
   yellow. **< Prev** and **Next >** step through all elements of the current
   mode, which helps with elements that are hard to click.

While an editable mesh is selected, its edges are drawn as a cyan wireframe on
top of everything (x-ray). In vertex mode every vertex gets a small marker.
Clicks pick the nearest element on screen, including hidden ones behind the
surface.

| Mode | Panel | What Apply does |
| --- | --- | --- |
| Vertex | X / Y / Z of the vertex in the object's local coordinates (mm) | **Move vertex** to that position |
| Edge | Offset X / Y / Z (mm), starting at 0 | **Translate edge**: both endpoints move by the offset, so length and direction are kept |
| Face | Distance (mm), default 10 | **Extrude face** along its normal. Positive goes outwards, negative inwards. The face is replaced by side faces and a new cap, and the cap becomes the selection. |

Press **Enter** in a field or click the action button. As with properties, an
invalid result leaves the object unchanged and shows the reason in the status
bar, for example:

```
Could not update 'Box 1': edge e9 would have zero length
Could not update 'Box 1': face f21 is not planar; only planar faces can be extruded
Could not update 'Box 1': extrusion distance must be a finite, non-zero number of at most 1000000 mm
```

Rules for editable meshes:

- The surface must stay closed: every edge is shared by exactly two faces.
- Edges cannot have zero length, and faces cannot collapse to zero area.
- Only planar, convex faces can be extruded. The inspector warns you
  ("Cannot extrude: ...") before you try.
- Moving vertices may make faces non-planar. That is allowed, but those faces
  can no longer be extruded.

**Esc** steps back one level at a time: first the element, then the mode, then
the object.

### Save, open, and start a new project

1. Type a path in the toolbar's **File** field. The default is `project.cad.json`. Relative
   paths are resolved against the directory you ran `cargo run` from.
2. Click **Save** (or press **Ctrl+S**) to write the project, or **Open** (or **Ctrl+O**) to
   load it.
3. Click **New** to clear the document.

Notes:

- **New** and **Open** replace the current document without asking for confirmation, so save
  your work first.
- If a file cannot be opened (missing, malformed, unsupported version, or invalid data), the
  status bar shows the reason and your current document stays as it was.
- Saving writes a temporary file first and then renames it, so a failed save never damages
  an existing file.

To try a ready-made project, open `examples/demo.cad.json`.

## Controls reference

| Input | Action |
| --- | --- |
| Left click | Select an object, or an element in vertex/edge/face mode; click empty space to deselect |
| Right drag | Orbit around the focus point |
| Middle drag, or Shift + right drag | Pan (the scene follows the cursor) |
| Mouse wheel | Zoom (over the object list: scroll the list) |
| Home, or **Reset view** | Reset the camera |
| Delete | Delete the selected object |
| Esc | Clear element, then mode, then selection; inside a text field, leave the field |
| Enter (in an inspector field) | Apply the properties, or the vertex/edge/face action |
| 1 / 2 / 3 / 4 | Object / vertex / edge / face mode (editable meshes) |
| Tab | Next inspector field |
| Ctrl+S / Ctrl+O | Save / open using the path in the toolbar |

While a text field has focus, only Enter, Esc, and Ctrl shortcuts work; other keys go to the
field.

## Troubleshooting

| Symptom | Cause and fix |
| --- | --- |
| Build fails with a `libudev` or `wayland-client` error | Install the Linux system libraries listed above. |
| `No such file or directory` when opening | The path is resolved relative to the directory you started `cargo run` from. Use an absolute path or check that directory. |
| `unsupported schema version N` | The file was written by a newer version of the editor. |
| `malformed project file: …` | The file is not valid JSON or does not match the [file format](file-format.md). |
| The viewport does not react to the mouse | The cursor is over a panel. Camera input only starts over the viewport. |
| Keys like Delete do nothing | A text field has focus. Press Esc first. |
| "Convert ... to an editable mesh to edit its topology" | Vertex/edge/face modes only work on converted objects. |
| A vertex behind the object gets picked | Picking is x-ray. Orbit the camera, or use Prev / Next. |
