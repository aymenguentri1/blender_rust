# blender_rust

## Object-mode editing

Selection, transforms (G / R / S), duplicate (Shift + D), delete, hide, undo / redo and the Add menu cycle
(Shift + A) are modelled on Blender. On top of those:

### Modifier stack
Modifiers are evaluated in order on the base mesh, then the Subdivision Surface level (Ctrl + 0..5).
Blender has no keys for modifiers (they live in the properties editor), so these are Ctrl + Alt chords:

| Keys | Operator |
| --- | --- |
| Ctrl + Alt + X / Y / Z | Mirror modifier: toggle that axis (the modifier goes away when no axis is left) |
| Ctrl + Alt + `+` / `-` | Array modifier: more / fewer copies (1 removes it), relative offset (1, 0, 0) |
| Ctrl + Alt + W | Solidify modifier: toggle (0.01 m, offset -1) |
| Ctrl + Alt + B | Bevel modifier: toggle (one segment, 0.1 m, clamped under half the shortest edge) |
| Ctrl + Alt + M | Smooth modifier: toggle (factor 0.5, 1 repeat) |
| Ctrl + Alt + T | Simple Deform modifier (Z axis): each press cycles Twist (45 degrees) -> Bend (45 degrees) -> Taper (0.5) -> Stretch (0.5) -> off |
| Ctrl + Alt + E | Weld modifier: toggle (merge by distance, 1 mm) |
| Ctrl + Alt + G | Displace modifier: toggle (noise along the normals, strength 0.2 m, midlevel 0.5) |
| Ctrl + Alt + R | Triangulate modifier: toggle |
| Ctrl + Alt + C | Cast modifier: each press cycles Sphere (factor 0.5) -> Cylinder -> off |
| Ctrl + Alt + Q | Wireframe modifier: toggle (0.02 m frames; stack it after Triangulate to frame triangles) |
| Ctrl + Alt + V | Decimate modifier (collapse): each press cycles ratio 0.5 -> 0.25 -> 0.1 -> off |
| Ctrl + Alt + N | Edge Split modifier: toggle (30 degrees) |
| Ctrl + Alt + U | Screw modifier: toggle (revolves the boundary edges about Z, 360 degrees, 16 steps; every edge of a closed mesh) |
| Ctrl + Alt + K | Wave modifier: toggle (circular sine ripple along Z, 0.1 m high, width 1.5; needs a mesh with vertices to ripple, so subdivide first) |
| Ctrl + Alt + L | Laplacian Smooth modifier: toggle (factor 0.5, 2 repeats, keeps the volume of a closed mesh) |
| Ctrl + Alt + I | Remesh modifier, Blocks mode: toggle (0.1 m voxels, closed meshes only; the grid is capped at 64 cells across) |
| Alt + C | Convert to Mesh: bakes the modifiers and the subdivision level into the mesh |

The modifier stack of the active object is listed in the info text at the top left of the 3D viewport, one
`+ ` line per modifier with its main setting (for example `+ Mirror X`, `+ Decimate 0.25`), followed by the
Subdivision Surface level. The active modifier (the newest one after adding) is marked `>` and is the one these
keys act on:

| Keys | Operator |
| --- | --- |
| Ctrl + Alt + Down / Up | Select the next / previous modifier |
| Ctrl + Alt + `]` / `[` | Raise / lower its main value (Array count, thickness, width, factor, angle, ratio, strength, distance) |
| Ctrl + Alt + Page Down / Page Up | Move it down / up in the stack |
| Ctrl + Shift + Alt + X | Remove it |

The line above shows the real active object (`Collection | Cube`).

### Modifiers panel (properties editor)
The properties editor on the right has a Modifiers panel for the active mesh object, so none of the chords above
are required:

| Control | Action |
| --- | --- |
| Add Modifier | Opens a list of all 18 modifier kinds; a click appends one (with its defaults) and closes the list. Shrinkwrap (Nearest Surface Point, offset 0 m) snaps the vertices onto another object's surface, uses the target's base mesh and follows it when it moves; it has no key chord because it needs a target |
| Modifier name | Makes it the active modifier (highlighted) |
| `^` / `v` | Move it up / down in the stack |
| `x` | Remove it |
| `Target:` (Shrinkwrap only) | Cycles the target through the other mesh objects, then no target |
| `-` / `+` (value row) | Lower / raise its main value, like Ctrl + Alt + `[` / `]` |

Every panel edit is one undo step (Ctrl + Z). Not modelled: collapsing panels, per-setting fields, the viewport /
render toggles and scrolling (rows that do not fit are left out).

### Object and mesh operators
| Keys | Operator |
| --- | --- |
| Ctrl + J | Join the selected meshes into the active one |
| Ctrl + A | Apply all transforms (location, rotation, scale) to the mesh |
| Shift + Ctrl + Alt + C | Origin to Geometry (median center) |
| Alt + N | Flip normals |
| Shift + N | Recalculate normals outside |
| Ctrl + T | Triangulate |
| Alt + M | Merge by distance (0.1 mm) |
| Ctrl + Alt + D | Subdivide (one cut, no smoothing) |
| P | Separate by loose parts |
| Ctrl + Alt + H | Convex Hull: replaces each selected mesh with the hull of its vertices (coplanar triangles joined into n-gons, so a box stays six quads; flat or collinear meshes are left alone) |
| Ctrl + Shift + X / Y / Z | Mirror: reflects the selected objects through the median point along that global axis and flips their orientation and scale (Blender's Ctrl + M, one chord per axis) |

### Parenting
| Keys | Operator |
| --- | --- |
| Ctrl + P | Parent (Object, Keep Transform): the active object becomes the parent of the other selected objects, which stay where they are (`parent_inv` stores the inverse parent matrix like Blender's `parentinv`). Parenting an object to one of its own descendants is refused |
| Alt + P | Clear and Keep Transformation: the object keeps its world transform as its own location, rotation and scale |
| Shift + Alt + P | Clear Parent: the object falls back to its own local transform |
| Ctrl + `[` / Ctrl + `]` | Select Parent / Select Child (first child by name); add Shift to keep the current selection |

The outliner nests children under their parent. Grab, rotate and scale leave out an object whose parent is also
selected (it moves with the parent) and convert world space offsets into the parent's space. Deleting a parent frees
its children in place; duplicating a parent and its children parents the copies to each other. Not modelled: rotation
and scale of a child under a rotated or non-uniformly scaled parent are applied to the child's own values (the
location is exact), shear is dropped when a transform is baked, and the parent is not shown by a relationship line.

### Selection
| Keys | Operator |
| --- | --- |
| Shift + G | Select Grouped, Type: adds every visible object of the active object's type |
| Shift + Alt + R | Select Random: adds about half of the visible objects; each press uses the next seed |

### Snapping and the 3D cursor
Blender's Shift + S pie has no pie here, so each entry is one chord:

| Keys | Operator |
| --- | --- |
| Shift + S | Cursor to Selected |
| Ctrl + Shift + S | Selection to Cursor (keeps the relative offsets) |
| Shift + Alt + S | Cursor to World Origin |
| Shift + Alt + C | Origin to 3D Cursor |

All of them are undoable and operate on every selected mesh. The geometry code lives in `src/mesh_ops.rs`
(mirror, array, solidify, normals, merge) and `src/mesh_deform.rs` (smooth, simple deform, displace, cast), `src/mesh_wire.rs` (wireframe), `src/mesh_reduce.rs` (decimate, edge split), `src/modifiers.rs` (the stack); the operators are in `src/scene.rs`.
