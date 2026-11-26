//! Background of the outliner's window region.
//!
//! Ported from Blender:
//!  - `outliner_back()`        (editors/space_outliner/outliner_draw.cc:3775-3811): every second row is drawn
//!    with `TH_BACK` blended towards `TH_ROW_ALTERNATE` by the alpha of the latter
//!  - `OL_Y_OFFSET`            (editors/space_outliner/outliner_intern.hh:200)
//!
//! The view is the initial one: `View2D::tot.ymax` is 0 and, with `V2D_ALIGN_NO_POS_Y`
//! (space_outliner.cc:75), `cur.ymax` is 0 too (the tree is not scrolled), so `cur.ymin` is minus the region
//! height. The names of the startup scene's tree rows are drawn by `draw_tree_names()`
//! (`outliner_draw_tree()` / `outliner_draw_tree_element()`, outliner_draw.cc:3162-3774); icons, expand
//! arrows, highlights and the restriction columns are not drawn.

use crate::area::{fill_rect, top_header_rect, Rcti};
use crate::font;
use crate::theme;
use crate::userdef::UserDef;

/// `OL_Y_OFFSET`.
const OL_Y_OFFSET: i32 = 2;

/// The outliner's window region: what is left of `totrct` below the (non-overlapping) header.
pub fn window_rect(user: &UserDef, totrct: Rcti) -> Option<Rcti> {
    let header = top_header_rect(user, totrct)?;
    let rect = Rcti { ymax: header.ymin - 1, ..totrct };
    if rect.is_empty() { None } else { Some(rect) }
}

/// `outliner_back()` for the window `region` (inclusive Blender rect, y up) of the `0x00RRGGBB` buffer.
pub fn draw_row_stripes(buf: &mut [u32], width: usize, height: usize, user: &UserDef, region: Rcti) {
    let rh = region.size_y() + 1;
    let unit = user.widget_unit;
    let ystart = -OL_Y_OFFSET;
    /* `int(floor(ystart - cur.ymin + 2 * UI_UNIT_Y)) / (2 * UI_UNIT_Y)`, with `cur.ymin = -rh`. */
    let tot = (ystart + rh + 2 * unit) / (2 * unit);

    /* `immUniformThemeColorBlend(TH_BACK, TH_ROW_ALTERNATE, alpha)`, stored as 8 bit. */
    let (back, alt) = (theme::SPACE_OUTLINER_BACK, theme::SPACE_OUTLINER_ROW_ALTERNATE);
    let fac = alt.a as f32 / 255.0;
    let mix = |a: u8, b: u8| ((1.0 - fac) * a as f32 + fac * b as f32).round() as u32;
    let color = (mix(back.r, alt.r) << 16) | (mix(back.g, alt.g) << 8) | mix(back.b, alt.b);

    for k in 1..=tot {
        /* View2D y of the stripe: `y1 .. y1 + UI_UNIT_Y`, with the region top at y = 0. */
        let y1 = ystart - 2 * unit * k;
        let y2 = y1 + unit;
        let stripe = Rcti { ymin: region.ymin + rh + y1, ymax: region.ymin + rh - 1 + y2, ..region };
        if let Some(visible) = region.isect(&stripe) {
            fill_rect(buf, width, height, visible, color);
        }
    }
}

/// The tree of the startup scene in the view layer display mode, top to bottom: the nesting level, the
/// name (`TreeElement::name`) and the text colour that `outliner_draw_tree_element()` picks:
/// `TH_TEXT`, `TH_TEXT_HI` for the active layer collection (`tree_element_type_active_state_get()`) and
/// `TH_ACTIVE_OBJECT` for the selected, active object. Collections and objects are sorted by name.
#[allow(dead_code)]
const TREE: [(i32, &str, theme::Rgba); 6] = [
    (0, "Scene Collection", theme::SPACE_OUTLINER_TEXT),
    (1, "Collection", theme::SPACE_OUTLINER_TEXT_HI),
    (2, "Camera", theme::SPACE_OUTLINER_TEXT),
    (2, "Cube", theme::SPACE_OUTLINER_ACTIVE_OBJECT),
    (2, "Light", theme::SPACE_OUTLINER_TEXT),
    (1, "XGen Guides", theme::SPACE_OUTLINER_TEXT),
];

