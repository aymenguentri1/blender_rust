//! The "Frames" keymap, ported item by item (same order) from `km_frames()` in `blender_default.py`.
//! `Params.spacebar_action` defaults to `'TOOL'`, `legacy` to false.

use crate::keymap::{Keymap, KeyMapItem as K, Prop};

/// `params.spacebar_action`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpacebarAction {
    Play,
    Tool,
    Search,
}

fn offset(kind: &'static str, delta: i64) -> K {
    K::new("screen.frame_offset", kind, "PRESS").prop("delta", Prop::Int(delta))
}

fn jump(kind: &'static str, end: bool) -> K {
    K::new("screen.frame_jump", kind, "PRESS").prop("end", Prop::Bool(end))
}

fn keyframe_jump(kind: &'static str, next: bool) -> K {
    K::new("screen.keyframe_jump", kind, "PRESS").prop("next", Prop::Bool(next))
}

/// `km_frames(params)`.
pub fn km_frames(spacebar_action: SpacebarAction, legacy: bool) -> Keymap {
    let mut items = vec![
        // Frame offsets
        offset("LEFT_ARROW", -1).repeat(),
        offset("RIGHT_ARROW", 1).repeat(),
        jump("RIGHT_ARROW", true).shift().repeat(),
        jump("LEFT_ARROW", false).shift().repeat(),
        K::new("screen.time_jump", "RIGHT_ARROW", "PRESS").ctrl().repeat().prop("backward", Prop::Bool(false)),
        K::new("screen.time_jump", "LEFT_ARROW", "PRESS").ctrl().repeat().prop("backward", Prop::Bool(true)),
        keyframe_jump("UP_ARROW", false).repeat(),
        keyframe_jump("DOWN_ARROW", true).repeat(),
        keyframe_jump("MEDIA_LAST", true),
        keyframe_jump("MEDIA_FIRST", false),
        offset("WHEELDOWNMOUSE", 1).alt(),
        offset("WHEELUPMOUSE", -1).alt(),
    ];

    if !legacy {
        // New playback
        match spacebar_action {
            SpacebarAction::Tool | SpacebarAction::Search => items.push(K::new("screen.animation_play", "SPACE", "PRESS").shift()),
            SpacebarAction::Play => items.push(K::new("screen.animation_play", "SPACE", "PRESS")),
        }
        items.push(K::new("screen.animation_play", "SPACE", "PRESS").shift().ctrl().prop("reverse", Prop::Bool(true)));
    } else {
        // Old playback
        items.extend([
            offset("UP_ARROW", 10).shift().repeat(),
            offset("DOWN_ARROW", -10).shift().repeat(),
            jump("UP_ARROW", true).shift().ctrl().repeat(),
            jump("DOWN_ARROW", false).shift().ctrl().repeat(),
            K::new("screen.animation_play", "A", "PRESS").alt(),
            K::new("screen.animation_play", "A", "PRESS").shift().alt().prop("reverse", Prop::Bool(true)),
        ]);
    }

    items.extend([
        K::new("screen.animation_cancel", "ESC", "PRESS"),
        K::new("screen.animation_play", "MEDIA_PLAY", "PRESS"),
        K::new("screen.animation_cancel", "MEDIA_STOP", "PRESS"),
    ]);
    Keymap::new("Frames", items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::Event;

    fn hit(km: &Keymap, kind: &'static str, shift: bool, ctrl: bool, alt: bool) -> Option<(&'static str, Vec<(&'static str, Prop)>)> {
        let mut e = Event::new(kind, "PRESS");
        (e.shift, e.ctrl, e.alt) = (shift, ctrl, alt);
        km.match_item(&e).map(|k| (k.idname, k.properties.clone()))
    }

    #[test]
    fn arrows_and_jumps() {
        let km = km_frames(SpacebarAction::Tool, false);
        assert_eq!(hit(&km, "LEFT_ARROW", false, false, false), Some(("screen.frame_offset", vec![("delta", Prop::Int(-1))])));
        assert_eq!(hit(&km, "RIGHT_ARROW", true, false, false), Some(("screen.frame_jump", vec![("end", Prop::Bool(true))])));
        assert_eq!(hit(&km, "LEFT_ARROW", true, false, false), Some(("screen.frame_jump", vec![("end", Prop::Bool(false))])));
        assert_eq!(hit(&km, "UP_ARROW", false, false, false), Some(("screen.keyframe_jump", vec![("next", Prop::Bool(false))])));
        assert_eq!(hit(&km, "DOWN_ARROW", false, false, false), Some(("screen.keyframe_jump", vec![("next", Prop::Bool(true))])));
        assert_eq!(hit(&km, "LEFT_ARROW", false, true, false), Some(("screen.time_jump", vec![("backward", Prop::Bool(true))])));
        assert_eq!(hit(&km, "WHEELUPMOUSE", false, false, true), Some(("screen.frame_offset", vec![("delta", Prop::Int(-1))])));
        assert_eq!(hit(&km, "ESC", false, false, false).unwrap().0, "screen.animation_cancel");
    }

    #[test]
    fn spacebar_action_decides_play_key() {
        let play = km_frames(SpacebarAction::Play, false);
        assert_eq!(hit(&play, "SPACE", false, false, false), Some(("screen.animation_play", vec![])));
        assert_eq!(hit(&play, "SPACE", true, false, false), None);
        for a in [SpacebarAction::Tool, SpacebarAction::Search] {
            let km = km_frames(a, false);
            assert_eq!(hit(&km, "SPACE", false, false, false), None);
            assert_eq!(hit(&km, "SPACE", true, false, false), Some(("screen.animation_play", vec![])));
        }
        let rev = hit(&play, "SPACE", true, true, false);
        assert_eq!(rev, Some(("screen.animation_play", vec![("reverse", Prop::Bool(true))])));
    }

    #[test]
    fn legacy_playback_and_counts() {
        let legacy = km_frames(SpacebarAction::Tool, true);
        assert_eq!(hit(&legacy, "UP_ARROW", true, false, false), Some(("screen.frame_offset", vec![("delta", Prop::Int(10))])));
        assert_eq!(hit(&legacy, "A", false, false, true), Some(("screen.animation_play", vec![])));
        assert_eq!(hit(&legacy, "SPACE", true, false, false), None);
        assert_eq!(km_frames(SpacebarAction::Tool, false).items.len(), 12 + 2 + 3);
        assert_eq!(legacy.items.len(), 12 + 6 + 3);
        assert_eq!(legacy.name, "Frames");
    }
}
