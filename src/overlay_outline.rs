//! The outline around selected objects in solid shading.
//!
//! Modelled on Blender's overlay outline pass (`draw/engines/overlay/overlay_outline.cc`): the selected objects
//! are drawn into an object id buffer, and every pixel that is not part of a selected object but touches one is
//! painted with the outline colour, so the line runs outside the silhouette and is hidden where another object is
//! in front. The colour is `TH_ACTIVE` for the active object and `TH_SELECT` for the other selected ones
//! (`.space_view3d.active` / `.select`, userdef_default_theme.c); the width is `U.pixelsize` (the theme's
//! `outline_width` is 1). Deviation: the ids come from the unjittered sample, so the line is not anti-aliased.

use crate::area::Rcti;
use crate::scene::Scene;
use crate::theme::{SPACE_VIEW3D_ACTIVE, SPACE_VIEW3D_SELECT};
use crate::userdef::UserDef;

const NONE: u8 = 0;
const SELECTED: u8 = 1;
const ACTIVE: u8 = 2;

/// The outline state of every id (index `Object::id + 1`; index 0 is "no object").
fn id_states(scene: &Scene) -> Vec<u8> {
    let max = scene.objects.iter().map(|o| o.id + 1).max().unwrap_or(0) as usize;
    let mut states = vec![NONE; max + 1];
    for o in scene.objects.iter().filter(|o| o.selected && !o.hidden && o.mesh().is_some()) {
        states[(o.id + 1) as usize] = if scene.active == Some(o.id) { ACTIVE } else { SELECTED };
    }
    states
}

/// Paints the outline into `region` (inclusive Blender rect, y up) of the `0x00RRGGBB` buffer from the object id
/// buffer of that region (`rw * rh` entries, row 0 at the bottom).
pub fn draw_outlines(buf: &mut [u32], width: usize, height: usize, region: Rcti, user: &UserDef, scene: &Scene, ids: &[u32]) {
    let (rw, rh) = (region.size_x() + 1, region.size_y() + 1);
    if rw <= 0 || rh <= 0 || ids.len() != (rw * rh) as usize {
        return;
    }
    let states = id_states(scene);
    if states.iter().all(|&s| s == NONE) {
        return;
    }
    let state_of = |id: u32| states.get(id as usize).copied().unwrap_or(NONE);
    let radius = user.pixelsize.max(1);
    let colors = [0, SPACE_VIEW3D_SELECT.to_xrgb(), SPACE_VIEW3D_ACTIVE.to_xrgb()];

    for py in 0..rh {
        for px in 0..rw {
            if state_of(ids[(py * rw + px) as usize]) != NONE {
                continue;
            }
            let mut best = NONE;
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    let (nx, ny) = (px + dx, py + dy);
                    if (dx == 0 && dy == 0) || nx < 0 || ny < 0 || nx >= rw || ny >= rh {
                        continue;
                    }
                    best = best.max(state_of(ids[(ny * rw + nx) as usize]));
                }
            }
            if best == NONE {
                continue;
            }
            let (x, y) = (region.xmin + px, region.ymin + py);
            if x < 0 || y < 0 || x as usize >= width || y as usize >= height {
                continue;
            }
            buf[(height - 1 - y as usize) * width + x as usize] = colors[best as usize];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BG: u32 = 0x0010_1010;

    fn setup() -> (Scene, UserDef, Rcti) {
        let mut scene = Scene::startup();
        scene.select_all(crate::scene::SelectAction::Deselect);
        (scene, UserDef::from_window_scale(1.0), Rcti { xmin: 0, xmax: 4, ymin: 0, ymax: 4 })
    }

    /* A 5x5 region where only the centre pixel belongs to the cube (id 2). */
    fn ids_with_centre(id: u32) -> Vec<u32> {
        let mut ids = vec![0u32; 25];
        ids[2 * 5 + 2] = id;
        ids
    }

    #[test]
    fn a_selected_object_gets_a_ring_in_the_select_colour() {
        let (mut scene, user, region) = setup();
        let cube = scene.objects[1].id;
        scene.select_pick(Some(cube), true);
        scene.active = None;
        let mut buf = vec![BG; 25];
        draw_outlines(&mut buf, 5, 5, region, &user, &scene, &ids_with_centre(cube + 1));
        let ring = SPACE_VIEW3D_SELECT.to_xrgb();
        let painted = buf.iter().filter(|&&p| p == ring).count();
        assert_eq!(painted, 8, "the 8 neighbours of the centre pixel");
        assert_eq!(buf[2 * 5 + 2], BG, "the object's own pixels are left alone");
        assert_eq!(buf[0], BG);
    }

    #[test]
    fn the_active_object_uses_the_active_colour() {
        let (mut scene, user, region) = setup();
        let cube = scene.objects[1].id;
        scene.select_pick(Some(cube), false);
        let mut buf = vec![BG; 25];
        draw_outlines(&mut buf, 5, 5, region, &user, &scene, &ids_with_centre(cube + 1));
        assert_eq!(buf[1 * 5 + 2], SPACE_VIEW3D_ACTIVE.to_xrgb());
    }

    #[test]
    fn unselected_objects_have_no_outline() {
        let (scene, user, region) = setup();
        let cube = scene.objects[1].id;
        let mut buf = vec![BG; 25];
        draw_outlines(&mut buf, 5, 5, region, &user, &scene, &ids_with_centre(cube + 1));
        assert!(buf.iter().all(|&p| p == BG));
    }

    /* The outline is also drawn over a pixel of another object next to the selected one, but not between two
     * selected objects, and the ids of hidden objects never select anything. */
    #[test]
    fn the_outline_runs_over_other_objects_but_not_between_selected_ones() {
        let (mut scene, user, region) = setup();
        let (cam, cube) = (scene.objects[0].id, scene.objects[1].id);
        scene.select_pick(Some(cube), true);
        let mut ids = ids_with_centre(cube + 1);
        ids[2 * 5 + 3] = cam + 1;
        let mut buf = vec![BG; 25];
        draw_outlines(&mut buf, 5, 5, region, &user, &scene, &ids);
        assert_ne!(buf[2 * 5 + 3], BG, "the other object's pixel next to the cube is outlined");
    }

    #[test]
    fn a_wrong_sized_id_buffer_is_ignored() {
        let (mut scene, user, region) = setup();
        scene.select_all(crate::scene::SelectAction::Select);
        let mut buf = vec![BG; 25];
        draw_outlines(&mut buf, 5, 5, region, &user, &scene, &[1, 2, 3]);
        assert!(buf.iter().all(|&p| p == BG));
    }

    /* The region is offset inside a larger window buffer and y is flipped. */
    #[test]
    fn the_region_offset_and_flip_are_respected() {
        let (mut scene, user, _) = setup();
        let cube = scene.objects[1].id;
        scene.select_pick(Some(cube), false);
        let (w, h) = (10usize, 10usize);
        let region = Rcti { xmin: 3, xmax: 7, ymin: 2, ymax: 6 };
        let mut buf = vec![BG; w * h];
        draw_outlines(&mut buf, w, h, region, &user, &scene, &ids_with_centre(cube + 1));
        /* Region pixel (2, 2) is window (5, 4); its right neighbour is window (6, 4) at row h - 1 - 4. */
        assert_eq!(buf[(h - 1 - 4) * w + 6], SPACE_VIEW3D_ACTIVE.to_xrgb());
        assert_eq!(buf[(h - 1 - 4) * w + 5], BG);
    }
}
