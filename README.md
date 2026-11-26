# blender_rust

## Object-mode editing

Selection, transforms (G / R / S), duplicate (Shift + D), delete, hide, undo / redo and the Add menu cycle
(Shift + A) are modelled on Blender. On top of those:

## Sculpt Mode

Ctrl + Tab (or a click on the mode dropdown of the header) toggles Sculpt Mode on the active mesh object; the dropdown
then reads `Sculpt Mode` with the brush icon. Strokes work on the base mesh (`src/sculpt.rs`), so use a mesh with
enough vertices (Add > UV Sphere, or Alt + C to bake a subdivision level into the mesh).

| Keys | Operator |
| --- | --- |
| Left mouse drag | Brush stroke: dabs every 10 % of the brush diameter along the path, found by ray casting the mesh |
| Ctrl + drag | Inverted brush (the ring turns light blue, like `cursor_color_subtract`) |
| Shift + drag | Smooth while held |
| X / S / G / I / C | Draw / Smooth / Grab / Inflate / Clay Strips brush |
| Ctrl + Shift + D | Subdivide the base mesh once (Catmull-Clark), for meshes too coarse to sculpt |
| Shift + T / P / Shift + C | Flatten / Pinch / Crease brush |
| Shift + X / Shift + S | Fill / Scrape brush (these two keys are not Blender's) |
| L | Layer brush: raises the surface to a capped height, so repeated passes build a flat topped layer instead of piling up (Ctrl digs one) |
| K | Snake Hook brush: the brush follows the mouse and drags the surface under it along, stretching it (falloff and mask apply, symmetry mirrors the pull) |
| N | Nudge brush: drags the surface under the brush along the stroke direction, kept on the surface |
| Shift + B | Blob brush: inflates with an outward push as well, so the surface swells into a ball (Ctrl dents it) |
| M | Mask brush: paints the mask (Ctrl + drag erases it). Masked vertices are drawn dark grey and every brush, Grab included, is scaled by `1 - mask` |
| Alt + M / Ctrl + I | Clear the mask / invert it (an empty mask becomes fully masked), one undo step each |
| F / Shift + F | Brush radius / strength: move the mouse, click or Enter confirms, Esc cancels |
| Alt + X / Y / Z | Toggle mirror symmetry on that axis (X is on, as in Blender) |
| Ctrl + Z | Undo, one step per stroke |

The tool header (`src/sculpt_header.rs`, `VIEW3D_HT_tool_header`) sits under the viewport header while Sculpt Mode is on:
the brush dropdown (a press selects the next brush, Shift + press the previous one, there is no brush popup), the Radius
and Strength sliders (press or drag to set the value, absolute instead of Blender's relative drag), the Add / Subtract
direction (Subtract works the brush inverted, like holding Ctrl) and the X / Y / Z symmetry toggles. A press in the
header never reaches the viewport.

The brush ring shows the radius (outer circle) and the strength (inner circle); the line at the bottom left of the
viewport names the brush, radius (default 50 px), strength (default 0.5) and symmetry. The falloff is Blender's
Smooth curve preset. Deviations: the displacement factors are tuned by eye, there is no pressure, face sets,
dynamic topology or multires, the brush curve cannot be changed, and the brush asset shelf is not drawn (the tool header has no Falloff,
Stabilize or other brush settings).

## Performance

- The workbench rasteriser defers the opaque triangles of a frame and draws them in horizontal bands of the viewport on
  up to 8 threads (`std::thread::scope`, no dependencies). Every band replays the triangles in the same order, so the
  image, depth and object id buffers are identical for any thread count (there is a test for that). X-ray keeps the
  single threaded path. The light constants (`exp2`, wrap factors, base colours) are computed once per frame, the edge
  functions are set up once per triangle, and a flat face in an orthographic view is shaded once instead of per pixel.
- Sculpt strokes build the vertex neighbours and the faces around each vertex once, take normals only for the vertices
  under the brush, and cast the pick ray in the local space of the mesh (no vertex is transformed per dab).
- `cargo test --release bench_draw_scene -- --ignored --nocapture` prints the milliseconds per frame of a smooth sphere
  at 1280 x 720.

### 3D viewport header
The header of the 3D viewport follows `VIEW3D_HT_header`: the editor type and `Object Mode` dropdowns, the
`View`, `Select`, `Add` and `Object` menus, the transform cluster (orientation, pivot point, snapping,
proportional editing), and right aligned the Overlays and X-ray toggles and the four shading buttons with their
option arrows. Icons are drawn as vectors (`src/icons.rs`), and the layout, drawing and hit testing live in
`src/view3d_header.rs`. Buttons light up under the cursor.

| Button | Action |
| --- | --- |
| Wireframe / Solid shading | Same as Shift + Z |
| X-ray | Same as Alt + Z (dimmed and inert in wire shading) |
| Overlays | Same as Shift + Alt + Z |
| Pivot point | Cycles Median Point, 3D Cursor, Individual Origins, like Period (the icon follows the mode) |
| `Add` | Opens the Add menu under the button |

Drawn but not modelled: the editor type and mode dropdowns, the `View`, `Select` and `Object` menus, orientation,
snapping, proportional editing, Material Preview and Rendered shading and every option arrow. When the header is
too narrow for the transform cluster it is dropped as a whole, where Blender scrolls the region.

### Modifier stack
Modifiers are evaluated in order on the base mesh, then the Subdivision Surface level (Ctrl + 0..5).
Blender has no keys for modifiers (they live in the properties editor), so these are Ctrl + Alt chords:

| Keys | Operator |
| --- | --- |
| Ctrl + Alt + X / Y / Z | Mirror modifier: toggle that axis (the modifier goes away when no axis is left) |
| Ctrl + Alt + `+` / `-` | Array modifier: more / fewer copies (1 removes it), relative offset (1, 0, 0) |
| Ctrl + Alt + O | Array modifier: cycles the offset through relative X (default) -> Y -> Z -> constant 1 m along X -> Y -> Z (the stack line shows it, for example `+ Array x3 +1 m Y`) |
| Ctrl + Alt + J | Array modifier: toggle Merge (1 mm): vertices that touch are joined, here across the whole array (Blender joins only neighbouring copies) |
| Ctrl + Alt + `,` | Array modifier: cycles Fit Length Fixed Count -> 2 m -> 4 m -> 8 m -> Fixed Count. With a length the copy count is the length divided by the offset length, rounded down (at least 1), as in Blender; the stack line shows `Array fit 4 m` and `-` / `+` in the panel move the length by 0.5 m. A zero offset has no length, so the fixed count is used |
| Ctrl + Alt + W | Solidify modifier: toggle (0.01 m, offset -1) |
| Ctrl + Alt + `;` | Solidify modifier: toggle Even Thickness. Each vertex is moved by the mean of 1 / cos(angle to its faces) times the offset (the cosine is held above 0.3), so a cube's walls keep their thickness at the corners instead of thinning to 58 % there; the stack line shows `Solidify 0.01 m even`. Blender's Nonmanifold Thickness modes, Thickness Clamp and vertex groups are not modelled |
| Ctrl + Alt + B | Bevel modifier: toggle (one segment, 0.1 m, clamped under half the shortest edge) |
| Ctrl + Alt + `.` | Bevel modifier: cycles the segments 1 -> 2 -> 3 -> 4 -> 6 -> 8 -> 1 (the rim follows a curve bulging towards the original edge, not Blender's superellipse profile; the stack line shows `x3`) |
| Ctrl + Alt + M | Smooth modifier: toggle (factor 0.5, 1 repeat) |
| Ctrl + Alt + T | Simple Deform modifier (Z axis): each press cycles Twist (45 degrees) -> Bend (45 degrees) -> Taper (0.5) -> Stretch (0.5) -> off |
| Ctrl + Alt + E | Weld modifier: toggle (merge by distance, 1 mm) |
| Ctrl + Alt + G | Displace modifier: toggle (noise along the normals, strength 0.2 m, midlevel 0.5) |
| Ctrl + Alt + `'` | Displace modifier: cycles the Direction Normal -> X -> Y -> Z -> Normal (X / Y / Z move every vertex along that local axis by the same noise value; the stack line shows `Displace 0.2 m Y`). Blender's texture slot, RGB directions and coordinate spaces are not modelled |
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
| Ctrl + Shift + Alt + A | Apply it: bakes the active modifier into the mesh and takes it off the stack. Like Blender it is applied on its own to the base mesh (the modifiers above it are not part of the result) and the rest of the stack then acts on the baked mesh |
| Ctrl + Shift + Alt + D | Duplicate it: a copy with the same settings goes right after it and becomes the active modifier |
| Ctrl + L | Copy Modifiers (Link/Transfer Data): the active object's whole stack and Subdivision Surface level replace those of the other selected meshes (objects that already match are left alone) |

The line above shows the real active object (`Collection | Cube`).

### Modifiers panel (properties editor)
The properties editor on the right has a Modifiers panel for the active mesh object, so none of the chords above
are required:

| Control | Action |
| --- | --- |
| Add Modifier | Opens a popup menu (`src/add_modifier_menu.rs`, same look as the Add and modifier menus) under the button with the categories Generate (Array, Bevel, Boolean, Build, Decimate, Edge Split, Mirror, Remesh, Screw, Solidify, Triangulate, Weld, Wireframe) and Deform (Cast, Corrective Smooth, Displace, Laplacian Smooth, Shrinkwrap, Simple Deform, Smooth, Wave). Hovering or clicking a category opens its submenu beside it, alphabetical with the modifier icons (it flips to the left near the window edge); a click on an entry appends that modifier (with its defaults) and closes the menu; Esc, a right click or a click elsewhere closes it. Blender's Edit, Normals and Physics categories have no modifiers here and are left out, and there is no search field or keyboard navigation Shrinkwrap (Nearest Surface Point, offset 0 m) snaps the vertices onto another object's surface, uses the target's base mesh and follows it when it moves; it has no key chord because it needs a target. Corrective Smooth (factor 0.5, 5 repeats, scale 1: smooths, then restores the detail the smoothing removed from the stack's starting mesh in a per-corner tangent space; Simple smoothing only, no Bind / vertex group / Only Smooth / Pin Boundary) has no chord either: use the list, then `-` / `+` for the factor | Boolean (`src/mesh_boolean.rs`, Difference by default; Union and Intersect) combines the mesh with another object's base mesh, so it needs a target and has no chord: use the list, pick the `Target:` row, then `-` / `+` steps through Difference, Union and Intersect. It is a BSP-tree CSG for closed, consistently wound meshes (Blender's Fast solver wants the same, its Exact solver and Self Intersection / Hole Tolerant options are not modelled). The cut faces stay as convex pieces and are not re-merged, so the result has more faces than Blender's; operands above 6000 triangles together leave the mesh unchanged. Hide the cutter with H to see the result |
| Triangle (left of the name) | Collapses / expands the panel: a collapsed modifier shows only its header row (down triangle expanded, right triangle collapsed) and the ones below move up. Like Blender it is UI state, not an undo step and no geometry change; it moves with the modifier when it is reordered. Ctrl + click collapses every other panel and opens this one; when it already is the only open one, it opens them all |
| Type icon (between the triangle and the name) | The kind's icon in Blender's modifier blue (`ICON_MOD_*`): Mirror, Array, Solidify, Bevel, Triangulate, Boolean, Wireframe, Decimate and Remesh have their own glyph; Smooth, Laplacian Smooth, Corrective Smooth and Wave share the wave, Simple Deform and Cast the curved grid, and every other kind (Displace, Edge Split, Weld, Screw, Shrinkwrap, Build) the generic wrench. The glyphs are simplified vector drawings, not Blender's bitmaps. A click selects the modifier like the name |
| Modifier name | Makes it the active modifier (highlighted). A double-click opens a `Rename Modifier` field (the F2 popup: type, Backspace / Delete, arrows, Enter applies, Esc or a click cancels). The name is kept unique among the object's modifiers (`Array.001`), naming it after its kind clears the custom name, it moves with the modifier and is one undo step; Duplicate gives the copy the next free `.001` name |
| Monitor icon | Show in Viewport (`show_viewport`): lit blue while the modifier is evaluated, grey when it is skipped. A skipped modifier keeps its settings and its place; the stack line in the viewport shows it as `Array x2 (off)`, and `Apply` leaves it alone as Blender does. The flag moves with the modifier when it is reordered and is copied with Duplicate Object / Copy Modifiers; the render toggle is the camera right of it |
| Edit mode icon (square with corner dots, left of the monitor) | Show in Edit Mode (`show_in_editmode`): lit blue while on (the default), grey when off. This port has no edit mode, so it is panel state only: no geometry changes, one undo step, it moves with the modifier and is copied with Duplicate / Copy to Selected / Copy Modifiers |
| Camera icon (right of the monitor) | Show in Render (`show_render`): lit blue while the modifier is enabled for rendering, grey when it is not. Nothing in this port renders, so it changes no geometry; a click is still one undo step, moves with the modifier when it is reordered, is copied with Duplicate / Copy to Selected / Copy Modifiers, and the stack line in the viewport shows it as `Array x2 (no render)` (combined with `(off)` when the viewport toggle is off too). `Apply` ignores it, as Blender does |
| Grip (six dots, left of the `x`) | Drag handle: press on it, move the cursor over the row of another modifier and release there to move the modifier to that place in the stack (the others shift, one undo step, the moved one becomes active). The release height decides: above all rows is the first place, below all the last. Blender also draws the panel following the cursor and a drop line while dragging; this port shows nothing until the release. The old `^` / `v` buttons are gone to make room, as in Blender; `Move to First` / `Move to Last` in the dropdown menu remain |
| `x` | Remove it |
| Dropdown arrow (right of the monitor) | Opens the modifier menu (`src/modifier_menu.rs`, same look as the Add menu): `Apply` (bakes it into the mesh like Ctrl + A over the modifier, same as Ctrl + Shift + Alt + A; dimmed while the modifier is off in the viewport), `Duplicate` (a copy right after it, same as Ctrl + Shift + Alt + D), `Copy to Selected` (appends a copy with its name and viewport flag to every other selected mesh, `OBJECT_OT_modifier_copy_to_selected`; dimmed when there is none), `Move to First` / `Move to Last` (`OBJECT_OT_modifier_move_to_index`; dimmed when it already is there). Entries that cannot run are dimmed and ignore clicks; Esc, a right click or a click outside closes the menu. Not modelled: the shape key entries, separators and shortcut hints |
| `Target:` (Shrinkwrap and Boolean) | Cycles the target through the other mesh objects, then no target |
| Number field (value row) | Drawn as Blender's number field: one grey box with a `<` and a `>` at its ends and the text (`Count: 2`) centred between them; while you type, the arrows go and the whole row is the dark text field with a blue outline. Click the middle to type the modifier's main number (`Count: 2`, `Width: 0.1 m`, `Angle: 45 deg`, `Factor: 0.5`, ...; the value the `-` / `+` buttons step): the whole text is selected, so the first character replaces it; digits, `.` / `,` and a leading `-` or `+` are accepted, Backspace deletes, Enter applies, Esc cancels, and a click anywhere else applies it too. The same limits as the buttons apply (a Count is a whole number from 1 to 1000, a Decimate ratio 0.01 to 1, a Screw angle at most 360 deg, ...). An unparsable or empty field changes nothing; one undo step. Triangulate and Boolean have no number, so their row only selects them and shows the summary |
| `X` `Y` `Z` (Mirror row) | One toggle per axis across the row (`use_axis`), lit blue while the axis is on; each click is one undo step and makes the modifier active. Unlike the Ctrl + Alt + X / Y / Z chords, switching off the last axis keeps the modifier, which then changes nothing, as in Blender. Mirror's other options (bisect, flip, merge, mirror object) are not modelled |
| `<` / `>` (ends of the number field) | Lower / raise its main value, like Ctrl + Alt + `[` / `]`. Triangulate and Boolean have no number and keep their summary field with separate `-` / `+` buttons, because Blender shows options there that are not modelled; dragging the value is not modelled either |

Every panel edit is one undo step (Ctrl + Z). Not modelled: the other per-setting fields, the on-cage toggle and scrolling (rows that do not fit are left out).

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
Shift + A opens one list of twenty-one entries: the nine mesh primitives, the seven empty types (Plain Axes, Arrows,
Single Arrow, Circle, Cube, Sphere, Cone, listed as `Empty: ...`), Camera and the four light types (`Light: Point`,
Sun, Spot, Area). The new object is placed at the 3D cursor, aligned to the world, selected and active, and is one
undo step. Lights are named like Blender names them (`Point`, `Sun`, `Spot`, `Area`). Empties are drawn with Blender's
shapes at display size 1 (the circle lies in the XZ plane, the cone points along +Y); Arrows are the three axis
lines without the screen aligned end markers and letters. Every light is a screen facing circle with the line down
to the ground; a Sun adds its eight rays and a 20 m direction line, a Spot its cone (45 degrees, blend 0.15, drawn
10 m long: cap ring, blend ring and four edges instead of the view dependent silhouette) and an Area light the 1 m
square. The light distance markers, colours, sizes and angles are not editable. Blender nests the meshes, empties
and lights in submenus and offers the Image empty and more camera types; those are not modelled.

### Parenting
| Keys | Operator |
| --- | --- |
| Ctrl + P | Parent (Object, Keep Transform): the active object becomes the parent of the other selected objects, which stay where they are (`parent_inv` stores the inverse parent matrix like Blender's `parentinv`). Parenting an object to one of its own descendants is refused |
| Alt + P | Clear and Keep Transformation: the object keeps its world transform as its own location, rotation and scale |
| Shift + Alt + P | Clear Parent: the object falls back to its own local transform |
| Ctrl + Shift + P | Parent without Inverse (`OBJECT_OT_parent_no_inverse_set`): the active object becomes the parent with an identity parent inverse, so the others land on its origin (their location is cleared, rotation and scale stay as local values) |
| Ctrl + Shift + Alt + P | The same with Keep Transform: their local transform is recomputed from the parent so they stay where they are. Cycles are refused |
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

### Box select
| Input | Operator |
| --- | --- |
| Left drag on empty space | Select Box (`VIEW3D_OT_select_box`, the default Select Box tool): the objects inside the dashed border replace the selection; Shift extends, Ctrl subtracts. A click without dragging still deselects |
| B, then left drag | Select Box in extend mode (Ctrl subtracts, Esc cancels); the middle mouse button subtract of Blender is not modelled |

Solid shading respects occlusion by casting a grid of pick rays across the box (at most about 2500), so an object much
smaller than the grid step can be missed; X-ray and wire shading test the projected triangles of every visible mesh.
Cameras, lights and empties are hit by their origin. A press that starts on an object still selects it at once (the
Tweak tool's drag-to-move is not modelled). The code is in `src/select_box.rs` and `Scene::select_box`.

### Copy and paste
| Keys | Operator |
| --- | --- |
| Ctrl + C | Copy (`VIEW3D_OT_copybuffer`): the selected objects and the active one go to an in-app clipboard |
| Ctrl + V | Paste (`VIEW3D_OT_pastebuffer`): the copies appear at their copied transforms (named `Cube.001`...), selected, with the copied active object active; one undo step. Pasting works after the originals are deleted. Parents copied together stay parents of each other; a parent that is not in the scene any more is dropped |

The buffer lives in the running program only (Blender writes a `.blend` file to the temp folder, so it also pastes into
other Blender sessions; this port does not). Code: `Scene::copy_selected` / `Scene::paste` in `src/scene.rs`.

### Rename
| Keys | Operator |
| --- | --- |
| F2 | Rename Active Object: a popup with a text field holding the name. Typing inserts at the caret, Backspace / Delete / Left / Right / Home / End edit, Enter applies (one undo step; a name already in use becomes `Name.001`), Esc or a click cancels |

Not modelled: text selection, the clipboard inside the field, and renaming from the outliner (double click). The name is
limited to 63 bytes like Blender's `MAX_NAME`. Code: `src/rename_popup.rs` and `Scene::rename_active`.

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

### Apply, snap to grid and local view
| Keys | Operator |
| --- | --- |
| Ctrl + Shift + Alt + L / R / Z | Apply Location / Rotation / Scale (`OBJECT_OT_transform_apply` with one option; Ctrl + A still applies all three). What is applied is baked into the mesh and reset, what is not stays on the object. Location is converted through the remaining rotation and scale so the mesh does not move. Applying Rotation to a non-uniformly scaled object shears it, as in Blender. A negative scale reverses the face winding so the mesh is not left inside out. The direct children of an applied object stay where they are (their parent inverse is adjusted), like Blender; before this they jumped |
| Ctrl + Shift + Alt + T | Selection to Grid (`VIEW3D_OT_snap_selected_to_grid`, 1 m): every selected object origin goes to the nearest grid point in world space, also under a rotated or scaled parent. An object that moves with a selected ancestor is left to it |
| Numpad `/` | Local View (`VIEW3D_OT_localview`): only the selected objects stay visible and the view frames them; press again to leave. Objects you had hidden stay hidden, and so do those you hide inside local view (Alt + H does not bring back what local view hides). Entering needs a selection. It follows undo. Blender also restores the previous view on exit and shows a local-view marker, neither is modelled |

### Transform precision
| Input | Effect |
| --- | --- |
| Shift held during G / R / S | Precision mode (`t->mouse.precision`): the mouse movement counts a tenth, for grab, rotate and scale alike. Releasing Shift keeps the value reached (no jump back to the cursor), and further movement counts in full again. Shift + X / Y / Z still picks the plane constraint |

### Link object data and cursor to grid
| Keys | Operator |
| --- | --- |
| Ctrl + Shift + L | Link Object Data (`OBJECT_OT_make_links_data`, Object Data): the other selected meshes get the active mesh's geometry and shading flag; names and transforms stay their own. Meshes are not shared between objects here, so they receive copies, not a live link. Objects that already match are left alone |
| Ctrl + Shift + Alt + U | Cursor to Grid: rounds the 3D cursor to the nearest metre (`snap_curs_to_grid`) |

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
