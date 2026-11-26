//! Step 1: the main window.
//!
//! Ported from `source/blender/windowmanager/intern/wm_window.cc`:
//!  - title:  `wm_window_title_text()`  -> "(Unsaved) — Blender <version>"
//!  - size:   `wm_init_state.size` = screen size, `start` = (0, 0)
//!            (used when there is no startup.blend window geometry)
//!  - paint:  window content is the editor-border colour (screen_draw.cc)
//!
//! No areas / widgets / viewport yet.

#[allow(dead_code)]
mod area;
#[allow(dead_code)]
mod blend_header;
mod screen;
mod object_mode;
mod sculpt;
mod sculpt_header;
mod screen_draw;
#[allow(dead_code)]
mod ui_layout;
mod widget_draw;
mod font;
mod frame_range;
mod buttons_context;
mod object_tab_layout;
mod outliner_draw;
mod face_set;
mod blend_sdna;
mod gltf_accessor;
mod gltf_prim_mode;
mod face_set_create;
mod obj_export;
#[allow(dead_code)]
mod keyframe_select;
#[allow(dead_code)]
mod multires;
#[allow(dead_code)]
mod obj_import;
mod gltf_mesh;
#[allow(dead_code)]
mod multires_reshape;
mod array_settings;
mod solidify_settings;
mod outliner_select;
mod scene_units;
mod outliner_rename;
mod playhead_draw;
#[allow(dead_code)]
mod view2d_grid;

mod status_input;
mod colorspace_lut;
mod theme;
mod userdef;
mod version;
#[allow(dead_code)]
mod transform_falloff;
#[allow(dead_code)]
mod math_base;
#[allow(dead_code)]
mod math_base_safe;
#[allow(dead_code)]
mod math_bits;
#[allow(dead_code)]
mod math_color;
#[allow(dead_code)]
mod math_half;
#[allow(dead_code)]
mod math_solvers;
#[allow(dead_code)]
mod fcurve_eval;
mod action;
#[allow(dead_code)]
mod math_time;
#[allow(dead_code)]
mod view3d;
#[allow(dead_code)]
mod overlay_grid;
mod overlay_grid_draw;
mod overlay_cursor;
mod overlay_text;
mod overlay_wireframe;
mod workbench;
#[allow(dead_code)]
mod taa;
#[allow(dead_code)]
mod material_tab_layout;
mod mesh;
#[allow(dead_code)]
mod mesh_bevel_edge;
mod mesh_bevel_meet;
mod mesh_extrude;
#[allow(dead_code)]
mod edit_select;
#[allow(dead_code)]
mod edit_overlay;
mod mesh_extrude_faces;
mod sculpt_hide;
mod mesh_edgering;
mod mesh_loopcut;
mod mesh_ops;
mod mesh_deform;
mod mesh_wire;
mod mesh_reduce;
mod mesh_hull;
mod mesh_gen;
mod mesh_build;
mod mesh_boolean;
mod mesh_corrective;
mod modifiers;
#[allow(dead_code)]
mod scene;
#[allow(dead_code)]
mod transform;
mod overlay_outline;
mod overlay_extra;
mod add_menu;
mod brush_menu;
mod add_modifier_menu;
mod icons;
mod view3d_header;
mod modifier_menu;
mod modifier_panel;
mod transform_panel;
mod select_box;
mod rename_popup;
mod gltf_node;
mod collection;
mod mirror_settings;
mod bevel_settings;
mod material;

use std::{num::NonZeroU32, sync::Arc};

