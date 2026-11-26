//! Port of the DPI / widget-unit maths in `wm_window.cc`
//! (`wm_window_set_dpi`, lines ~740-776). Defaults: ui_scale lazily
//! initialised to 1 (virtual_pixel native), ui_line_width = 0.

#[derive(Clone, Copy, Debug)]
pub struct UserDef {
    #[allow(dead_code)]
    pub dpi: f32,
    pub pixelsize: i32,
    pub scale_factor: f32,
    #[allow(dead_code)]
    pub inv_scale_factor: f32,
    /// `U.widget_unit` == UI_UNIT_X == UI_UNIT_Y (20 px at 1x).
    pub widget_unit: i32,
    /// `U.border_width` (DNA_userdef_types.h: `char border_width = 2;`).
    pub border_width: i32,
    /// `uiStyle::buttonspacex` (8) scaled like `ui::style_get_dpi()` (interface_style.cc:90, 434).
    pub button_space_x: i32,
    /// `uiStyle::buttonspacey` (2), scaled the same way (interface_style.cc:91, 435).
    #[allow(dead_code)]
    pub button_space_y: i32,
}

impl UserDef {
    /// `window_scale` is the OS scale factor (1.0 == 96 DPI).
    pub fn from_window_scale(window_scale: f64) -> Self {
        let ui_scale: f32 = 1.0;
        let ui_line_width: i32 = 0;

        // auto_dpi = max(auto_dpi, 96); native pixel size is 1 on Windows/Linux.
        let auto_dpi = (96.0 * window_scale as f32).max(96.0);
        let native_pixel_size = 1.0;
        let auto_dpi = auto_dpi * native_pixel_size;

        let dpi = auto_dpi * ui_scale * (72.0 / 96.0);

        let mut pixelsize = ((dpi / 64.0) as i32).max(1);
        pixelsize = (pixelsize + ui_line_width).max(1);

        let scale_factor = dpi / 72.0;
        let inv_scale_factor = 1.0 / scale_factor;

        // 18 user-scaled units plus left/right borders of line-width.
        let widget_unit = (18.0 * scale_factor).round() as i32 + 2 * pixelsize;

        /* `short(UI_SCALE_FAC * buttonspace)`. */
        let button_space_x = (scale_factor * 8.0) as i16 as i32;
        let button_space_y = (scale_factor * 2.0) as i16 as i32;

        Self { dpi, pixelsize, scale_factor, inv_scale_factor, widget_unit, border_width: 2, button_space_x, button_space_y }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_spacing_scales() {
        let u = UserDef::from_window_scale(1.0);
        assert_eq!((u.button_space_x, u.button_space_y), (8, 2));
        let u = UserDef::from_window_scale(2.0);
        assert_eq!((u.button_space_x, u.button_space_y), (16, 4));
    }
}
