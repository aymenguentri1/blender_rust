//! The tab column of the properties editor navigation bar (`RGN_TYPE_NAV_BAR`, `prop_tabs_enum` of
//! `space_properties.py`, tab icons from `buttons_context_items`, rna_space.cc:603). Visual only.
//! Deviation: the geometry (tab height 1.4 units, 7 px above the first tab and in a separator at 1x) is measured
//! from Blender 5.1.1, the panel layout that produces it was not ported.

use crate::area::Rcti;
use crate::buttons_context::{tab_icon, Context, Tab};
use crate::icons::{self};
use crate::scene::ObjectData;
use crate::theme;
use crate::userdef::UserDef;
use crate::widget_draw::draw_round_box;

/// Rect of every tab of `tabs` inside the nav bar `nav` (window coordinates, y up), top to bottom.
pub fn layout(user: &UserDef, nav: Rcti, tabs: &[Tab]) -> Vec<(Context, Rcti)> {
    let s = user.scale_factor;
    let (tab_h, gap) = ((user.widget_unit as f32 * 1.4) as i32, (7.0 * s) as i32);
    let mut y = nav.ymax + 1 - gap;
    let mut out = Vec::new();
    for t in tabs {
        match *t {
            Tab::Separator => y -= gap,
            Tab::Tab(c) => {
                out.push((c, Rcti { ymin: y - tab_h, ymax: y - 1, ..nav }));
                y -= tab_h;
            }
        }
    }
    out
}

pub fn draw(buf: &mut [u32], width: usize, height: usize, user: &UserDef, nav: Rcti, tabs: &[Tab], active: Context, data: Option<&ObjectData>) {
    for (c, r) in layout(user, nav, tabs) {
        let on = c == active;
        let base = if on { theme::WCOL_TAB_INNER_SEL } else { theme::WCOL_TAB_INNER };
        let hovered = crate::hover::over(r);
        if on || hovered {
            let inner = crate::hover::colors(r, base, theme::WCOL_TAB_TEXT).0;
            draw_round_box(buf, width, height, user, r, inner, theme::WCOL_TAB_OUTLINE, theme::WCOL_TAB_ROUNDNESS);
        }
        let icon = tab_icon(c, data);
        let mono = if on { theme::WCOL_TAB_TEXT_SEL } else { theme::WCOL_TAB_TEXT };
        icons::draw_icon(buf, width, height, user, r, icon, crate::icon_theme::theme_color(icon).unwrap_or(mono));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buttons_context::{path_flags, tabs_list};
    use crate::scene::Scene;

    #[test]
    fn tabs_stack_downwards_inside_the_bar() {
        let user = UserDef::from_window_scale(1.0);
        let nav = Rcti { xmin: 1580, xmax: 1608, ymin: 20, ymax: 700 };
        let tabs = tabs_list(path_flags(&Scene::startup()));
        let l = layout(&user, nav, &tabs);
        assert_eq!(l.len(), 14);
        assert_eq!(l[0].0, Context::Tool);
        for w in l.windows(2) {
            assert!(w[1].1.ymax < w[0].1.ymin, "{:?} below {:?}", w[1].0, w[0].0);
        }
        /* A separator adds a gap: Tool -> Render is further apart than Render -> Output. */
        assert!(l[0].1.ymin - l[1].1.ymax > l[1].1.ymin - l[2].1.ymax);
        assert!(l.iter().all(|(_, r)| r.xmin == 1580 && r.xmax == 1608));
    }
}
