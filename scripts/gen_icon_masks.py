#!/usr/bin/env python3
"""Render Blender's own icons (release/datafiles/icons_svg/<name>.svg of the Blender reference) to alpha masks.

Writes assets/icon_masks.txt (one svg name per line) and assets/icon_masks.bin (SIZE x SIZE bytes per name, row 0
on top, in that order). Needs rsvg-convert, PIL and scripts/blender_ref.sh setup. Usage: python3 scripts/gen_icon_masks.py
"""
import io, subprocess, sys
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
light_spot light_area""".split()


def main():
    out = bytearray()
    for n in NAMES:
        svg = subprocess.run(["scripts/blender_ref.sh", "show", f"release/datafiles/icons_svg/{n}.svg"],
                             check=True, capture_output=True, env={"LIMIT": "100000", "PATH": "/usr/bin:/bin"}).stdout
        if b"<svg" not in svg:
            sys.exit(f"{n}: not an svg")
        png = subprocess.run(["rsvg-convert", "-w", str(SIZE), "-h", str(SIZE), "-f", "png"], input=svg,
                             check=True, capture_output=True).stdout
        img = Image.open(io.BytesIO(png)).convert("RGBA")
        out += img.getchannel("A").tobytes()
    open("assets/icon_masks.bin", "wb").write(out)
    open("assets/icon_masks.txt", "w").write("\n".join(NAMES) + "\n")
    print(len(NAMES), "icons,", len(out), "bytes")


if __name__ == "__main__":
    main()
