//! Popup menu geometry (window space, y up) ported from Blender:
//! `interface.cc` (`block_bounds_calc_text`, `block_bounds_set_popup` clamp), `interface_region_popup.cc`
//! (`popup_block_position`: direction and offset against the parent button, `popup_block_clip`) and
//! `interface_layout.cc` (`Layout::separator`, `text_icon_width_ex`). Pure functions: text widths come in as arguments.

pub const UI_SCREEN_MARGIN: f32 = 10.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rf {
    pub xmin: f32,
    pub ymin: f32,
    pub xmax: f32,
    pub ymax: f32,
}

impl Rf {
    pub fn new(xmin: f32, ymin: f32, xmax: f32, ymax: f32) -> Self {
        Rf { xmin, ymin, xmax, ymax }
    }
    pub fn width(&self) -> f32 {
        self.xmax - self.xmin
    }
    pub fn height(&self) -> f32 {
        self.ymax - self.ymin
    }
    fn translated(&self, dx: f32, dy: f32) -> Rf {
        Rf::new(self.xmin + dx, self.ymin + dy, self.xmax + dx, self.ymax + dy)
    }
}

/// `U.widget_unit` and `UI_SCALE_FAC`.
#[derive(Clone, Copy, Debug)]
pub struct Metrics {
    pub unit: i32,
    pub scale: f32,
}