/// The names of `outliner_draw_tree()` for the window `region` (inclusive Blender rect, y up) of the
/// `0x00RRGGBB` buffer: the first row has `starty = tot.ymax - UI_UNIT_Y - OL_Y_OFFSET` and each row is
/// `UI_UNIT_Y` lower; the element is at `startx = level * UI_UNIT_X`, then `offsx` skips the expand arrow
/// slot (`UI_UNIT_X`) and the icon slot (`UI_UNIT_X + 4 * ufac`), and the text baseline is `starty + 5 * ufac`
/// with `ufac = UI_UNIT_X / 20`. The icons that fill those slots are not drawn, and a row is skipped when it
/// is not entirely inside the region (Blender clips with the region scissor instead).
#[allow(dead_code)]
pub fn draw_tree_names(buf: &mut [u32], width: usize, height: usize, user: &UserDef, region: Rcti) {
    let rows: Vec<(i32, String, theme::Rgba)> = TREE.iter().map(|&(l, n, c)| (l, n.to_string(), c)).collect();
    draw_tree_rows(buf, width, height, user, region, &rows);
}

/// The rows of `scene` with the colours `outliner_draw_tree_element()` picks: `TH_TEXT_HI` for the active layer
/// collection, `TH_ACTIVE_OBJECT` for the selected active object, `TH_SELECTED_OBJECT` for the other selected
/// ones and `TH_TEXT` for the rest.
pub fn scene_rows(scene: &crate::scene::Scene) -> Vec<(i32, String, theme::Rgba)> {
    use crate::scene::RowKind;
    scene
        .outliner_rows()
        .into_iter()
        .map(|r| {
            let color = match r.kind {
                RowKind::ActiveCollection => theme::SPACE_OUTLINER_TEXT_HI,
                RowKind::Object(id) => match scene.index_of(id).map(|i| &scene.objects[i]) {
                    Some(o) if o.selected && !o.hidden && scene.active == Some(id) => theme::SPACE_OUTLINER_ACTIVE_OBJECT,
                    Some(o) if o.selected && !o.hidden => theme::SPACE_OUTLINER_SELECTED_OBJECT,
                    _ => theme::SPACE_OUTLINER_TEXT,
                },
                _ => theme::SPACE_OUTLINER_TEXT,
            };
            (r.level, r.name, color)
        })
        .collect()
}

/// The index of the tree row under region y `y` (window coordinates, y up) of the window `region`, as
/// `outliner_find_element()` would find it: row `i` spans view y `starty .. starty + UI_UNIT_Y`.
pub fn row_at(user: &UserDef, region: Rcti, y: i32) -> Option<usize> {
    let rh = region.size_y() + 1;
    let unit = user.widget_unit;
    let view_y = y - (region.ymin + rh);
    let below = -view_y - OL_Y_OFFSET;
    if below <= 0 {
        return None;
    }
    let i = (below - 1) / unit;
    Some(i as usize)
}

pub fn draw_tree_rows(buf: &mut [u32], width: usize, height: usize, user: &UserDef, region: Rcti, rows: &[(i32, String, theme::Rgba)]) {
    let rh = region.size_y() + 1;
    let unit = user.widget_unit;
    let ufac = unit as f32 / 20.0;
    let offsx = (2.0 * unit as f32 + 4.0 * ufac) as i32;

    for (i, (level, name, color)) in rows.iter().enumerate() {
        let (level, color) = (*level, *color);
        let starty = -OL_Y_OFFSET - unit * (i as i32 + 1);
        if starty < -rh {
            break;
        }
        let x = region.xmin + level * unit + offsx;
        /* View2D y of the baseline to window y: the region top is view y 0. */
        let baseline = region.ymin + rh + (starty as f32 + 5.0 * ufac).round() as i32;
        font::draw_text(buf, width, height, user, x, baseline, name, color);
    }
}

