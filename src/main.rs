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
mod screen;
mod screen_draw;
#[allow(dead_code)]
mod ui_layout;
mod widget_draw;
mod font;
mod outliner_draw;
mod playhead_draw;

mod status_input;
mod colorspace_lut;
mod theme;
mod userdef;
mod version;
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
    /// Depth buffer of the main region from the last redraw (`rw * rh`, 1.0 where nothing is drawn).
    scene_depth: Vec<f32>,
    /// `View3D::shading.flag & V3D_SHADING_XRAY` of the solid shading.
    xray: bool,
    /// `View3D::shading.type == OB_WIRE` (toggled with Shift + Z, `view3d.toggle_shading`).
    wire: bool,
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
                    let in_main = area::in_view3d_main(user, layout.main, self.cursor);
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
                if !m.shift_key() || m.control_key() || m.alt_key() || !area::in_view3d_main(user, layout.main, self.cursor) {
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
                let on_wire_button = match (self.layout.as_ref(), self.user.as_ref()) {
                    (Some(layout), Some(user)) => area::view3d_header_rect(user, layout.main)
                        .is_some_and(|header| ui_layout::wire_button_rect(user, header).isect_pt_v(self.cursor)),
                    _ => false,
                };
                if on_wire_button {
                    /* Same as `view3d.toggle_shading` (Shift + Z): entering wire drops the solid depth
                     * buffer, leaving it restarts the TAA accumulation. */
                    self.wire = !self.wire;
                    if self.wire {
                        self.scene_depth = Vec::new();
                    } else {
                        self.taa = taa::TaaState::default();
                    }
                    gfx.window.request_redraw();
                    return;
                }
                let on_xray_button = match (self.layout.as_ref(), self.user.as_ref()) {
                    (Some(layout), Some(user)) => area::view3d_header_rect(user, layout.main)
                        .is_some_and(|header| ui_layout::xray_button_rect(user, header).isect_pt_v(self.cursor)),
                    _ => false,
                };
                if on_xray_button {
                    /* Same as `view3d.toggle_xray` (Alt + Z); not modelled in wire shading. */
                    if !self.wire {
                        self.xray = !self.xray;
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
                }
            }
            /* `WM_gesture_box_modal()`: releasing LEFTMOUSE confirms the border and runs `view3d.zoom_border`. */
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Left, .. } => {
                self.scrub_drag = false;
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
                let in_main = area::in_view3d_main(user, layout.main, self.cursor);
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
                /* `screen.frame_offset` on LEFT_ARROW / RIGHT_ARROW (delta -1 / +1) and `screen.frame_jump` on
                 * Shift + LEFT_ARROW / RIGHT_ARROW (start / end of the scene range). */
                if matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight) && !ctrl_k && !alt_k {
                    let right = code == KeyCode::ArrowRight;
                    if shift_k {
                        if !key_event.repeat {
                            self.frame = Some(if right { playhead_draw::EFRA as i32 } else { playhead_draw::SFRA as i32 });
                        }
                    } else {
                        self.frame = Some((self.frame.unwrap_or(1) + if right { 1 } else { -1 }).max(0));
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
                    if area::in_view3d_main(user, layout.main, self.cursor) {
                        self.cursor3d = [0.0; 3];
                        let (rw, rh) = (layout.main.size_x() + 1, layout.main.size_y() + 1);
                        smooth_finish(&mut self.rv3d, &mut self.smooth);
                        let smooth_src = self.rv3d.state_backup();
                        let (bounds_min, bounds_max) = self.rv3d.visible_bounds();
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
                    if area::in_view3d_main(user, layout.main, self.cursor) {
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
                    if area::in_view3d_main(user, layout.main, self.cursor) {
                        let (rw, rh) = (layout.main.size_x() + 1, layout.main.size_y() + 1);
                        smooth_finish(&mut self.rv3d, &mut self.smooth);
                        let smooth_src = self.rv3d.state_backup();
                        let (min, max) = self.rv3d.visible_bounds();
                        self.rv3d.view_from_minmax(&self.v3d, rw, rh, min, max);
                        self.smooth = self.rv3d.smooth_view_begin(smooth_src, view3d::SMOOTH_VIEWTX).map(|sms| (sms, std::time::Instant::now()));
                        gfx.window.request_redraw();
                    }
                    return;
                }
                /* `view3d.zoom_border` on Shift + B (`WM_gesture_box_invoke()`); not over the header region. */
                if code == KeyCode::KeyB && self.modifiers.shift_key() && !self.modifiers.control_key() && !self.modifiers.alt_key() && !key_event.repeat {
                    let (Some(layout), Some(user)) = (self.layout.as_ref(), self.user.as_ref()) else { return };
                    if area::in_view3d_main(user, layout.main, self.cursor) {
                        self.zoom_border = Some(ZoomBorder { start: None, clip: false });
                    }
                    return;
                }
                /* `view3d.clip_border` on Alt + B (`view3d_clipping_invoke()`): turns an active clipping border
                 * off, else starts the box gesture (`WM_gesture_box_invoke()`). Works in wire and solid shading,
                 * both always available here. Not over the header region. */
                if code == KeyCode::KeyB && self.modifiers.alt_key() && !self.modifiers.shift_key() && !self.modifiers.control_key() && !key_event.repeat {
                    let (Some(layout), Some(user)) = (self.layout.as_ref(), self.user.as_ref()) else { return };
                    if area::in_view3d_main(user, layout.main, self.cursor) {
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
                /* `view3d.toggle_shading` with `type = WIREFRAME` on Shift + Z: switches between wire and solid
                 * shading (`prev_type_wire` is solid, the only other type modelled). Not over the header region.
                 * Entering wire drops the solid depth buffer; leaving it restarts the TAA accumulation. */
                if code == KeyCode::KeyZ && self.modifiers.shift_key() && !self.modifiers.control_key() && !self.modifiers.alt_key() && !key_event.repeat {
                    let (Some(layout), Some(user)) = (self.layout.as_ref(), self.user.as_ref()) else { return };
                    if area::in_view3d_main(user, layout.main, self.cursor) {
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
                    if area::in_view3d_main(user, layout.main, self.cursor) {
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
                if !area::in_view3d_main(user, layout.main, self.cursor) {
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
                            outliner_draw::draw_tree_names(&mut buf, w, h, user, window);
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
                            let depth = workbench::draw_cube(&mut tmp, rw as usize, rh as usize, tmp_region, &jittered, self.xray);
                            if sample == 0 {
                                self.scene_depth = depth;
                            }
                            self.taa.accumulate(&tmp, rw as usize, rh as usize, offset);
                            if self.taa.sample < taa::SAMPLES_LEN {
                                gfx.window.request_redraw();
                            }
                        }
                        if self.wire {
                            if let Some(user) = self.user.as_ref() {
                                overlay_wireframe::draw_cube_wire(&mut buf, w, h, layout.main, user, &self.rv3d);
                            }
                        } else {
                            self.taa.resolve(&mut buf, w, h, layout.main);
                        }
                        overlay_grid_draw::draw_grid(&mut buf, w, h, layout.main, &self.v3d, &self.rv3d, &self.scene_depth);
                        if let Some(user) = self.user.as_ref() {
                            overlay_cursor::draw_cursor(&mut buf, w, h, layout.main, user, &self.rv3d, self.cursor3d, self.cursor3d_quat);
                            overlay_text::draw(&mut buf, w, h, layout.main, user, &self.rv3d, self.frame.unwrap_or(1));
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
                            /* Wireframe toggle: wired to the same state as Shift + Z (`self.wire`). */
                            widget_draw::draw_round_box(
                                &mut buf,
                                w,
                                h,
                                user,
                                ui_layout::wire_button_rect(user, header),
                                if self.wire { theme::WCOL_TOGGLE_INNER_SEL } else { theme::WCOL_TOOL_INNER },
                                theme::WCOL_TOOL_OUTLINE,
                                theme::WCOL_TOOL_ROUNDNESS,
                            );
                            /* X-ray toggle: wired to `self.xray` (Alt + Z). In wire shading the operator flips
                             * `V3D_SHADING_XRAY_WIREFRAME`, which is not modelled, so it is drawn dimmed and
                             * does nothing there. */
                            let (xray_inner, xray_outline) = if self.wire {
                                (
                                    theme::Rgba { a: theme::WCOL_TOOL_INNER.a / 2, ..theme::WCOL_TOOL_INNER },
                                    theme::Rgba { a: theme::WCOL_TOOL_OUTLINE.a / 2, ..theme::WCOL_TOOL_OUTLINE },
                                )
                            } else if self.xray {
                                (theme::WCOL_TOGGLE_INNER_SEL, theme::WCOL_TOOL_OUTLINE)
                            } else {
                                (theme::WCOL_TOOL_INNER, theme::WCOL_TOOL_OUTLINE)
                            };
                            widget_draw::draw_round_box(
                                &mut buf,
                                w,
                                h,
                                user,
                                ui_layout::xray_button_rect(user, header),
                                xray_inner,
                                xray_outline,
                                theme::WCOL_TOOL_ROUNDNESS,
                            );
                            /* Pulldown menu labels (`View`, `Select`, `Add`, `Object`) after the two toggles.
                             * Visual only: no menu opens. */
                            let mut row = ui_layout::HeaderRow::new_global(user, header.size_y() + 1);
                            row.place(user.widget_unit * 3);
                            row.place(user.widget_unit * 3);
                            for label in ["View", "Select", "Add", "Object"] {
                                let mut rect = row.place(font::label_width(user, label));
                                rect.translate(header.xmin, header.ymin);
                                font::draw_label(&mut buf, w, h, user, rect, label, theme::WCOL_PULLDOWN_TEXT);
                            }
                        }
                    }
                    /* ED_screen_draw_edges(): the border and outline of every area, `totrct` with the area vertices as scissor. */
                    if let Some(user) = self.user.as_ref() {
                        for (kind, totrct, vertices) in layout.editor_areas() {
                            screen_draw::draw_area_border(&mut buf, w, h, user, totrct, vertices);
                            screen_draw::draw_area_outline(&mut buf, w, h, user, totrct, vertices, self.active_area == Some(kind));
                        }
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
