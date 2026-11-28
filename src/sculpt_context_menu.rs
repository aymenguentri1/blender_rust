//! VIEW3D_PT_sculpt_context_menu: the sculpt context popover (right click in the viewport), as pure
//! layout rows chosen by the brush capabilities.
//!
//! Ported from `scripts/startup/bl_ui/space_view3d.py` (`class VIEW3D_PT_sculpt_context_menu`, `draw`).

/// The `brush.sculpt_capabilities` flags the popover reads.
#[derive(Clone, Copy, Default, Debug)]
pub struct Capabilities {
    pub has_color: bool,
    pub has_auto_smooth: bool,
    pub has_normal_weight: bool,
    pub has_pinch_factor: bool,
    pub has_rake_factor: bool,
    pub has_plane_offset: bool,
    pub has_height: bool,
}

/// Brush state the popover reads besides the capabilities.
#[derive(Clone, Copy, Debug)]
pub struct State {
    pub caps: Capabilities,
    pub brush_type: &'static str,
    /// `size_owner.use_locked_size == 'SCENE'`
    pub size_locked_to_scene: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Row {
    /// Color swatch (10% split) with the color picker.
    Color,
    /// `layout.prop(brush, "blend", text="")`
    Blend,
    /// A unified-paint property: value property, unified toggle, pressure toggle.
    Unified {
        prop: &'static str,
        label: &'static str,
        unified: &'static str,
        pressure: &'static str,
    },
    Slider {
        prop: &'static str,
        label: &'static str,
    },
}

pub fn rows(s: &State) -> Vec<Row> {
    let c = &s.caps;
    let mut r = Vec::new();
    if c.has_color {
        r.push(Row::Color);
        r.push(Row::Blend);
    }
    r.push(Row::Unified {
        prop: if s.size_locked_to_scene {
            "unprojected_size"
        } else {
            "size"
        },
        label: "Size",
        unified: "use_unified_size",
        pressure: "use_pressure_size",
    });
    r.push(Row::Unified {
        prop: "strength",
        label: "Strength",
        unified: "use_unified_strength",
        pressure: "use_pressure_strength",
    });
    let slider = |prop, label| Row::Slider { prop, label };
    if c.has_auto_smooth {
        r.push(slider("auto_smooth_factor", "Auto-Smooth"));
    }
    if c.has_normal_weight {
        r.push(slider("normal_weight", "Normal Weight"));
    }
    if c.has_pinch_factor {
        let text = if matches!(s.brush_type, "BLOB" | "SNAKE_HOOK") {
            "Magnify"
        } else {
            "Pinch"
        };
        r.push(slider("crease_pinch_factor", text));
    }
    if c.has_rake_factor {
        r.push(slider("rake_factor", "Rake"));
    }
    if c.has_plane_offset {
        r.push(slider("plane_offset", "Plane Offset"));
        r.push(slider("plane_trim", "Distance"));
    }
    if c.has_height {
        r.push(slider("height", "Height"));
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(caps: Capabilities, t: &'static str, scene: bool) -> State {
        State {
            caps,
            brush_type: t,
            size_locked_to_scene: scene,
        }
    }

    #[test]
    fn minimal_has_size_and_strength() {
        let r = rows(&st(Capabilities::default(), "DRAW", false));
        assert_eq!(r.len(), 2);
        assert!(matches!(
            &r[0],
            Row::Unified {
                prop: "size",
                unified: "use_unified_size",
                pressure: "use_pressure_size",
                ..
            }
        ));
        assert!(matches!(
            &r[1],
            Row::Unified {
                prop: "strength",
                ..
            }
        ));
    }

    #[test]
    fn scene_lock_uses_unprojected_size() {
        let r = rows(&st(Capabilities::default(), "DRAW", true));
        assert!(matches!(
            &r[0],
            Row::Unified {
                prop: "unprojected_size",
                ..
            }
        ));
    }

    #[test]
    fn color_rows_come_first() {
        let r = rows(&st(
            Capabilities {
                has_color: true,
                ..Default::default()
            },
            "PAINT",
            false,
        ));
        assert_eq!(&r[..2], [Row::Color, Row::Blend]);
        assert_eq!(r.len(), 4);
    }

    #[test]
    fn pinch_is_magnify_for_blob_and_snake_hook() {
        let caps = Capabilities {
            has_pinch_factor: true,
            ..Default::default()
        };
        for (t, want) in [
            ("BLOB", "Magnify"),
            ("SNAKE_HOOK", "Magnify"),
            ("CREASE", "Pinch"),
        ] {
            let r = rows(&st(caps, t, false));
            assert_eq!(
                r[2],
                Row::Slider {
                    prop: "crease_pinch_factor",
                    label: want
                }
            );
        }
    }

    #[test]
    fn all_caps_order() {
        let caps = Capabilities {
            has_color: true,
            has_auto_smooth: true,
            has_normal_weight: true,
            has_pinch_factor: true,
            has_rake_factor: true,
            has_plane_offset: true,
            has_height: true,
        };
        let r = rows(&st(caps, "DRAW", false));
        let props: Vec<_> = r
            .iter()
            .map(|x| match x {
                Row::Slider { prop, .. } | Row::Unified { prop, .. } => *prop,
                Row::Color => "color",
                Row::Blend => "blend",
            })
            .collect();
        assert_eq!(
            props,
            [
                "color",
                "blend",
                "size",
                "strength",
                "auto_smooth_factor",
                "normal_weight",
                "crease_pinch_factor",
                "rake_factor",
                "plane_offset",
                "plane_trim",
                "height"
            ]
        );
    }
}