/// The text field of an open rename (`TSE_TEXTBUT`) as window rect: it starts at the element's name and ends at the
/// right edge of the region minus `V2D_SCROLL_WIDTH`, over the row `UI_UNIT_Y` high.
pub fn rename_field_rect(user: &UserDef, region: Rcti, scene: &crate::scene::Scene, id: u32) -> Option<Rcti> {
    let rows = scene.outliner_rows();
    let i = rows.iter().position(|r| matches!(r.kind, crate::scene::RowKind::Object(o) if o == id))?;
    let rh = region.size_y() + 1;
    let unit = user.widget_unit;
    let offsx = (2.0 * unit as f32 + 4.0 * (unit as f32 / 20.0)) as i32;
    let ymin = region.ymin + rh - OL_Y_OFFSET - unit * (i as i32 + 1);
    let xmin = region.xmin + rows[i].level * unit + offsx;
    let scroll = (0.45 * unit as f32 + 2.0 * user.pixelsize as f32) as i32;
    let xmax = region.xmin + region.size_x() + 1 - scroll - 1;
    (xmax > xmin).then_some(Rcti { xmin, xmax, ymin, ymax: ymin + unit - 1 })
}

/// Draws the rename field: outline, inner, the selection, the text and the caret (colours as `rename_popup`'s field).
pub fn draw_rename_field(buf: &mut [u32], width: usize, height: usize, user: &UserDef, region: Rcti, scene: &crate::scene::Scene, edit: &crate::outliner_rename::Rename) {
    let Some(r) = rename_field_rect(user, region, scene, edit.id) else { return };
    let b = user.pixelsize.max(1);
    let (outline, inner, text) = (theme::Rgba::hex(0x4772b3ff), theme::Rgba::hex(0x282828ff), theme::Rgba::hex(0xffffffff));
    fill_rect(buf, width, height, r, outline.to_xrgb());
    let f = Rcti { xmin: r.xmin + b, xmax: r.xmax - b, ymin: r.ymin + b, ymax: r.ymax - b };
    fill_rect(buf, width, height, f, inner.to_xrgb());
    let pad = 4 * b;
    let at = |n: usize| r.xmin + pad + font::string_width(user, &edit.text.chars().take(n).collect::<String>());
    if edit.selend > edit.selsta {
        let sel = Rcti { xmin: at(edit.selsta), xmax: at(edit.selend) - 1, ymin: f.ymin + b, ymax: f.ymax - b };
        fill_rect(buf, width, height, sel, outline.to_xrgb());
    }
    let ufac = user.widget_unit as f32 / 20.0;
    font::draw_text(buf, width, height, user, r.xmin + pad, r.ymin + (5.0 * ufac).round() as i32, &edit.text, text);
    let x = at(edit.pos);
    fill_rect(buf, width, height, Rcti { xmin: x, xmax: x + b - 1, ymin: f.ymin + b + 1, ymax: f.ymax - b - 1 }, text.to_xrgb());
}

/// The restriction columns of the View Layer display mode (`SO_RESTRICT_*` of `SpaceOutliner::show_restrict_flags`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestrictCol {
    Render,
    Viewport,
    Hide,
    Select,
}

/// `SpaceOutliner::show_restrict_flags` as the startup file has it (space_outliner.cc:417): `SO_RESTRICT_ENABLE |
/// SO_RESTRICT_HIDE | SO_RESTRICT_RENDER`; only the columns that exist as object properties are modelled.
pub const DEFAULT_RESTRICT: [RestrictCol; 2] = [RestrictCol::Hide, RestrictCol::Render];

/// `V2D_SCROLL_WIDTH` (UI_view2d.hh:79), truncated like the `int` it ends up in.
fn scroll_width(user: &UserDef) -> i32 {
    (0.45 * user.widget_unit as f32 + 2.0 * user.pixelsize as f32) as i32
}

