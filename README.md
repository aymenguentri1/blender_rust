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
| Alt + C | Convert to Mesh: bakes the modifiers and the subdivision level into the mesh |

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

### Snapping and the 3D cursor
Blender's Shift + S pie has no pie here, so each entry is one chord:

| Keys | Operator |
| --- | --- |
| Shift + S | Cursor to Selected |
| Ctrl + Shift + S | Selection to Cursor (keeps the relative offsets) |
| Shift + Alt + S | Cursor to World Origin |
| Shift + Alt + C | Origin to 3D Cursor |

All of them are undoable and operate on every selected mesh. The geometry code lives in `src/mesh_ops.rs`
(mirror, array, solidify, normals, merge) and `src/mesh_deform.rs` (smooth and simple deform), `src/modifiers.rs` (the stack); the operators are in `src/scene.rs`.
