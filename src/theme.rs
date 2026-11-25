//! Port of `release/datafiles/userdef/userdef_default_theme.c`.
//! Only the values needed so far are ported; more are added as the UI grows.
//! `RGBA(0xRRGGBBAA)` in the C file maps to `Rgba::hex` here.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    /// Same layout as the `RGBA(0xRRGGBBAA)` macro in the theme file.
    pub const fn hex(v: u32) -> Self {
        Self {
            r: (v >> 24) as u8,
            g: (v >> 16) as u8,
            b: (v >> 8) as u8,
            a: v as u8,
        }
    }

    /// Packed 0x00RRGGBB, the pixel format softbuffer expects.
    pub const fn to_xrgb(self) -> u32 {
        ((self.r as u32) << 16) | ((self.g as u32) << 8) | self.b as u32
    }
}

/// `.tui.editor_border` (userdef_default_theme.c:255).
/// This is the colour drawn between areas (screen_draw.cc uses TH_EDITOR_BORDER),
/// so it is what the window shows wherever no area covers it.
pub const EDITOR_BORDER: Rgba = Rgba::hex(0x161616ff);


/// `.tui.editor_outline` (userdef_default_theme.c:256).
/// Outline drawn by `ED_screen_draw_edges()` around every area (TH_EDITOR_OUTLINE).
pub const EDITOR_OUTLINE: Rgba = Rgba::hex(0xffffff15);


/// `.tui.editor_outline_active` (userdef_default_theme.c:257).
/// Outline drawn around the active area (TH_EDITOR_OUTLINE_ACTIVE).
pub const EDITOR_OUTLINE_ACTIVE: Rgba = Rgba::hex(0xffffff2a);


/// `.space_view3d.back` (userdef_default_theme.c:366).
pub const SPACE_VIEW3D_BACK: Rgba = Rgba::hex(0x3d3d3dff);


/// `.space_view3d.header` (userdef_default_theme.c:370).
/// Alpha is not 255, so `ED_region_is_overlap()` treats the 3D viewport header as an
/// overlapping region: it is cleared to the premultiplied colour and blended over the window region.
pub const SPACE_VIEW3D_HEADER: Rgba = Rgba::hex(0x303030b3);


/// `.space_view3d.grid` (userdef_default_theme.c:371), `TH_GRID` in the 3D viewport.
pub const SPACE_VIEW3D_GRID: Rgba = Rgba::hex(0x54545480);

/// `.space_view3d.grid_major` (userdef_default_theme.c:372), `TH_GRID_MAJOR`.
pub const SPACE_VIEW3D_GRID_MAJOR: Rgba = Rgba::hex(0x545454ff);

/// `.space_view3d.grid_axis_brightness` (userdef_default_theme.c:373).
pub const SPACE_VIEW3D_GRID_AXIS_BRIGHTNESS: f32 = 0.46;


/// `.space_view3d.wire` (userdef_default_theme.c:374), `TH_WIRE`: unselected object wire colour.
pub const SPACE_VIEW3D_WIRE: Rgba = Rgba::hex(0x000000ff);

/// `.space_view3d.select` (userdef_default_theme.c:376), `TH_SELECT`: selected object outline and wire colour.
pub const SPACE_VIEW3D_SELECT: Rgba = Rgba::hex(0xed5700ff);

/// `.space_view3d.active` (userdef_default_theme.c:381), `TH_ACTIVE`: active object outline and wire colour.
pub const SPACE_VIEW3D_ACTIVE: Rgba = Rgba::hex(0xffa028ff);

/// `.tui.xaxis`, `.tui.yaxis`, `.tui.zaxis` (userdef_default_theme.c:265-267): `TH_AXIS_X/Y/Z`.
pub const TUI_XAXIS: Rgba = Rgba::hex(0xff3352ff);
pub const TUI_YAXIS: Rgba = Rgba::hex(0x8bdc00ff);
pub const TUI_ZAXIS: Rgba = Rgba::hex(0x2890ffff);


/// `.space_topbar.header` (userdef_default_theme.c:714).
/// Header regions are cleared with `ui::theme::frame_buffer_clear(TH_HEADER)` (resources.cc:1549),
/// which uses the RGB of the colour with alpha forced to 1.0.
pub const SPACE_TOPBAR_HEADER: Rgba = Rgba::hex(0x181818b3);

/// `.space_statusbar.header` (userdef_default_theme.c:724).
pub const SPACE_STATUSBAR_HEADER: Rgba = Rgba::hex(0x181818b3);

/// `.space_statusbar.text` (userdef_default_theme.c:722), `TH_TEXT` of the status bar.
pub const SPACE_STATUSBAR_TEXT: Rgba = Rgba::hex(0x838383ff);

/// `.wcol_tool` (userdef_default_theme.c:32-40): `outline`, `inner` and `roundness`.
pub const WCOL_TOOL_OUTLINE: Rgba = Rgba::hex(0x3d3d3dff);
pub const WCOL_TOOL_INNER: Rgba = Rgba::hex(0x545454ff);
pub const WCOL_TOOL_ROUNDNESS: f32 = 0.2;

/// `.wcol_toggle.inner_sel` (userdef_default_theme.c:82-90); the other toggle values equal `wcol_tool`'s.
pub const WCOL_TOGGLE_INNER_SEL: Rgba = Rgba::hex(0x4772b3ff);