/// The left edge (window x) of the button of column `col` in `outliner_draw_restrictbuts()`
/// (outliner_draw.cc:1221-1250): `cur.xmax - offset` with the offset `(++column) * UI_UNIT_X + V2D_SCROLL_WIDTH`,
/// counting from the right in the order render, viewport, hide, select over the columns in `shown`.
pub fn restrict_col_x(user: &UserDef, region: Rcti, shown: &[RestrictCol], col: RestrictCol) -> Option<i32> {
    if !shown.contains(&col) {
        return None;
    }
    let order = [RestrictCol::Render, RestrictCol::Viewport, RestrictCol::Hide, RestrictCol::Select];
    let n = order.iter().take_while(|&&c| c != col).filter(|c| shown.contains(c)).count() as i32 + 1;
    let xmax = region.xmin + region.size_x() + 1;
    Some(xmax - (n * user.widget_unit + scroll_width(user)))
}

/// The column whose button is under window x `x`, if any (each button is `UI_UNIT_X` wide).
pub fn restrict_col_at(user: &UserDef, region: Rcti, shown: &[RestrictCol], x: i32) -> Option<RestrictCol> {
    [RestrictCol::Render, RestrictCol::Viewport, RestrictCol::Hide, RestrictCol::Select]
        .into_iter()
        .find(|&c| restrict_col_x(user, region, shown, c).is_some_and(|x0| x >= x0 && x < x0 + user.widget_unit))
}

/// The buttons of `outliner_draw_restrictbuts()` for the object rows of `scene`: the eye of the hide column
/// (`Object::hidden`, `ICON_HIDE_OFF`) and the camera of the render column (`Object::hide_render`,
/// `ICON_RESTRICT_RENDER_OFF`), one `UI_UNIT_X` square per row at `restrict_col_x()`. A restricted object's icon
/// is drawn dimmed (Blender swaps to the `_ON` icon variants, which are not in `icons`).
pub fn draw_restrict_buttons(buf: &mut [u32], width: usize, height: usize, user: &UserDef, region: Rcti, scene: &crate::scene::Scene) {
    use crate::icons::{draw_icon, Icon};
    use crate::scene::RowKind;
    let rh = region.size_y() + 1;
    let unit = user.widget_unit;
    let dim = |c: theme::Rgba, off: bool| theme::Rgba { a: if off { 70 } else { c.a }, ..c };
    for (i, row) in scene.outliner_rows().iter().enumerate() {
        let starty = -OL_Y_OFFSET - unit * (i as i32 + 1);
        if starty < -rh {
            break;
        }
        let RowKind::Object(id) = row.kind else { continue };
        let Some(o) = scene.index_of(id).map(|k| &scene.objects[k]) else { continue };
        let ymin = region.ymin + rh + starty;
        for (col, icon, off) in [(RestrictCol::Hide, Icon::Eye, o.hidden), (RestrictCol::Render, Icon::Camera, o.hide_render)] {
            let Some(x0) = restrict_col_x(user, region, &DEFAULT_RESTRICT, col) else { continue };
            let rect = Rcti { xmin: x0, xmax: x0 + unit - 1, ymin, ymax: ymin + unit - 1 };
            draw_icon(buf, width, height, user, rect, icon, dim(theme::SPACE_OUTLINER_TEXT, off));
        }
    }
}

/// What a click at window `(x, y)` on the restriction columns toggles: the object id and the column.
pub fn restrict_click(user: &UserDef, region: Rcti, scene: &crate::scene::Scene, x: i32, y: i32) -> Option<(u32, RestrictCol)> {
    let col = restrict_col_at(user, region, &DEFAULT_RESTRICT, x)?;
    match scene.outliner_rows().get(row_at(user, region, y)?)?.kind {
        crate::scene::RowKind::Object(id) => Some((id, col)),
        _ => None,
    }
}

#[cfg(test)]
mod restrict_tests {
    use super::*;

