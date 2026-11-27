//! The toolbar region of the 3D viewport (`RGN_TYPE_TOOLS`, `VIEW3D_PT_tools_active`): the default tools of Object
//! Mode as one column of cells. Sources: `_tools_default` / `_tools` `'OBJECT'` (space_toolsystem_toolbar.py:3898-3950,
//! `_defs_*` idnames, labels and icons), the single column layout (`_layout_generator_single_column`,
//! `draw_cls(scale_y=1.75)`, space_toolsystem_common.py:597, 686) and `UI_TOOLBAR_WIDTH` (space_view3d.cc:1737,
//! UI_interface_c.hh:442-446). A tuple of the tool lists is a group: one cell showing its first tool.
//! Deviations: layout only; the gap between groups is `button_space_y`, the top margin is half `UI_TOOLBAR_MARGIN`.

use crate::area::Rcti;
use crate::theme::Rgba;
use crate::userdef::UserDef;
use crate::widget_draw::draw_round_box;

/// `ICON_DEFAULT_HEIGHT_TOOLBAR`.
const ICON_HEIGHT_TOOLBAR: f32 = 32.0;
/// `UI_TOOLBAR_COLUMN`, `UI_TOOLBAR_MARGIN`.
const COLUMN: f32 = 1.25 * ICON_HEIGHT_TOOLBAR;
const MARGIN: f32 = 0.5 * ICON_HEIGHT_TOOLBAR;
/// `scale_y` of `draw_cls` times `UI_UNIT_Y` (20 px at scale 1).
const CELL_SCALE_Y: f32 = 1.75;
const UNIT_Y: f32 = 20.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tool {
    pub idname: &'static str,
    pub label: &'static str,
    /// `icon=` of the ToolDef (`release/datafiles/icons/<icon>.dat`).
    pub icon: &'static str,
}

const fn t(idname: &'static str, label: &'static str, icon: &'static str) -> Tool {
    Tool { idname, label, icon }
}

pub const SELECT: Tool = t("builtin.select", "Tweak", "ops.generic.select");
pub const SELECT_BOX: Tool = t("builtin.select_box", "Select Box", "ops.generic.select_box");
pub const SELECT_CIRCLE: Tool = t("builtin.select_circle", "Select Circle", "ops.generic.select_circle");
pub const SELECT_LASSO: Tool = t("builtin.select_lasso", "Select Lasso", "ops.generic.select_lasso");
pub const CURSOR: Tool = t("builtin.cursor", "Cursor", "ops.generic.cursor");
pub const MOVE: Tool = t("builtin.move", "Move", "ops.transform.translate");
pub const ROTATE: Tool = t("builtin.rotate", "Rotate", "ops.transform.rotate");
pub const SCALE: Tool = t("builtin.scale", "Scale", "ops.transform.resize");
pub const SCALE_CAGE: Tool = t("builtin.scale_cage", "Scale Cage", "ops.transform.resize.cage");
pub const TRANSFORM: Tool = t("builtin.transform", "Transform", "ops.transform.transform");
pub const ANNOTATE: Tool = t("builtin.annotate", "Annotate", "ops.gpencil.draw");
pub const ANNOTATE_LINE: Tool = t("builtin.annotate_line", "Annotate Line", "ops.gpencil.draw.line");
pub const ANNOTATE_POLY: Tool = t("builtin.annotate_polygon", "Annotate Polygon", "ops.gpencil.draw.poly");
pub const ANNOTATE_ERASER: Tool = t("builtin.annotate_eraser", "Annotate Eraser", "ops.gpencil.draw.eraser");
pub const MEASURE: Tool = t("builtin.measure", "Measure", "ops.view3d.ruler");
pub const ADD_CUBE: Tool = t("builtin.primitive_cube_add", "Add Cube", "ops.mesh.primitive_cube_add_gizmo");

/// One cell of the column: a tool or a group of tools (the cell shows the first).
pub type Cell = &'static [Tool];

/// `None` is a separator, a start of the next column of the layout generator.
pub const OBJECT_MODE: [Option<Cell>; 11] = [
    Some(&[SELECT, SELECT_BOX, SELECT_CIRCLE, SELECT_LASSO]),
    Some(&[CURSOR]),
    None,
    Some(&[MOVE]),
    Some(&[ROTATE]),
    Some(&[SCALE, SCALE_CAGE]),
    Some(&[TRANSFORM]),
    None,
    Some(&[ANNOTATE, ANNOTATE_LINE, ANNOTATE_POLY, ANNOTATE_ERASER]),
    Some(&[MEASURE]),
    None,
];
/* The add tools group follows the last separator: cube, cone, cylinder, UV sphere, ico sphere. */
pub const ADD_GROUP: Cell = &[ADD_CUBE];

/// `UI_TOOLBAR_WIDTH` in pixels.
pub fn width(user: &UserDef) -> i32 {
    ((MARGIN + COLUMN) * user.scale_factor) as i32
}