/// `.space_outliner.back` (userdef_default_theme.c:606), `.space_properties.back` (:356) and
/// `.space_action.back` (:477), the timeline: window region clear colours (alpha is ignored).
pub const SPACE_OUTLINER_BACK: Rgba = Rgba::hex(0x28282800);
/// `.space_outliner.row_alternate` (userdef_default_theme.c:618), `TH_ROW_ALTERNATE`.
pub const SPACE_OUTLINER_ROW_ALTERNATE: Rgba = Rgba::hex(0xffffff04);
/// `.space_outliner.text`, `.text_hi` and `.active_object` (userdef_default_theme.c): `TH_TEXT`, `TH_TEXT_HI`
/// and `TH_ACTIVE_OBJECT` of the outliner's window region (`resources.cc` takes `ts->text`, `ts->text_hi` and
/// `ts->active_object` of the space).
pub const SPACE_OUTLINER_TEXT: Rgba = Rgba::hex(0xc3c3c3ff);
pub const SPACE_OUTLINER_TEXT_HI: Rgba = Rgba::hex(0xffffffff);
pub const SPACE_OUTLINER_ACTIVE_OBJECT: Rgba = Rgba::hex(0xffaf29ff);
/// `.space_outliner.selected_object` (userdef_default_theme.c:616), `TH_SELECTED_OBJECT`: a selected object that
/// is not the active one.
pub const SPACE_OUTLINER_SELECTED_OBJECT: Rgba = Rgba::hex(0xe96a00ff);
pub const SPACE_PROPERTIES_BACK: Rgba = Rgba::hex(0x30303000);
pub const SPACE_ACTION_BACK: Rgba = Rgba::hex(0x30303000);

/// `.space_outliner.header` (userdef_default_theme.c:609), `.space_properties.header` (:359) and
/// `.space_action.header` (:480): `TH_HEADER` of a non-overlapping header region, which is cleared
/// with the RGB of the colour and alpha forced to 1.0.
pub const SPACE_OUTLINER_HEADER: Rgba = Rgba::hex(0x282828b3);
pub const SPACE_PROPERTIES_HEADER: Rgba = Rgba::hex(0x303030b3);
pub const SPACE_ACTION_HEADER: Rgba = Rgba::hex(0x303030b3);

/// `.space_action.text_hi`, `TH_TEXT_HI` of the timeline's window region (`resources.cc` takes `ts->text_hi`
/// of the space for every region but the channels): the playhead frame number (`.space_action.text_hi` of
/// `userdef_default_theme.c`).
pub const SPACE_ACTION_TEXT_HI: Rgba = Rgba::hex(0xffffffff);

/// `.regions.scrubbing.back` (userdef_default_theme.c:303), `TH_TIME_SCRUB_BACKGROUND`.
pub const SCRUBBING_BACK: Rgba = Rgba::hex(0x1d1d1dff);

/// `.regions.scrubbing.text` (userdef_default_theme.c:304), `TH_TIME_SCRUB_TEXT`: the frame numbers of the
/// time scrub strip.
pub const SCRUBBING_TEXT: Rgba = Rgba::hex(0x808080ff);

/// `.regions.sidebars.tab_back` (userdef_default_theme.c:310): `TH_BACK` of an `RGN_TYPE_NAV_BAR` region
/// (resources.cc:195).
pub const SIDEBARS_TAB_BACK: Rgba = Rgba::hex(0x18181800);


/// `.common.anim.playhead` (userdef_default_theme.c:315), `TH_CFRAME`: the current frame marker.
pub const ANIM_PLAYHEAD: Rgba = Rgba::hex(0x4772b3ff);


/// `.wcol_pulldown.text` (userdef_default_theme.c:148): text of the header pulldown menus.
pub const WCOL_PULLDOWN_TEXT: Rgba = Rgba::hex(0xd9d9d9ff);


/// `.wcol_tab` (userdef_default_theme.c:112-121): the workspace tabs of the top bar
/// (`template_ID_tabs()`): `inner` / `text` of an inactive tab, `inner_sel` / `text_sel` of the active one.
pub const WCOL_TAB_INNER: Rgba = Rgba::hex(0x1d1d1dff);
pub const WCOL_TAB_INNER_SEL: Rgba = Rgba::hex(0x303030ff);
pub const WCOL_TAB_OUTLINE: Rgba = Rgba::hex(0x1d1d1dff);
pub const WCOL_TAB_TEXT: Rgba = Rgba::hex(0x989898ff);
pub const WCOL_TAB_TEXT_SEL: Rgba = Rgba::hex(0xffffffff);
pub const WCOL_TAB_ROUNDNESS: f32 = 0.2;

/// `.wcol_menu` (userdef_default_theme.c:132-141): the ID browse / search fields of the top bar.
pub const WCOL_MENU_OUTLINE: Rgba = Rgba::hex(0x3d3d3dff);
pub const WCOL_MENU_INNER: Rgba = Rgba::hex(0x282828ff);
pub const WCOL_MENU_TEXT: Rgba = Rgba::hex(0xe6e6e6ff);
pub const WCOL_MENU_ROUNDNESS: f32 = 0.2;

/// `.space_view3d.text_hi` (userdef_default_theme.c:369), `TH_TEXT_HI`: the viewport info text colour
/// (`ED_view3d_text_colors_get()` keeps it because it is far from the background).
pub const SPACE_VIEW3D_TEXT_HI: Rgba = Rgba::hex(0xffffffff);