    /* 1x: unit 20, pixelsize 1 => V2D_SCROLL_WIDTH = 9 + 2 = 11; a 200 px wide region has xmax 200. */
    #[test]
    fn columns_count_from_the_right() {
        let user = UserDef::from_window_scale(1.0);
        let region = Rcti { xmin: 0, xmax: 199, ymin: 0, ymax: 99 };
        assert_eq!(restrict_col_x(&user, region, &DEFAULT_RESTRICT, RestrictCol::Render), Some(200 - 31));
        assert_eq!(restrict_col_x(&user, region, &DEFAULT_RESTRICT, RestrictCol::Hide), Some(200 - 51));
        assert_eq!(restrict_col_x(&user, region, &DEFAULT_RESTRICT, RestrictCol::Select), None);
        let all = [RestrictCol::Select, RestrictCol::Hide, RestrictCol::Viewport, RestrictCol::Render];
        assert_eq!(restrict_col_x(&user, region, &all, RestrictCol::Select), Some(200 - 91));
    }

    #[test]
    fn click_finds_object_and_column() {
        use crate::scene::Scene;
        let user = UserDef::from_window_scale(1.0);
        let region = Rcti { xmin: 0, xmax: 199, ymin: 0, ymax: 162 };
        let s = Scene::startup();
        /* Row 3 (Cube) spans region y 81..=100 (row i: 161 - 20 * (i + 1) ..); the render column is x 169..=188, the hide column x 149..=168. */
        let cube = s.outliner_rows()[3].kind.clone();
        let crate::scene::RowKind::Object(id) = cube else { panic!() };
        assert_eq!(restrict_click(&user, region, &s, 175, 90), Some((id, RestrictCol::Render)));
        assert_eq!(restrict_click(&user, region, &s, 155, 90), Some((id, RestrictCol::Hide)));
        assert_eq!(restrict_click(&user, region, &s, 100, 90), None);
        assert_eq!(restrict_click(&user, region, &s, 175, 130), None); /* the Collection row, y 121..=140 */
    }

    #[test]
    fn hit_test_columns() {
        let user = UserDef::from_window_scale(1.0);
        let region = Rcti { xmin: 0, xmax: 199, ymin: 0, ymax: 99 };
        assert_eq!(restrict_col_at(&user, region, &DEFAULT_RESTRICT, 170), Some(RestrictCol::Render));
        assert_eq!(restrict_col_at(&user, region, &DEFAULT_RESTRICT, 150), Some(RestrictCol::Hide));
        assert_eq!(restrict_col_at(&user, region, &DEFAULT_RESTRICT, 100), None);
        assert_eq!(restrict_col_at(&user, region, &DEFAULT_RESTRICT, 195), None);
    }
}

#[cfg(test)]
mod row_tests {
    use super::*;

    /* 1x, 200x163 region (rh = 163, unit 20): row 0 spans view y -22..-2, i.e. region y 141..=160. */
    #[test]
    fn rows_are_found_by_height() {
        let user = UserDef::from_window_scale(1.0);
        let region = Rcti { xmin: 0, xmax: 199, ymin: 0, ymax: 162 };
        assert_eq!(row_at(&user, region, 162), None);
        assert_eq!(row_at(&user, region, 160), Some(0));
        assert_eq!(row_at(&user, region, 141), Some(0));
        assert_eq!(row_at(&user, region, 140), Some(1));
        assert_eq!(row_at(&user, region, 100), Some(3));
    }

    #[test]
    fn rename_field_spans_name_to_scroll_edge() {
        use crate::scene::Scene;
        let user = UserDef::from_window_scale(1.0);
        let region = Rcti { xmin: 0, xmax: 199, ymin: 0, ymax: 162 };
        let s = Scene::startup();
        let id = s.active.unwrap();
        /* Cube row 3: level 2, x = 40 + 44; right edge 200 - 11 - 1 = 188; y 81..=100. */
        let r = rename_field_rect(&user, region, &s, id).unwrap();
        assert_eq!((r.xmin, r.xmax, r.ymin, r.ymax), (84, 188, 81, 100));
        assert_eq!(rename_field_rect(&user, region, &s, 9999), None);
        let (w, h) = (200usize, 163usize);
        let mut buf = vec![0u32; w * h];
        let edit = crate::outliner_rename::start_active(&s).unwrap();
        draw_rename_field(&mut buf, w, h, &user, region, &s, &edit);
        assert_eq!(buf[(h - 1 - 90) * w + 84], 0x0047_72b3); /* outline */
    }

