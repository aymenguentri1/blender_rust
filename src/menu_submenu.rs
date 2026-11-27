//! Submenu open/close on mouse move: `handle_menus_recursive()` (interface_handlers.cc) and the safety-rect
//! calculus of `ui_block_calc_pupock()` (regions/interface_region_popup.cc). A submenu opens when its parent row is
//! hovered; it stays while the cursor is inside it or its parent row (`SafetyRect::parent`) or its safety rect;
//! it closes when the cursor leaves all of those (`RETURN_OUT`, `level > 0`), or when another row of the parent is
//! hovered. `USER_MENU_CLOSE_LEAVE` is off (default) and the "mouse moving towards the block" triangle
//! (`mouse_motion_towards_check`) is not ported yet. Coordinates are window pixels, y up.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub xmin: f32,
    pub xmax: f32,
    pub ymin: f32,
    pub ymax: f32,
}

impl Rect {
    pub fn new(xmin: f32, xmax: f32, ymin: f32, ymax: f32) -> Self {
        Self { xmin, xmax, ymin, ymax }
    }
    /// `BLI_rctf_isect_pt()`.
    pub fn contains(&self, p: [f32; 2]) -> bool {
        p[0] >= self.xmin && p[0] <= self.xmax && p[1] >= self.ymin && p[1] <= self.ymax
    }
}

/// `3 * UI_SCALE_FAC`: safety margin next to the parent button.
pub const SAFETY_NEAR: f32 = 3.0;

/// The safety rect of a block opened from `parent`: small (`s2`) on the side facing the parent button, the window
/// size (`s1`, as `USER_MENU_CLOSE_LEAVE` is off) on the other sides. The "switched pull-down" exception is not ported.
pub fn safety_rect(block: Rect, parent: Rect, win_size: [f32; 2]) -> Rect {
    let midx = (parent.xmin + parent.xmax) * 0.5;
    let midy = (parent.ymin + parent.ymax) * 0.5;
    let (s1, s2) = (win_size[0], SAFETY_NEAR);
    Rect {
        xmin: block.xmin - if midx < block.xmin { s2 } else { s1 },
        xmax: block.xmax + if midx > block.xmax { s2 } else { s1 },
        ymin: block.ymin - if midy < block.ymin { s2 } else { s1 },
        ymax: block.ymax + if midy > block.ymax { s2 } else { s1 },
    }
}

/// A row of a menu block; `sub` is the block rect of its submenu, if it has one.
#[derive(Clone, Copy, Debug)]
pub struct Row {
    pub rect: Rect,
    pub sub: Option<Rect>,
}

#[derive(Clone, Debug)]
pub struct Level {
    pub block: Rect,
    /// `SafetyRect::parent`: the row (or pull-down button) that opened this block.
    pub parent: Rect,
    pub safety: Rect,
    pub rows: Vec<Row>,
    /// Row whose submenu is open or that is highlighted.
    pub hovered: Option<usize>,
}

/// The chain of open menu blocks, root first.
#[derive(Clone, Debug)]
pub struct MenuStack {
    pub levels: Vec<Level>,
    pub win_size: [f32; 2],
}

impl MenuStack {
    pub fn new(root: Rect, parent: Rect, rows: Vec<Row>, win_size: [f32; 2]) -> Self {
        let safety = safety_rect(root, parent, win_size);
        Self { levels: vec![Level { block: root, parent, safety, rows, hovered: None }], win_size }
    }

    pub fn depth(&self) -> usize {
        self.levels.len()
    }

