//! Headers of the outliner, properties editor and timeline: the editor type button
//! (`layout.template_header()`), the `filter_text` search field (`OUTLINER_HT_header`, space_outliner.py;
//! `PROPERTIES_HT_header`) and, in the outliner, the filter popover and `outliner.collection_new`.
//! Visual only. Deviations: the display mode dropdown always shows the default View Layer mode and is as wide as the
//! editor type button, the search field has a fixed width.

use crate::area::{global_header_layout_start, Rcti};
use crate::font;
use crate::icons::{self, Icon};
use crate::theme::{self, Rgba};
use crate::userdef::UserDef;
use crate::widget_draw::{draw_round_box, draw_round_box_sides};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Outliner,
    Properties,
    Timeline,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    /// Editor type: the icon of the space and the arrow.
    EditorType(Icon),
    /// `layout.prop(space, "display_mode", icon_only=True)` of the outliner: the icon of the mode (`display_mode_items`,
    /// rna_space.cc:4504; `SO_VIEW_LAYER` is `ICON_RENDER_RESULT`) and the arrow.
    DisplayMode(Icon),
    /// `prop(space, "filter_text", icon='VIEWZOOM')` with the placeholder text.
    Search,
    /// `row.popover(panel="OUTLINER_PT_filter", text="")`: no icon, only the popover arrow.
    Filter,
    /// `operator("outliner.collection_new", text="", icon='COLLECTION_NEW')`.
    Button(Icon),
}

#[derive(Clone, Copy, Debug)]
pub struct Item {
    pub rect: Rcti,
    pub look: Look,
}

/// `.wcol_text` of userdef_default_theme.c (inner, outline, text); `.wcol_tool` is in `theme`.
const TEXT_INNER: Rgba = Rgba::hex(0x1d1d1dff);
const TEXT_OUTLINE: Rgba = Rgba::hex(0x3d3d3dff);
const ICON_COLOR: Rgba = Rgba::hex(0xe6e6e6ff);
const PLACEHOLDER: Rgba = Rgba::hex(0xe6e6e680);

pub fn editor_icon(kind: Kind) -> Icon {
    match kind {
        Kind::Outliner => Icon::EditorOutliner,
        Kind::Properties => Icon::EditorProperties,
        Kind::Timeline => Icon::EditorTimeline,
    }
}

/// Lays the header `header` (window coordinates) out.
pub fn layout(user: &UserDef, header: Rcti, kind: Kind) -> Vec<Item> {
    let (offset, top_y, height) = global_header_layout_start(user, header.size_y() + 1);
    let (y0, y1) = (header.ymin + top_y - height, header.ymin + top_y - 1);
    let (u, space) = (user.widget_unit, user.button_space_x);
    let item = |x0: i32, w: i32, look| Item { rect: Rcti { xmin: x0, xmax: x0 + w - 1, ymin: y0, ymax: y1 }, look };
    let mut x = header.xmin + offset;
    let mut items = vec![item(x, u + u * 3 / 4, Look::EditorType(editor_icon(kind)))];
    x += u + u * 3 / 4 + space;
    if kind == Kind::Timeline {
        return items;
    }
    if kind == Kind::Outliner {
        items.push(item(x, u + u * 3 / 4, Look::DisplayMode(Icon::RenderResult)));
        x += u + u * 3 / 4 + space;
    }
    let mut right = header.xmax + 1 - offset;
    if kind == Kind::Outliner {
        right -= u * 3 / 2;
        items.push(item(right, u * 3 / 2, Look::Button(Icon::CollectionNew)));
        right -= space + u;
        items.push(item(right, u, Look::Filter));
        right -= space;
    }
    let w = (right - x).min(u * 6);
    if w >= u * 2 {
        items.push(item(x, w, Look::Search));
    }
    items
}

pub fn draw(buf: &mut [u32], width: usize, height: usize, user: &UserDef, items: &[Item]) {
    let u = user.widget_unit;
    for it in items {
        let r = it.rect;
        match it.look {
            Look::EditorType(icon) | Look::DisplayMode(icon) => {
                draw_round_box(buf, width, height, user, r, theme::WCOL_TOOL_INNER, theme::WCOL_TOOL_OUTLINE, theme::WCOL_TOOL_ROUNDNESS);
                icons::draw_icon(buf, width, height, user, Rcti { xmax: r.xmin + u - 1, ..r }, icon, ICON_COLOR);
                icons::draw_icon(buf, width, height, user, Rcti { xmin: r.xmax + 1 - u * 3 / 4, ..r }, Icon::DownArrow, ICON_COLOR);
            }
            Look::Filter => {
                draw_round_box(buf, width, height, user, r, theme::WCOL_TOOL_INNER, theme::WCOL_TOOL_OUTLINE, theme::WCOL_TOOL_ROUNDNESS);
                icons::draw_icon(buf, width, height, user, r, Icon::DownArrow, ICON_COLOR);
            }
            Look::Button(icon) => {
                draw_round_box_sides(buf, width, height, user, r, theme::WCOL_TOOL_INNER, theme::WCOL_TOOL_OUTLINE, theme::WCOL_TOOL_ROUNDNESS, true, true);
                icons::draw_icon(buf, width, height, user, r, icon, ICON_COLOR);
            }
            Look::Search => {
                draw_round_box(buf, width, height, user, r, TEXT_INNER, TEXT_OUTLINE, theme::WCOL_TOOL_ROUNDNESS);
                icons::draw_icon(buf, width, height, user, Rcti { xmax: r.xmin + u - 1, ..r }, Icon::Search, ICON_COLOR);
                font::draw_label_pad(buf, width, height, user, Rcti { xmin: r.xmin + u, ..r }, "Search", PLACEHOLDER, 0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header() -> Rcti {
        Rcti { xmin: 1580, xmax: 1915, ymin: 980, ymax: 1005 }
    }

    #[test]
    fn items_fit_and_do_not_overlap() {
        let user = UserDef::from_window_scale(1.0);
        for kind in [Kind::Outliner, Kind::Properties, Kind::Timeline] {
            let items = layout(&user, header(), kind);
            for (i, a) in items.iter().enumerate() {
                assert!(a.rect.xmin >= header().xmin && a.rect.xmax <= header().xmax, "{kind:?}");
                for b in &items[i + 1..] {
                    assert!(a.rect.isect(&b.rect).is_none(), "{kind:?} {:?} {:?}", a.look, b.look);
                }
            }
        }
    }

    #[test]
    fn which_items_each_header_has() {
        let user = UserDef::from_window_scale(1.0);
        let looks = |k| layout(&user, header(), k).iter().map(|i| i.look).collect::<Vec<_>>();
        assert_eq!(looks(Kind::Timeline), vec![Look::EditorType(Icon::EditorTimeline)]);
        assert!(looks(Kind::Properties).contains(&Look::Search));
        assert!(!looks(Kind::Properties).contains(&Look::Filter));
        assert!(looks(Kind::Outliner).contains(&Look::Button(Icon::CollectionNew)));
        assert_eq!(looks(Kind::Outliner)[1], Look::DisplayMode(Icon::RenderResult));
        assert!(!looks(Kind::Properties).iter().any(|l| matches!(l, Look::DisplayMode(_))));
    }

    #[test]
    fn search_is_dropped_when_it_does_not_fit() {
        let user = UserDef::from_window_scale(1.0);
        let narrow = Rcti { xmax: 1580 + 70, ..header() };
        assert!(!layout(&user, narrow, Kind::Properties).iter().any(|i| i.look == Look::Search));
    }
}