    #[test]
    fn scene_rows_colour_the_selection() {
        use crate::scene::{Scene, SelectAction};
        let mut s = Scene::startup();
        let rows = scene_rows(&s);
        assert_eq!(rows[3].2, theme::SPACE_OUTLINER_ACTIVE_OBJECT);
        assert_eq!(rows[1].2, theme::SPACE_OUTLINER_TEXT_HI);
        s.select_all(SelectAction::Select);
        let rows = scene_rows(&s);
        assert_eq!(rows[2].2, theme::SPACE_OUTLINER_SELECTED_OBJECT);
        assert_eq!(rows[3].2, theme::SPACE_OUTLINER_ACTIVE_OBJECT);
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    /* 1x, 100x100 region: tot = (-2 + 100 + 40) / 40 = 3; the first stripe is the second row, 22..42 px below
     * the top, i.e. region y 58..=77; the next one is y 18..=37. 0x28 blended 4/255 towards 0xff is 43. */
    #[test]
    fn stripes_on_every_second_row() {
        let user = UserDef::from_window_scale(1.0);
        let (w, h) = (100usize, 100usize);
        let bg = theme::SPACE_OUTLINER_BACK.to_xrgb();
        let mut buf = vec![bg; w * h];
        let region = Rcti { xmin: 0, xmax: 99, ymin: 0, ymax: 99 };
        draw_row_stripes(&mut buf, w, h, &user, region);

        let at = |y: usize| buf[(h - 1 - y) * w + 50];
        assert_eq!(at(58), 0x002b_2b2b);
        assert_eq!(at(77), 0x002b_2b2b);
        assert_eq!(at(78), bg);
        assert_eq!(at(40), bg);
        assert_eq!(at(18), 0x002b_2b2b);
        assert_eq!(at(37), 0x002b_2b2b);
        assert_eq!(at(0), bg);
    }

    #[test]
    fn window_below_header() {
        let user = UserDef::from_window_scale(1.0);
        let totrct = Rcti { xmin: 1579, xmax: 1917, ymin: 864, ymax: 1052 };
        assert_eq!(window_rect(&user, totrct), Some(Rcti { xmin: 1579, xmax: 1917, ymin: 864, ymax: 1026 }));
    }

    /* With the font file present, in a 200x163 region at 1x (rh = 163): the Cube row (index 3) has
     * starty = -2 - 80 = -82, so its baseline is y = 163 - 82 + 5 = 86 and its text starts at x = 40 + 44 =
     * 84 in the active object colour (red well above blue); the rows end above y = 40 (XGen Guides baseline 46). */
    #[test]
    fn tree_names_are_drawn() {
        let user = UserDef::from_window_scale(1.0);
        if font::string_width(&user, "Cube") == 0 {
            return;
        }
        let (w, h) = (200usize, 163usize);
        let bg = theme::SPACE_OUTLINER_BACK.to_xrgb();
        let mut buf = vec![bg; w * h];
        let region = Rcti { xmin: 0, xmax: 199, ymin: 0, ymax: 162 };
        draw_tree_names(&mut buf, w, h, &user, region);

        let at = |x: usize, y: usize| buf[(h - 1 - y) * w + x];
        let orange = (84..110).any(|x| (84..98).any(|y| {
            let p = at(x, y);
            ((p >> 16) & 0xff) > (p & 0xff) + 60
        }));
        assert!(orange);
        assert!((0..w).all(|x| (0..40).all(|y| at(x, y) == bg)));
    }
}