/// The region of the toolbar in the 3D viewport area `main`: left aligned, from the bottom up to `top`
/// (the lower edge of the header regions), `region_rect_recursive()` for `RGN_ALIGN_LEFT`.
pub fn region_rect(user: &UserDef, main: Rcti, top: i32) -> Rcti {
    Rcti { xmin: main.xmin, xmax: main.xmin + width(user) - 1, ymin: main.ymin, ymax: top }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placed {
    pub rect: Rcti,
    /// The tool the cell shows (the first of its group).
    pub tool: Tool,
    /// The cell is a group (`operator_menu_hold`).
    pub group: bool,
    /// `depress=is_active`: the active tool is any tool of the cell.
    pub active: bool,
}

/// The cells of the column in window pixels (y up), top to bottom, for the active tool idname `active`.
pub fn layout(user: &UserDef, region: Rcti, active: &str) -> Vec<Placed> {
    let s = user.scale_factor;
    let cell_h = (CELL_SCALE_Y * UNIT_Y * s).round() as i32;
    let gap = user.button_space_y;
    let x0 = region.xmin + (MARGIN * s / 2.0) as i32;
    let x1 = region.xmax - (MARGIN * s / 2.0) as i32;
    let mut y = region.ymax - (MARGIN * s / 2.0) as i32;
    let mut out = Vec::new();
    let all = OBJECT_MODE.iter().copied().chain(std::iter::once(Some(ADD_GROUP)));
    for item in all {
        let Some(cell) = item else {
            y -= gap;
            continue;
        };
        out.push(Placed {
            rect: Rcti { xmin: x0, xmax: x1, ymax: y, ymin: y - cell_h + 1 },
            tool: cell[0],
            group: cell.len() > 1,
            active: cell.iter().any(|t| t.idname == active),
        });
        y -= cell_h;
    }
    out
}

/// `.wcol_toolbar_item` (userdef_default_theme.c:42-51): `inner_sel` is the depressed (active tool) colour.
pub const WCOL_TOOLBAR_ITEM_INNER: Rgba = Rgba::hex(0x282828ff);
pub const WCOL_TOOLBAR_ITEM_INNER_SEL: Rgba = Rgba::hex(0x4772b3ff);
pub const WCOL_TOOLBAR_ITEM_ROUNDNESS: f32 = 0.2;

/// The background of a cell: `inner_sel` while the tool is active (`UI_SELECT`/`UI_BUT_DRAW_ACTIVE` depress),
/// brightened by the hover colour; an inactive cell is only drawn under the cursor (the toolbar items have no
/// frame at rest, `widget_state()` of `ToolbarItem` draws no box without `UI_ACTIVE`/`UI_SELECT`).
pub fn cell_inner(cell: &Placed) -> Option<Rgba> {
    cell.active.then_some(WCOL_TOOLBAR_ITEM_INNER_SEL)
}

/// Draws the cell backgrounds of `cells` (see `layout`).
pub fn draw_cells(buf: &mut [u32], width: usize, height: usize, user: &UserDef, cells: &[Placed]) {
    let none = Rgba::hex(0);
    for c in cells {
        let hovered = crate::hover::inside(c.rect);
        let inner = match (cell_inner(c), hovered) {
            (Some(i), _) => i,
            (None, true) => WCOL_TOOLBAR_ITEM_INNER,
            (None, false) => continue,
        };
        let (inner, _) = crate::hover::colors(c.rect, inner, none);
        draw_round_box(buf, width, height, user, c.rect, inner, none, WCOL_TOOLBAR_ITEM_ROUNDNESS);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placed(active: &str) -> Vec<Placed> {
        let user = UserDef::from_window_scale(1.0);
        let region = region_rect(&user, Rcti { xmin: 0, xmax: 999, ymin: 0, ymax: 799 }, 740);
        layout(&user, region, active)
    }

    #[test]
    fn column_width_is_ui_toolbar_width() {
        assert_eq!(width(&UserDef::from_window_scale(1.0)), 56);
        assert_eq!(width(&UserDef::from_window_scale(2.0)), 112);
        let p = placed("");
        assert_eq!((p[0].rect.xmin, p[0].rect.xmax), (8, 47), "UI_TOOLBAR_COLUMN wide cells");
    }

    #[test]
    fn tools_come_in_blenders_object_mode_order() {
        let labels: Vec<&str> = placed("").iter().map(|c| c.tool.label).collect();
        assert_eq!(labels, ["Tweak", "Cursor", "Move", "Rotate", "Scale", "Transform", "Annotate", "Measure", "Add Cube"]);
        let groups: Vec<bool> = placed("").iter().map(|c| c.group).collect();
        assert_eq!(groups, [true, false, false, false, true, false, true, false, false]);
    }

    #[test]
    fn the_active_tool_depresses_its_cell_even_inside_a_group() {
        let on: Vec<&str> = placed("builtin.select_box").iter().filter(|c| c.active).map(|c| c.tool.label).collect();
        assert_eq!(on, ["Tweak"]);
        assert!(placed("builtin.measure")[7].active);
        assert!(placed("builtin.nothing").iter().all(|c| !c.active));
    }

    #[test]
    fn cells_are_35_px_and_groups_are_spaced_by_a_gap() {
        let p = placed("");
        assert_eq!(p[0].rect.size_y() + 1, 35);
        assert_eq!(p[0].rect.ymin - p[1].rect.ymax, 1, "no gap inside a column");
        assert_eq!(p[1].rect.ymin - p[2].rect.ymax - 1, 2, "a separator adds button_space_y");
        assert_eq!(p[0].rect.ymax, 740 - 8);
    }

    #[test]
    fn active_cell_is_filled_with_the_depressed_colour() {
        let user = UserDef::from_window_scale(1.0);
        let cells = placed("builtin.select_box");
        let (w, h) = (1000usize, 800usize);
        let mut buf = vec![0u32; w * h];
        crate::hover::begin_frame(None);
        draw_cells(&mut buf, w, h, &user, &cells);
        let px = |r: Rcti| buf[(h - 1 - r.cent_y() as usize) * w + r.cent_x() as usize];
        let active = cells.iter().find(|c| c.active).unwrap();
        let other = cells.iter().find(|c| !c.active).unwrap();
        assert_eq!(cell_inner(active), Some(WCOL_TOOLBAR_ITEM_INNER_SEL));
        assert_ne!(px(active.rect), 0);
        assert_eq!(px(other.rect), 0);
    }
}