use winit::{
    application::ApplicationHandler,
    dpi::{PhysicalPosition, PhysicalSize},
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

use userdef::UserDef;

/// `wm_window_title_text()` for a main window with no file path.
/// (`wm->file_saved` is true on a fresh startup file, so no "* " prefix;
/// non-macOS uses "(Unsaved)", macOS uses "Untitled".)
fn window_title() -> String {
    let name = if cfg!(target_os = "macos") { "Untitled" } else { "(Unsaved)" };
    format!("{} — Blender {}", name, version::version_string())
}

/// `ED_view3d_smooth_view_force_finish()`: the running smooth view jumps to its destination.
fn smooth_finish(rv3d: &mut view3d::RegionView3D, smooth: &mut Option<(view3d::SmoothView, std::time::Instant)>) {
    if let Some((sms, _)) = smooth.take() {
        rv3d.smooth_view_finish(&sms);
    }
}


struct Gfx {
    window: Arc<Window>,
    surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
}

/// The running `transform.translate` of the 3D cursor (`cursor_transform` keymap property) started by
/// Shift + RIGHTMOUSE `CLICK_DRAG`: `start` is the press position (`T_EVENT_DRAG_START`, the mouse
/// input origin), `loc` the cursor location when it began (`TransData::iloc`), `zfac` the
/// `ED_view3d_calc_zfac()` of that location (`calculateZfac()`).
struct CursorDrag {
    start: [i32; 2],
    loc: [f32; 3],
    zfac: f32,
    active: bool,
    /// `MouseInput::virtual_mval`: the movement already consumed and the accumulated (scaled) movement.
    prev: [f64; 2],
    accum: [f64; 2],
    /// `MouseInput::precision` (`TFM_MODAL_PRECISION`).
    precision: bool,
}


/// The running `WM_gesture_box_invoke()` of `view3d.zoom_border` (Shift + B): waits for the LEFTMOUSE
/// press that sets `start` (region pixels, y up); the release confirms and runs the operator.
struct ZoomBorder {
    start: Option<[i32; 2]>,
    /// The box runs `view3d.clip_border` (Alt + B) instead of `view3d.zoom_border`.
    clip: bool,
}


/// The running `view3d.select_box`: armed by B (waits for the press, `armed`) or started by a press on empty
/// space of the viewport (the Select Box tool). `start` and `cur` are region pixels, y up; the box only shows and
/// acts once the cursor has moved `select_box::DRAG_THRESHOLD` pixels (`dragging`), otherwise the release is the
/// click on nothing that deselects.
struct BoxSelect {
    armed: bool,
    start: Option<[i32; 2]>,
    cur: [i32; 2],
    dragging: bool,
    mode: select_box::BoxMode,
}

/// `Scene::r.frs / frs_sec_base` of the startup scene (24 fps).
const PLAY_FPS: f64 = 24.0;

/// The running `screen.animation_play`: direction, the frame to restore on cancel (`screen.animation_cancel`)
/// and the time the next frame is due.
struct PlayState {
    reverse: bool,
    start_frame: i32,
    next: std::time::Instant,
}


#[derive(Default)]
struct App {
    gfx: Option<Gfx>,
    user: Option<UserDef>,
    layout: Option<screen::ScreenLayout>,
    /// The active editor area (`screen->active_region` is inside it).
    active_area: Option<screen::AreaKind>,
    /// Cursor position in Blender window coordinates (y up).
    cursor: [i32; 2],
    v3d: view3d::View3D,
    rv3d: view3d::RegionView3D,
    modifiers: winit::keyboard::ModifiersState,
    /// Cursor position at the previous event while a `view3d.rotate` drag is running.
    orbit_prev: Option<[i32; 2]>,
    /// `ViewOpsData::reverse` of the running drag.
    orbit_reverse: f32,
    /// Cursor position at the previous event while a `view3d.move` drag is running.
    pan_prev: Option<[i32; 2]>,
    /// `ViewOpsData::init.zfac` of the running pan drag.
    pan_zfac: f32,
    /// Cursor y and view distance at the start of a running `view3d.zoom` drag.
    zoom_init: Option<(i32, f32)>,
    /// Cursor y and view offset at the start of a running `view3d.dolly` drag.
    dolly_init: Option<(i32, [f32; 3])>,
    /// `ViewOpsData::init` (`viewquat`, `dist`, `ofs`) of the running view drag, for `state_restore()`.
    nav_init: Option<([f32; 4], f32, [f32; 3], bool, view3d::View, usize)>,
    /// `ViewOpsData::curr.viewquat` of the running `view3d.rotate` drag: the unsnapped rotation.
    orbit_curr: [f32; 4],
    /// `ViewOpsData::axis_snap` (Alt held during the drag).
    orbit_snap: bool,
    /// `ViewOpsData::init.persp_with_auto_persp_applied` as an `ortho` flag.
    orbit_ortho: bool,
    /// `View3DCursor::location` of the scene (the origin in the default scene).
    cursor3d: [f32; 3],
    /// `View3DCursor::rotation()` as a quaternion (identity in the default scene).
    cursor3d_quat: [f32; 4],
    /// `RegionView3D::runtime->sms` and the start time of its timer.
    smooth: Option<(view3d::SmoothView, std::time::Instant)>,
    /// Cursor position of an Alt + MIDDLEMOUSE press that may still become a `CLICK_DRAG`.
    alt_drag_start: Option<[i32; 2]>,
    /// The Shift + RIGHTMOUSE press that places the 3D cursor and may still become a drag.
    cursor_drag: Option<CursorDrag>,
    /// The running `view3d.zoom_border` box gesture.
    zoom_border: Option<ZoomBorder>,
    /// The running `view3d.select_box` gesture.
    box_select: Option<BoxSelect>,
    /// The F2 rename popup.
    rename: Option<rename_popup::Rename>,
    /// The outliner's rename text field (`TSE_TEXTBUT`).
    outliner_rename: Option<outliner_rename::Rename>,
    /// The last click on an outliner object row, for the double-click that renames it.
    last_outliner_click: Option<(std::time::Instant, u32)>,
    /// The modifier value field being typed into.
    mod_edit: Option<modifier_panel::ValueEdit>,
    /// The open modifier dropdown menu: the modifier and the window position it opened at.
    mod_menu: Option<(usize, [i32; 2])>,
    /// The Add Modifier popup: the position of its top left corner and the category whose submenu is open.
    add_mod_menu: Option<([i32; 2], Option<usize>)>,
    /// A modifier being dragged by its handle: its index and the header tops of the panel when the press happened.
    mod_drag: Option<(usize, Vec<(usize, i32)>)>,
    /// The last click on a modifier panel button, for the double-click on a name (rename).
    last_panel_click: Option<(std::time::Instant, modifier_panel::Action)>,
    /// The Ctrl + C buffer of `view3d.copybuffer`.
    clipboard: scene::Clipboard,
    /// Depth buffer of the main region from the last redraw (`rw * rh`, 1.0 where nothing is drawn).
    scene_depth: Vec<f32>,
    /// `View3D::shading.flag & V3D_SHADING_XRAY` of the solid shading.
    xray: bool,
    /// `View3D::shading.type == OB_WIRE` (toggled with Shift + Z, `view3d.toggle_shading`).
    wire: bool,
    /// `View3D::flag2 & V3D_HIDE_OVERLAYS` (`wm.context_toggle` of `space_data.overlay.show_overlays`, Shift + Alt + Z).
    overlays_hidden: bool,
    /// The statistics overlay (Shift + Alt + O): object, vertex, face and triangle counts in the info text.
    show_stats: bool,
    /// The transform pivot point (Period cycles it: Median Point, 3D Cursor, Individual Origins).
    pivot_point: transform::PivotPoint,
    /// The 3D viewport header button under the cursor (drawn lighter).
    header_hover: Option<view3d_header::Action>,
    /// The button of the sculpt tool header under the cursor, and the slider being dragged.
    sculpt_hover: Option<sculpt_header::Action>,
    sculpt_drag: Option<sculpt_header::Action>,
    /// Workbench temporal anti-aliasing accumulation of the main region.
    taa: taa::TaaState,
    /// `Scene::r.cfra`, the current frame; `None` is the startup frame 1.
    frame: Option<i32>,
    /// A LEFTMOUSE drag in the timeline's time scrub strip is running (the frame follows the cursor).
    scrub_drag: bool,
    /// The frame when the scrub drag started, restored when it is cancelled.
    scrub_init: i32,
    /// The running animation playback.
    playing: Option<PlayState>,
    /// The cursor is over the header region of an editor area (changes the input status of the status bar).
    status_header: bool,
    /// The objects of the scene (the startup scene until something is added or deleted).
    scene: scene::Scene,
    /// Object id buffer of the main region from the last redraw (`Object::id + 1`, 0 where nothing is drawn).
    scene_ids: Vec<u32>,
    /// The running modal transform (G / R / S).
    xform: Option<transform::Transform>,
    /// The open Shift + A popup menu: the window position (y up) it was opened at.
    add_menu: Option<[i32; 2]>,
    /// The brush popup of the sculpt tool header, opened at this window position.
    brush_menu: Option<[i32; 2]>,
    /// The Add Modifier list of the properties editor is open.
    mod_add_open: bool,
    /// The field of the Transform panel that is being typed into.
    field_edit: Option<transform_panel::Edit>,
    /// Sculpt Mode of the active object (`Some` while the mode is on).
    sculpt: Option<sculpt::Session>,
    /// Edit Mode (Tab); only the mode flag exists so far.
    edit_mode: bool,
}

/// After a change of geometry, visibility or transforms: builds the modifier results and restarts the TAA
/// accumulation (selection changes only need a redraw).
fn scene_changed(scene: &mut scene::Scene, taa: &mut taa::TaaState) {
    scene.update_eval();
    *taa = taa::TaaState::default();
}

/// Sets the window title to the modal operator's status text (the viewport header text of Blender), or back to
/// the default one when there is none.
fn set_status_title(window: &Window, text: Option<&str>) {
    match text {
        Some(t) => window.set_title(&format!("{t} - {}", window_title())),
        None => window.set_title(&window_title()),
    }
}

/// What the 3D viewport header shows.
/// The mode the header shows: Edit and Sculpt are exclusive (`mode_set`).
fn mode_of(edit: bool, sculpt: bool) -> object_mode::ObjectMode {
    if edit { object_mode::ObjectMode::Edit } else if sculpt { object_mode::ObjectMode::Sculpt } else { object_mode::ObjectMode::Object }
}

fn header_state(wire: bool, xray: bool, overlays_hidden: bool, pivot: transform::PivotPoint, mode: object_mode::ObjectMode) -> view3d_header::HeaderState {
    view3d_header::HeaderState { wire, xray, overlays: !overlays_hidden, pivot, mode }
}

/// `object.mode_set` between Object Mode and Sculpt Mode (Ctrl + Tab, the header mode dropdown): Sculpt Mode needs an
/// active mesh object.
fn toggle_sculpt(sculpt: &mut Option<sculpt::Session>, scene: &scene::Scene) {
    if sculpt.is_some() {
        *sculpt = None;
    } else if scene.active_object().is_some_and(|o| o.mesh().is_some()) {
        *sculpt = Some(sculpt::Session::new());
    }
}

/// Same as `view3d.toggle_shading` (Shift + Z): entering wire drops the solid depth buffer,
/// leaving it restarts the TAA accumulation.
fn set_wire(wire: &mut bool, scene_depth: &mut Vec<f32>, taa: &mut taa::TaaState, to: bool) {
    if *wire == to {
        return;
    }
    *wire = to;
    if to {
        *scene_depth = Vec::new();
    } else {
        *taa = taa::TaaState::default();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gfx.is_some() {
            return;
        }

        /* GHOST_WINDOW_STATE_DEFAULT is GHOST_kWindowStateMaximized (wm_window.cc:133, used at :1143). */
        let mut attrs = Window::default_attributes()
            .with_title(window_title())
            .with_maximized(true);

        // wm_init_state: size = screen size, start = (0, 0).
        if let Some(monitor) = event_loop.primary_monitor() {
            let size: PhysicalSize<u32> = monitor.size();
            attrs = attrs
                .with_inner_size(size)
                .with_position(PhysicalPosition::new(0, 0));
        }

        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        self.user = Some(UserDef::from_window_scale(window.scale_factor()));
        self.rv3d.viewquat = view3d::STARTUP_VIEWQUAT;
        self.cursor3d_quat = [1.0, 0.0, 0.0, 0.0];
        let size = window.inner_size();
        self.layout = self
            .user
            .as_ref()
            .map(|user| screen::screen_layout(user, size.width as i32, size.height as i32));

        let context = softbuffer::Context::new(window.clone()).expect("softbuffer context");
        let surface = softbuffer::Surface::new(&context, window.clone()).expect("softbuffer surface");

        window.request_redraw();
        self.gfx = Some(Gfx { window, surface });
    }

    /* `screen.animation_play` timer: one frame per `1 / fps`, wrapping at the scene range ends. */
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(play) = self.playing.as_mut() else {
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        };
        let interval = std::time::Duration::from_secs_f64(1.0 / PLAY_FPS);
        let now = std::time::Instant::now();
        if now >= play.next {
            let (sfra, efra) = (playhead_draw::SFRA as i32, playhead_draw::EFRA as i32);
            let mut frame = self.frame.unwrap_or(1) + if play.reverse { -1 } else { 1 };
            if frame > efra {
                frame = sfra;
            } else if frame < sfra {
                frame = efra;
            }
            self.frame = Some(frame);
            play.next += interval;
            if play.next < now {
                play.next = now + interval;
            }
            if let Some(gfx) = self.gfx.as_ref() {
                gfx.window.request_redraw();
            }
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(play.next));
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(gfx) = self.gfx.as_mut() else { return };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let user = UserDef::from_window_scale(scale_factor);
                let size = gfx.window.inner_size();
                self.layout = Some(screen::screen_layout(&user, size.width as i32, size.height as i32));
                self.user = Some(user);
                gfx.window.request_redraw();
            }

            WindowEvent::CursorMoved { position, .. } => {
                let Some(layout) = self.layout.as_ref() else { return };
                let height = gfx.window.inner_size().height as i32;
                let x = position.x.floor() as i32;
                let y = height - 1 - position.y.floor() as i32;
                self.cursor = [x, y];
                /* Sculpt Mode: the radius / strength modal follows the cursor, a running stroke dabs along it,
                 * and the brush ring is redrawn at the new position. */
                if let (Some(sc), Some(action)) = (self.sculpt.as_mut(), self.sculpt_drag) {
                    /* A slider of the tool header is being dragged. */
                    if let (Some(layout), Some(user)) = (self.layout.as_ref(), self.user.as_ref()) {
                        let items = area::tool_header_rect(user, layout.main).map(|r| sculpt_header::layout(user, r)).unwrap_or_default();
                        if let Some(rect) = sculpt_header::rect_of(&items, action) {
                            sculpt_header::set_slider(action, rect, x, &mut sc.settings);
                        }
                    }
                    gfx.window.request_redraw();
                    return;
                }
                if let Some(sc) = self.sculpt.as_mut() {
                    if sc.adjust.is_some() {
                        sc.adjust_move([x, y]);
                    } else if sc.in_stroke() {
                        let (rw, rh) = (layout.main.size_x() + 1, layout.main.size_y() + 1);
                        self.rv3d.update(&self.v3d, rw, rh);
                        let mval = [x - layout.main.xmin, y - layout.main.ymin];
                        if sc.move_stroke(&mut self.scene, &self.rv3d, rw, rh, mval) {
                            scene_changed(&mut self.scene, &mut self.taa);
                        }
                    }
                    gfx.window.request_redraw();
                    if sc.in_stroke() {
                        return;
                    }
                }
                /* The tool header buttons light up under the cursor too. */
                if let Some(user) = self.user.as_ref() {
                    let hover = self.sculpt.as_ref().and_then(|_| area::tool_header_rect(user, layout.main)).filter(|r| r.isect_pt_v([x, y])).and_then(|r| sculpt_header::action_at(&sculpt_header::layout(user, r), [x, y]));
                    if hover != self.sculpt_hover {
                        self.sculpt_hover = hover;
                        gfx.window.request_redraw();
                    }
                }
                /* The header buttons light up under the cursor. */
                if let Some(user) = self.user.as_ref() {
                    let hover = area::view3d_header_rect(user, layout.main)
                        .filter(|header| header.isect_pt_v([x, y]))
                        .and_then(|header| view3d_header::action_at(&view3d_header::layout(user, header, &header_state(self.wire, self.xray, self.overlays_hidden, self.pivot_point, mode_of(self.edit_mode, self.sculpt.is_some()))), [x, y]));
                    if hover != self.header_hover {
                        self.header_hover = hover;
                        gfx.window.request_redraw();
                    }
                }
                /* The popup menu highlights the entry under the cursor. */
                if self.add_menu.is_some() || self.mod_menu.is_some() || self.brush_menu.is_some() {
                    gfx.window.request_redraw();
                }
                if let (Some((pos, open)), Some(user)) = (self.add_mod_menu, self.user.as_ref()) {
                    let size = gfx.window.inner_size();
                    let menu = add_modifier_menu::layout(user, pos, size.width as i32, size.height as i32, open);
                    if let Some(add_modifier_menu::Hit::Category(c)) = add_modifier_menu::hit(&menu, [x, y]) {
                        self.add_mod_menu = Some((pos, Some(c)));
                    }
                    gfx.window.request_redraw();
                }
                /* `view3d.select_box`: the border follows the cursor once it left the drag threshold. */
                if let Some(bs) = self.box_select.as_mut() {
                    if let Some(start) = bs.start {
                        bs.cur = [x - layout.main.xmin, y - layout.main.ymin];
                        if !bs.dragging && ((bs.cur[0] - start[0]).abs() > select_box::DRAG_THRESHOLD || (bs.cur[1] - start[1]).abs() > select_box::DRAG_THRESHOLD) {
                            bs.dragging = true;
                        }
                        if bs.dragging {
                            gfx.window.request_redraw();
                        }
                    }
                }
                /* Modal object transform (G / R / S): the cursor movement drives the transform. */
                if let Some(xf) = self.xform.as_mut() {
                    let (rw, rh) = (layout.main.size_x() + 1, layout.main.size_y() + 1);
                    xf.snap = self.modifiers.control_key();
                    xf.precision = self.modifiers.shift_key();
                    self.rv3d.update(&self.v3d, rw, rh);
                    let text = xf.apply(&mut self.scene, [x - layout.main.xmin, y - layout.main.ymin], &self.rv3d, rw, rh);
                    scene_changed(&mut self.scene, &mut self.taa);
                    set_status_title(&gfx.window, Some(&text));
                    gfx.window.request_redraw();
                }
                /* Time scrub drag: the frame under the cursor in the timeline's window region. */
                if self.scrub_drag {
                    if let Some(window) = self.user.as_ref().and_then(|_| outliner_draw::window_rect(self.user.as_ref().unwrap(), layout.timeline)) {
                        self.frame = Some(playhead_draw::frame_from_region_x(window, (x - window.xmin) as f32));
                        gfx.window.request_redraw();
                    }
                }
                /* `view3d.view_axis` (relative, the default `v3d_alt_mmb_drag_action`) on Alt +
                 * MIDDLEMOUSE CLICK_DRAG: `WM_event_drag_test()` with `U.drag_threshold_mouse` (3 px,
                 * times `UI_SCALE_FAC`), then `WM_event_drag_direction()`: eight 45 degree sectors,
                 * north = TOP, south = BOTTOM, east = RIGHT, west = LEFT; the diagonals have no binding. */
                if let Some(start) = self.alt_drag_start {
                    let scale = self.user.as_ref().map_or(1.0, |u| u.scale_factor);
                    let threshold = (3.0 * scale) as i32;
                    let (dx, dy) = (x - start[0], y - start[1]);
                    if dx.abs() > threshold || dy.abs() > threshold {
                        self.alt_drag_start = None;
                        let theta = (4.0 * (dy as f32).atan2(dx as f32) / std::f32::consts::PI).round() as i32;
                        let view = match theta {
                            0 => Some(view3d::View::Right),
                            2 => Some(view3d::View::Top),
                            -2 => Some(view3d::View::Bottom),
                            4 | -4 => Some(view3d::View::Left),
                            _ => None,
                        };
                        if let Some(view) = view {
                            smooth_finish(&mut self.rv3d, &mut self.smooth);
                            let smooth_src = self.rv3d.state_backup();
                            self.rv3d.view_axis_relative(view);
                            self.smooth = self.rv3d.smooth_view_begin(smooth_src, view3d::SMOOTH_VIEWTX).map(|sms| (sms, std::time::Instant::now()));
                            gfx.window.request_redraw();
                        }
                    }
                }
                if let Some(prev) = self.orbit_prev {
                    let scale = self.user.as_ref().map_or(1.0, |u| u.scale_factor);
                    let sensitivity = view3d::VIEW_ROTATE_SENSITIVITY_TURNTABLE / scale;
                    self.rv3d.rotate_apply(&mut self.orbit_curr, self.orbit_reverse, sensitivity, x - prev[0], y - prev[1], self.orbit_snap, self.orbit_ortho);
                    self.orbit_prev = Some([x, y]);
                    gfx.window.request_redraw();
                }
                if let Some(prev) = self.pan_prev {
                    let (rw, rh) = (layout.main.size_x() + 1, layout.main.size_y() + 1);
                    self.rv3d.pan(rw, rh, self.pan_zfac, x - prev[0], y - prev[1]);
                    self.pan_prev = Some([x, y]);
                    gfx.window.request_redraw();
                }
                if let Some((y_init, dist_init)) = self.zoom_init {
                    let scale = self.user.as_ref().map_or(1.0, |u| u.scale_factor);
                    self.rv3d.zoom_drag(&self.v3d, scale, layout.main.ymax, y_init, y, dist_init);
                    gfx.window.request_redraw();
                }
                if let Some((y_init, ofs_init)) = self.dolly_init {
                    self.rv3d.dolly_drag(layout.main.ymax, y_init, y, ofs_init);
                    gfx.window.request_redraw();
                }
                /* `transform.translate` (`cursor_transform`) on Shift + RIGHTMOUSE CLICK_DRAG: the drag starts
                 * after `U.drag_threshold_mouse`, then `InputVector()` turns the movement since the press
                 * into a view plane vector (`convertViewVec()`) added to the initial cursor location. */
                if let Some(drag) = self.cursor_drag.as_mut() {
                    let (dx, dy) = (x - drag.start[0], y - drag.start[1]);
                    let scale = self.user.as_ref().map_or(1.0, |u| u.scale_factor);
                    let threshold = (3.0 * scale) as i32;
                    if dx.abs() > threshold || dy.abs() > threshold {
                        drag.active = true;
                    }
                    if drag.active {
                        let (rw, rh) = (layout.main.size_x() + 1, layout.main.size_y() + 1);
                        /* `applyMouseInput()` with `use_virtual_mval`: the movement is accumulated and scaled by
                         * `precision_factor` (1/10) while precision is on. */
                        let mut delta = [dx as f64 - drag.prev[0], dy as f64 - drag.prev[1]];
                        drag.prev = [dx as f64, dy as f64];
                        if drag.precision {
                            let factor = (1.0f32 / 10.0f32) as f64;
                            delta = [delta[0] * factor, delta[1] * factor];
                        }
                        drag.accum = [drag.accum[0] + delta[0], drag.accum[1] + delta[1]];
                        let vec = self.rv3d.win_to_delta(rw, rh, drag.zfac, drag.accum[0] as f32, drag.accum[1] as f32);
                        self.cursor3d = std::array::from_fn(|i| drag.loc[i] + vec[i]);
                        gfx.window.request_redraw();
                    }
                }
                /* The active region follows the cursor. Global areas (top bar, status bar) are not in
                 * `screen->areabase`, so over them no area is active; elsewhere it is left unchanged. */
                let active = match layout.area_at([x, y]) {
                    Some(screen::AreaKind::TopBar | screen::AreaKind::StatusBar) => None,
                    Some(kind) => Some(kind),
                    None => self.active_area,
                };
                if active != self.active_area {
                    self.active_area = active;
                    gfx.window.request_redraw();
                }
                let over_header = self.user.as_ref().is_some_and(|u| status_input::over_editor_header(u, layout, [x, y]));
                if over_header != self.status_header {
                    self.status_header = over_header;
                    gfx.window.request_redraw();
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }

            /* `view3d.rotate` on MIDDLEMOUSE PRESS without modifiers (shift pans, ctrl zooms);
             * not over the header region. */
            WindowEvent::MouseInput { state, button: MouseButton::Middle, .. } => match state {
                ElementState::Pressed => {
                    let (Some(layout), Some(user)) = (self.layout.as_ref(), self.user.as_ref()) else { return };
                    let in_main = area::in_view3d_main(user, layout.main, self.cursor, self.sculpt.is_some());
                    let m = self.modifiers;
                    /* Alt + MIDDLEMOUSE (no Shift / Ctrl) is a `CLICK_DRAG` candidate for `view3d.view_axis`. */
                    self.alt_drag_start = if in_main && m.alt_key() && !m.shift_key() && !m.control_key() {
                        Some(self.cursor)
                    } else {
                        None
                    };
                    if in_main && !m.alt_key() {
                        smooth_finish(&mut self.rv3d, &mut self.smooth);
                        self.nav_init = Some((
                            self.rv3d.viewquat,
                            self.rv3d.dist,
                            self.rv3d.ofs,
                            self.rv3d.ortho,
                            self.rv3d.view,
                            self.rv3d.view_axis_roll,
                        ));
                        if m.control_key() {
                            if m.shift_key() {
                                /* `view3d.dolly`. */
                                self.dolly_init = Some((self.cursor[1], self.rv3d.ofs));
                            } else {
                                /* `view3d.zoom`. */
                                self.zoom_init = Some((self.cursor[1], self.rv3d.dist));
                            }
                        } else if m.shift_key() {
                            /* `view3d.move`: `init.zfac` is taken at `-ofs`. */
                            let ofs = self.rv3d.ofs;
                            self.pan_zfac = self.rv3d.calc_zfac([-ofs[0], -ofs[1], -ofs[2]]);
                            self.pan_prev = Some(self.cursor);
                        } else {
                            /* `VIEWOPS_FLAG_PERSP_ENSURE` of `view3d.rotate`. */
                            self.rv3d.persp_ensure();
                            self.orbit_reverse = if self.rv3d.persmat[2][1] < 0.0 { -1.0 } else { 1.0 };
                            self.orbit_curr = self.rv3d.viewquat;
                            self.orbit_snap = false;
                            self.orbit_ortho = self.rv3d.ortho;
                            self.orbit_prev = Some(self.cursor);
                        }
                    }
                }
                ElementState::Released => {
                    /* `view3d.view_center_pick` on Alt + MIDDLEMOUSE CLICK: released without the drag
                     * of `view3d.view_axis` having started. */
                    if self.alt_drag_start.take().is_some() {
                        if let Some(layout) = self.layout.as_ref() {
                            let (rw, rh) = (layout.main.size_x() + 1, layout.main.size_y() + 1);
                            let mval = [self.cursor[0] - layout.main.xmin, self.cursor[1] - layout.main.ymin];
                            smooth_finish(&mut self.rv3d, &mut self.smooth);
                            self.rv3d.update(&self.v3d, rw, rh);
                            let smooth_src = self.rv3d.state_backup();
                            self.rv3d.center_pick(rw, rh, mval, &self.scene_depth);
                            self.smooth = self.rv3d.smooth_view_begin(smooth_src, view3d::SMOOTH_VIEWTX).map(|sms| (sms, std::time::Instant::now()));
                            gfx.window.request_redraw();
                        }
                    }
                    self.orbit_prev = None;
                    self.pan_prev = None;
                    self.zoom_init = None;
                    self.dolly_init = None;
                    self.nav_init = None;
                }
            },
            /* `view3d.cursor3d` on Shift + RIGHTMOUSE PRESS; not over the header region. `use_depth` is
             * on by default but there is no geometry to hit yet, so only the view plane fallback of
             * `ED_view3d_cursor3d_position()` is done, and the cursor orientation is not modelled. */
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Right, .. } => {
                /* A popup menu closes on RIGHTMOUSE. */
                if self.add_menu.take().is_some() || self.brush_menu.take().is_some() || self.mod_menu.take().is_some() || self.add_mod_menu.take().is_some() {
                    gfx.window.request_redraw();
                    return;
                }
                /* `Transform Modal Map` CANCEL on RIGHTMOUSE: the objects go back to where they were. */
                if let Some(xf) = self.xform.take() {
                    xf.cancel(&mut self.scene);
                    scene_changed(&mut self.scene, &mut self.taa);
                    set_status_title(&gfx.window, None);
                    gfx.window.request_redraw();
                    return;
                }
                /* `change_frame` modal CANCEL: the frame goes back to where the scrub started. */
                if self.scrub_drag {
                    self.scrub_drag = false;
                    self.frame = Some(self.scrub_init);
                    gfx.window.request_redraw();
                    return;
                }
                /* `CANCEL` of the `View3D Rotate / Move / Zoom / Dolly Modal` keymaps: RIGHTMOUSE press
                 * with any modifier; `ViewOpsData::state_restore()` puts back `ofs`, `dist` and `viewquat`. */
                if let Some((quat, dist, ofs, ortho, view, roll)) = self.nav_init.take() {
                    self.rv3d.viewquat = quat;
                    self.rv3d.dist = dist;
                    self.rv3d.ofs = ofs;
                    self.rv3d.ortho = ortho;
                    self.rv3d.view = view;
                    self.rv3d.view_axis_roll = roll;
                    self.orbit_prev = None;
                    self.pan_prev = None;
                    self.zoom_init = None;
                    self.dolly_init = None;
                    gfx.window.request_redraw();
                    return;
                }
                let (Some(layout), Some(user)) = (self.layout.as_ref(), self.user.as_ref()) else { return };
                let [x, y] = self.cursor;
                let m = self.modifiers;
                if !m.shift_key() || m.control_key() || m.alt_key() || !area::in_view3d_main(user, layout.main, self.cursor, self.sculpt.is_some()) {
                    return;
                }
                let (rw, rh) = (layout.main.size_x() + 1, layout.main.size_y() + 1);
                let mval = [x - layout.main.xmin, y - layout.main.ymin];
                self.cursor3d = self.rv3d.cursor3d_position_depth(rw, rh, mval, self.cursor3d, &self.scene_depth);
                /* `orientation` defaults to VIEW: `ED_view3d_cursor3d_position_rotation()` copies
                 * `viewquat` with the sign of `w` flipped. */
                let q = self.rv3d.viewquat;
                self.cursor3d_quat = [-q[0], q[1], q[2], q[3]];
                self.cursor_drag = Some(CursorDrag {
                    start: self.cursor,
                    loc: self.cursor3d,
                    zfac: self.rv3d.calc_zfac(self.cursor3d),
                    active: false,
                    prev: [0.0; 2],
                    accum: [0.0; 2],
                    precision: false,
                });
                gfx.window.request_redraw();
            }
            /* `release_confirm` of the cursor drag: releasing RIGHTMOUSE confirms the translation. */
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Right, .. } => {
                self.cursor_drag = None;
            }
            /* `Transform Modal Map`: CONFIRM on LEFTMOUSE press (any modifier) while the cursor drag runs. */
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                /* A click outside the properties editor applies the number being typed into a modifier field
                 * (and the click goes on to whatever is under it, as in Blender). */
                if self.mod_edit.is_some() {
                    let inside = match (self.layout.as_ref(), self.user.as_ref()) {
                        (Some(layout), Some(user)) => modifier_panel::window_rect(user, layout.properties).is_some_and(|w| w.isect_pt_v(self.cursor)),
                        _ => false,
                    };
                    if !inside {
                        if self.mod_edit.take().is_some_and(|e| e.commit(&mut self.scene)) {
                            scene_changed(&mut self.scene, &mut self.taa);
                        }
                        gfx.window.request_redraw();
                    }
                }
                /* A click in the popup menu runs the entry under the cursor (`object.*_add`: the new object is
                 * placed at the 3D cursor); any click closes the menu, outside it nothing else happens. */
                if self.rename.take().is_some() {
                    gfx.window.request_redraw();
                    return;
                }
                /* A click in the Add Modifier menu: a category row keeps it open, an entry adds that modifier, anything else closes it. */
                if let Some((pos, open)) = self.add_mod_menu.take() {
                    if let Some(user) = self.user.as_ref() {
                        let size = gfx.window.inner_size();
                        let menu = add_modifier_menu::layout(user, pos, size.width as i32, size.height as i32, open);
                        match add_modifier_menu::hit(&menu, self.cursor) {
                            Some(add_modifier_menu::Hit::Category(c)) => self.add_mod_menu = Some((pos, Some(c))),
                            Some(add_modifier_menu::Hit::Item(idx)) => {
                                if modifier_panel::Action::Add(idx).run_add(&mut self.scene) {
                                    scene_changed(&mut self.scene, &mut self.taa);
                                }
                            }
                            None => {}
                        }
                    }
                    gfx.window.request_redraw();
                    return;
                }
                /* A click in the modifier dropdown runs the entry under the cursor; any click closes the menu. */
                if let Some((i, pos)) = self.mod_menu.take() {
                    if let Some(user) = self.user.as_ref() {
                        let size = gfx.window.inner_size();
                        let menu = modifier_menu::layout(user, &self.scene, i, pos, size.width as i32, size.height as i32);
                        if let Some(item) = modifier_menu::item_at(&menu, self.cursor) {
                            if item.run(&mut self.scene, i) {
                                scene_changed(&mut self.scene, &mut self.taa);
                            }
                        }
                    }
                    gfx.window.request_redraw();
                    return;
                }
                if let Some(pos) = self.brush_menu.take() {
                    if let (Some(user), Some(sc)) = (self.user.as_ref(), self.sculpt.as_mut()) {
                        let size = gfx.window.inner_size();
                        let menu = brush_menu::layout(user, pos, size.width as i32, size.height as i32);
                        if let Some(brush) = brush_menu::item_at(&menu, self.cursor) {
                            sc.settings.brush = brush;
                        }
                    }
                    gfx.window.request_redraw();
                    return;
                }
                if let Some(pos) = self.add_menu.take() {
                    if let Some(user) = self.user.as_ref() {
                        let size = gfx.window.inner_size();
                        let menu = add_menu::layout(user, pos, size.width as i32, size.height as i32);
                        if let Some(item) = add_menu::item_at(&menu, self.cursor) {
                            self.scene.add_item(item, self.cursor3d);
                            scene_changed(&mut self.scene, &mut self.taa);
                        }
                    }
                    gfx.window.request_redraw();
                    return;
                }
                /* `Transform Modal Map` CONFIRM on LEFTMOUSE: the transform stays as it is. */
                if self.xform.take().is_some() {
                    set_status_title(&gfx.window, None);
                    gfx.window.request_redraw();
                    return;
                }
                /* Sculpt tool header (`VIEW3D_HT_tool_header`): sliders start a drag, the other buttons act on the
                 * brush settings; a press anywhere in the region stays there. */
                if self.sculpt.is_some() {
                    if let (Some(layout), Some(user)) = (self.layout.as_ref(), self.user.as_ref()) {
                        if let Some(region) = area::tool_header_rect(user, layout.main).filter(|r| r.isect_pt_v(self.cursor)) {
                            let items = sculpt_header::layout(user, region);
                            let shift = self.modifiers.shift_key();
                            let x = self.cursor[0];
                            if let (Some(action), Some(sc)) = (sculpt_header::action_at(&items, self.cursor), self.sculpt.as_mut()) {
                                match sculpt_header::rect_of(&items, action) {
                                    Some(rect) if matches!(action, sculpt_header::Action::Radius | sculpt_header::Action::Strength) => {
                                        sculpt_header::set_slider(action, rect, x, &mut sc.settings);
                                        self.sculpt_drag = Some(action);
                                    }
                                    /* The brush dropdown opens the brush popup under it. */
                                    Some(rect) if action == sculpt_header::Action::Brush => self.brush_menu = Some([rect.xmin, rect.ymin]),
                                    _ => {
                                        sculpt_header::press(action, shift, &mut sc.settings);
                                    }
                                }
                            }
                            gfx.window.request_redraw();
                            return;
                        }
                    }
                }
                /* 3D viewport header buttons (`VIEW3D_HT_header`): a press anywhere in the header region stays
                 * there, it never reaches the viewport. */
                let header_hit = match (self.layout.as_ref(), self.user.as_ref()) {
                    (Some(layout), Some(user)) => area::view3d_header_rect(user, layout.main)
                        .filter(|header| header.isect_pt_v(self.cursor))
                        .map(|header| {
                            let items = view3d_header::layout(user, header, &header_state(self.wire, self.xray, self.overlays_hidden, self.pivot_point, mode_of(self.edit_mode, self.sculpt.is_some())));
                            let action = view3d_header::action_at(&items, self.cursor);
                            let anchor = action.and_then(|a| view3d_header::rect_of(&items, a)).map(|r| [r.xmin, r.ymin]);
                            (action, anchor)
                        }),
                    _ => None,
                };
                if let Some((action, anchor)) = header_hit {
                    /* Buttons for things this port does not model (the editor type and mode dropdowns, the View,
                     * Select and Object menus, orientation, snapping, proportional editing, the option arrows and
                     * the Material Preview and Rendered shading) are drawn but do nothing. */
                    use view3d_header::Action;
                    match action {
                        Some(Action::ShadeWire) => set_wire(&mut self.wire, &mut self.scene_depth, &mut self.taa, true),
                        Some(Action::ShadeSolid) => set_wire(&mut self.wire, &mut self.scene_depth, &mut self.taa, false),
                        /* `view3d.toggle_xray` (Alt + Z); not modelled in wire shading. */
                        Some(Action::Xray) if !self.wire => self.xray = !self.xray,
                        /* `show_overlays` (Shift + Alt + Z). */
                        Some(Action::Overlays) => self.overlays_hidden = !self.overlays_hidden,
                        /* The pivot dropdown cycles the modes like Period does. */
                        Some(Action::Pivot) => self.pivot_point = self.pivot_point.next(),
                        /* Opens the Add menu below the button, like Shift + A does at the cursor. */
                        Some(Action::MenuAdd) => self.add_menu = anchor,
                        /* The mode dropdown toggles Object Mode and Sculpt Mode (it has no menu here). */
                        Some(Action::Mode) => {
                            toggle_sculpt(&mut self.sculpt, &self.scene);
                            self.edit_mode &= self.sculpt.is_none();
                        }
                        _ => {}
                    }
                    gfx.window.request_redraw();
                    return;
                }
                /* Time scrub strip of the timeline (`ED_time_scrub_event_in_region()`): the press sets the frame
                 * under the cursor and starts the drag. */
                if let (Some(layout), Some(user)) = (self.layout.as_ref(), self.user.as_ref()) {
                    if let Some(window) = outliner_draw::window_rect(user, layout.timeline) {
                        if area::time_scrub_event_rect(user, window).isect_pt_v(self.cursor) {
                            self.scrub_init = self.frame.unwrap_or(1);
                            self.scrub_drag = true;
                            self.frame = Some(playhead_draw::frame_from_region_x(window, (self.cursor[0] - window.xmin) as f32));
                            gfx.window.request_redraw();
                            return;
                        }
                    }
                }
                if let (Some(zb), Some(layout)) = (self.zoom_border.as_mut(), self.layout.as_ref()) {
                    zb.start = Some([self.cursor[0] - layout.main.xmin, self.cursor[1] - layout.main.ymin]);
                    return;
                }
                if self.cursor_drag.as_ref().is_some_and(|d| d.active) {
                    self.cursor_drag = None;
                    return;
                }
                /* `WM_gesture_box_modal()` after B: the press sets the start corner (LEFTMOUSE extends, Ctrl subtracts). */
                if let (Some(bs), Some(layout)) = (self.box_select.as_mut(), self.layout.as_ref()) {
                    if bs.armed && bs.start.is_none() {
                        let mval = [self.cursor[0] - layout.main.xmin, self.cursor[1] - layout.main.ymin];
                        bs.start = Some(mval);
                        bs.cur = mval;
                        bs.dragging = true;
                        bs.mode = if self.modifiers.control_key() { select_box::BoxMode::Sub } else { select_box::BoxMode::Add };
                        return;
                    }
                }
                /* `view3d.select` on LEFTMOUSE CLICK in the viewport (Shift extends and toggles): the nearest
                 * object under the cursor, or nothing. In the outliner (`outliner.item_activate`) the clicked
                 * object row. */
                let (Some(layout), Some(user)) = (self.layout.as_ref(), self.user.as_ref()) else { return };
                let extend = self.modifiers.shift_key();
                /* The Transform panel of the properties editor: a click on a field starts typing into it; a click
                 * anywhere else applies the text of the open field and then does what it normally does. */
                let tp_window = modifier_panel::window_rect(user, layout.properties);
                let tp = tp_window.map(|w| transform_panel::layout(user, w, &self.scene, self.field_edit.as_ref()));
                let clicked_field = match (&tp_window, &tp) {
                    (Some(w), Some(l)) if w.isect_pt_v(self.cursor) => transform_panel::field_at(l, self.cursor),
                    _ => None,
                };
                if let Some(f) = clicked_field {
                    if !self.field_edit.as_ref().is_some_and(|e| e.field == f) {
                        let changed = self.field_edit.take().is_some_and(|e| e.commit(&mut self.scene));
                        self.field_edit = transform_panel::Edit::start(&self.scene, f);
                        if changed {
                            scene_changed(&mut self.scene, &mut self.taa);
                        }
                    }
                    gfx.window.request_redraw();
                    return;
                }
                if let Some(e) = self.field_edit.take() {
                    if e.commit(&mut self.scene) {
                        scene_changed(&mut self.scene, &mut self.taa);
                    }
                    gfx.window.request_redraw();
                }
                if area::in_view3d_main(user, layout.main, self.cursor, self.sculpt.is_some()) {
                    let (rw, rh) = (layout.main.size_x() + 1, layout.main.size_y() + 1);
                    self.rv3d.update(&self.v3d, rw, rh);
                    let mval = [self.cursor[0] - layout.main.xmin, self.cursor[1] - layout.main.ymin];
                    /* Sculpt Mode: a press confirms the radius / strength modal or starts a brush stroke (Ctrl
                     * inverts the brush, Shift smooths); nothing is selected. */
                    if let Some(sc) = self.sculpt.as_mut() {
                        if sc.adjust.is_some() {
                            sc.adjust_end(false);
                        } else if sc.begin_stroke(&mut self.scene, &self.rv3d, rw, rh, mval, self.modifiers.control_key(), self.modifiers.shift_key()) {
                            scene_changed(&mut self.scene, &mut self.taa);
                        }
                        gfx.window.request_redraw();
                        return;
                    }
                    let mut hit = self.scene.pick(&self.rv3d, rw, rh, mval);
                    if hit.is_none() && !self.overlays_hidden {
                        /* Cameras and lights are overlay shapes: a click near their lines picks them. */
                        hit = overlay_extra::pick(&self.scene, &self.rv3d, rw, rh, mval);
                    }
                    if hit.is_none() {
                        /* The Select Box tool: a press on empty space waits to see whether it is a click (deselect)
                         * or the start of a box. */
                        self.box_select = Some(BoxSelect { armed: false, start: Some(mval), cur: mval, dragging: false, mode: select_box::BoxMode::from_keys(extend, self.modifiers.control_key()) });
                    } else {
                        self.scene.select_pick(hit, extend);
                    }
                    gfx.window.request_redraw();
                } else if let Some(window) = modifier_panel::window_rect(user, layout.properties).filter(|w| w.isect_pt_v(self.cursor)) {
                    /* The Modifiers panel of the properties editor acts on the active object. */
                    let below = area::Rcti { ymax: tp.as_ref().map_or(window.ymax, |l| l.bottom), ..window };
                    let widgets = modifier_panel::widgets(user, below, &modifier_panel::PanelState::of(&self.scene, self.mod_add_open).with_edit(self.mod_edit.as_ref()));
                    if let Some(action) = modifier_panel::action_at(&widgets, self.cursor) {
                        let action = if self.modifiers.control_key() { modifier_panel::with_ctrl(action) } else { action };
                        /* A double-click on a modifier's name edits it (Blender's text field of the panel header). */
                        /* A click on the field being typed into stays in it; any other panel click applies it first. */
                        if let modifier_panel::Action::EditValue(i) = action {
                            if !self.mod_edit.as_ref().is_some_and(|e| e.index == i) {
                                if self.mod_edit.take().is_some_and(|e| e.commit(&mut self.scene)) {
                                    scene_changed(&mut self.scene, &mut self.taa);
                                }
                                self.scene.panel_select_modifier(i);
                                self.mod_edit = modifier_panel::ValueEdit::start(&self.scene, i);
                            }
                            gfx.window.request_redraw();
                            return;
                        }
                        if self.mod_edit.take().is_some_and(|e| e.commit(&mut self.scene)) {
                            scene_changed(&mut self.scene, &mut self.taa);
                        }
                        if action == modifier_panel::Action::ToggleAddList {
                            if let Some(w) = widgets.iter().find(|w| w.action == Some(action)) {
                                self.add_mod_menu = Some(([w.rect.xmin, w.rect.ymin - 1], None));
                            }
                            gfx.window.request_redraw();
                            return;
                        }
                        if let modifier_panel::Action::DragStart(i) = action {
                            self.mod_drag = Some((i, modifier_panel::header_tops(&widgets)));
                            gfx.window.request_redraw();
                            return;
                        }
                        if let modifier_panel::Action::OpenMenu(i) = action {
                            self.mod_menu = Some((i, [self.cursor[0], self.cursor[1]]));
                            gfx.window.request_redraw();
                            return;
                        }
                        let now = std::time::Instant::now();
                        let double = matches!(action, modifier_panel::Action::Select(_))
                            && self.last_panel_click.is_some_and(|(t, a)| a == action && now.duration_since(t).as_millis() < 400);
                        self.last_panel_click = if double { None } else { Some((now, action)) };
                        if let (true, modifier_panel::Action::Select(i)) = (double, action) {
                            if let Some(name) = self.scene.active_object().and_then(|o| o.modifier_name(i)) {
                                self.rename = Some(rename_popup::Rename::open_modifier(self.cursor, &name, i));
                            }
                            gfx.window.request_redraw();
                            return;
                        }
                        if modifier_panel::apply(&mut self.scene, &mut self.mod_add_open, action) {
                            scene_changed(&mut self.scene, &mut self.taa);
                        }
                        gfx.window.request_redraw();
                    }
                } else if let Some(window) = outliner_draw::window_rect(user, layout.outliner).filter(|w| w.isect_pt_v(self.cursor)) {
                    if let Some(edit) = self.outliner_rename.as_ref() {
                        /* Clicking inside the field keeps editing, anywhere else confirms it. */
                        let inside = outliner_draw::rename_field_rect(user, window, &self.scene, edit.id).is_some_and(|r| r.isect_pt_v(self.cursor));
                        if !inside {
                            if let Some(e) = self.outliner_rename.take() {
                                if outliner_rename::commit(&mut self.scene, &e) {
                                    scene_changed(&mut self.scene, &mut self.taa);
                                }
                            }
                        }
                        gfx.window.request_redraw();
                    } else if let Some((id, col)) = outliner_draw::restrict_click(user, window, &self.scene, self.cursor[0], self.cursor[1]) {
                        let which = match col {
                            outliner_draw::RestrictCol::Hide | outliner_draw::RestrictCol::Viewport => scene::Restrict::Viewport,
                            outliner_draw::RestrictCol::Select => scene::Restrict::Select,
                            outliner_draw::RestrictCol::Render => scene::Restrict::Render,
                        };
                        if self.scene.toggle_restrict(id, which) {
                            scene_changed(&mut self.scene, &mut self.taa);
                        }
                        gfx.window.request_redraw();
                    } else if let Some(row) = outliner_draw::row_at(user, window, self.cursor[1]) {
                        let rows = self.scene.outliner_rows();
                        /* `LEFTMOUSE` `DOUBLE_CLICK` -> `outliner.item_rename` on the name of the element under the cursor. */
                        if let Some(scene::RowKind::Object(id)) = rows.get(row).map(|r| r.kind.clone()) {
                            let now = std::time::Instant::now();
                            let double = self.last_outliner_click.is_some_and(|(t, i)| i == id && now.duration_since(t).as_millis() < 400);
                            self.last_outliner_click = if double { None } else { Some((now, id)) };
                            let name_w = self.scene.index_of(id).map_or(0, |i| font::string_width(user, &self.scene.objects[i].name));
                            let over_name = outliner_draw::rename_field_rect(user, window, &self.scene, id)
                                .is_some_and(|r| self.cursor[0] >= r.xmin && self.cursor[0] <= r.xmin + name_w);
                            if double && over_name {
                                self.outliner_rename = outliner_rename::start(&self.scene, &scene::RowKind::Object(id));
                                gfx.window.request_redraw();
                                return;
                            }
                        }
                        if let Some(scene::RowKind::Object(id)) = rows.get(row).map(|r| r.kind.clone()) {
                            let click = outliner_select::Click::from_keys(self.modifiers.control_key(), extend);
                            outliner_select::item_activate(&mut self.scene, id, click);
                            gfx.window.request_redraw();
                        }
                    }
                }
            }
            /* `WM_gesture_box_modal()`: releasing LEFTMOUSE confirms the border and runs `view3d.zoom_border`. */
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Left, .. } => {
                self.scrub_drag = false;
                if self.sculpt_drag.take().is_some() {
                    gfx.window.request_redraw();
                    return;
                }
                if let Some(sc) = self.sculpt.as_mut().filter(|sc| sc.in_stroke()) {
                    sc.end_stroke();
                    gfx.window.request_redraw();
                    return;
                }
                if let Some((i, tops)) = self.mod_drag.take() {
                    if modifier_panel::drop_index(&tops, self.cursor[1]).is_some_and(|to| self.scene.panel_move_modifier_index(i, to)) {
                        scene_changed(&mut self.scene, &mut self.taa);
                    }
                    gfx.window.request_redraw();
                    return;
                }
                if let Some(bs) = self.box_select.take() {
                    if let (Some(start), Some(layout)) = (bs.start, self.layout.as_ref()) {
                        let (rw, rh) = (layout.main.size_x() + 1, layout.main.size_y() + 1);
                        if bs.dragging {
                            self.rv3d.update(&self.v3d, rw, rh);
                            let rect = select_box::rect_of(start, bs.cur);
                            self.scene.select_box(&self.rv3d, rw, rh, rect, bs.mode, self.xray || self.wire);
                        } else if !bs.armed {
                            /* A click on nothing: deselect unless extending. */
                            self.scene.select_pick(None, bs.mode != select_box::BoxMode::Set);
                        }
                        gfx.window.request_redraw();
                    }
                    return;
                }
                let Some((start, clip)) = self.zoom_border.as_ref().and_then(|z| z.start.map(|s| (s, z.clip))) else { return };
                self.zoom_border = None;
                let Some(layout) = self.layout.as_ref() else { return };
                let (rw, rh) = (layout.main.size_x() + 1, layout.main.size_y() + 1);
                let cur = [self.cursor[0] - layout.main.xmin, self.cursor[1] - layout.main.ymin];
                let rect = [start[0].min(cur[0]), start[0].max(cur[0]), start[1].min(cur[1]), start[1].max(cur[1])];
                if rect[0] == rect[1] || rect[2] == rect[3] {
                    return;
                }
                smooth_finish(&mut self.rv3d, &mut self.smooth);
                self.rv3d.update(&self.v3d, rw, rh);
                /* `view3d_clipping_exec()`: `RV3D_CLIPPING` with the planes of `ED_view3d_clipping_calc()`. */
                if clip {
                    self.rv3d.clip = Some(self.rv3d.clipping_from_rect(rw, rh, rect));
                    self.taa = taa::TaaState::default();
                    gfx.window.request_redraw();
                    return;
                }
                let depth_close = view3d::RegionView3D::depth_near(&self.scene_depth, rw, rh, rect);
                let smooth_src = self.rv3d.state_backup();
                if self.rv3d.zoom_border(&self.v3d, rw, rh, rect, depth_close, true) {
                    self.smooth = self.rv3d.smooth_view_begin(smooth_src, view3d::SMOOTH_VIEWTX).map(|sms| (sms, std::time::Instant::now()));
                    gfx.window.request_redraw();
                }
            }
            /* `view3d.zoom` on WHEELINMOUSE / WHEELOUTMOUSE; not over the header region. */
            WindowEvent::MouseWheel { delta: MouseScrollDelta::LineDelta(hlines, lines), .. } => {
                let (Some(layout), Some(user)) = (self.layout.as_ref(), self.user.as_ref()) else { return };
                let in_main = area::in_view3d_main(user, layout.main, self.cursor, self.sculpt.is_some());
                let m = self.modifiers;
                /* `screen.frame_offset` on Alt + WHEELUPMOUSE (delta -1) / WHEELDOWNMOUSE (delta +1), any area. */
                if m.alt_key() && !m.control_key() && !m.shift_key() {
                    if lines != 0.0 {
                        let delta = if lines > 0.0 { -1 } else { 1 } * (lines.abs().round() as i32).max(1);
                        self.frame = Some((self.frame.unwrap_or(1) + delta).max(0));
                        gfx.window.request_redraw();
                    }
                    return;
                }
                /* `view3d.view_orbit` on Alt + Ctrl + WHEELUPMOUSE / WHEELDOWNMOUSE (ORBITLEFT / ORBITRIGHT) and
                 * Alt + Shift + WHEELUPMOUSE / WHEELDOWNMOUSE (ORBITUP / ORBITDOWN), by `U.pad_rot_angle`
                 * with the smooth view of the numpad orbit. */
                if m.alt_key() && m.control_key() != m.shift_key() && in_main {
                    if lines != 0.0 {
                        let sign = if lines > 0.0 { 1.0 } else { -1.0 };
                        let about_z = m.control_key();
                        smooth_finish(&mut self.rv3d, &mut self.smooth);
                        let smooth_src = self.rv3d.state_backup();
                        for _ in 0..(lines.abs().round() as i32).max(1) {
                            self.rv3d.orbit_step(about_z, sign * view3d::PAD_ROT_ANGLE);
                        }
                        self.smooth = self.rv3d.smooth_view_begin(smooth_src, view3d::SMOOTH_VIEWTX).map(|sms| (sms, std::time::Instant::now()));
                        gfx.window.request_redraw();
                    }
                    return;
                }
                if !in_main || m.alt_key() || (m.control_key() && m.shift_key()) {
                    return;
                }
                /* `view3d.view_pan` on Ctrl / Shift + WHEELUPMOUSE / WHEELDOWNMOUSE (default keymap):
                 * Ctrl + up is PANRIGHT (x = -32), Ctrl + down PANLEFT (x = 32), Shift + up PANUP
                 * (y = -25), Shift + down PANDOWN (y = 25), with `init.zfac` taken at `-ofs`. */
                if m.control_key() || m.shift_key() {
                    if lines != 0.0 {
                        let (rw, rh) = (layout.main.size_x() + 1, layout.main.size_y() + 1);
                        let up = lines > 0.0;
                        let (dx, dy) = if m.control_key() {
                            (if up { -32 } else { 32 }, 0)
                        } else {
                            (0, if up { -25 } else { 25 })
                        };
                        for _ in 0..(lines.abs().round() as i32).max(1) {
                            let ofs = self.rv3d.ofs;
                            let zfac = self.rv3d.calc_zfac([-ofs[0], -ofs[1], -ofs[2]]);
                            self.rv3d.pan(rw, rh, zfac, dx, dy);
                        }
                        gfx.window.request_redraw();
                    }
                    return;
                }
                /* `view3d.view_pan` on WHEELLEFTMOUSE / WHEELRIGHTMOUSE (default keymap): PANLEFT is
                 * x = 32, PANRIGHT x = -32, with `init.zfac` taken at `-ofs` like `view3d.move`. A
                 * positive horizontal delta moves the content right, i.e. the wheel turns left. */
                if hlines != 0.0 {
                    let (rw, rh) = (layout.main.size_x() + 1, layout.main.size_y() + 1);
                    let dx = if hlines > 0.0 { 32 } else { -32 };
                    for _ in 0..(hlines.abs().round() as i32).max(1) {
                        let ofs = self.rv3d.ofs;
                        let zfac = self.rv3d.calc_zfac([-ofs[0], -ofs[1], -ofs[2]]);
                        self.rv3d.pan(rw, rh, zfac, dx, 0);
                    }
                    gfx.window.request_redraw();
                }
                if lines == 0.0 {
                    return;
                }
                let step = if lines > 0.0 { 1 } else { -1 };
                for _ in 0..(lines.abs().round() as i32).max(1) {
                    self.rv3d.zoom_step(&self.v3d, step);
                }
                gfx.window.request_redraw();
            }
            /* `view3d.zoom` (`delta = +1 / -1`) on NUMPAD_PLUS / NUMPAD_MINUS without modifiers and
             * `view3d.view_pan` on Ctrl + NUMPAD_2 / 4 / 6 / 8; not over the header region. */
            WindowEvent::KeyboardInput { event: key_event, .. } => {
                use winit::keyboard::{KeyCode, PhysicalKey};
                /* `View3D Rotate Modal`: AXIS_SNAP_ENABLE on LEFT_ALT / RIGHT_ALT press and
                 * AXIS_SNAP_DISABLE on release (`any` modifier). Disabling restores the perspective
                 * the drag started with, then the unsnapped rotation is applied again. */
                if matches!(key_event.physical_key, PhysicalKey::Code(KeyCode::AltLeft | KeyCode::AltRight)) {
                    if self.orbit_prev.is_some() {
                        let enable = key_event.state == ElementState::Pressed;
                        if !enable {
                            self.rv3d.ortho = self.orbit_ortho;
                        }
                        self.orbit_snap = enable;
                        let scale = self.user.as_ref().map_or(1.0, |u| u.scale_factor);
                        let sensitivity = view3d::VIEW_ROTATE_SENSITIVITY_TURNTABLE / scale;
                        self.rv3d.rotate_apply(&mut self.orbit_curr, self.orbit_reverse, sensitivity, 0, 0, self.orbit_snap, self.orbit_ortho);
                        gfx.window.request_redraw();
                    }
                    return;
                }
                /* `Transform Modal Map`: PRECISION on LEFT_SHIFT / RIGHT_SHIFT press and release (no key repeat). */
                if matches!(key_event.physical_key, PhysicalKey::Code(KeyCode::ShiftLeft | KeyCode::ShiftRight)) && !key_event.repeat {
                    if let Some(drag) = self.cursor_drag.as_mut().filter(|d| d.active) {
                        drag.precision = key_event.state == ElementState::Pressed;
                    }
                }
                /* `Transform Modal Map`: CONFIRM on RET / NUMPAD_ENTER / SPACE press (any modifier, no repeat). */
                if key_event.state == ElementState::Pressed
                    && !key_event.repeat
                    && self.cursor_drag.as_ref().is_some_and(|d| d.active)
                    && matches!(key_event.physical_key, PhysicalKey::Code(KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space))
                {
                    self.cursor_drag = None;
                    return;
                }
                if key_event.state != ElementState::Pressed {
                    return;
                }
                let PhysicalKey::Code(code) = key_event.physical_key else { return };
                let (shift_k, ctrl_k, alt_k) = (self.modifiers.shift_key(), self.modifiers.control_key(), self.modifiers.alt_key());

                /* A field of the Transform panel that is being typed into takes the keyboard: Enter applies, ESC
                 * cancels, Backspace deletes, the number characters are typed. */
                /* A modifier value field that is being typed into takes the keyboard: Enter applies, ESC cancels,
                 * Backspace deletes, the number characters are typed. */
                if self.mod_edit.is_some() {
                    match code {
                        KeyCode::Enter | KeyCode::NumpadEnter => {
                            if self.mod_edit.take().is_some_and(|e| e.commit(&mut self.scene)) {
                                scene_changed(&mut self.scene, &mut self.taa);
                            }
                        }
                        KeyCode::Escape => self.mod_edit = None,
                        KeyCode::Backspace => {
                            if let Some(e) = self.mod_edit.as_mut() {
                                e.backspace();
                            }
                        }
                        _ => {
                            if let (Some(e), Some(text)) = (self.mod_edit.as_mut(), key_event.text.as_ref()) {
                                if !ctrl_k && !alt_k {
                                    e.push(text);
                                }
                            }
                        }
                    }
                    gfx.window.request_redraw();
                    return;
                }
                /* The outliner's rename field: Enter confirms, ESC cancels (`ui_do_but_textedit()`). */
                if self.outliner_rename.is_some() {
                    use outliner_rename::Key;
                    let mut done = None;
                    if let Some(r) = self.outliner_rename.as_mut() {
                        let key = match code {
                            KeyCode::Enter | KeyCode::NumpadEnter => {
                                done = Some(true);
                                None
                            }
                            KeyCode::Escape => {
                                done = Some(false);
                                None
                            }
                            KeyCode::ArrowLeft => Some(Key::Left),
                            KeyCode::ArrowRight => Some(Key::Right),
                            KeyCode::Home => Some(Key::Home),
                            KeyCode::End => Some(Key::End),
                            KeyCode::Backspace => Some(Key::Backspace),
                            KeyCode::Delete => Some(Key::Delete),
                            KeyCode::KeyA if ctrl_k => Some(Key::SelectAll),
                            _ => None,
                        };
                        if let Some(k) = key {
                            r.key(k, shift_k);
                        } else if done.is_none() && !ctrl_k && !alt_k {
                            for ch in key_event.text.as_ref().map(|t| t.chars().collect::<Vec<_>>()).unwrap_or_default() {
                                r.key(Key::Char(ch), false);
                            }
                        }
                    }
                    if let Some(apply) = done {
                        if let Some(e) = self.outliner_rename.take() {
                            if apply && outliner_rename::commit(&mut self.scene, &e) {
                                scene_changed(&mut self.scene, &mut self.taa);
                            }
                        }
                    }
                    gfx.window.request_redraw();
                    return;
                }
                /* The F2 rename popup takes the keyboard while it is open. */
                if self.rename.is_some() {
                    let mut outcome = rename_popup::Outcome::Editing;
                    if let Some(r) = self.rename.as_mut() {
                        match code {
                            KeyCode::Enter | KeyCode::NumpadEnter => outcome = rename_popup::Outcome::Apply,
                            KeyCode::Escape => outcome = rename_popup::Outcome::Cancel,
                            KeyCode::Backspace => r.backspace(),
                            KeyCode::Delete => r.delete(),
                            KeyCode::ArrowLeft => r.left(),
                            KeyCode::ArrowRight => r.right(),
                            KeyCode::Home => r.home(),
                            KeyCode::End => r.end(),
                            _ => {
                                if !ctrl_k && !alt_k {
                                    if let Some(text) = key_event.text.as_ref() {
                                        r.insert(text);
                                    }
                                }
                            }
                        }
                    }
                    if outcome != rename_popup::Outcome::Editing {
                        let edit = self.rename.take();
                        let edit_modifier = edit.as_ref().and_then(|e| e.modifier);
                        if let (rename_popup::Outcome::Apply, Some(name)) = (outcome, edit.and_then(|e| e.result())) {
                            let changed = match edit_modifier {
                                Some(i) => self.scene.panel_rename_modifier(i, &name),
                                None => self.scene.rename_active(&name),
                            };
                            if changed {
                                scene_changed(&mut self.scene, &mut self.taa);
                            }
                        }
                    }
                    gfx.window.request_redraw();
                    return;
                }
                if self.field_edit.is_some() {
                    match code {
                        KeyCode::Enter | KeyCode::NumpadEnter => {
                            if let Some(e) = self.field_edit.take() {
                                if e.commit(&mut self.scene) {
                                    scene_changed(&mut self.scene, &mut self.taa);
                                }
                            }
                        }
                        /* Tab / Shift + Tab: apply this field and type into the next / previous one. */
                        KeyCode::Tab => {
                            if let Some(e) = self.field_edit.take() {
                                let changed = e.commit(&mut self.scene);
                                self.field_edit = transform_panel::Edit::start(&self.scene, e.field.step_for(&self.scene, if shift_k { -1 } else { 1 }));
                                if changed {
                                    scene_changed(&mut self.scene, &mut self.taa);
                                }
                            }
                        }
                        KeyCode::Escape => self.field_edit = None,
                        KeyCode::Backspace => {
                            if let Some(e) = self.field_edit.as_mut() {
                                e.backspace();
                            }
                        }
                        _ => {
                            if let (Some(e), Some(text)) = (self.field_edit.as_mut(), key_event.text.as_ref()) {
                                for c in text.chars() {
                                    e.push(c);
                                }
                            }
                        }
                    }
                    gfx.window.request_redraw();
                    return;
                }

                /* An open popup menu takes the keyboard: ESC closes it, other keys do nothing. */
                if self.add_menu.is_some() || self.brush_menu.is_some() || self.mod_menu.is_some() || self.add_mod_menu.is_some() {
                    if code == KeyCode::Escape {
                        self.add_menu = None;
                        self.brush_menu = None;
                        self.mod_menu = None;
                        self.add_mod_menu = None;
                        gfx.window.request_redraw();
                    }
                    return;
                }

                /* `Transform Modal Map` while a G / R / S transform runs: CONFIRM on RET / NUMPAD_ENTER / SPACE,
                 * CANCEL on ESC, the X / Y / Z axis constraints (Shift for the plane), numeric input. */
                if self.xform.is_some() {
                    let Some(layout) = self.layout.as_ref() else { return };
                    let (rw, rh) = (layout.main.size_x() + 1, layout.main.size_y() + 1);
                    let mval = [self.cursor[0] - layout.main.xmin, self.cursor[1] - layout.main.ymin];
                    match code {
                        KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space => {
                            self.xform = None;
                            set_status_title(&gfx.window, None);
                            gfx.window.request_redraw();
                            return;
                        }
                        KeyCode::Escape => {
                            if let Some(xf) = self.xform.take() {
                                xf.cancel(&mut self.scene);
                            }
                            scene_changed(&mut self.scene, &mut self.taa);
                            set_status_title(&gfx.window, None);
                            gfx.window.request_redraw();
                            return;
                        }
                        _ => {}
                    }
                    if let Some(xf) = self.xform.as_mut() {
                        match code {
                            KeyCode::KeyX => xf.set_axis(transform::Axis::X, shift_k),
                            KeyCode::KeyY => xf.set_axis(transform::Axis::Y, shift_k),
                            KeyCode::KeyZ => xf.set_axis(transform::Axis::Z, shift_k),
                            KeyCode::Backspace => {
                                xf.numeric_backspace();
                            }
                            _ => {
                                if let Some(text) = key_event.text.as_ref() {
                                    for c in text.chars() {
                                        xf.numeric_key(c);
                                    }
                                }
                            }
                        }
                        xf.snap = ctrl_k;
                        xf.precision = shift_k;
                        self.rv3d.update(&self.v3d, rw, rh);
                        let text = xf.apply(&mut self.scene, mval, &self.rv3d, rw, rh);
                        scene_changed(&mut self.scene, &mut self.taa);
                        set_status_title(&gfx.window, Some(&text));
                        gfx.window.request_redraw();
                    }
                    return;
                }

                /* `object.mode_set`: Tab toggles Edit Mode on the active mesh object. */
                if code == KeyCode::Tab && !ctrl_k && !shift_k && !alt_k && !key_event.repeat {
                    let cur = if self.edit_mode { object_mode::ObjectMode::Edit } else if self.sculpt.is_some() { object_mode::ObjectMode::Sculpt } else { object_mode::ObjectMode::Object };
                    let is_mesh = self.scene.active_object().is_some_and(|o| o.mesh().is_some());
                    self.edit_mode = object_mode::toggle_edit(cur, is_mesh) == object_mode::ObjectMode::Edit;
                    if self.edit_mode {
                        self.sculpt = None;
                    }
                    gfx.window.request_redraw();
                    return;
                }
                /* `object.mode_set`: Ctrl + Tab toggles Sculpt Mode on the active mesh object. */
                if code == KeyCode::Tab && ctrl_k && !shift_k && !alt_k && !key_event.repeat {
                    toggle_sculpt(&mut self.sculpt, &self.scene);
                    self.edit_mode &= self.sculpt.is_none(); /* Sculpt and Edit Mode are exclusive */
                    gfx.window.request_redraw();
                    return;
                }
                /* The "Sculpt" keymap over the viewport: X Draw, S Smooth, G Grab, I Inflate, Shift + T Flatten,
                 * P Pinch, Shift + C Crease; F / Shift + F radius / strength; Alt + X / Y / Z symmetry. The object
                 * operators that would act on the object instead (R, A, H, Delete, Shift + D) do nothing. */
                if self.sculpt.is_some() {
                    let cursor = self.cursor;
                    let in_main = match (self.layout.as_ref(), self.user.as_ref()) {
                        (Some(l), Some(u)) => area::in_view3d_main(u, l.main, cursor, self.sculpt.is_some()),
                        _ => false,
                    };
                    let sc = self.sculpt.as_mut().unwrap();
                    if sc.adjust.is_some() && matches!(code, KeyCode::Escape | KeyCode::Enter | KeyCode::NumpadEnter) {
                        sc.adjust_end(code == KeyCode::Escape);
                        gfx.window.request_redraw();
                        return;
                    }
                    if in_main && !key_event.repeat && !sc.in_stroke() {
                        let plain = !shift_k && !ctrl_k && !alt_k;
                        let shift_only = shift_k && !ctrl_k && !alt_k;
                        let alt_only = alt_k && !ctrl_k && !shift_k;
                        let mut handled = true;
                        match code {
                            KeyCode::KeyX if plain => sc.settings.brush = sculpt::Brush::Draw,
                            KeyCode::KeyS if plain => sc.settings.brush = sculpt::Brush::Smooth,
                            KeyCode::KeyG if plain => sc.settings.brush = sculpt::Brush::Grab,
                            KeyCode::KeyI if plain => sc.settings.brush = sculpt::Brush::Inflate,
                            KeyCode::KeyT if shift_only => sc.settings.brush = sculpt::Brush::Flatten,
                            KeyCode::KeyP if plain => sc.settings.brush = sculpt::Brush::Pinch,
                            KeyCode::KeyC if shift_only => sc.settings.brush = sculpt::Brush::Crease,
                            KeyCode::KeyC if plain => sc.settings.brush = sculpt::Brush::Clay,
                            KeyCode::KeyX if shift_only => sc.settings.brush = sculpt::Brush::Fill,
                            KeyCode::KeyS if shift_only => sc.settings.brush = sculpt::Brush::Scrape,
                            KeyCode::KeyM if plain => sc.settings.brush = sculpt::Brush::Mask,
                            KeyCode::KeyL if plain => sc.settings.brush = sculpt::Brush::Layer,
                            KeyCode::KeyK if plain => sc.settings.brush = sculpt::Brush::SnakeHook,
                            KeyCode::KeyN if plain => sc.settings.brush = sculpt::Brush::Nudge,
                            KeyCode::KeyB if shift_only => sc.settings.brush = sculpt::Brush::Blob,
                            /* `paint.mask_flood_fill`: Alt + M clears the mask, Ctrl + I inverts it. */
                            KeyCode::KeyM if alt_only => {
                                if self.scene.clear_mask_active() {
                                    scene_changed(&mut self.scene, &mut self.taa);
                                }
                            }
                            /* `sculpt.mask_filter`: Ctrl + Shift + G / H grow / shrink the mask, S smooths it, X sharpens
                             * it (Blender has these in the Mask menu, no keys). */
                            KeyCode::KeyG | KeyCode::KeyH | KeyCode::KeyS | KeyCode::KeyX if ctrl_k && shift_k && !alt_k => {
                                let filter = match code {
                                    KeyCode::KeyG => sculpt::MaskFilter::Grow,
                                    KeyCode::KeyH => sculpt::MaskFilter::Shrink,
                                    KeyCode::KeyS => sculpt::MaskFilter::Smooth,
                                    _ => sculpt::MaskFilter::Sharpen,
                                };
                                if self.scene.mask_filter_active(filter) {
                                    scene_changed(&mut self.scene, &mut self.taa);
                                }
                            }
                            KeyCode::KeyI if ctrl_k && !shift_k && !alt_k => {
                                if self.scene.invert_mask_active() {
                                    scene_changed(&mut self.scene, &mut self.taa);
                                }
                            }
                            /* Subdivides the base mesh once (Catmull-Clark), for meshes too coarse to sculpt. */
                            KeyCode::KeyD if ctrl_k && shift_k && !alt_k => {
                                if self.scene.subdivide_active_base() {
                                    scene_changed(&mut self.scene, &mut self.taa);
                                }
                            }
                            KeyCode::KeyF if plain => sc.start_adjust(false, cursor),
                            KeyCode::KeyF if shift_only => sc.start_adjust(true, cursor),
                            KeyCode::KeyX if alt_only => sc.toggle_symmetry(0),
                            KeyCode::KeyY if alt_only => sc.toggle_symmetry(1),
                            KeyCode::KeyZ if alt_only => sc.toggle_symmetry(2),
                            KeyCode::KeyR | KeyCode::KeyA | KeyCode::KeyH | KeyCode::Delete if plain => {}
                            KeyCode::KeyD if shift_only => {}
                            _ => handled = false,
                        }
                        if handled {
                            gfx.window.request_redraw();
                            return;
                        }
                    }
                }

                /* Object operators of the "Object Mode" keymap, over the viewport: G / R / S grab, rotate, scale;
                 * Alt + G / R / S clear the transform; Shift + D duplicate (then grab); X / Delete delete; H hide,
                 * Alt + H reveal; A / Alt + A / Ctrl + I select all / none / invert; Ctrl + Z undo, Ctrl + Shift + Z
                 * redo; Shift + A adds the next primitive at the 3D cursor; Ctrl + 0..5 the Subdivision Surface
                 * level; NUMPAD_PERIOD frames the selection. Ctrl + Alt + S / F set smooth / flat shading (the
                 * menu entries of Object > Shade Smooth / Flat have no key in Blender). */
                let in_main = match (self.layout.as_ref(), self.user.as_ref()) {
                    (Some(l), Some(u)) => area::in_view3d_main(u, l.main, self.cursor, self.sculpt.is_some()),
                    _ => false,
                };
                if in_main && !key_event.repeat {
                    let layout = self.layout.as_ref().unwrap();
                    let (rw, rh) = (layout.main.size_x() + 1, layout.main.size_y() + 1);
                    let mval = [self.cursor[0] - layout.main.xmin, self.cursor[1] - layout.main.ymin];
                    let plain = !shift_k && !ctrl_k && !alt_k;
                    let mut geometry = false;
                    let mut handled = true;
                    match code {
                        KeyCode::KeyG | KeyCode::KeyR | KeyCode::KeyS if plain => {
                            let (mode, name) = match code {
                                KeyCode::KeyG => (transform::Mode::Translate, "Grab"),
                                KeyCode::KeyR => (transform::Mode::Rotate, "Rotate"),
                                _ => (transform::Mode::Scale, "Scale"),
                            };
                            self.rv3d.update(&self.v3d, rw, rh);
                            self.xform = transform::Transform::begin_with_pivot(mode, &mut self.scene, mval, &self.rv3d, true, self.pivot_point, self.cursor3d);
                            if self.xform.is_some() {
                                set_status_title(&gfx.window, Some(name));
                            }
                        }
                        KeyCode::KeyG | KeyCode::KeyR | KeyCode::KeyS if alt_k && !shift_k && !ctrl_k => {
                            let kind = match code {
                                KeyCode::KeyG => scene::ClearKind::Location,
                                KeyCode::KeyR => scene::ClearKind::Rotation,
                                _ => scene::ClearKind::Scale,
                            };
                            geometry = self.scene.clear_transform(kind) > 0;
                        }
                        KeyCode::KeyD if shift_k && !ctrl_k && !alt_k => {
                            if self.scene.duplicate_selected() > 0 {
                                self.rv3d.update(&self.v3d, rw, rh);
                                /* The duplicate and the grab are one undo step. */
                                self.xform = transform::Transform::begin_with_pivot(transform::Mode::Translate, &mut self.scene, mval, &self.rv3d, false, self.pivot_point, self.cursor3d);
                                set_status_title(&gfx.window, Some("Duplicate"));
                                geometry = true;
                            }
                        }
                        KeyCode::KeyX | KeyCode::Delete if plain => geometry = self.scene.delete_selected() > 0,
                        KeyCode::KeyH if plain => geometry = self.scene.hide_selected() > 0,
                        KeyCode::KeyH if alt_k && !shift_k && !ctrl_k => geometry = self.scene.reveal_all() > 0,
                        KeyCode::KeyH if shift_k && !alt_k && !ctrl_k => geometry = self.scene.hide_unselected() > 0,
                        KeyCode::KeyA if plain => {
                            self.scene.select_all(scene::SelectAction::Select);
                        }
                        KeyCode::KeyA if alt_k && !shift_k && !ctrl_k => {
                            self.scene.select_all(scene::SelectAction::Deselect);
                        }
                        KeyCode::KeyI if ctrl_k && !shift_k && !alt_k => {
                            self.scene.select_all(scene::SelectAction::Invert);
                        }
                        KeyCode::KeyZ if ctrl_k && !alt_k => {
                            geometry = if shift_k { self.scene.redo() } else { self.scene.undo() };
                        }
                        KeyCode::KeyA if shift_k && !ctrl_k && !alt_k => {
                            /* `wm.call_menu` of `VIEW3D_MT_add`: the popup opens at the cursor. */
                            self.add_menu = Some(self.cursor);
                        }
                        KeyCode::Digit0 | KeyCode::Digit1 | KeyCode::Digit2 | KeyCode::Digit3 | KeyCode::Digit4 | KeyCode::Digit5
                            if ctrl_k && !shift_k && !alt_k =>
                        {
                            let level = match code {
                                KeyCode::Digit1 => 1,
                                KeyCode::Digit2 => 2,
                                KeyCode::Digit3 => 3,
                                KeyCode::Digit4 => 4,
                                KeyCode::Digit5 => 5,
                                _ => 0,
                            };
                            geometry = self.scene.set_subsurf(level) > 0;
                        }
                        /* Modifiers: Blender has no keys for them (they live in the properties editor), so these are
                         * Ctrl + Alt chords: X / Y / Z toggle a Mirror axis, + / - change the Array count and W toggles
                         * Solidify. */
                        KeyCode::KeyX | KeyCode::KeyY | KeyCode::KeyZ if ctrl_k && alt_k && !shift_k => {
                            let axis = match code {
                                KeyCode::KeyX => 0,
                                KeyCode::KeyY => 1,
                                _ => 2,
                            };
                            geometry = self.scene.toggle_mirror(axis) > 0;
                        }
                        KeyCode::Equal | KeyCode::NumpadAdd if ctrl_k && alt_k && !shift_k => {
                            geometry = self.scene.set_array(self.scene.array_count().max(1) + 1) > 0;
                        }
                        KeyCode::Minus | KeyCode::NumpadSubtract if ctrl_k && alt_k && !shift_k => {
                            let count = self.scene.array_count();
                            geometry = count > 1 && self.scene.set_array(count - 1) > 0;
                        }
                        KeyCode::KeyW if ctrl_k && alt_k && !shift_k => geometry = self.scene.toggle_solidify() > 0,
                        KeyCode::KeyB if ctrl_k && alt_k && !shift_k => geometry = self.scene.toggle_bevel() > 0,
                        /* Smooth (M), Simple Deform (T: Twist, then Taper, then Stretch, then off) and Weld (E). */
                        KeyCode::KeyM if ctrl_k && alt_k && !shift_k => geometry = self.scene.toggle_smooth_modifier() > 0,
                        KeyCode::KeyT if ctrl_k && alt_k && !shift_k => geometry = self.scene.cycle_simple_deform() > 0,
                        KeyCode::KeyE if ctrl_k && alt_k && !shift_k => geometry = self.scene.toggle_weld() > 0,
                        /* Displace (G) and Triangulate (R) modifiers; Bend is the second step of the Simple Deform cycle. */
                        KeyCode::KeyG if ctrl_k && alt_k && !shift_k => geometry = self.scene.toggle_displace() > 0,
                        KeyCode::KeyR if ctrl_k && alt_k && !shift_k => geometry = self.scene.toggle_triangulate_modifier() > 0,
                        /* Cast (C: Sphere, then Cylinder, then off) and Wireframe (Q) modifiers. */
                        KeyCode::KeyC if ctrl_k && alt_k && !shift_k => geometry = self.scene.cycle_cast() > 0,
                        KeyCode::KeyQ if ctrl_k && alt_k && !shift_k => geometry = self.scene.toggle_wireframe() > 0,
                        /* Decimate (V: ratio 0.5, 0.25, 0.1, off) and Edge Split (N) modifiers. */
                        KeyCode::KeyV if ctrl_k && alt_k && !shift_k => geometry = self.scene.cycle_decimate() > 0,
                        KeyCode::KeyN if ctrl_k && alt_k && !shift_k => geometry = self.scene.toggle_edge_split() > 0,
                        /* Screw (U) and Wave (K) modifiers. */
                        KeyCode::KeyU if ctrl_k && alt_k && !shift_k => geometry = self.scene.toggle_screw() > 0,
                        KeyCode::KeyK if ctrl_k && alt_k && !shift_k => geometry = self.scene.toggle_wave() > 0,
                        /* Laplacian Smooth (L) and Remesh in Blocks mode (I) modifiers. */
                        KeyCode::KeyL if ctrl_k && alt_k && !shift_k => geometry = self.scene.toggle_laplacian_smooth() > 0,
                        KeyCode::KeyI if ctrl_k && alt_k && !shift_k => geometry = self.scene.toggle_remesh() > 0,
                        /* Build modifier (A): reveals the faces from frame 1 over 100 frames. */
                        KeyCode::KeyA if ctrl_k && alt_k && !shift_k => geometry = self.scene.toggle_build() > 0,
                        /* Editing the stack (the active modifier is marked `>` in the viewport text): Ctrl + Alt + Up /
                         * Down select it, `]` / `[` raise / lower its main value, Page Up / Page Down move it in the
                         * stack and Ctrl + Shift + Alt + X removes it. */
                        KeyCode::ArrowDown if ctrl_k && alt_k && !shift_k => {
                            self.scene.select_modifier(1);
                        }
                        KeyCode::ArrowUp if ctrl_k && alt_k && !shift_k => {
                            self.scene.select_modifier(-1);
                        }
                        KeyCode::BracketRight if ctrl_k && alt_k && !shift_k => geometry = self.scene.adjust_modifier(1) > 0,
                        KeyCode::BracketLeft if ctrl_k && alt_k && !shift_k => geometry = self.scene.adjust_modifier(-1) > 0,
                        KeyCode::PageDown if ctrl_k && alt_k && !shift_k => geometry = self.scene.move_modifier(1) > 0,
                        KeyCode::PageUp if ctrl_k && alt_k && !shift_k => geometry = self.scene.move_modifier(-1) > 0,
                        KeyCode::KeyX if ctrl_k && alt_k && shift_k => geometry = self.scene.remove_modifier() > 0,
                        /* `mesh.subdivide` (one cut, no smoothing) and `mesh.separate` by loose parts (P). */
                        KeyCode::KeyD if ctrl_k && alt_k && !shift_k => geometry = self.scene.subdivide_simple() > 0,
                        /* `wm.call_panel` of `TOPBAR_PT_name` on F2: rename the active object. */
                        KeyCode::F2 if plain && self.layout.as_ref().zip(self.user.as_ref()).and_then(|(l, u)| outliner_draw::window_rect(u, l.outliner)).is_some_and(|w| w.isect_pt_v(self.cursor)) => {
                            /* `outliner.item_rename` with `use_active` while the cursor is over the outliner. */
                            self.outliner_rename = outliner_rename::start_active(&self.scene);
                        }
                        KeyCode::F2 if plain => {
                            if let Some(name) = self.scene.active_object().map(|o| o.name.clone()) {
                                self.rename = Some(rename_popup::Rename::open(self.cursor, &name));
                            }
                        }
                        KeyCode::KeyP if plain => geometry = self.scene.separate_loose() > 0,
                        /* `object.parent_set` (Ctrl + P, Object with Keep Transform), `object.parent_clear` (Alt + P: Clear
                         * and Keep Transformation, Shift + Alt + P: Clear Parent) and `object.select_hierarchy`
                         * (Ctrl + [ parent, Ctrl + ] child, with Shift to extend the selection). */
                        KeyCode::KeyP if ctrl_k && !shift_k && !alt_k => geometry = self.scene.parent_set() > 0,
                        KeyCode::KeyP if alt_k && !shift_k && !ctrl_k => geometry = self.scene.parent_clear(true) > 0,
                        KeyCode::KeyP if alt_k && shift_k && !ctrl_k => geometry = self.scene.parent_clear(false) > 0,
                        KeyCode::BracketLeft | KeyCode::BracketRight if ctrl_k && !alt_k => {
                            self.scene.select_hierarchy(code == KeyCode::BracketLeft, shift_k);
                        }
                        /* `mesh.convex_hull` (Ctrl + Alt + H). */
                        KeyCode::KeyH if ctrl_k && alt_k && !shift_k => geometry = self.scene.convex_hull() > 0,
                        /* `transform.mirror` (Ctrl + M then an axis in Blender) as one chord per axis: Ctrl + Shift + X / Y / Z. */
                        KeyCode::KeyX | KeyCode::KeyY | KeyCode::KeyZ if ctrl_k && shift_k && !alt_k => {
                            let axis = match code {
                                KeyCode::KeyX => 0,
                                KeyCode::KeyY => 1,
                                _ => 2,
                            };
                            geometry = self.scene.mirror_objects(axis) > 0;
                        }
                        /* Object > Transform > Align Objects (Shift + Alt + 1 / 2 / 3 aligns the centers on X / Y / Z to the
                         * active object, add Ctrl for the negative sides), Randomize Transform (Ctrl + Shift + R). */
                        KeyCode::Digit1 | KeyCode::Digit2 | KeyCode::Digit3 if shift_k && alt_k => {
                            let axis = match code {
                                KeyCode::Digit1 => 0,
                                KeyCode::Digit2 => 1,
                                _ => 2,
                            };
                            let mode = if ctrl_k { scene::AlignMode::NegativeSides } else { scene::AlignMode::Centers };
                            geometry = self.scene.align_objects(axis, mode, scene::AlignTo::Active) > 0;
                        }
                        KeyCode::KeyR if ctrl_k && shift_k && !alt_k => {
                            geometry = self.scene.randomize_transform(&scene::Randomize::default()) > 0;
                        }
                        /* `object.select_grouped` by relationship: Ctrl + Shift + G Children, Shift + Alt + G Immediate
                         * Children, Ctrl + Shift + Alt + G Parent, Ctrl + Shift + Alt + S Siblings. */
                        KeyCode::KeyG if ctrl_k && shift_k && !alt_k => {
                            self.scene.select_grouped(scene::GroupKind::Children);
                        }
                        KeyCode::KeyG if shift_k && alt_k && !ctrl_k => {
                            self.scene.select_grouped(scene::GroupKind::ChildrenImmediate);
                        }
                        KeyCode::KeyG if ctrl_k && shift_k && alt_k => {
                            self.scene.select_grouped(scene::GroupKind::Parent);
                        }
                        KeyCode::KeyS if ctrl_k && shift_k && alt_k => {
                            self.scene.select_grouped(scene::GroupKind::Siblings);
                        }
                        /* `tool_settings.transform_pivot_point`: Period cycles Median Point, 3D Cursor and Individual
                         * Origins (Blender's pivot pie). Ctrl + Shift + C is Cursor to Active of the snap pie. */
                        KeyCode::Period if plain => self.pivot_point = self.pivot_point.next(),
                        KeyCode::KeyC if ctrl_k && shift_k && !alt_k => {
                            if let Some(p) = self.scene.active_origin() {
                                self.cursor3d = p;
                            }
                        }
                        /* Snap > Selection to Active (Ctrl + Shift + A), Set Origin > Origin to Bounds Center (Ctrl + Shift + Alt + B),
                         * Parent > Clear Parent Inverse (Ctrl + Alt + P), Select Mirror (Ctrl + Shift + M, add Alt to extend). */
                        KeyCode::KeyA if ctrl_k && shift_k && !alt_k => geometry = self.scene.selection_to_active() > 0,
                        KeyCode::KeyB if ctrl_k && shift_k && alt_k => geometry = self.scene.origin_to_bounds_center() > 0,
                        KeyCode::KeyP if ctrl_k && alt_k && !shift_k => geometry = self.scene.parent_clear_inverse() > 0,
                        KeyCode::KeyM if ctrl_k && shift_k && !alt_k => {
                            self.scene.select_mirror(false);
                        }
                        KeyCode::KeyM if shift_k && alt_k && !ctrl_k => {
                            self.scene.select_mirror(true);
                        }
                        /* Array modifier: Ctrl + Alt + O cycles the offset (relative X / Y / Z, then 1 m constant along
                         * X / Y / Z), Ctrl + Alt + J toggles Merge (1 mm). */
                        KeyCode::KeyO if ctrl_k && alt_k && !shift_k => geometry = self.scene.cycle_array_offset() > 0,
                        KeyCode::KeyJ if ctrl_k && alt_k && !shift_k => geometry = self.scene.toggle_array_merge() > 0,
                        /* Ctrl + Alt + `'` cycles the Displace direction (Normal, X, Y, Z). */
                        KeyCode::Quote if ctrl_k && alt_k && !shift_k => geometry = self.scene.cycle_displace_direction() > 0,
                        /* Ctrl + Alt + `;` toggles Solidify Even Thickness. */
                        KeyCode::Semicolon if ctrl_k && alt_k && !shift_k => geometry = self.scene.toggle_solidify_even() > 0,
                        /* Ctrl + Alt + `,` cycles Array Fit Length (Fixed Count, 2, 4, 8 m). */
                        KeyCode::Comma if ctrl_k && alt_k && !shift_k => geometry = self.scene.cycle_array_fit() > 0,
                        /* `object.select_linked`: Shift + L Object Data, Shift + Alt + L Modifiers. */
                        KeyCode::KeyL if shift_k && !ctrl_k && !alt_k => {
                            self.scene.select_linked_data();
                        }
                        KeyCode::KeyL if shift_k && alt_k && !ctrl_k => {
                            self.scene.select_linked_modifiers();
                        }
                        /* Bevel modifier: Ctrl + Alt + `.` cycles the segments (1, 2, 3, 4, 6, 8). */
                        KeyCode::Period if ctrl_k && alt_k && !shift_k => geometry = self.scene.cycle_bevel_segments() > 0,
                        /* Set Origin > Origin to Center of Mass (Volume). */
                        KeyCode::KeyM if ctrl_k && shift_k && alt_k => geometry = self.scene.origin_to_center_of_mass() > 0,
                        /* `object.select_grouped` by type (Shift + G) and `object.select_random` (Shift + Alt + R). */
                        KeyCode::KeyG if shift_k && !ctrl_k && !alt_k => {
                            self.scene.select_grouped_type();
                        }
                        KeyCode::KeyR if shift_k && alt_k && !ctrl_k => {
                            self.scene.select_random(0.5);
                        }
                        /* The Shift + S snap pie of Blender, as one chord per entry: Shift + S is Cursor to Selected,
                         * Ctrl + Shift + S Selection to Cursor, Shift + Alt + S Cursor to World Origin and
                         * Shift + Alt + C Origin to 3D Cursor. */
                        KeyCode::KeyS if shift_k && !ctrl_k && !alt_k => {
                            if let Some(p) = self.scene.cursor_target() {
                                self.cursor3d = p;
                            }
                        }
                        KeyCode::KeyS if shift_k && ctrl_k && !alt_k => geometry = self.scene.selection_to_cursor(self.cursor3d) > 0,
                        KeyCode::KeyS if shift_k && alt_k && !ctrl_k => {
                            self.cursor3d = [0.0; 3];
                            self.cursor3d_quat = [1.0, 0.0, 0.0, 0.0];
                        }
                        KeyCode::KeyC if shift_k && alt_k && !ctrl_k => geometry = self.scene.origin_to_point(self.cursor3d) > 0,
                        /* Object > Join, Apply > All Transforms, Convert to Mesh (applies the modifiers), Set Origin >
                         * Origin to Geometry (the Shift + Ctrl + Alt + C pie of Blender). */
                        /* `view3d.copybuffer` / `view3d.pastebuffer` (Ctrl + C / Ctrl + V). */
                        KeyCode::KeyC if ctrl_k && !shift_k && !alt_k => {
                            if let Some(c) = self.scene.copy_selected() {
                                self.clipboard = c;
                            }
                        }
                        KeyCode::KeyV if ctrl_k && !shift_k && !alt_k => geometry = self.scene.paste(&self.clipboard) > 0,
                        KeyCode::KeyJ if ctrl_k && !shift_k && !alt_k => geometry = self.scene.join_selected() > 0,
                        /* Modifier > Apply (Ctrl + Shift + Alt + A): bakes the active modifier into the mesh. */
                        KeyCode::KeyA if ctrl_k && shift_k && alt_k => geometry = self.scene.apply_active_modifier(),
                        /* Link/Transfer Data > Copy Modifiers (Ctrl + L): the active object's stack goes to the other selected meshes. */
                        KeyCode::KeyL if ctrl_k && !shift_k && !alt_k => geometry = self.scene.copy_modifiers_to_selected() > 0,
                        /* Modifier > Duplicate (Ctrl + Shift + Alt + D): a copy right after the active modifier. */
                        KeyCode::KeyD if ctrl_k && shift_k && alt_k => geometry = self.scene.duplicate_active_modifier(),
                        KeyCode::KeyA if ctrl_k && !shift_k && !alt_k => geometry = self.scene.apply_transforms() > 0,
                        KeyCode::KeyC if alt_k && !shift_k && !ctrl_k => geometry = self.scene.apply_modifiers() > 0,
                        KeyCode::KeyC if ctrl_k && shift_k && alt_k => geometry = self.scene.origin_to_geometry() > 0,
                        /* Mesh editing keys of Blender's edit mode, run on the whole selected mesh. */
                        KeyCode::KeyN if alt_k && !shift_k && !ctrl_k => geometry = self.scene.flip_normals() > 0,
                        KeyCode::KeyN if shift_k && !ctrl_k && !alt_k => geometry = self.scene.recalc_normals() > 0,
                        KeyCode::KeyT if ctrl_k && !shift_k && !alt_k => geometry = self.scene.triangulate() > 0,
                        KeyCode::KeyM if alt_k && !shift_k && !ctrl_k => geometry = self.scene.merge_by_distance(0.0001) > 0,
                        KeyCode::KeyS if ctrl_k && alt_k && !shift_k => geometry = self.scene.set_smooth(true) > 0,
                        KeyCode::KeyF if ctrl_k && alt_k && !shift_k => geometry = self.scene.set_smooth(false) > 0,
                        /* `view3d.localview` (Numpad /): only the selected objects stay visible, framed in the view. */
                        KeyCode::NumpadDivide if plain => {
                            let entering = !self.scene.in_local_view();
                            if self.scene.toggle_local_view() {
                                geometry = true;
                                if entering {
                                    if let Some(bounds) = self.scene.bounds(false) {
                                        smooth_finish(&mut self.rv3d, &mut self.smooth);
                                        let smooth_src = self.rv3d.state_backup();
                                        let (min, max) = self.rv3d.visible_bounds_of(Some(bounds));
                                        self.rv3d.view_from_minmax(&self.v3d, rw, rh, min, max);
                                        self.smooth = self.rv3d.smooth_view_begin(smooth_src, view3d::SMOOTH_VIEWTX).map(|sms| (sms, std::time::Instant::now()));
                                    }
                                }
                            }
                        }
                        /* `object.transform_apply` with a single option, and Selection to Grid (1 m). */
                        /* `object.parent_no_inverse_set` (Ctrl + Shift + P, Alt added: Keep Transform), `object.make_links_data`
                         * Object Data (Ctrl + Shift + L) and Cursor to Grid (Ctrl + Shift + Alt + U). */
                        KeyCode::KeyP if ctrl_k && shift_k && !alt_k => geometry = self.scene.parent_set_no_inverse(false) > 0,
                        KeyCode::KeyP if ctrl_k && shift_k && alt_k => geometry = self.scene.parent_set_no_inverse(true) > 0,
                        KeyCode::KeyL if ctrl_k && shift_k && !alt_k => geometry = self.scene.link_object_data() > 0,
                        KeyCode::KeyU if ctrl_k && shift_k && alt_k => self.cursor3d = scene::snap_to_grid(self.cursor3d, 1.0),
                        KeyCode::KeyL if ctrl_k && shift_k && alt_k => geometry = self.scene.apply_transform(scene::ApplyMask::LOCATION) > 0,
                        KeyCode::KeyR if ctrl_k && shift_k && alt_k => geometry = self.scene.apply_transform(scene::ApplyMask::ROTATION) > 0,
                        KeyCode::KeyZ if ctrl_k && shift_k && alt_k => geometry = self.scene.apply_transform(scene::ApplyMask::SCALE) > 0,
                        KeyCode::KeyT if ctrl_k && shift_k && alt_k => geometry = self.scene.snap_selected_to_grid(1.0) > 0,
                        KeyCode::NumpadDecimal if plain => {
                            if let Some(bounds) = self.scene.bounds(true) {
                                smooth_finish(&mut self.rv3d, &mut self.smooth);
                                let smooth_src = self.rv3d.state_backup();
                                let (min, max) = self.rv3d.visible_bounds_of(Some(bounds));
                                self.rv3d.view_from_minmax(&self.v3d, rw, rh, min, max);
                                self.smooth = self.rv3d.smooth_view_begin(smooth_src, view3d::SMOOTH_VIEWTX).map(|sms| (sms, std::time::Instant::now()));
                            }
                        }
                        _ => handled = false,
                    }
                    if handled {
                        if geometry {
                            scene_changed(&mut self.scene, &mut self.taa);
                        }
                        gfx.window.request_redraw();
                        return;
                    }
                }

                /* `screen.animation_cancel` (restores the start frame) and the scrub CANCEL: ESC. */
                if code == KeyCode::Escape {
                    let mut handled = false;
                    if let Some(play) = self.playing.take() {
                        self.frame = Some(play.start_frame);
                        handled = true;
                    }
                    if self.scrub_drag {
                        self.scrub_drag = false;
                        self.frame = Some(self.scrub_init);
                        handled = true;
                    }
                    if handled {
                        gfx.window.request_redraw();
                        return;
                    }
                }
                /* `screen.animation_play` on SPACE (forward) and Shift + Ctrl + SPACE (`reverse`): toggles playback. */
                if code == KeyCode::Space && !key_event.repeat && !alt_k && ctrl_k == shift_k {
                    if self.playing.take().is_none() {
                        let interval = std::time::Duration::from_secs_f64(1.0 / PLAY_FPS);
                        self.playing = Some(PlayState {
                            reverse: shift_k && ctrl_k,
                            start_frame: self.frame.unwrap_or(1),
                            next: std::time::Instant::now() + interval,
                        });
                    }
                    gfx.window.request_redraw();
                    return;
                }
                /* `screen.time_jump` on Ctrl + LEFT_ARROW (`backward`) / RIGHT_ARROW (`frame_jump_delta_exec()`,
                 * screen_ops.cc:4022-4078): the startup scene's `time_jump_delta` is 1 with the unit
                 * `SCE_TIME_JUMP_SECOND`, so the step is `round(1 * frs_sec / frs_sec_base)` frames (sub-frames are
                 * off by default), and the frame is clamped at `MINAFRAME`. */
                if matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight) && ctrl_k && !shift_k && !alt_k {
                    let step = PLAY_FPS.round() as i32;
                    let delta = if code == KeyCode::ArrowRight { step } else { -step };
                    self.frame = Some((self.frame.unwrap_or(1) + delta).max(0));
                    gfx.window.request_redraw();
                    return;
                }
                /* `screen.frame_offset` on LEFT_ARROW / RIGHT_ARROW (delta -1 / +1) and `screen.frame_jump` on
                 * Shift + LEFT_ARROW / RIGHT_ARROW (start / end of the scene range). */
                if matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight) && !ctrl_k && !alt_k {
                    let right = code == KeyCode::ArrowRight;
                    if shift_k {
                        if !key_event.repeat {
                            let range = frame_range::FrameRange::default();
                            self.frame = Some(range.jump(right));
                        }
                    } else {
                        self.frame = Some(frame_range::offset(self.frame.unwrap_or(1), if right { 1 } else { -1 }));
                    }
                    gfx.window.request_redraw();
                    return;
                }
                /* `view3d.view_all` with `center = True` on Shift + C (`view3d_all_exec()`): the 3D cursor goes
                 * back to the origin (`View3DCursor::set_matrix(identity)`: location and rotation), then the
                 * view frames the bounds of the visible objects extended to include the origin
                 * (`minmax_v3v3_v3(min, max, float3(0))`) with `ED_view3d_smooth_view()`. Not over the header region. */
                if code == KeyCode::KeyC && self.modifiers.shift_key() && !self.modifiers.control_key() && !self.modifiers.alt_key() && !key_event.repeat {
                    let (Some(layout), Some(user)) = (self.layout.as_ref(), self.user.as_ref()) else { return };
                    if area::in_view3d_main(user, layout.main, self.cursor, self.sculpt.is_some()) {
                        self.cursor3d = [0.0; 3];
                        let (rw, rh) = (layout.main.size_x() + 1, layout.main.size_y() + 1);
                        smooth_finish(&mut self.rv3d, &mut self.smooth);
                        let smooth_src = self.rv3d.state_backup();
                        let (bounds_min, bounds_max) = self.rv3d.visible_bounds_of(self.scene.bounds(false));
                        let min = bounds_min.map(|v| v.min(0.0));
                        let max = bounds_max.map(|v| v.max(0.0));
                        self.rv3d.view_from_minmax(&self.v3d, rw, rh, min, max);
                        self.smooth = self.rv3d.smooth_view_begin(smooth_src, view3d::SMOOTH_VIEWTX).map(|sms| (sms, std::time::Instant::now()));
                        self.cursor3d_quat = [1.0, 0.0, 0.0, 0.0];
                        gfx.window.request_redraw();
                    }
                    return;
                }
                /* `view3d.view_center_cursor` on Alt + HOME: `ofs` becomes the negated 3D cursor
                 * location, animated with `ED_view3d_smooth_view()`. Not over the header region. */
                if code == KeyCode::Home && self.modifiers.alt_key() && !self.modifiers.shift_key() && !self.modifiers.control_key() && !key_event.repeat {
                    let (Some(layout), Some(user)) = (self.layout.as_ref(), self.user.as_ref()) else { return };
                    if area::in_view3d_main(user, layout.main, self.cursor, self.sculpt.is_some()) {
                        /* `ED_view3d_smooth_view_force_finish()` precedes `ED_view3d_smooth_view()` in `viewcenter_cursor_exec()`. */
                        smooth_finish(&mut self.rv3d, &mut self.smooth);
                        let smooth_src = self.rv3d.state_backup();
                        self.rv3d.center_cursor(self.cursor3d);
                        self.smooth = self.rv3d.smooth_view_begin(smooth_src, view3d::SMOOTH_VIEWTX).map(|sms| (sms, std::time::Instant::now()));
                        gfx.window.request_redraw();
                    }
                    return;
                }
                /* `view3d.view_all` on HOME and Ctrl + HOME (one region, so `use_all_regions` is the same),
                 * `center = False`: `ED_view3d_smooth_view_force_finish()`, then the view frames the bounds
                 * of the visible objects with `ED_view3d_smooth_view()`. Not over the header region. */
                if code == KeyCode::Home && !self.modifiers.alt_key() && !self.modifiers.shift_key() && !key_event.repeat {
                    let (Some(layout), Some(user)) = (self.layout.as_ref(), self.user.as_ref()) else { return };
                    if area::in_view3d_main(user, layout.main, self.cursor, self.sculpt.is_some()) {
                        let (rw, rh) = (layout.main.size_x() + 1, layout.main.size_y() + 1);
                        smooth_finish(&mut self.rv3d, &mut self.smooth);
                        let smooth_src = self.rv3d.state_backup();
                        let (min, max) = self.rv3d.visible_bounds_of(self.scene.bounds(false));
                        self.rv3d.view_from_minmax(&self.v3d, rw, rh, min, max);
                        self.smooth = self.rv3d.smooth_view_begin(smooth_src, view3d::SMOOTH_VIEWTX).map(|sms| (sms, std::time::Instant::now()));
                        gfx.window.request_redraw();
                    }
                    return;
                }
                /* `view3d.select_box` on B (`WM_gesture_box_invoke()`): the next drag extends the selection (Ctrl
                 * subtracts). Not over the header region. */
                if code == KeyCode::KeyB && !self.modifiers.shift_key() && !self.modifiers.control_key() && !self.modifiers.alt_key() && !key_event.repeat {
                    let (Some(layout), Some(user)) = (self.layout.as_ref(), self.user.as_ref()) else { return };
                    if area::in_view3d_main(user, layout.main, self.cursor, self.sculpt.is_some()) && self.xform.is_none() {
                        self.box_select = Some(BoxSelect { armed: true, start: None, cur: [0, 0], dragging: false, mode: select_box::BoxMode::Add });
                    }
                    return;
                }
                /* `view3d.zoom_border` on Shift + B (`WM_gesture_box_invoke()`); not over the header region. */
                if code == KeyCode::KeyB && self.modifiers.shift_key() && !self.modifiers.control_key() && !self.modifiers.alt_key() && !key_event.repeat {
                    let (Some(layout), Some(user)) = (self.layout.as_ref(), self.user.as_ref()) else { return };
                    if area::in_view3d_main(user, layout.main, self.cursor, self.sculpt.is_some()) {
                        self.zoom_border = Some(ZoomBorder { start: None, clip: false });
                    }
                    return;
                }
                /* `view3d.clip_border` on Alt + B (`view3d_clipping_invoke()`): turns an active clipping border
                 * off, else starts the box gesture (`WM_gesture_box_invoke()`). Works in wire and solid shading,
                 * both always available here. Not over the header region. */
                if code == KeyCode::KeyB && self.modifiers.alt_key() && !self.modifiers.shift_key() && !self.modifiers.control_key() && !key_event.repeat {
                    let (Some(layout), Some(user)) = (self.layout.as_ref(), self.user.as_ref()) else { return };
                    if area::in_view3d_main(user, layout.main, self.cursor, self.sculpt.is_some()) {
                        if self.rv3d.clip.is_some() {
                            self.rv3d.clip = None;
                            self.taa = taa::TaaState::default();
                            gfx.window.request_redraw();
                        } else {
                            self.zoom_border = Some(ZoomBorder { start: None, clip: true });
                        }
                    }
                    return;
                }
                /* `wm.context_toggle` of `space_data.overlay.show_overlays` on Shift + Alt + Z (the "3D View" keymap):
                 * flips `V3D_HIDE_OVERLAYS`. With it set the overlay engine draws no grid (`Grid::init()`,
                 * overlay_grid.hh:139), no 3D cursor (`is_cursor_visible_3d()`, overlay_cursor.hh:147) and
                 * `view3d_draw_region_info()` (view3d_draw.cc:1602) no info text. The wireframe of the wire shading
                 * stays. Not over the header region. */
                if code == KeyCode::KeyZ && shift_k && alt_k && !ctrl_k && !key_event.repeat {
                    let (Some(layout), Some(user)) = (self.layout.as_ref(), self.user.as_ref()) else { return };
                    if area::in_view3d_main(user, layout.main, self.cursor, self.sculpt.is_some()) {
                        self.overlays_hidden = !self.overlays_hidden;
                        gfx.window.request_redraw();
                    }
                    return;
                }
                /* The statistics overlay (Shift + Alt + O): counts of the visible objects under the info text. */
                if code == KeyCode::KeyO && shift_k && alt_k && !ctrl_k && !key_event.repeat {
                    self.show_stats = !self.show_stats;
                    gfx.window.request_redraw();
                    return;
                }
                /* `view3d.toggle_shading` with `type = WIREFRAME` on Shift + Z: switches between wire and solid
                 * shading (`prev_type_wire` is solid, the only other type modelled). Not over the header region.
                 * Entering wire drops the solid depth buffer; leaving it restarts the TAA accumulation. */
                if code == KeyCode::KeyZ && self.modifiers.shift_key() && !self.modifiers.control_key() && !self.modifiers.alt_key() && !key_event.repeat {
                    let (Some(layout), Some(user)) = (self.layout.as_ref(), self.user.as_ref()) else { return };
                    if area::in_view3d_main(user, layout.main, self.cursor, self.sculpt.is_some()) {
                        self.wire = !self.wire;
                        if self.wire {
                            self.scene_depth = Vec::new();
                        } else {
                            self.taa = taa::TaaState::default();
                        }
                        gfx.window.request_redraw();
                    }
                    return;
                }
                /* `view3d.toggle_xray` on Alt + Z: flips `V3D_SHADING_XRAY` of the solid shading (the default
                 * scene has no active object in edit or pose mode). Not over the header region. */
                if code == KeyCode::KeyZ && self.modifiers.alt_key() && !self.modifiers.shift_key() && !self.modifiers.control_key() && !key_event.repeat {
                    let (Some(layout), Some(user)) = (self.layout.as_ref(), self.user.as_ref()) else { return };
                    if area::in_view3d_main(user, layout.main, self.cursor, self.sculpt.is_some()) {
                        /* In wire shading the operator flips `V3D_SHADING_XRAY_WIREFRAME`, which is not modelled. */
                        if !self.wire {
                            self.xray = !self.xray;
                        }
                        gfx.window.request_redraw();
                    }
                    return;
                }
                if code == KeyCode::Escape {
                    /* `WM_gesture_box_modal()`: ESC cancels the box gesture. */
                    self.zoom_border = None;
                    self.box_select = None;
                    /* Transform cancel: the cursor goes back to where the drag started. */
                    if let Some(drag) = self.cursor_drag.take() {
                        if drag.active {
                            self.cursor3d = drag.loc;
                            gfx.window.request_redraw();
                        }
                    }
                    /* `VIEW_CANCEL`: `ViewOpsData::state_restore()` puts back `ofs`, `dist` and `viewquat`. */
                    if let Some((quat, dist, ofs, ortho, view, roll)) = self.nav_init.take() {
                        self.rv3d.viewquat = quat;
                        self.rv3d.dist = dist;
                        self.rv3d.ofs = ofs;
                        self.rv3d.ortho = ortho;
                        self.rv3d.view = view;
                        self.rv3d.view_axis_roll = roll;
                        self.orbit_prev = None;
                        self.pan_prev = None;
                        self.zoom_init = None;
                        self.dolly_init = None;
                        gfx.window.request_redraw();
                    }
                    return;
                }
                let m = self.modifiers;
                if m.alt_key() {
                    return;
                }
                let shift = m.shift_key();
                let ctrl = m.control_key();
                let zoom = match (code, ctrl) {
                    (KeyCode::NumpadAdd, false) | (KeyCode::Equal, true) => Some(1),
                    (KeyCode::NumpadSubtract, false) | (KeyCode::Minus, true) => Some(-1),
                    _ => None,
                };
                /* `viewpan_invoke_impl()`: PANRIGHT x = -32, PANLEFT x = 32, PANUP y = -25, PANDOWN y = 25. */
                let pan = match (code, ctrl) {
                    (KeyCode::Numpad6, true) => Some((-32, 0)),
                    (KeyCode::Numpad4, true) => Some((32, 0)),
                    (KeyCode::Numpad8, true) => Some((0, -25)),
                    (KeyCode::Numpad2, true) => Some((0, 25)),
                    _ => None,
                };
                /* `view3d.view_orbit`: NUMPAD_4 / 6 turn about Z (ORBITLEFT / ORBITRIGHT), NUMPAD_8 / 2
                 * about the view X axis (ORBITUP / ORBITDOWN), by `U.pad_rot_angle`; NUMPAD_9 turns by
                 * 180 degrees about Z (`angle = pi`, ORBITRIGHT). Tuple is (about_z, signed angle). */
                let orbit = match (code, ctrl) {
                    (KeyCode::Numpad4, false) => Some((true, view3d::PAD_ROT_ANGLE)),
                    (KeyCode::Numpad6, false) => Some((true, -view3d::PAD_ROT_ANGLE)),
                    (KeyCode::Numpad8, false) => Some((false, view3d::PAD_ROT_ANGLE)),
                    (KeyCode::Numpad2, false) => Some((false, -view3d::PAD_ROT_ANGLE)),
                    (KeyCode::Numpad9, false) if !key_event.repeat => Some((true, -std::f32::consts::PI)),
                    _ => None,
                };
                /* `view3d.view_roll`: Shift + NUMPAD_4 / 6 (LEFT / RIGHT); the other keys need Shift released. */
                let roll = match (code, shift && !ctrl) {
                    (KeyCode::Numpad4, true) => Some(-1.0),
                    (KeyCode::Numpad6, true) => Some(1.0),
                    _ => None,
                };
                /* `view3d.dolly` with `delta = +1 / -1`: Shift + NUMPAD_PLUS / NUMPAD_MINUS. */
                let dolly = match (code, shift, ctrl) {
                    (KeyCode::NumpadAdd, true, false) | (KeyCode::Equal, true, true) => Some(1),
                    (KeyCode::NumpadSubtract, true, false) | (KeyCode::Minus, true, true) => Some(-1),
                    _ => None,
                };
                /* `view3d.view_persportho` on NUMPAD_5 without modifiers. */
                let persp_toggle = code == KeyCode::Numpad5 && !shift && !ctrl && !key_event.repeat;
                /* `view3d.view_axis` on NUMPAD_1 / 3 / 7 (Ctrl for the opposite view). With Shift the
                 * keymap sets `align_active`, which only changes the result when there is an active
                 * object; the default scene has none, so the plain axis view is used. */
                let axis_view = match (code, ctrl) {
                    (KeyCode::Numpad1, false) => Some(view3d::View::Front),
                    (KeyCode::Numpad1, true) => Some(view3d::View::Back),
                    (KeyCode::Numpad3, false) => Some(view3d::View::Right),
                    (KeyCode::Numpad3, true) => Some(view3d::View::Left),
                    (KeyCode::Numpad7, false) => Some(view3d::View::Top),
                    (KeyCode::Numpad7, true) => Some(view3d::View::Bottom),
                    _ => None,
                };
                /* The `view3d.view_axis` keymap items have no `repeat`, so auto-repeat events are ignored. */
                let axis_view = if key_event.repeat { None } else { axis_view };
                let (zoom, pan, orbit) = if shift { (None, None, None) } else { (zoom, pan, orbit) };
                if zoom.is_none() && pan.is_none() && orbit.is_none() && roll.is_none() && dolly.is_none() && !persp_toggle && axis_view.is_none() {
                    return;
                }
                let (Some(layout), Some(user)) = (self.layout.as_ref(), self.user.as_ref()) else { return };
                if !area::in_view3d_main(user, layout.main, self.cursor, self.sculpt.is_some()) {
                    return;
                }
                /* `ED_view3d_smooth_view_force_finish()` of `view3d.view_orbit`, `view_roll`, `dolly`,
                 * `view_axis` and `view_persportho` (`viewpersportho_exec()`); zoom and pan don't call it. */
                if orbit.is_some() || roll.is_some() || dolly.is_some() || axis_view.is_some() || persp_toggle {
                    smooth_finish(&mut self.rv3d, &mut self.smooth);
                }
                let smooth_src = self.rv3d.state_backup();
                if let Some(step) = zoom {
                    self.rv3d.zoom_step(&self.v3d, step);
                }
                if let Some((dx, dy)) = pan {
                    /* `viewmove_apply()`: `init.zfac` is taken at `-ofs`, like `view3d.move`. */
                    let (rw, rh) = (layout.main.size_x() + 1, layout.main.size_y() + 1);
                    let ofs = self.rv3d.ofs;
                    let zfac = self.rv3d.calc_zfac([-ofs[0], -ofs[1], -ofs[2]]);
                    self.rv3d.pan(rw, rh, zfac, dx, dy);
                }
                if let Some((about_z, angle)) = orbit {
                    self.rv3d.orbit_step(about_z, angle);
                }
                if let Some(sign) = roll {
                    self.rv3d.roll_step(sign * view3d::PAD_ROT_ANGLE);
                }
                if let Some(delta) = dolly {
                    self.rv3d.dolly_step(delta);
                }
                if persp_toggle {
                    self.rv3d.toggle_persp_ortho();
                }
                if let Some(view) = axis_view {
                    self.rv3d.view_axis(view);
                }
                /* `ED_view3d_smooth_view()` of `view3d.view_orbit`, `view_roll` and `view_axis`. */
                if orbit.is_some() || roll.is_some() || axis_view.is_some() {
                    self.smooth = self.rv3d.smooth_view_begin(smooth_src, view3d::SMOOTH_VIEWTX).map(|sms| (sms, std::time::Instant::now()));
                }
                gfx.window.request_redraw();
            }
            WindowEvent::Resized(size) => {
                if let Some(user) = self.user.as_ref() {
                    self.layout = Some(screen::screen_layout(user, size.width as i32, size.height as i32));
                }
                gfx.window.request_redraw();
            }

            WindowEvent::RedrawRequested => {
                /* Sculpt Mode ends with the mesh object it works on. */
                if self.sculpt.is_some() && !self.scene.active_object().is_some_and(|o| o.mesh().is_some()) {
                    self.sculpt = None;
                }
                /* The modifiers that depend on time (Build) follow the current frame. */
                if self.scene.set_frame(self.frame.unwrap_or(1)) {
                    scene_changed(&mut self.scene, &mut self.taa);
                }
                let PhysicalSize { width, height } = gfx.window.inner_size();
                let (Some(w), Some(h)) = (NonZeroU32::new(width), NonZeroU32::new(height)) else {
                    return;
                };
                gfx.surface.resize(w, h).expect("resize surface");
                let mut buf = gfx.surface.buffer_mut().expect("surface buffer");
                buf.fill(theme::EDITOR_BORDER.to_xrgb());
                if let Some(layout) = self.layout.as_ref() {
                    let (w, h) = (width as usize, height as usize);
                    area::fill_rect(&mut buf, w, h, layout.main, theme::SPACE_VIEW3D_BACK.to_xrgb());
                    for (area_rect, back) in [
                        (layout.outliner, theme::SPACE_OUTLINER_BACK),
                        (layout.properties, theme::SPACE_PROPERTIES_BACK),
                        (layout.timeline, theme::SPACE_ACTION_BACK),
                    ] {
                        area::fill_rect(&mut buf, w, h, area_rect, back.to_xrgb());
                    }
                    if let Some(user) = self.user.as_ref() {
                        if let Some(nav) = area::properties_nav_bar_rect(user, layout.properties) {
                            area::fill_rect(&mut buf, w, h, nav, theme::SIDEBARS_TAB_BACK.to_xrgb());
                        }
                    }
                    /* Non-overlapping header regions (`RGN_TYPE_HEADER`, `RGN_ALIGN_TOP`) of the outliner,
                     * properties editor and timeline: cleared with `TH_HEADER` (alpha forced to 1). */
                    if let Some(user) = self.user.as_ref() {
                        for (area_rect, header_color) in [
                            (layout.outliner, theme::SPACE_OUTLINER_HEADER),
                            (layout.properties, theme::SPACE_PROPERTIES_HEADER),
                            (layout.timeline, theme::SPACE_ACTION_HEADER),
                        ] {
                            if let Some(header) = area::top_header_rect(user, area_rect) {
                                area::fill_rect(&mut buf, w, h, header, header_color.to_xrgb());
                            }
                        }
                    }
                    /* Timeline header (`space_time.py`, `playback_controls()`): the `View`, `Marker` and `Playback`
                     * labels, after a gap for the editor type button (not drawn, no icons yet). Visual only. */
                    if let Some(user) = self.user.as_ref() {
                        if let Some(header) = area::top_header_rect(user, layout.timeline) {
                            let mut row = ui_layout::HeaderRow::new_global(user, header.size_y() + 1);
                            row.place(user.widget_unit * 2);
                            for label in ["View", "Marker", "Playback"] {
                                let mut rect = row.place(font::label_width(user, label));
                                rect.translate(header.xmin, header.ymin);
                                font::draw_label(&mut buf, w, h, user, rect, label, theme::WCOL_PULLDOWN_TEXT);
                            }
                        }
                    }
                    if let Some(user) = self.user.as_ref() {
                        if let Some(window) = outliner_draw::window_rect(user, layout.outliner) {
                            outliner_draw::draw_row_stripes(&mut buf, w, h, user, window);
                            outliner_draw::draw_tree_rows(&mut buf, w, h, user, window, &outliner_draw::scene_rows(&self.scene));
                            outliner_draw::draw_restrict_buttons(&mut buf, w, h, user, window, &self.scene);
                            if let Some(edit) = self.outliner_rename.as_ref() {
                                outliner_draw::draw_rename_field(&mut buf, w, h, user, window, &self.scene, edit);
                            }
                        }
                    }
                    /* Modifiers panel of the properties editor. */
                    if let Some(user) = self.user.as_ref() {
                        if let Some(window) = modifier_panel::window_rect(user, layout.properties) {
                            let tp = transform_panel::layout(user, window, &self.scene, self.field_edit.as_ref());
                            modifier_panel::draw(&mut buf, w, h, user, &tp.widgets);
                            let below = area::Rcti { ymax: tp.bottom, ..window };
                            let widgets = modifier_panel::widgets(user, below, &modifier_panel::PanelState::of(&self.scene, self.mod_add_open).with_edit(self.mod_edit.as_ref()));
                            modifier_panel::draw(&mut buf, w, h, user, &widgets);
                        }
                    }
                    /* `ED_time_scrub_draw()` background of the timeline (`draw_background()`, `TH_TIME_SCRUB_BACKGROUND`).
                     * The markers are not drawn yet. */
                    if let Some(user) = self.user.as_ref() {
                        if let Some(window) = outliner_draw::window_rect(user, layout.timeline) {
                            /* `ANIM_draw_framerange()`: drawn in the main region before the scrub region covers its top. */
                            playhead_draw::draw_framerange(&mut buf, w, h, user, window);
                            area::fill_rect(&mut buf, w, h, area::time_scrub_rect(user, window), theme::SCRUBBING_BACK.to_xrgb());
                            /* `ED_time_scrub_draw()` numbers (`view2d_draw_scale_x()`), drawn under the playhead. */
                            playhead_draw::draw_scrub_numbers(&mut buf, w, h, user, window);
                            /* `ED_time_scrub_draw_current_frame()`: the playhead with its frame number. */
                            playhead_draw::draw_playhead(&mut buf, w, h, user, window, self.frame.unwrap_or(1));
                        }
                    }
                    /* Overlay grid: drawn into the main region, before the header overlap region. */
                    {
                        /* `view3d_smoothview_apply_from_timer()`: advance until the time allowed is over. */
                        if let Some((sms, start)) = self.smooth {
                            if self.rv3d.smooth_view_apply(&sms, start.elapsed().as_secs_f64()) {
                                self.smooth = None;
                            } else {
                                gfx.window.request_redraw();
                            }
                        }
                        self.rv3d.update(&self.v3d, layout.main.size_x() + 1, layout.main.size_y() + 1);
                        /* Workbench solid pass: the cube, whose depth the grid is tested against. One jittered
                         * sample is drawn per redraw until the 8 samples of `U.viewport_aa` are accumulated;
                         * the depth is that of sample 0 (`sample0_depth_tx_`), which has no jitter. */
                        let (rw, rh) = (layout.main.size_x() + 1, layout.main.size_y() + 1);
                        let sample = if self.wire { taa::SAMPLES_LEN } else { self.taa.begin(self.rv3d.persmat, rw, rh, self.xray) };
                        if sample < taa::SAMPLES_LEN && rw > 0 && rh > 0 {
                            let offset = self.taa.offset(sample);
                            let jittered = self.rv3d.taa_jittered(offset, rw, rh);
                            let mut tmp = vec![theme::SPACE_VIEW3D_BACK.to_xrgb(); (rw * rh) as usize];
                            let tmp_region = area::Rcti { xmin: 0, xmax: rw - 1, ymin: 0, ymax: rh - 1 };
                            self.scene.update_eval();
                            let render = workbench::draw_scene(&mut tmp, rw as usize, rh as usize, tmp_region, &jittered, self.xray, &self.scene);
                            if sample == 0 {
                                self.scene_depth = render.depth;
                                self.scene_ids = render.ids;
                            }
                            self.taa.accumulate(&tmp, rw as usize, rh as usize, offset);
                            if self.taa.sample < taa::SAMPLES_LEN {
                                gfx.window.request_redraw();
                            }
                        }
                        if self.wire {
                            if let Some(user) = self.user.as_ref() {
                                overlay_wireframe::draw_scene_wire(&mut buf, w, h, layout.main, user, &self.rv3d, &self.scene);
                            }
                        } else {
                            self.taa.resolve(&mut buf, w, h, layout.main);
                            /* Overlay outline of the selected objects (hidden with the overlays). */
                            if let Some(user) = self.user.as_ref().filter(|_| !self.overlays_hidden) {
                                overlay_outline::draw_outlines(&mut buf, w, h, layout.main, user, &self.scene, &self.scene_ids);
                            }
                        }
                        if !self.overlays_hidden {
                            overlay_grid_draw::draw_grid(&mut buf, w, h, layout.main, &self.v3d, &self.rv3d, &self.scene_depth);
                        }
                        if let Some(user) = self.user.as_ref().filter(|_| !self.overlays_hidden) {
                            overlay_extra::draw_extras(&mut buf, w, h, layout.main, user, &self.rv3d, &self.scene);
                            overlay_cursor::draw_cursor(&mut buf, w, h, layout.main, user, &self.rv3d, self.cursor3d, self.cursor3d_quat);
                            overlay_text::draw(&mut buf, w, h, layout.main, user, &self.v3d, &self.rv3d, self.frame.unwrap_or(1), &{ let mut info = overlay_text::ObjectInfo::of(&self.scene); if self.pivot_point != transform::PivotPoint::Median { info.pivot = Some(self.pivot_point.label().to_string()); } if self.show_stats { info.with_stats(&self.scene) } else { info } });
                        }
                    }
                    /* Sculpt Mode: the brush ring at the cursor and the brush line at the bottom left. */
                    if let (Some(sc), Some(user)) = (self.sculpt.as_ref(), self.user.as_ref()) {
                        if sc.adjust.is_some() || area::in_view3d_main(user, layout.main, self.cursor, self.sculpt.is_some()) {
                            sc.draw_cursor(&mut buf, w, h, layout.main, user, self.cursor, self.modifiers.control_key());
                        }
                    }
                    if let Some(bs) = self.box_select.as_ref().filter(|b| b.dragging) {
                        if let Some(start) = bs.start {
                            select_box::draw_box(&mut buf, w, h, layout.main, start, bs.cur);
                        }
                    }
                    area::fill_rect(&mut buf, w, h, layout.topbar, theme::SPACE_TOPBAR_HEADER.to_xrgb());
                    area::fill_rect(&mut buf, w, h, layout.statusbar, theme::SPACE_STATUSBAR_HEADER.to_xrgb());
                    /* `STATUSBAR_HT_header` / `uiTemplateStatusInfo()`: the compact Blender version label on the
                     * right of the status bar (`STATUSBAR_SHOW_VERSION`). Visual only. */
                    if let Some(user) = self.user.as_ref() {
                        let bar = area::statusbar_header_rect(layout.statusbar);
                        let version = version::version_string_compact();
                        let mut row = ui_layout::HeaderRow::new_global(user, bar.size_y() + 1);
                        let mut rect = row.place(font::label_width(user, &version));
                        let right_edge = bar.xmax + 1 - rect.xmin;
                        rect.translate(right_edge - (rect.xmax + 1), bar.ymin);
                        font::draw_label(&mut buf, w, h, user, rect, &version, theme::SPACE_STATUSBAR_TEXT);
                    }
                    /* `STATUSBAR_HT_header` / `uiTemplateInputStatus()`: the keymap hints of the cursor position.
                     * Only the header region case is modelled, without the mouse icons. */
                    if let Some(user) = self.user.as_ref() {
                        status_input::draw(&mut buf, w, h, user, layout, self.cursor, area::statusbar_header_rect(layout.statusbar));
                    }
                    /* Top bar header row (`ED_region_header_layout()`, `TOPBAR_HT_upper_bar.draw_left()`): the
                     * `TOPBAR_MT_editor_menus` labels (`File`, `Edit`, `Render`, `Window`, `Help`), then the
                     * workspace tabs of `template_ID_tabs()` (`Layout` active, `wcol_tab`). Visual only: no
                     * menu opens and the tabs do not switch. */
                    if let Some(user) = self.user.as_ref() {
                        if let Some(header) = area::top_header_rect(user, layout.topbar) {
                            let mut row = ui_layout::HeaderRow::new_global(user, header.size_y() + 1);
                            for label in ["File", "Edit", "Render", "Window", "Help"] {
                                let mut rect = row.place(font::label_width(user, label));
                                rect.translate(header.xmin, header.ymin);
                                font::draw_label(&mut buf, w, h, user, rect, label, theme::WCOL_PULLDOWN_TEXT);
                            }
                            for (i, label) in [
                                "Layout",
                                "Modeling",
                                "Sculpting",
                                "UV Editing",
                                "Texture Paint",
                                "Shading",
                                "Animation",
                                "Rendering",
                                "Compositing",
                                "Geometry Nodes",
                                "Scripting",
                            ]
                            .into_iter()
                            .enumerate()
                            {
                                let mut rect = row.place(font::label_width(user, label));
                                rect.translate(header.xmin, header.ymin);
                                let active = i == 0;
                                widget_draw::draw_round_box(
                                    &mut buf,
                                    w,
                                    h,
                                    user,
                                    rect,
                                    if active { theme::WCOL_TAB_INNER_SEL } else { theme::WCOL_TAB_INNER },
                                    theme::WCOL_TAB_OUTLINE,
                                    theme::WCOL_TAB_ROUNDNESS,
                                );
                                let text = if active { theme::WCOL_TAB_TEXT_SEL } else { theme::WCOL_TAB_TEXT };
                                font::draw_label(&mut buf, w, h, user, rect, label, text);
                            }
                        }
                    }
                    /* `TOPBAR_HT_upper_bar.draw_right()`: the `template_ID(window, "scene")` and the
                     * `template_search(window, "view_layer")` fields, right aligned (the right header
                     * region is `RGN_ALIGN_RIGHT | RGN_FLAG_DYNAMIC_SIZE`). Deviation: icons and the new /
                     * unlink buttons are not drawn and the widths are fixed at 7 widget units. Visual only. */
                    if let Some(user) = self.user.as_ref() {
                        if let Some(header) = area::top_header_rect(user, layout.topbar) {
                            let mut row = ui_layout::HeaderRow::new_global(user, header.size_y() + 1);
                            let offset = row.place(0).xmin;
                            let field = user.widget_unit * 7;
                            let spacer = header.size_x() + 1 - 2 * offset - (2 * field + user.button_space_x) - user.button_space_x;
                            row.place(spacer);
                            for label in ["Scene", "ViewLayer"] {
                                let mut rect = row.place(field);
                                rect.translate(header.xmin, header.ymin);
                                widget_draw::draw_round_box(
                                    &mut buf,
                                    w,
                                    h,
                                    user,
                                    rect,
                                    theme::WCOL_MENU_INNER,
                                    theme::WCOL_MENU_OUTLINE,
                                    theme::WCOL_MENU_ROUNDNESS,
                                );
                                font::draw_label(&mut buf, w, h, user, rect, label, theme::WCOL_MENU_TEXT);
                            }
                        }
                    }
                    /* 3D viewport header: overlap region, blended over the main region. */
                    if let Some(user) = self.user.as_ref() {
                        if let Some(header) = area::view3d_header_rect(user, layout.main) {
                            area::blend_rect(&mut buf, w, h, header, theme::SPACE_VIEW3D_HEADER);
                            /* Editor type, mode, menus, transform cluster, overlays, X-ray and shading buttons. The ones
                             * this port models are wired to the same state as their keys (Shift + Z, Alt + Z,
                             * Shift + Alt + Z, Period). */
                            let items = view3d_header::layout(user, header, &header_state(self.wire, self.xray, self.overlays_hidden, self.pivot_point, mode_of(self.edit_mode, self.sculpt.is_some())));
                            view3d_header::draw(&mut buf, w, h, user, &items, self.header_hover);
                        }
                    }
                    /* Sculpt Mode: the tool header under the header, same overlap region look. */
                    if let (Some(sc), Some(user)) = (self.sculpt.as_ref(), self.user.as_ref()) {
                        if let Some(region) = area::tool_header_rect(user, layout.main) {
                            area::blend_rect(&mut buf, w, h, region, theme::SPACE_VIEW3D_HEADER);
                            let items = sculpt_header::layout(user, region);
                            sculpt_header::draw(&mut buf, w, h, user, &items, &sc.settings, self.sculpt_hover);
                        }
                    }
                    /* ED_screen_draw_edges(): the border and outline of every area, `totrct` with the area vertices as scissor. */
                    if let Some(user) = self.user.as_ref() {
                        for (kind, totrct, vertices) in layout.editor_areas() {
                            screen_draw::draw_area_border(&mut buf, w, h, user, totrct, vertices);
                            screen_draw::draw_area_outline(&mut buf, w, h, user, totrct, vertices, self.active_area == Some(kind));
                        }
                    }
                    if let (Some(edit), Some(user)) = (self.rename.as_ref(), self.user.as_ref()) {
                        let l = rename_popup::layout(user, edit.pos, w as i32, h as i32);
                        rename_popup::draw(&mut buf, w, h, user, &l, edit);
                    }
                    /* Popup menus are drawn above everything else. */
                    if let (Some((pos, open)), Some(user)) = (self.add_mod_menu, self.user.as_ref()) {
                        let menu = add_modifier_menu::layout(user, pos, w as i32, h as i32, open);
                        add_modifier_menu::draw(&mut buf, w, h, user, &menu, self.cursor);
                    }
                    if let (Some((i, pos)), Some(user)) = (self.mod_menu, self.user.as_ref()) {
                        let menu = modifier_menu::layout(user, &self.scene, i, pos, w as i32, h as i32);
                        modifier_menu::draw(&mut buf, w, h, user, &menu, self.cursor);
                    }
                    if let (Some(pos), Some(user), Some(sc)) = (self.brush_menu, self.user.as_ref(), self.sculpt.as_ref()) {
                        let menu = brush_menu::layout(user, pos, w as i32, h as i32);
                        brush_menu::draw(&mut buf, w, h, user, &menu, sc.settings.brush, self.cursor);
                    }
                    if let (Some(pos), Some(user)) = (self.add_menu, self.user.as_ref()) {
                        let menu = add_menu::layout(user, pos, w as i32, h as i32);
                        add_menu::draw(&mut buf, w, h, user, &menu, self.cursor);
                    }
                }
                buf.present().expect("present");
            }

            _ => {}
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Wait); // redraw only on demand
    event_loop.run_app(&mut App::default()).expect("run");
}
