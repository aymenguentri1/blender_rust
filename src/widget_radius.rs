//! Port of `widget_radius_from_zoom()` and `widget_radius_from_rcti()` from
//! `source/blender/editors/interface/interface_widgets.cc` (lines 3382-3390).
//! Blender does not clamp either result.

/// `widget_radius_from_zoom()`: `wcol->roundness * U.widget_unit * zoom`.
pub fn widget_radius_from_zoom(zoom: f32, roundness: f32, widget_unit: i32) -> f32 {
    roundness * widget_unit as f32 * zoom
}

/// `widget_radius_from_rcti()`: `wcol->roundness * BLI_rcti_size_y(rect)`.
pub fn widget_radius_from_rcti(ymin: i32, ymax: i32, roundness: f32) -> f32 {
    roundness * (ymax - ymin) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_radius_scales() {
        assert_eq!(widget_radius_from_zoom(1.0, 0.2, 20), 4.0);
        assert_eq!(widget_radius_from_zoom(2.0, 0.2, 20), 8.0);
        assert_eq!(widget_radius_from_zoom(1.0, 0.0, 20), 0.0);
    }

    #[test]
    fn rcti_radius_uses_height_only() {
        assert_eq!(widget_radius_from_rcti(10, 30, 0.2), 4.0);
        assert_eq!(widget_radius_from_rcti(0, 40, 0.5), 20.0);
        assert_eq!(widget_radius_from_rcti(5, 5, 0.5), 0.0);
    }
}
