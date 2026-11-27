#!/usr/bin/env python3
"""Render Blender's own icons (release/datafiles/icons_svg/<name>.svg of the Blender reference) to alpha masks.

Writes assets/icon_masks.txt (one svg name per line) and assets/icon_masks.bin (SIZE x SIZE bytes per name, row 0
on top, in that order). Needs rsvg-convert, PIL and scripts/blender_ref.sh setup. Usage: python3 scripts/gen_icon_masks.py
"""
import io, re, subprocess, sys
from PIL import Image

SIZE = 64
# Order matters: src/icons.rs looks an icon up by its name in assets/icon_masks.txt.
NAMES = """view3d object_datamode sculptmode_hlt downarrow_hlt orientation_global pivot_median pivot_cursor
pivot_individual snap_off prop_off overlay xray shading_wire shading_solid material shading_rendered
disclosure_tri_right restrict_view_off restrict_render_off hide_off grip editmode_hlt modifier mod_array
mod_mirror mod_solidify mod_bevel mod_smooth mod_simpledeform mod_triangulate mod_boolean mod_wireframe
mod_decim mod_remesh outliner_collection outliner_ob_mesh outliner_ob_camera outliner_ob_light
outliner_ob_empty mesh_plane mesh_cube mesh_circle mesh_uvsphere mesh_icosphere mesh_cylinder mesh_cone
mesh_torus mesh_grid empty_axis empty_arrows empty_single_arrow cube sphere cone light_point light_sun
light_spot light_area rightarrow view_zoom view_pan view_camera_unselected view_camera
view_perspective view_ortho outliner properties time filter viewzoom collection_new mouse_mmb_drag mouse_rmb tool_settings scene output scene_data render_result world object_data constraint physics mesh_data camera_data outliner_data_light empty_data radiobut_off rew frame_prev prev_keyframe play_reverse play pause next_keyframe frame_next ff time gizmo add restrict_view_on restrict_render_on x select_set select_extend select_subtract select_difference select_intersect checkmark duplicate tria_up tria_down checkbox_hlt checkbox_dehlt decorate_unlocked decorate_locked""".split()


def main():
    out = bytearray()
    for n in NAMES:
        svg = subprocess.run(["scripts/blender_ref.sh", "show", f"release/datafiles/icons_svg/{n}.svg"],
                             check=True, capture_output=True, env={"LIMIT": "100000", "PATH": "/usr/bin:/bin"}).stdout
        if b"<svg" not in svg:
            sys.exit(f"{n}: not an svg")
        # Blender's icons are drawn on a 16 x 16 grid (100 svg units per pixel); an icon whose canvas is smaller
        # (the arrows, the eye, the grip) keeps its size and is centred, it is not stretched to the grid.
        m = re.search(rb'viewBox="0 0 ([\d.]+) ([\d.]+)"', svg)
        w, h = float(m.group(1)), float(m.group(2))
        k = SIZE / 1600.0
        pw, ph = max(1, round(w * k)), max(1, round(h * k))
        png = subprocess.run(["rsvg-convert", "-w", str(pw), "-h", str(ph), "-f", "png"], input=svg,
                             check=True, capture_output=True).stdout
        icon = Image.open(io.BytesIO(png)).convert("RGBA")
        img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
        img.paste(icon, ((SIZE - pw) // 2, (SIZE - ph) // 2))
        out += img.getchannel("A").tobytes()
    open("assets/icon_masks.bin", "wb").write(out)
    open("assets/icon_masks.txt", "w").write("\n".join(NAMES) + "\n")
    print(len(NAMES), "icons,", len(out), "bytes")


if __name__ == "__main__":
    main()