    /// Mouse move: close submenus the cursor left, then update the hovered row of the innermost block under it,
    /// opening its submenu (`sub_rows` gives the rows of the submenu that opens for `(level, row)`).
    pub fn mouse_move(&mut self, p: [f32; 2], sub_rows: &dyn Fn(usize, usize) -> Vec<Row>) {
        /* Deepest first: a level > 0 quits when the cursor is outside its block, its parent row and its safety rect. */
        for i in (1..self.levels.len()).rev() {
            let l = &self.levels[i];
            if !l.block.contains(p) && !l.parent.contains(p) && !l.safety.contains(p) {
                self.levels.truncate(i);
            }
        }
        let Some(i) = self.levels.iter().rposition(|l| l.block.contains(p)) else {
            return;
        };
        let hit = self.levels[i].rows.iter().position(|r| r.rect.contains(p));
        let Some(row) = hit else {
            return;
        };
        if self.levels[i].hovered == Some(row) && self.levels.len() > i + 1 {
            return;
        }
        self.levels.truncate(i + 1);
        self.levels[i].hovered = Some(row);
        if let Some(sub) = self.levels[i].rows[row].sub {
            let parent = self.levels[i].rows[row].rect;
            let safety = safety_rect(sub, parent, self.win_size);
            let rows = sub_rows(i, row);
            self.levels.push(Level { block: sub, parent, safety, rows, hovered: None });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIN: [f32; 2] = [1000.0, 800.0];

    fn row(y: f32, sub: Option<Rect>) -> Row {
        Row { rect: Rect::new(0.0, 100.0, y, y + 20.0), sub }
    }

    /// Root block x 0..100 with rows 0..20 (plain), 20..40 (submenu A at x 103..203), 40..60 (plain).
    /// A has rows 20..40 (plain) and 40..60 (submenu B at x 206..306, y 40..80).
    fn stack() -> (MenuStack, impl Fn(usize, usize) -> Vec<Row>) {
        let a = Rect::new(103.0, 203.0, 20.0, 60.0);
        let b = Rect::new(206.0, 306.0, 40.0, 80.0);
        let root_rows = vec![row(0.0, None), row(20.0, Some(a)), row(40.0, None)];
        let s = MenuStack::new(Rect::new(0.0, 100.0, 0.0, 60.0), Rect::new(0.0, 100.0, 60.0, 80.0), root_rows, WIN);
        let sub = move |lvl: usize, r: usize| -> Vec<Row> {
            let rw = |x: f32, y: f32, sub| Row { rect: Rect::new(x, x + 100.0, y, y + 20.0), sub };
            match (lvl, r) {
                (0, 1) => vec![rw(103.0, 20.0, None), rw(103.0, 40.0, Some(b))],
                (1, 1) => vec![rw(206.0, 40.0, None), rw(206.0, 60.0, None)],
                _ => vec![],
            }
        };
        (s, sub)
    }

    #[test]
    fn hover_opens_the_submenu_only_on_a_submenu_row() {
        let (mut s, sub) = stack();
        s.mouse_move([50.0, 10.0], &sub);
        assert_eq!(s.depth(), 1);
        s.mouse_move([50.0, 30.0], &sub);
        assert_eq!(s.depth(), 2);
        assert_eq!(s.levels[0].hovered, Some(1));
    }

    #[test]
    fn stays_open_over_the_gap_and_inside_the_child() {
        let (mut s, sub) = stack();
        s.mouse_move([50.0, 30.0], &sub);
        s.mouse_move([101.5, 30.0], &sub); /* in the gap: parent column, inside the safety rect */
        assert_eq!(s.depth(), 2);
        s.mouse_move([150.0, 30.0], &sub);
        assert_eq!(s.depth(), 2);
        s.mouse_move([150.0, 100.0], &sub); /* above the child but within the large vertical safety */
        assert_eq!(s.depth(), 2);
    }

    #[test]
    fn hovering_another_parent_row_closes_it() {
        let (mut s, sub) = stack();
        s.mouse_move([50.0, 30.0], &sub);
        s.mouse_move([50.0, 50.0], &sub);
        assert_eq!(s.depth(), 1);
        assert_eq!(s.levels[0].hovered, Some(2));
        s.mouse_move([50.0, 30.0], &sub);
        assert_eq!(s.depth(), 2);
    }

    #[test]
    fn moving_back_over_the_parent_row_keeps_the_submenu() {
        let (mut s, sub) = stack();
        s.mouse_move([50.0, 30.0], &sub);
        s.mouse_move([150.0, 30.0], &sub);
        s.mouse_move([50.0, 25.0], &sub);
        assert_eq!(s.depth(), 2);
    }

    #[test]
    fn nested_two_levels_open_and_close_innermost_first() {
        let (mut s, sub) = stack();
        s.mouse_move([50.0, 30.0], &sub);
        s.mouse_move([150.0, 50.0], &sub);
        assert_eq!(s.depth(), 3);
        s.mouse_move([250.0, 50.0], &sub);
        assert_eq!(s.depth(), 3);
        /* Hover the plain row of A: B closes, A stays. */
        s.mouse_move([150.0, 30.0], &sub);
        assert_eq!(s.depth(), 2);
        /* Leave towards the far left (the safety rects are small on the parent side): all submenus close. */
        s.mouse_move([150.0, 50.0], &sub);
        s.mouse_move([-500.0, 790.0], &sub);
        assert_eq!(s.depth(), 1);
    }

    #[test]
    fn leaving_the_root_block_does_not_close_it() {
        let (mut s, sub) = stack();
        s.mouse_move([900.0, 700.0], &sub);
        assert_eq!(s.depth(), 1);
    }

    #[test]
    fn safety_rect_is_small_towards_the_parent() {
        let r = safety_rect(Rect::new(103.0, 203.0, 20.0, 60.0), Rect::new(0.0, 100.0, 20.0, 40.0), WIN);
        assert_eq!((r.xmin, r.xmax), (100.0, 1203.0));
        assert_eq!((r.ymin, r.ymax), (20.0 - 1000.0, 60.0 + 1000.0));
    }
}
