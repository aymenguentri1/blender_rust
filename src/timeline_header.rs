//! The timeline header after the `View`, `Marker`, `Playback` labels: `playback_controls()` (space_time.py) and
//! the overlay selector of `DOPESHEET_HT_editor_buttons.draw_header()` (space_dopesheet.py:224). Left to right,
//! with `separator_spacer()` sharing the free width: auto keying and the key type, the transport buttons and their
//! options, playhead snapping | frame field, preview range toggle, `Start` and `End` | overlays.
//! Visual only. Deviations: the field widths and the `scale_x` factors are measured from Blender 5.1.1 (the
//! number button width logic is not ported); the key type is always `Keyframe`.

use crate::area::{global_header_layout_start, Rcti};
use crate::font;
use crate::icons::{self, Icon};
use crate::keyframe_draw::{self, KeyType};
use crate::theme::{self, Rgba};
use crate::userdef::UserDef;
use crate::widget_draw::draw_round_box_sides;

#[derive(Clone, Debug, PartialEq)]
pub enum Look {
    Glyph(Icon),
    /// `popover(text="")` without an icon: the arrow.
    Arrow,
    /// The key type of `TIME_PT_keyframing` (`ICON_KEYTYPE_KEYFRAME_VEC`, a diamond).
    KeyType,
    Field { label: Option<&'static str>, value: i32 },
}

#[derive(Clone, Debug)]
pub struct Item {
    pub rect: Rcti,
    pub look: Look,
    /// The neighbours that touch this item in an aligned row (square corners on those sides).
    pub left: bool,
    pub right: bool,
}

const ICON_COLOR: Rgba = Rgba::hex(0xe6e6e6ff);

/// One item of the description: a width in units of `UI_UNIT_X` and its look.
enum Part {
    Group(Vec<(f32, Look)>),
    Spacer,
}

fn parts(playing: bool, frame: i32, start: i32, end: i32) -> Vec<Part> {
    use Look::{Arrow, Field, KeyType};
    let g = Look::Glyph;
    let mut transport = vec![(1.0, g(Icon::Rew)), (1.0, g(Icon::FramePrev)), (1.0, g(Icon::PrevKeyframe))];
    if playing {
        transport.push((2.0, g(Icon::Pause)));
    } else {
        transport.push((1.0, g(Icon::PlayReverse)));
        transport.push((1.0, g(Icon::Play)));
    }
    transport.extend([(1.0, g(Icon::NextKeyframe)), (1.0, g(Icon::FrameNext)), (1.0, g(Icon::Ff)), (0.75, Arrow)]);
    vec![
        Part::Spacer,
        Part::Group(vec![(1.0, g(Icon::RadioButOff)), (1.0, KeyType)]),
        Part::Group(transport),
        Part::Group(vec![(1.0, g(Icon::Snap)), (0.75, Arrow)]),
        Part::Spacer,
        /* `row.scale_x = 0.95` on `frame_current`. */
        Part::Group(vec![(3.8, Field { label: None, value: frame })]),
        /* `use_preview_range`, then `Start` / `End` with `scale_x = 0.8`. */
        Part::Group(vec![
            (1.0, g(Icon::Time)),
            (4.4, Field { label: Some("Start"), value: start }),
            (4.4, Field { label: Some("End"), value: end }),
        ]),
        Part::Group(vec![(1.5, g(Icon::Overlays)), (0.75, Arrow)]),
    ]
}

/// Lays the items out from `x_start` (the end of the `Playback` label) to the right edge of `header`.
pub fn layout(user: &UserDef, header: Rcti, x_start: i32, playing: bool, frame: i32, range: (i32, i32)) -> Vec<Item> {
    let (offset, top_y, height) = global_header_layout_start(user, header.size_y() + 1);
    let (y0, y1) = (header.ymin + top_y - height, header.ymin + top_y - 1);
    let (u, space) = (user.widget_unit as f32, user.button_space_x);
    let list = parts(playing, frame, range.0, range.1);
    let w = |units: f32| (units * u) as i32;
    let groups = list.iter().filter(|p| matches!(p, Part::Group(_))).count() as i32;
    let spacers = list.iter().filter(|p| matches!(p, Part::Spacer)).count() as i32;
    let fixed: i32 = list
        .iter()
        .map(|p| if let Part::Group(g) = p { g.iter().map(|(x, _)| w(*x)).sum() } else { 0 })
        .sum::<i32>()
        + space * groups;
    let right = header.xmax + 1 - offset;
    let spacer_w = ((right - x_start - fixed) / spacers).max(0);
    let mut x = x_start;
    let mut items = Vec::new();
    for p in &list {
        match p {
            Part::Spacer => x += spacer_w,
            Part::Group(g) => {
                for (i, (units, look)) in g.iter().enumerate() {
                    let width = w(*units);
                    let rect = Rcti { xmin: x, xmax: x + width - 1, ymin: y0, ymax: y1 };
                    items.push(Item { rect, look: look.clone(), left: i > 0, right: i + 1 < g.len() });
                    x += width;
                }
                x += space;
            }
        }
    }
    items
}

pub fn draw(buf: &mut [u32], width: usize, height: usize, user: &UserDef, items: &[Item]) {
    let u = user.widget_unit;
    for it in items {
        let r = it.rect;
        draw_round_box_sides(buf, width, height, user, r, theme::WCOL_TOOL_INNER, theme::WCOL_TOOL_OUTLINE, theme::WCOL_TOOL_ROUNDNESS, !it.left, !it.right);
        match &it.look {
            Look::Glyph(icon) => icons::draw_icon(buf, width, height, user, r, *icon, ICON_COLOR),
            Look::Arrow => icons::draw_icon(buf, width, height, user, r, Icon::DownArrow, ICON_COLOR),
            Look::KeyType => {
                let (cx, cy) = (r.xmin as f32 + (r.size_x() + 1) as f32 / 2.0, r.ymin as f32 + (r.size_y() + 1) as f32 / 2.0);
                keyframe_draw::draw_keyframe(buf, width, height, cx, height as f32 - cy, 0.5 * u as f32, false, KeyType::Keyframe, 1.0);
            }
            Look::Field { label, value } => {
                let text = value.to_string();
                let inner = Rcti { xmin: r.xmin + u / 2, xmax: r.xmax - u / 2, ..r };
                match label {
                    Some(l) => {
                        font::draw_label_pad(buf, width, height, user, inner, l, theme::WCOL_MENU_TEXT, 0);
                        let x = inner.xmax + 1 - font::string_width(user, &text);
                        font::draw_label_pad(buf, width, height, user, Rcti { xmin: x, ..inner }, &text, theme::WCOL_MENU_TEXT, 0);
                    }
                    None => {
                        let x = r.xmin + (r.size_x() + 1 - font::string_width(user, &text)) / 2;
                        font::draw_label_pad(buf, width, height, user, Rcti { xmin: x, ..r }, &text, theme::WCOL_MENU_TEXT, 0);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header() -> Rcti {
        Rcti { xmin: 2, xmax: 1577, ymin: 20, ymax: 45 }
    }

    #[test]
    fn items_stay_inside_and_in_order() {
        let user = UserDef::from_window_scale(1.0);
        let items = layout(&user, header(), 260, false, 1, (1, 250));
        assert_eq!(items.len(), 19);
        for w in items.windows(2) {
            assert!(w[1].rect.xmin > w[0].rect.xmax, "{:?} {:?}", w[0].look, w[1].look);
        }
        assert!(items[0].rect.xmin > 260);
        assert!(items.last().unwrap().rect.xmax <= header().xmax);
    }

    #[test]
    fn playing_swaps_reverse_and_play_for_a_double_width_pause() {
        let user = UserDef::from_window_scale(1.0);
        let idle = layout(&user, header(), 260, false, 1, (1, 250));
        let busy = layout(&user, header(), 260, true, 1, (1, 250));
        assert_eq!(idle.len(), busy.len() + 1);
        let pause = busy.iter().find(|i| i.look == Look::Glyph(Icon::Pause)).unwrap();
        assert_eq!(pause.rect.size_x() + 1, 2 * user.widget_unit);
        assert!(!idle.iter().any(|i| i.look == Look::Glyph(Icon::Pause)));
    }

    #[test]
    fn the_transport_group_is_centred_by_the_two_spacers() {
        let user = UserDef::from_window_scale(1.0);
        let items = layout(&user, header(), 260, false, 1, (1, 250));
        let rew = items.iter().find(|i| i.look == Look::Glyph(Icon::Rew)).unwrap();
        let frame = items.iter().find(|i| matches!(i.look, Look::Field { label: None, .. })).unwrap();
        assert!(rew.rect.xmin > 260 && rew.rect.xmax < frame.rect.xmin);
    }
}
