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
| Ctrl + Alt + O | Array modifier: cycles the offset through relative X (default) -> Y -> Z -> constant 1 m along X -> Y -> Z (the stack line shows it, for example `+ Array x3 +1 m Y`) |
| Ctrl + Alt + J | Array modifier: toggle Merge (1 mm): vertices that touch are joined, here across the whole array (Blender joins only neighbouring copies) |
| Ctrl + Alt + W | Solidify modifier: toggle (0.01 m, offset -1) |
| Ctrl + Alt + B | Bevel modifier: toggle (one segment, 0.1 m, clamped under half the shortest edge) |
| Ctrl + Alt + `.` | Bevel modifier: cycles the segments 1 -> 2 -> 3 -> 4 -> 6 -> 8 -> 1 (the rim follows a curve bulging towards the original edge, not Blender's superellipse profile; the stack line shows `x3`) |
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
| Ctrl + Alt + A | Build modifier: toggle (reveals the faces from frame 1 over 100 frames, following the timeline: scrub, play or Alt + wheel; the same random order as Blender for a seed; `]` / `[` change the duration; a mesh shows nothing until its first face is due, Blender would build loose edges first) |
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
| Add Modifier | Opens a list of all 20 modifier kinds; a click appends one (with its defaults) and closes the list. Shrinkwrap (Nearest Surface Point, offset 0 m) snaps the vertices onto another object's surface, uses the target's base mesh and follows it when it moves; it has no key chord because it needs a target. Corrective Smooth (factor 0.5, 5 repeats, scale 1: smooths, then restores the detail the smoothing removed from the stack's starting mesh in a per-corner tangent space; Simple smoothing only, no Bind / vertex group / Only Smooth / Pin Boundary) has no chord either: use the list, then `-` / `+` for the factor |
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

### Transform panel (properties editor)
Above the Modifiers panel, the properties editor shows Location (m), Rotation (degrees, XYZ Euler) and Scale of the
active object as three rows of X / Y / Z fields, plus Dimensions (m) for a mesh: the size of its evaluated bounds (modifiers
included) times the scale. Typing a dimension sets the scale to match and keeps the sign of a mirrored axis; zero and
negative sizes are refused. Click a field to type into it: the first key replaces the value,
Backspace deletes, Enter (or a click anywhere else) applies it, Tab / Shift + Tab apply it and move to the
next / previous field, and Escape cancels. Each applied edit is one undo
step, and the values are the object's own (local) transform, so they also work on a parented object. Not modelled:
dragging a field, expressions, lock toggles, rotation modes and delta transforms.

### Add menu
Shift + A opens one list of eighteen entries: the nine mesh primitives, the seven empty types (Plain Axes, Arrows,
Single Arrow, Circle, Cube, Sphere, Cone, listed as `Empty: ...`), Camera and Light (Point). The new object is placed at
the 3D cursor, aligned to the world, selected and active, and is one undo step. Empties are drawn with Blender's
shapes at display size 1 (the circle lies in the XZ plane, the cone points along +Y); Arrows are the three axis
lines without the screen aligned end markers and letters. Blender nests the meshes and empties in submenus and
offers the Image empty and more light and camera types; those are not modelled.

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
| Shift + L | Select Linked, Object Data: adds every visible object whose mesh equals the active object's (meshes are not shared between objects here, so identical geometry stands in for linked data) |
| Shift + Alt + L | Select Linked, Modifiers: adds every visible mesh with the same modifier stack and Subdivision Surface level as the active object's |
| Shift + Alt + R | Select Random: adds about half of the visible objects; each press uses the next seed |
| Shift + H | Hide Unselected: hides every visible object that is not selected (H hides the selection, Alt + H shows everything again and selects it) |

### Align, randomize and group
| Keys | Operator |
| --- | --- |
| Shift + Alt + 1 / 2 / 3 | Align Objects (Centers): moves the selected objects along global X / Y / Z so the centers of their bounds meet the active object's |
| Ctrl + Shift + Alt + 1 / 2 / 3 | Align Objects (Negative Sides): the minimum sides meet the active object's. The code also offers Positive Sides and the Scene Origin, 3D Cursor and Selection references (`Scene::align_objects`) |
| Ctrl + Shift + R | Randomize Transform: a random offset of location (1 m), rotation (15 degrees) and scale (20 %) per selected object. The seed counts up, so pressing it again gives another set. Blender's own defaults are all zero, so the amounts here are this port's choice |
| Ctrl + Shift + G | Select Grouped, Children (every descendant of the active object) |
| Shift + Alt + G | Select Grouped, Immediate Children |
| Ctrl + Shift + Alt + G | Select Grouped, Parent |
| Ctrl + Shift + Alt + S | Select Grouped, Siblings |
| Ctrl + Shift + Alt + M | Origin to Center of Mass (Volume): the volume centroid of a closed mesh, the median of the vertices for an open one |
| Ctrl + Shift + A | Selection to Active: the other selected objects' origins land on the active object's (a child of a selected parent goes along with it) |
| Ctrl + Shift + Alt + B | Origin to Geometry, Bounds Center: the center of the bounding box instead of the vertex median |
| Ctrl + Alt + P | Clear Parent Inverse: resets the stored inverse parent matrix of the selected children, which then follow the parent directly (they usually move) |
| Ctrl + Shift + M | Select Mirror: selects the objects named like the left / right flip of the selected ones (`.L` / `.R`, `_l` / `_r`, `Left` / `Right`, as prefix or suffix, a `.001` number is kept) instead of the selection |
| Shift + Alt + M | Select Mirror, extending the selection |
| Shift + Alt + O | Statistics overlay: Objects (selected / visible), Vertices, Faces and Triangles of the visible objects, evaluated with their modifiers, under the info text |

All but the overlay are undoable (Align pushes no step when nothing moves). The code is in `src/scene/ops.rs`.

### Pivot point
Period cycles the transform pivot point (Blender's pivot pie): Median Point (default), 3D Cursor and Individual Origins.
G, R and S and Shift + D use it: the 3D cursor mode rotates and scales about the cursor, Individual Origins turns and
scales each object about its own origin without moving it. A non-default mode is listed as `Pivot: ...` in the info
text. Bounding Box Center and Active Element are not modelled.

### Snapping and the 3D cursor
Blender's Shift + S pie has no pie here, so each entry is one chord:

| Keys | Operator |
| --- | --- |
| Shift + S | Cursor to Selected |
| Ctrl + Shift + S | Selection to Cursor (keeps the relative offsets) |
| Shift + Alt + S | Cursor to World Origin |
| Shift + Alt + C | Origin to 3D Cursor |
| Ctrl + Shift + C | Cursor to Active |

All of them are undoable and operate on every selected mesh. The geometry code lives in `src/mesh_ops.rs`
(mirror, array, solidify, normals, merge) and `src/mesh_deform.rs` (smooth, simple deform, displace, cast), `src/mesh_wire.rs` (wireframe), `src/mesh_reduce.rs` (decimate, edge split), `src/modifiers.rs` (the stack); the operators are in `src/scene.rs`.
