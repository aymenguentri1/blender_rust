//! What the main region of the properties editor shows: `SpaceProperties.context` (`sbuts->mainb`) picks the tab,
//! `buttons_context_compute()` falls back to an available one (`buttons_context::effective_tab`), and only the panels
//! whose `bl_context` matches are drawn (`ED_region_panels_layout_ex` with the context string of
//! `buttons_main_region_context_string`, space_buttons.cc). Object tab: `properties_object.py`; Modifiers tab:
//! `DATA_PT_modifiers` (`bl_context = "modifier"`, `HIDE_HEADER`, properties_data_modifier.py:55).
//! Tabs whose panels are not ported yet show only the context path row.

use crate::area::Rcti;
use crate::buttons_context::{effective_tab, path_flags, Context};
use crate::object_tab_context::ContextRows;
use crate::scene::Scene;

/// The panel set of the tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Content {
    /// `OBJECT_PT_context_object` and the Object panels.
    Object,
    /// `DATA_PT_modifiers`: the modifier stack of the active object.
    Modifiers,
    /// Not ported yet.
    Empty,
}

/// The tab drawn for the user's choice `chosen` (`None`: the startup file's Object tab).
pub fn tab(scene: &Scene, chosen: Option<Context>) -> Context {
    effective_tab(path_flags(scene), chosen.unwrap_or(Context::Object))
}

pub fn content(tab: Context) -> Content {
    match tab {
        Context::Object => Content::Object,
        Context::Modifier => Content::Modifiers,
        _ => Content::Empty,
    }
}

/// Where the panels of a tab without the ID row start: the Modifiers panel has no header and comes right under the
/// path row. Deviation: the gap is the one between the path row and the Object tab's ID row (not measured on the
/// Modifiers tab itself).
pub fn panels_below_path(window: Rcti, rows: Option<&ContextRows>) -> Rcti {
    Rcti { ymax: rows.map_or(window.ymax, |r| r.browse.ymax), ..window }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::userdef::UserDef;

    #[test]
    fn object_and_modifier_tabs_have_their_panels() {
        assert_eq!(content(Context::Object), Content::Object);
        assert_eq!(content(Context::Modifier), Content::Modifiers);
        assert_eq!(content(Context::Render), Content::Empty);
    }

    #[test]
    fn the_chosen_tab_is_kept_when_available_else_object() {
        let mut s = Scene::startup();
        let cube = s.objects.iter().find(|o| o.name == "Cube").map(|o| o.id);
        s.active = cube;
        assert_eq!(tab(&s, None), Context::Object);
        assert_eq!(tab(&s, Some(Context::Modifier)), Context::Modifier);
        assert_eq!(tab(&s, Some(Context::World)), Context::World);
        /* The camera has no modifiers: buttons_context_compute falls back to Object. */
        s.active = s.objects.iter().find(|o| o.name == "Camera").map(|o| o.id);
        assert_eq!(tab(&s, Some(Context::Modifier)), Context::Object);
    }

    #[test]
    fn modifier_panels_start_under_the_path_row() {
        let user = UserDef::from_window_scale(1.0);
        let window = Rcti { xmin: 28, xmax: 337, ymin: 0, ymax: 757 };
        let rows = crate::object_tab_context::layout(&user, window).unwrap();
        let r = panels_below_path(window, Some(&rows));
        assert!(r.ymax < rows.path_icon.ymin && r.ymax > rows.panels_top);
        assert_eq!((r.xmin, r.xmax, r.ymin), (window.xmin, window.xmax, window.ymin));
        assert_eq!(panels_below_path(window, None), window);
    }
}