impl Metrics {
    /// `UI_MENU_PADDING = int(0.2 * UI_UNIT_Y)`.
    pub fn menu_padding(&self) -> i32 {
        (0.2 * self.unit as f32) as i32
    }
    /// `UI_MENU_WIDTH_MIN = UI_UNIT_Y * 9`.
    pub fn menu_width_min(&self) -> i32 {
        self.unit * 9
    }
    /// `UI_MENU_SUBMENU_PADDING = 6 * UI_SCALE_FAC`, added to every button's `xmax` when the block has a submenu row.
    pub fn submenu_padding(&self) -> f32 {
        6.0 * self.scale
    }
    /// `UI_POPUP_MENU_TOP = int(10 * UI_SCALE_FAC)`.
    pub fn popup_menu_top(&self) -> f32 {
        (10.0 * self.scale) as i32 as f32
    }
    /// `Layout::separator`: `int(7 * UI_SCALE_FAC * factor)` in a menu, the height of a separator row.
    pub fn separator_height(&self, factor: f32) -> i32 {
        (7.0 * self.scale * factor) as i32
    }
    /// `block_bounds_calc_text` for `BLOCK_BOUNDS_POPUP_MENU` with `BLOCK_LOOP`: `bounds = 2.5 * UI_UNIT_X`, the column is
    /// `max(widest drawstr + bounds, minbounds)` wide. Separators do not count.
    pub fn column_width(&self, widest_text: i32, minbounds: i32) -> i32 {
        (widest_text + (2.5 * self.unit as f32) as i32).max(minbounds)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Row {
    Item,
    Separator(f32),
}

/// Rows stacked top to bottom (the layout starts at y = 0 and goes down): each `Item` is `UI_UNIT_Y` high, a separator
/// `separator_height`. Returns the rects (relative, top at 0, y up so the first row is `-unit..0`).
pub fn stack_rows(m: &Metrics, rows: &[Row], width: i32) -> Vec<Rf> {
    let mut y = 0.0;
    rows.iter()
        .map(|r| {
            let h = match r {
                Row::Item => m.unit as f32,
                Row::Separator(f) => m.separator_height(*f) as f32,
            };
            y -= h;
            Rf::new(0.0, y, width as f32, y + h)
        })
        .collect()
}

/// `BLI_rcti_clamp` against `[margin, win - margin] x [margin, win_h - UI_POPUP_MENU_TOP]` (block_bounds_calc popup path):
/// the rect is moved, never resized, unless it is larger than the bounds.
pub fn clamp_to_window(m: &Metrics, rect: Rf, win: (f32, f32)) -> Rf {
    let (bxmin, bymin, bxmax, bymax) = (UI_SCREEN_MARGIN, UI_SCREEN_MARGIN, win.0 - UI_SCREEN_MARGIN, win.1 - m.popup_menu_top());
    let mut dx = 0.0;
    let mut dy = 0.0;
    if rect.xmax > bxmax {
        dx = bxmax - rect.xmax;
    }
    if rect.xmin + dx < bxmin {
        dx = bxmin - rect.xmin;
    }
    if rect.ymax > bymax {
        dy = bymax - rect.ymax;
    }
    if rect.ymin + dy < bymin {
        dy = bymin - rect.ymin;
    }
    rect.translated(dx, dy)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Down,
    Up,
    Left,
    Right,
}

/// `popup_block_position` offsets for a block of `size` against the parent button `but` (primary direction `dir1`, with
/// `dir2_left` the secondary alignment for up/down). `max(pixelsize, 1)` is `overlap`. Returns the placed rect, then
/// `popup_block_clip`: shifted into `[margin, win_w - margin]` horizontally, `ymin >= margin`, `ymax <= win_h - top`.
pub fn place_popup(m: &Metrics, size: (f32, f32), but: Rf, dir1: Dir, dir2_left: bool, overlap: f32, win: (f32, f32)) -> Rf {
    let (w, h) = size;
    let center_y = 0.0;
    let pad = m.menu_padding() as f32;
    let size_x = w + 0.2 * m.unit as f32;
    let size_y = h + 0.2 * m.unit as f32;
    /* "check if there's space at all", then flip the primary direction when it does not fit. */
    let left = but.xmin - size_x > 0.0;
    let right = but.xmax + size_x < win.0;
    let down = but.ymin - size_y + center_y > 0.0;
    let top = but.ymax + size_y - center_y < win.1;
    let mut dir1 = dir1;
    if left || right {
        if dir1 == Dir::Left && !left {
            dir1 = Dir::Right;
        }
        if dir1 == Dir::Right && !right {
            dir1 = Dir::Left;
        }
    }
    if down || top {
        if dir1 == Dir::Up && !top {
            dir1 = Dir::Down;
        }
        if dir1 == Dir::Down && !down {
            dir1 = Dir::Up;
        }
    }
    let (ox, oy) = match dir1 {
        Dir::Left => (but.xmin - w + overlap, but.ymax - h + pad),
        Dir::Right => (but.xmax - overlap, but.ymax - h + pad),
        Dir::Up => (if dir2_left { but.xmin } else { but.xmax - w }, but.ymax - overlap),
        Dir::Down => (if dir2_left { but.xmin } else { but.xmax - w }, but.ymin - h + overlap),
    };
    clip(m, Rf::new(ox, oy, ox + w, oy + h), win)
}

/// `popup_block_clip`.
pub fn clip(m: &Metrics, rect: Rf, win: (f32, f32)) -> Rf {
    let mut r = rect;
    if r.xmax > win.0 - UI_SCREEN_MARGIN {
        r = r.translated(win.0 - UI_SCREEN_MARGIN - r.xmax, 0.0);
    }
    if r.xmin < UI_SCREEN_MARGIN {
        r = r.translated(UI_SCREEN_MARGIN - r.xmin, 0.0);
    }
    r.ymin = r.ymin.max(UI_SCREEN_MARGIN);
    r.ymax = r.ymax.min(win.1 - m.popup_menu_top());
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    const M: Metrics = Metrics { unit: 20, scale: 1.0 };

    #[test]
    fn constants_at_scale_one() {
        assert_eq!(M.menu_padding(), 4);
        assert_eq!(M.menu_width_min(), 180);
        assert_eq!(M.submenu_padding(), 6.0);
        assert_eq!(M.popup_menu_top(), 10.0);
        assert_eq!(M.separator_height(1.0), 7);
    }

    #[test]
    fn width_from_text_and_minimum() {
        assert_eq!(M.column_width(100, 180), 180);
        assert_eq!(M.column_width(200, 180), 250);
    }

    #[test]
    fn rows_stack_top_to_bottom() {
        let r = stack_rows(&M, &[Row::Item, Row::Separator(1.0), Row::Item], 200);
        assert_eq!(r[0], Rf::new(0.0, -20.0, 200.0, 0.0));
        assert_eq!(r[1], Rf::new(0.0, -27.0, 200.0, -20.0));
        assert_eq!(r[2], Rf::new(0.0, -47.0, 200.0, -27.0));
    }

    #[test]
    fn submenu_beside_parent_row_top_aligned() {
        let row = Rf::new(100.0, 500.0, 300.0, 520.0);
        let r = place_popup(&M, (180.0, 100.0), row, Dir::Right, false, 1.0, (1920.0, 1080.0));
        assert_eq!(r.xmin, 299.0);
        assert_eq!(r.ymax, 520.0 + 4.0);
    }

    #[test]
    fn submenu_flips_left_when_no_room_right() {
        let row = Rf::new(1700.0, 500.0, 1900.0, 520.0);
        let r = place_popup(&M, (180.0, 100.0), row, Dir::Right, false, 1.0, (1920.0, 1080.0));
        assert_eq!(r.xmax, 1700.0 + 1.0);
    }

    #[test]
    fn under_header_button_aligned_left() {
        let b = Rf::new(50.0, 1050.0, 100.0, 1070.0);
        let r = place_popup(&M, (200.0, 300.0), b, Dir::Down, true, 1.0, (1920.0, 1080.0));
        assert_eq!((r.xmin, r.ymax), (50.0, 1051.0));
    }

    #[test]
    fn clamped_to_window_edges() {
        let r = clip(&M, Rf::new(-30.0, -5.0, 150.0, 2000.0), (800.0, 600.0));
        assert_eq!(r.xmin, 10.0);
        assert_eq!(r.ymin, 10.0);
        assert_eq!(r.ymax, 590.0);
        let r = clip(&M, Rf::new(700.0, 100.0, 900.0, 200.0), (800.0, 600.0));
        assert_eq!(r.xmax, 790.0);
        let c = clamp_to_window(&M, Rf::new(700.0, 100.0, 900.0, 650.0), (800.0, 600.0));
        assert_eq!((c.xmax, c.ymax), (790.0, 590.0));
    }
}
