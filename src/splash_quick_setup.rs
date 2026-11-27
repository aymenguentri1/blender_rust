//! The first-run "Quick Setup" menu of the splash popup (`WM_MT_splash_quick_setup`, scripts/startup/bl_operators/wm.py):
//! optional "Import Preferences From Previous Version" block, then "Create New Preferences" / "Quick Setup" with
//! Language, Theme, Keymap, Mouse Select, Spacebar Action and "Save New Preferences" / "Continue". The content sits in
//! `split(0.20)` then `split(0.73)` of the menu width and uses property split. Row positions are measured from Blender
//! 5.4 at UI scale 1 (row pitch 22, 25 below a label, 8 extra before Keymap). Window coordinates are y up.

use crate::area::Rcti;
use crate::modifier_panel::{Style, Widget};
use crate::userdef::UserDef;

/// Centre of each row below the top of the menu column, scale 1, with the import block present.
const IMPORT_LABEL: i32 = 10;
const IMPORT_BUTTON: i32 = 35;
const IMPORT_LINE: i32 = 63;
const TITLE_AFTER_IMPORT: i32 = 82;
const HEIGHT_IMPORT: i32 = 265;
const HEIGHT_PLAIN: i32 = 193;

pub struct QuickSetup {
    pub widgets: Vec<Widget>,
    /// The separator line after the import block.
    pub line: Option<Rcti>,
    /// The "Save New Preferences" / "Continue" button.
    pub save: Rcti,
}

/// Height of the menu column in pixels.
pub fn menu_height(user: &UserDef, can_import: bool) -> i32 {
    ((if can_import { HEIGHT_IMPORT } else { HEIGHT_PLAIN }) as f32 * user.scale_factor) as i32
}

pub fn layout(user: &UserDef, menu: Rcti, can_import: bool, old_version: [i32; 2]) -> QuickSetup {
    let s = user.scale_factor;
    let unit = user.widget_unit;
    let sc = |v: i32| (v as f32 * s) as i32;
    /* Row `c` (centre offset from the top, scale 1) as a rectangle `x0..=x1`. */
    let row = |c: i32, x0: i32, x1: i32| {
        let ymax = menu.ymax - sc(c - 10);
        Rcti { xmin: x0, xmax: x1, ymin: ymax - unit + 1, ymax }
    };
    let margin = (menu.size_x() as f32 * 0.20) as i32;
    let cx0 = menu.xmin + margin;
    let cw = ((menu.size_x() - margin) as f32 * 0.73) as i32;
    let cx1 = cx0 + cw;
    let fx0 = cx0 + (cw as f32 * 0.436) as i32;
    let mut out = Vec::new();
    let mut w = |rect: Rcti, label: &str, style: Style| out.push(Widget { rect, label: label.into(), style, action: None, centered: false });
    let (mut line, shift) = (None, if can_import { 0 } else { -(TITLE_AFTER_IMPORT - IMPORT_LABEL) });
    if can_import {
        w(row(IMPORT_LABEL, menu.xmin, menu.xmax), "Import Preferences From Previous Version", Style::Title);
        w(row(IMPORT_BUTTON, cx0, cx1), &format!("Import Blender {}.{} Preferences", old_version[0], old_version[1]), Style::Button);
        let y = menu.ymax - sc(IMPORT_LINE - 10) - unit / 2;
        line = Some(Rcti { xmin: menu.xmin, xmax: menu.xmax, ymin: y, ymax: y });
    }
    let r = |c: i32| c + shift;
    w(row(r(TITLE_AFTER_IMPORT), menu.xmin, menu.xmax), if can_import { "Create New Preferences" } else { "Quick Setup" }, Style::Title);
    let lbl = |rect: Rcti| Rcti { xmin: cx0, xmax: fx0 - sc(9), ..rect };
    for (c, name, value) in [(107, "Language", "English (US)"), (129, "Theme", "Blender Dark"), (159, "Keymap", "Blender")] {
        let rect = row(r(c), fx0, cx1);
        w(lbl(rect), name, Style::PropLabel);
        w(rect, value, Style::DropBox);
    }
    let mouse = row(r(181), fx0, cx1);
    w(lbl(mouse), "Mouse Select", Style::PropLabel);
    let mid = (mouse.xmin + mouse.xmax) / 2;
    w(Rcti { xmax: mid, ..mouse }, "Left", Style::Selected);
    w(Rcti { xmin: mid, ..mouse }, "Right", Style::Button);
    let space = row(r(203), fx0, cx1);
    w(lbl(space), "Spacebar Action", Style::PropLabel);
    w(space, "Play", Style::DropBox);
    let save = row(r(239), cx0, cx1);
    w(save, if can_import { "Save New Preferences" } else { "Continue" }, Style::Button);
    QuickSetup { widgets: out, line, save }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn menu(user: &UserDef, can_import: bool) -> Rcti {
        Rcti { xmin: 26, xmax: 469, ymin: -menu_height(user, can_import), ymax: 0 }
    }

    #[test]
    fn the_rows_stay_inside_the_menu_and_the_save_button_is_last() {
        let u = UserDef::from_window_scale(1.0);
        for imp in [true, false] {
            let m = menu(&u, imp);
            let q = layout(&u, m, imp, [5, 1]);
            for w in &q.widgets {
                assert!(w.rect.xmin >= m.xmin && w.rect.xmax <= m.xmax && w.rect.ymin >= m.ymin && w.rect.ymax <= m.ymax, "{}", w.label);
            }
            assert!(q.widgets.iter().all(|w| w.rect.ymin >= q.save.ymin), "the save button is the lowest row");
            assert_eq!(q.line.is_some(), imp);
        }
    }

    #[test]
    fn the_content_column_matches_blender_at_scale_1() {
        let u = UserDef::from_window_scale(1.0);
        let q = layout(&u, menu(&u, true), true, [5, 1]);
        assert_eq!(q.save.xmin - 26, 88, "split(0.20) of the menu width");
        assert!((258..=260).contains(&q.save.size_x()), "split(0.73) of the rest: {}", q.save.size_x());
        assert_eq!(q.widgets[1].label, "Import Blender 5.1 Preferences");
        let lang = q.widgets.iter().find(|w| w.label == "English (US)").unwrap();
        assert_eq!(lang.rect.xmax, q.save.xmax);
    }

    #[test]
    fn without_a_previous_version_the_title_is_quick_setup_and_the_button_continue() {
        let u = UserDef::from_window_scale(1.0);
        let q = layout(&u, menu(&u, false), false, [0, 0]);
        assert_eq!(q.widgets[0].label, "Quick Setup");
        assert_eq!(q.widgets.last().unwrap().label, "Continue");
    }
}
