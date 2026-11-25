//! Default 3D viewport state and its view / projection matrices.
//!
//! Ported from Blender:
//!  - `view3d_create()`                         (editors/space_view3d/space_view3d.cc:253-265)
//!  - `View3D` defaults `lens`, `clip_start`, `clip_end`
//!                                              (makesdna/DNA_view3d_types.h:758-759)
//!  - `ED_view3d_viewplane_get()`               (editors/space_view3d/view3d_utils.cc:198)
//!  - `BKE_camera_params_init()`, `BKE_camera_params_from_view3d()` (perspective branch),
//!    `BKE_camera_params_compute_viewplane()`, `BKE_camera_frame_size()`,
//!    `BKE_camera_sensor_size()`, `BKE_camera_sensor_fit()`
//!                                              (blenkernel/intern/camera.cc:329-540)
//!  - `view3d_winmatrix_set()`, `view3d_viewmatrix_set()`
//!                                              (editors/space_view3d/view3d_view.cc:319, 406)
//!  - `mat4_frustum_set()`                      (gpu/intern/gpu_matrix.cc:309)
//!  - `quat_to_mat4()`, `translate_m4()`        (blenlib/intern/math_rotation_c.cc:244,
//!                                               math_matrix_c.cc:2108)
//!  - `ED_view3d_update_viewmat()` (utility matrices and `pixsize`)
//!                                              (editors/space_view3d/view3d_draw.cc:104-173)
//!  - `mul_m4_m4m4()`, `invert_m4_m4_fallback()` (blenlib/intern/math_matrix_c.cc:217, 1078)
//!
//! Only the default state is handled: perspective view (`RV3D_PERSP`), no camera object
//! (`RV3D_CAMOB`), no orthographic view, no `ob_center` / `ob_center_cursor` and no region
//! lock offset. Matrices are column-major like Blender's `float[4][4]`: `m[col][row]`.

pub type Mat4 = [[f32; 4]; 4];

/// `unified_viewquat` (blenloader/intern/versioning_defaults.cc:260): the levelled-out rotation
/// of the default startup 3D viewport. The startup file's `dist` and `ofs` are stored in the
/// binary startup.blend, which is not available in the source, so those keep the
/// `view3d_create()` defaults.
pub const STARTUP_VIEWQUAT: [f32; 4] = [
    0.712_375_879_287_719_7,
    -0.441_062_092_781_066_9,
    -0.287_358_254_194_259_64,
    -0.464_122_921_466_827_4,
];

/// `DEFAULT_SENSOR_WIDTH` / `DEFAULT_SENSOR_HEIGHT` (makesdna/DNA_camera_types.h:88).
const DEFAULT_SENSOR_WIDTH: f32 = 36.0;
/// Used only for the sensor fit, which is `CAMERA_SENSOR_FIT_AUTO` here (width is the size).
#[allow(dead_code)]
const DEFAULT_SENSOR_HEIGHT: f32 = 24.0;

/// `CAMERA_PARAM_ZOOM_INIT_PERSP` (blenkernel/BKE_camera.h:100).
const CAMERA_PARAM_ZOOM_INIT_PERSP: f32 = 2.0;

/// Blender's `rctf`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rctf {
    pub xmin: f32,
    pub xmax: f32,
    pub ymin: f32,
    pub ymax: f32,
}

/// The `View3D` fields used for the projection.
#[derive(Clone, Copy, Debug)]
pub struct View3D {
    pub lens: f32,
    pub clip_start: f32,
    pub clip_end: f32,
    /// `View3D::grid` (grid scale, 1.0 by default).
    pub grid: f32,
}

impl Default for View3D {
    /// `float lens = 50.0f; float clip_start = 0.01f, clip_end = 1000.0f;`
    fn default() -> Self {
        Self { lens: 50.0, clip_start: 0.01, clip_end: 1000.0, grid: 1.0 }
    }
}

/// `eRegionView3D_View`: the axis view of the region, or `User` (`RV3D_VIEW_USER`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    User,
    Front,
    Back,
    Left,
    Right,
    Top,
    Bottom,
}

impl View {
    /// `ED_view3d_axis_view_opposite()` (editors/space_view3d/view3d_utils.cc:1618).
    pub fn opposite(self) -> View {
        match self {
            View::Front => View::Back,
            View::Back => View::Front,
            View::Left => View::Right,
            View::Right => View::Left,
            View::Top => View::Bottom,
            View::Bottom => View::Top,
            View::User => View::User,
        }
    }
}

const SQRT1_2: f32 = std::f32::consts::FRAC_1_SQRT_2;

/// `view3d_quat_axis` (editors/space_view3d/view3d_utils.cc:1486): front, back, left, right, top,
/// bottom, each for the axis rolls 0, 90, 180 and 270 degrees.
const VIEW_QUAT_AXIS: [[[f32; 4]; 4]; 6] = [
    [
        [SQRT1_2, -SQRT1_2, 0.0, 0.0],
        [0.5, -0.5, -0.5, 0.5],
        [0.0, 0.0, -SQRT1_2, SQRT1_2],
        [-0.5, 0.5, -0.5, 0.5],
    ],
    [
        [0.0, 0.0, -SQRT1_2, -SQRT1_2],
        [0.5, 0.5, -0.5, -0.5],
        [SQRT1_2, SQRT1_2, 0.0, 0.0],
        [0.5, 0.5, 0.5, 0.5],
    ],
    [
        [0.5, -0.5, 0.5, 0.5],
        [0.0, -SQRT1_2, 0.0, SQRT1_2],
        [-0.5, -0.5, -0.5, 0.5],
        [-SQRT1_2, 0.0, -SQRT1_2, 0.0],
    ],
    [
        [0.5, -0.5, -0.5, -0.5],
        [SQRT1_2, 0.0, -SQRT1_2, 0.0],
        [0.5, 0.5, -0.5, 0.5],
        [0.0, SQRT1_2, 0.0, SQRT1_2],
    ],
    [
        [1.0, 0.0, 0.0, 0.0],
        [SQRT1_2, 0.0, 0.0, SQRT1_2],
        [0.0, 0.0, 0.0, 1.0],
        [-SQRT1_2, 0.0, 0.0, SQRT1_2],
    ],
    [
        [0.0, -1.0, 0.0, 0.0],
        [0.0, -SQRT1_2, -SQRT1_2, 0.0],
        [0.0, 0.0, -1.0, 0.0],
        [0.0, SQRT1_2, -SQRT1_2, 0.0],
    ],
];

/// `ED_view3d_quat_from_axis_view()`: `None` for the user view.
pub fn quat_from_axis_view(view: View, roll: usize) -> Option<[f32; 4]> {
    let index = match view {
        View::User => return None,
        View::Front => 0,
        View::Back => 1,
        View::Left => 2,
        View::Right => 3,
        View::Top => 4,
        View::Bottom => 5,
    };
    Some(VIEW_QUAT_AXIS[index][roll & 3])
}


/// The `RegionView3D` fields used for the view and window matrices.
#[derive(Clone, Copy, Debug)]
pub struct RegionView3D {
    pub viewquat: [f32; 4],
    pub dist: f32,
    pub ofs: [f32; 3],
    pub winmat: Mat4,
    pub viewmat: Mat4,
    pub persmat: Mat4,
    pub persinv: Mat4,
    pub viewinv: Mat4,
    pub pixsize: f32,
    /// `RegionView3D::persp == RV3D_ORTHO` (the view is not a perspective one, `!is_persp`).
    pub ortho: bool,
    /// `RegionView3D::view`.
    pub view: View,
    /// `RegionView3D::view_axis_roll` as a number of quarter turns.
    pub view_axis_roll: usize,
    /// `ARegion::winx`, set by `update()`.
    pub winx: i32,
    /// `RegionView3D::clip`, the four planes of the view clipping border (`RV3D_CLIPPING` with `clipbb`);
    /// `None` when clipping is off. A point is kept where `dot(plane.xyz, co) + plane.w >= 0`.
    pub clip: Option<[[f32; 4]; 4]>,
}

impl Default for RegionView3D {
    /// `view3d_create()`: `viewquat[0] = 1`, `persp = RV3D_PERSP`, `dist = 10`.
    /// The matrices are filled by `update()`.
    fn default() -> Self {
        Self {
            viewquat: [1.0, 0.0, 0.0, 0.0],
            dist: 10.0,
            ofs: [0.0; 3],
            winmat: IDENTITY,
            viewmat: IDENTITY,
            persmat: IDENTITY,
            persinv: IDENTITY,
            viewinv: IDENTITY,
            pixsize: 0.0,
            ortho: false,
            view: View::User,
            view_axis_roll: 0,
            winx: 0,
            clip: None,
        }
    }
}

const IDENTITY: Mat4 = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

/// `quat_to_mat4()`.
pub fn quat_to_mat4(q: [f32; 4]) -> Mat4 {
    let q0 = std::f64::consts::SQRT_2 * q[0] as f64;
    let q1 = std::f64::consts::SQRT_2 * q[1] as f64;
    let q2 = std::f64::consts::SQRT_2 * q[2] as f64;
    let q3 = std::f64::consts::SQRT_2 * q[3] as f64;

    let qda = q0 * q1;
    let qdb = q0 * q2;
    let qdc = q0 * q3;
    let qaa = q1 * q1;
    let qab = q1 * q2;
    let qac = q1 * q3;
    let qbb = q2 * q2;
    let qbc = q2 * q3;
    let qcc = q3 * q3;

    [
        [(1.0 - qbb - qcc) as f32, (qdc + qab) as f32, (-qdb + qac) as f32, 0.0],
        [(-qdc + qab) as f32, (1.0 - qaa - qcc) as f32, (qda + qbc) as f32, 0.0],
        [(qdb + qac) as f32, (-qda + qbc) as f32, (1.0 - qaa - qbb) as f32, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

/// `U.view_rotate_sensitivity_turntable` default, `DEG2RAD(0.4)` (makesdna/DNA_userdef_types.h:1264):
/// radians per pixel, divided by `UI_SCALE_FAC` when applied.
pub const VIEW_ROTATE_SENSITIVITY_TURNTABLE: f32 = 0.4 * std::f32::consts::PI / 180.0;

/// `U.pad_rot_angle` default, 15 degrees (makesdna/DNA_userdef_types.h:1173), in radians.
pub const PAD_ROT_ANGLE: f32 = 15.0 * std::f32::consts::PI / 180.0;

/// `mul_qt_qtqt()` (blenlib/intern/math_rotation_c.cc:64).
fn mul_qt_qtqt(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[0] * b[0] - a[1] * b[1] - a[2] * b[2] - a[3] * b[3],
        a[0] * b[1] + a[1] * b[0] + a[2] * b[3] - a[3] * b[2],
        a[0] * b[2] + a[2] * b[0] + a[3] * b[1] - a[1] * b[3],
        a[0] * b[3] + a[3] * b[0] + a[1] * b[2] - a[2] * b[1],
    ]
}

/// `axis_angle_to_quat()`: a zero axis gives the unit quaternion.
fn axis_angle_to_quat(axis: [f32; 3], angle: f32) -> [f32; 4] {
    let len = dot_v3(axis, axis).sqrt();
    if len == 0.0 {
        return [1.0, 0.0, 0.0, 0.0];
    }
    let phi = 0.5 * angle;
    let (si, co) = (phi.sin(), phi.cos());
    [co, axis[0] / len * si, axis[1] / len * si, axis[2] / len * si]
}

/// `normalize_qt()`.
fn normalize_qt(q: [f32; 4]) -> [f32; 4] {
    let len = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if len != 0.0 {
        [q[0] / len, q[1] / len, q[2] / len, q[3] / len]
    } else {
        [0.0, 1.0, 0.0, 0.0]
    }
}

fn dot_v3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross_v3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// `angle_normalized_v3v3()` (blenlib/intern/math_vector.cc:336).
fn angle_normalized_v3v3(a: [f32; 3], b: [f32; 3]) -> f32 {
    let len = |p: [f32; 3], q: [f32; 3]| {
        ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()
    };
    if dot_v3(a, b) >= 0.0 {
        2.0 * (len(a, b) / 2.0).clamp(-1.0, 1.0).asin()
    } else {
        std::f32::consts::PI - 2.0 * (len(a, [-b[0], -b[1], -b[2]]) / 2.0).clamp(-1.0, 1.0).asin()
    }
}


/// `normalize_v3()` (blenlib/intern/math_vector_inline.cc:878-936): returns the length; a vector
/// that is (nearly) zero becomes zero.
fn normalize_v3(v: &mut [f32; 3]) -> f32 {
    let d = dot_v3(*v, *v);
    if d > 1.0e-35 {
        let d = d.sqrt();
        let m = 1.0 / d;
        *v = [v[0] * m, v[1] * m, v[2] * m];
        d
    } else {
        *v = [0.0; 3];
        0.0
    }
}

/// `angle_v3v3()` (blenlib/intern/math_vector.cc:276).
fn angle_v3v3(a: [f32; 3], b: [f32; 3]) -> f32 {
    let (mut a, mut b) = (a, b);
    normalize_v3(&mut a);
    normalize_v3(&mut b);
    angle_normalized_v3v3(a, b)
}

/// `mul_qt_v3()` (blenlib/intern/math_rotation_c.cc:77): rotates `r` by the quaternion `q`.
fn mul_qt_v3(q: [f32; 4], r: [f32; 3]) -> [f32; 3] {
    let mut r = r;
    let t0 = -q[1] * r[0] - q[2] * r[1] - q[3] * r[2];
    let t1 = q[0] * r[0] + q[2] * r[2] - q[3] * r[1];
    let t2 = q[0] * r[1] + q[3] * r[0] - q[1] * r[2];
    r[2] = q[0] * r[2] + q[1] * r[1] - q[2] * r[0];
    r[0] = t1;
    r[1] = t2;

    let t1 = t0 * -q[1] + r[0] * q[0] - r[1] * q[3] + r[2] * q[2];
    let t2 = t0 * -q[2] + r[1] * q[0] - r[2] * q[1] + r[0] * q[3];
    r[2] = t0 * -q[3] + r[2] * q[0] - r[0] * q[2] + r[1] * q[1];
    r[0] = t1;
    r[1] = t2;
    r
}

/// `invert_qt_normalized()`: the inverse of a unit quaternion is its conjugate.
fn invert_qt_normalized(q: [f32; 4]) -> [f32; 4] {
    [q[0], -q[1], -q[2], -q[3]]
}

/// `angle_qtqt()` (math_rotation_c.cc:653): `rotation_between_quats_to_quat()` of the normalized
/// quaternions, then `angle_normalized_qt()` = `2 * safe_acosf(w)`.
fn angle_qtqt(q1: [f32; 4], q2: [f32; 4]) -> f32 {
    let (q1, q2) = (normalize_qt(q1), normalize_qt(q2));
    let mut t = invert_qt_normalized(q1);
    let f = 1.0 / (t[0] * t[0] + t[1] * t[1] + t[2] * t[2] + t[3] * t[3]);
    t = [t[0] * f, t[1] * f, t[2] * f, t[3] * f];
    let d = mul_qt_qtqt(t, q2);
    /* `safe_acosf()`. */
    2.0 * d[0].clamp(-1.0, 1.0).acos()
}

/// `angle_signed_qtqt()` (math_rotation_c.cc:705).
fn angle_signed_qtqt(q1: [f32; 4], q2: [f32; 4]) -> f32 {
    if q1[0] * q2[0] + q1[1] * q2[1] + q1[2] * q2[2] + q1[3] * q2[3] >= 0.0 {
        angle_qtqt(q1, q2)
    } else {
        -angle_qtqt(q1, [-q2[0], -q2[1], -q2[2], -q2[3]])
    }
}

/// `rotation_between_vecs_to_quat()` (math_rotation_c.cc:530) for normalized `v1`, `v2`.
fn rotation_between_vecs_to_quat(v1: [f32; 3], v2: [f32; 3]) -> [f32; 4] {
    let mut axis = cross_v3(v1, v2);
    if normalize_v3(&mut axis) > f32::EPSILON {
        let angle = angle_normalized_v3v3(v1, v2);
        let phi = 0.5 * angle;
        [phi.cos(), axis[0] * phi.sin(), axis[1] * phi.sin(), axis[2] * phi.sin()]
    } else if dot_v3(v1, v2) > 0.0 {
        [1.0, 0.0, 0.0, 0.0]
    } else {
        /* Colinear but opposed vectors: `ortho_v3_v3()` (math_vector.cc:593) and a 180 degree turn. */
        let (x, y, z) = (v1[0].abs(), v1[1].abs(), v1[2].abs());
        let ortho = if x > y {
            if x > z { [-v1[1] - v1[2], v1[0], v1[0]] } else { [v1[2], v1[2], -v1[0] - v1[1]] }
        } else if y > z {
            [v1[1], -v1[0] - v1[2], v1[1]]
        } else {
            [v1[2], v1[2], -v1[0] - v1[1]]
        };
        axis_angle_to_quat(ortho, std::f32::consts::PI)
    }
}

/// `vec_to_quat(q, vec, OB_NEGZ, OB_POSY)` (math_rotation_c.cc:722): the quaternion that turns the
/// negative Z axis towards `vec` with the Y axis up.
fn vec_to_quat_negz_posy(vec: [f32; 3]) -> [f32; 4] {
    let eps = 1e-4f32;
    let len = dot_v3(vec, vec).sqrt();
    if len == 0.0 {
        return [1.0, 0.0, 0.0, 0.0];
    }
    /* Z axis: `nor` is `(-y, x, 0)`, and the cosine is `z / len`. */
    let mut nor = [-vec[1], vec[0], 0.0];
    if vec[0].abs() + vec[1].abs() < eps {
        nor[0] = 1.0;
    }
    let co = vec[2] / len;
    normalize_v3(&mut nor);
    let phi = 0.5 * co.clamp(-1.0, 1.0).acos();
    let q = [phi.cos(), nor[0] * phi.sin(), nor[1] * phi.sin(), nor[2] * phi.sin()];

    /* The Z axis differs from the Y up flag: roll about `vec` so that Y points up. */
    let mat = quat_to_mat4(q);
    let fp = mat[2];
    let angle = -0.5 * (-fp[0]).atan2(-fp[1]);
    let (co, si) = (angle.cos(), angle.sin() / len);
    mul_qt_qtqt([co, vec[0] * si, vec[1] * si, vec[2] * si], q)
}

/// `ED_view3d_quat_to_axis_view_and_reset_quat()` (view3d_utils.cc:1541-1603) for an `epsilon`
/// below 45 degrees: the first axis view and roll within `epsilon` of `quat`, whose table
/// quaternion then replaces `quat`; otherwise `quat` is kept and the view is `User`.
fn quat_to_axis_view_and_reset_quat(quat: [f32; 4], epsilon: f32) -> ([f32; 4], View, usize) {
    const VIEWS: [View; 6] = [View::Front, View::Back, View::Left, View::Right, View::Top, View::Bottom];
    for (view, rolls) in VIEWS.iter().zip(VIEW_QUAT_AXIS.iter()) {
        for (roll, table) in rolls.iter().enumerate() {
            if angle_signed_qtqt(quat, *table).abs() < epsilon {
                return (quat_from_axis_view(*view, roll).unwrap_or(quat), *view, roll);
            }
        }
    }
    (quat, View::User, 0)
}


/// `interp_qt_qtqt()` (math_rotation_c.cc:900) with `interp_dot_slerp()` (math_rotation_c.cc:876).
fn interp_qt_qtqt(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let mut cosom = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
    /* Rotate around the shortest angle. */
    let quat = if cosom < 0.0 {
        cosom = -cosom;
        [-a[0], -a[1], -a[2], -a[3]]
    } else {
        a
    };
    let w = if cosom.abs() < 1.0 - 1e-4 {
        let omega = cosom.acos();
        let sinom = omega.sin();
        [((1.0 - t) * omega).sin() / sinom, (t * omega).sin() / sinom]
    } else {
        /* Fall back to lerp. */
        [1.0 - t, t]
    };
    [
        w[0] * quat[0] + w[1] * b[0],
        w[0] * quat[1] + w[1] * b[1],
        w[0] * quat[2] + w[1] * b[2],
        w[0] * quat[3] + w[1] * b[3],
    ]
}

/// `U.smooth_viewtx` default, 200 ms (makesdna/DNA_userdef_types.h:1181), in seconds.
pub const SMOOTH_VIEWTX: f64 = 0.2;

/// `VIEW3D_MARGIN` (editors/include/ED_view3d.hh:1354).
const VIEW3D_MARGIN: f32 = 1.4;

/// Bounds of the startup scene's default cube (2 m, at the origin) as `view3d_calc_minmax_visible()`
/// finds them. The startup Light and Camera objects live in the binary startup.blend, which is not
/// available in the source, so they are not part of these bounds.
pub const DEFAULT_CUBE_MIN: [f32; 3] = [-1.0, -1.0, -1.0];
pub const DEFAULT_CUBE_MAX: [f32; 3] = [1.0, 1.0, 1.0];

/// `SmoothView3DState` (editors/space_view3d/view3d_navigate_smoothview.cc:111-116) without `lens`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewState {
    pub quat: [f32; 4],
    pub ofs: [f32; 3],
    pub dist: f32,
}

/// `SmoothView3DStore` (view3d_navigate_smoothview.cc:118-150) for a view without a camera object
/// or dynamic orbit offset.
#[derive(Clone, Copy, Debug)]
pub struct SmoothView {
    src: ViewState,
    dst: ViewState,
    org_view: View,
    org_view_axis_roll: usize,
    time_allowed: f64,
}


/// `translate_m4()`.
fn translate_m4(m: &mut Mat4, tx: f32, ty: f32, tz: f32) {
    for i in 0..3 {
        m[3][i] += tx * m[0][i] + ty * m[1][i] + tz * m[2][i];
    }
}

/// `mul_m4_m4m4()`: `R[j][k] = B[j][i] . A[i][k]`.
fn mul_m4_m4m4(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut r = [[0.0f32; 4]; 4];
    for j in 0..4 {
        for k in 0..4 {
            r[j][k] = b[j][0] * a[0][k] + b[j][1] * a[1][k] + b[j][2] * a[2][k] + b[j][3] * a[3][k];
        }
    }
    r
}

/// `invert_m4_m4()` (Gauss-Jordan with row pivoting, `invert_m4_m4_fallback()`). Like Blender,
/// a singular matrix leaves the partially computed inverse.
fn invert_m4_m4(mat: &Mat4) -> Mat4 {
    let mut inverse = IDENTITY;
    let mut t = *mat;

    for i in 0..4 {
        /* Look for row with max pivot. */
        let mut max = t[i][i].abs();
        let mut maxj = i;
        for j in (i + 1)..4 {
            if t[j][i].abs() > max {
                max = t[j][i].abs();
                maxj = j;
            }
        }
        /* Swap rows if necessary. */
        if maxj != i {
            t.swap(i, maxj);
            inverse.swap(i, maxj);
        }

        if t[i][i] == 0.0 {
            return inverse; /* No non-zero pivot. */
        }
        let pivot = t[i][i] as f64;
        for k in 0..4 {
            t[i][k] = (t[i][k] as f64 / pivot) as f32;
            inverse[i][k] = (inverse[i][k] as f64 / pivot) as f32;
        }
        for j in 0..4 {
            if j != i {
                let factor = t[j][i] as f64;
                for k in 0..4 {
                    t[j][k] -= (t[i][k] as f64 * factor) as f32;
                    inverse[j][k] -= (inverse[i][k] as f64 * factor) as f32;
                }
            }
        }
    }
    inverse
}


/// `ED_view3d_viewplane_get()` for a perspective view, or an orthographic one when `ortho` is set
/// (`dist` is the view distance): returns the view plane and `(clip_start, clip_end)`.
///
/// `BKE_camera_params_from_view3d()` (perspective and `RV3D_ORTHO` branches) followed by
/// `BKE_camera_params_compute_viewplane(&params, winx, winy, 1.0f, 1.0f)`.
pub fn viewplane_get(v3d: &View3D, ortho: bool, dist: f32, winx: i32, winy: i32) -> (Rctf, f32, f32) {
    /* BKE_camera_params_init() + BKE_camera_params_from_view3d(). */
    let zoom = CAMERA_PARAM_ZOOM_INIT_PERSP;
    let mut clip_start = v3d.clip_start;
    let mut clip_end = v3d.clip_end;
    if ortho {
        /* Halve, otherwise too extreme low Z-buffer quality. */
        clip_end *= 0.5;
        clip_start = -clip_end;
    }
    /* `ortho_scale = rv3d->dist * sensor_size / v3d->lens`. */
    let ortho_scale = (dist * DEFAULT_SENSOR_WIDTH / v3d.lens).max(1e-9);
    /* Ensure it's possible to compute a valid projection matrix. */
    let lens = v3d.lens.max(1e-9);
    let clip_end = clip_end.max(clip_start + 1e-3);

    /* BKE_camera_params_compute_viewplane(): `ycor` is 1 since aspx == aspy. */
    let ycor = 1.0f32;
    let (winx_f, winy_f) = (winx as f32, winy as f32);
    let winy_cor = ycor * winy_f;
    let frame_aspect = winy_cor / winx_f;

    /* BKE_camera_frame_size(). */
    let frame_x = winx_f.min(winy_cor / frame_aspect);
    let frame_size = [frame_x, frame_x * frame_aspect];

    /* CAMERA_SENSOR_FIT_AUTO: the sensor size is the sensor width. */
    let sensor_size = DEFAULT_SENSOR_WIDTH;
    let mut pixsize = if ortho { ortho_scale } else { (sensor_size * clip_start) / lens };

    /* BKE_camera_sensor_fit(): auto picks horizontal when `1.0 >= frame_aspect`. */
    let viewfac = if 1.0 >= frame_aspect { frame_size[0] } else { frame_size[1] };
    pixsize /= viewfac;
    pixsize *= zoom;

    /* No lens shift, offset or roll for the viewport. */
    let viewplane = Rctf {
        xmin: -0.5 * winx_f * pixsize,
        xmax: 0.5 * winx_f * pixsize,
        ymin: -0.5 * ycor * winy_f * pixsize,
        ymax: 0.5 * ycor * winy_f * pixsize,
    };
    (viewplane, clip_start, clip_end)
}

/// `mat4_ortho_set()` (gpu/intern/gpu_matrix.cc:283).
fn ortho_m4(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Mat4 {
    [
        [2.0 / (right - left), 0.0, 0.0, 0.0],
        [0.0, 2.0 / (top - bottom), 0.0, 0.0],
        [0.0, 0.0, -2.0 / (far - near), 0.0],
        [-(right + left) / (right - left), -(top + bottom) / (top - bottom), -(far + near) / (far - near), 1.0],
    ]
}


/// `mat4_frustum_set()`.
fn frustum_m4(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Mat4 {
    [
        [2.0 * near / (right - left), 0.0, 0.0, 0.0],
        [0.0, 2.0 * near / (top - bottom), 0.0, 0.0],
        [
            (right + left) / (right - left),
            (top + bottom) / (top - bottom),
            -(far + near) / (far - near),
            -1.0,
        ],
        [0.0, 0.0, -2.0 * far * near / (far - near), 0.0],
    ]
}

impl RegionView3D {
    /// `ED_view3d_clipping_calc()` without an object, with `ED_view3d_clipping_calc_from_boundbox()`
    /// (editors/space_view3d/view3d_utils.cc:322-368): the four side planes of the frustum through
    /// the border `rect` (xmin, xmax, ymin, ymax; region pixels, y up) of a `winx` x `winy` region,
    /// built from the corners unprojected at depth 0 and 1 (`normal_tri_v3()` of corner `val`, the
    /// next corner and the far corner). The matrices must be up to date.
    pub fn clipping_from_rect(&self, winx: i32, winy: i32, rect: [i32; 4]) -> [[f32; 4]; 4] {
        /* Unprojecting can fail: the corners then stay zero like the `memset()` of the original. */
        let mut bb = [[0.0f32; 3]; 8];
        for val in 0..4 {
            let xs = (if val == 0 || val == 3 { rect[0] } else { rect[1] }) as f32;
            let ys = (if val == 0 || val == 1 { rect[2] } else { rect[3] }) as f32;
            bb[val] = self.unproject_v3(winx, winy, xs, ys, 0.0).unwrap_or([0.0; 3]);
            bb[4 + val] = self.unproject_v3(winx, winy, xs, ys, 1.0).unwrap_or([0.0; 3]);
        }
        std::array::from_fn(|val| {
            let (v1, v2, v3) = (bb[val], bb[if val == 3 { 0 } else { val + 1 }], bb[val + 4]);
            let n1 = [v1[0] - v2[0], v1[1] - v2[1], v1[2] - v2[2]];
            let n2 = [v2[0] - v3[0], v2[1] - v3[1], v2[2] - v3[2]];
            let mut n = cross_v3(n1, n2);
            normalize_v3(&mut n);
            [n[0], n[1], n[2], -dot_v3(n, v1)]
        })
    }
    /// `ED_view3d_clipping_clamp_minmax()` (editors/space_view3d/view3d_utils.cc:389-422): the box `min` ..
    /// `max` is cut by the view clipping border. The 6 box planes and the clipping planes (negated, so that a
    /// point is inside where `dot(plane.xyz, co) + plane.w <= 0`) are intersected three at a time and the
    /// min / max of the corners that lie inside every plane become the new box, as in `isect_planes_v3_fn()`
    /// (blenlib/intern/math_geom.cc:856-905) with `eps_coplanar = 1e-4` and `eps_isect = 1e-6`. Blender's fifth
    /// and sixth clipping planes are zero (`ED_view3d_clipping_calc()` sets four) and its coplanar tests skip
    /// them, so only the four are used. Returns false and leaves the box unchanged without clipping or when
    /// nothing intersects.
    pub fn clipping_clamp_minmax(&self, min: &mut [f32; 3], max: &mut [f32; 3]) -> bool {
        let Some(clip) = self.clip.as_ref() else { return false };
        const EPS_COPLANAR: f32 = 1e-4;
        const EPS_ISECT: f32 = 1e-6;

        let mut planes = [[0.0f32; 4]; 10];
        for i in 0..3 {
            planes[i * 2][i] = -1.0;
            planes[i * 2][3] = min[i];
            planes[i * 2 + 1][i] = 1.0;
            planes[i * 2 + 1][3] = -max[i];
        }
        for (i, p) in clip.iter().enumerate() {
            planes[6 + i] = [-p[0], -p[1], -p[2], -p[3]];
        }
        let normal = |i: usize| [planes[i][0], planes[i][1], planes[i][2]];

        let (mut found, mut out_min, mut out_max) = (false, [f32::MAX; 3], [-f32::MAX; 3]);
        for i in 0..planes.len() {
            let n1 = normal(i);
            for j in (i + 1)..planes.len() {
                let n2 = normal(j);
                let n1n2 = cross_v3(n1, n2);
                if dot_v3(n1n2, n1n2) <= EPS_COPLANAR {
                    continue;
                }
                for k in (j + 1)..planes.len() {
                    let n3 = normal(k);
                    let n2n3 = cross_v3(n2, n3);
                    if dot_v3(n2n3, n2n3) <= EPS_COPLANAR {
                        continue;
                    }
                    let n3n1 = cross_v3(n3, n1);
                    if dot_v3(n3n1, n3n1) <= EPS_COPLANAR {
                        continue;
                    }
                    let quotient = -dot_v3(n1, n2n3);
                    if quotient.abs() < EPS_COPLANAR {
                        continue;
                    }
                    let co: [f32; 3] = std::array::from_fn(|a| {
                        ((n2n3[a] * planes[i][3]) + (n3n1[a] * planes[j][3]) + (n1n2[a] * planes[k][3])) / quotient
                    });
                    /* A point may lie outside the planes it was made from (low epsilon values). */
                    let inside = (0..planes.len())
                        .all(|t| t == i || t == j || t == k || dot_v3(normal(t), co) + planes[t][3] <= EPS_ISECT);
                    if inside {
                        for a in 0..3 {
                            out_min[a] = out_min[a].min(co[a]);
                            out_max[a] = out_max[a].max(co[a]);
                        }
                        found = true;
                    }
                }
            }
        }
        if found {
            *min = out_min;
            *max = out_max;
        }
        found
    }

    /// `view3d_calc_minmax_visible()` (editors/space_view3d/view3d_navigate_view_all.cc:176-235) for the
    /// startup scene, whose only object with bounds in the source is the default cube (`DEFAULT_CUBE_MIN` /
    /// `DEFAULT_CUBE_MAX`): with `clip_bounds` the bounds are clamped to the view clipping border when
    /// `RV3D_CLIPPING_ENABLED()` (wire and solid shading, both modelled, so only `clip` matters).
    pub fn visible_bounds(&self) -> ([f32; 3], [f32; 3]) {
        let (mut min, mut max) = (DEFAULT_CUBE_MIN, DEFAULT_CUBE_MAX);
        if self.clip.is_some() {
            self.clipping_clamp_minmax(&mut min, &mut max);
        }
        (min, max)
    }
    /// `ED_view3d_project_int_global(region, co, r_co, V3D_PROJ_TEST_CLIP_NEAR)`
    /// (editors/space_view3d/view3d_project.cc:105-150, 195-215) for a region of `winx` x `winy`
    /// pixels: `None` when `co` is clipped by the near plane or the result overflows.
    pub fn project_int_clip_near(&self, winx: i32, winy: i32, co: [f32; 3]) -> Option<[i32; 2]> {
        let p = &self.persmat;
        let mut v = [0.0f32; 4];
        for i in 0..4 {
            v[i] = p[0][i] * co[0] + p[1][i] * co[1] + p[2][i] * co[2] + p[3][i];
        }
        let w = v[3].abs();
        if v[2] <= -w {
            return None;
        }
        let scalar = if w != 0.0 { 1.0 / w } else { 0.0 };
        let fx = (winx as f32 / 2.0) * (1.0 + v[0] * scalar);
        let fy = (winy as f32 / 2.0) * (1.0 + v[1] * scalar);
        if !(fx > -2_140_000_000.0 && fx < 2_140_000_000.0 && fy > -2_140_000_000.0 && fy < 2_140_000_000.0) {
            return None;
        }
        Some([fx.floor() as i32, fy.floor() as i32])
    }
    /// `viewzoom_apply_3d()` with the default `USER_ZOOM_DOLLY` style, vertical drag, no
    /// inversion and no zoom to mouse position (editors/space_view3d/view3d_navigate_view_zoom.cc:
    /// 189-255, 340-372).
    ///
    /// `ui_scale` is `UI_SCALE_FAC`, `region_ymax` the top of the region (`winrct.ymax`),
    /// `y_init` / `y_curr` the cursor y at the start of the drag and now (y up), and `init_dist`
    /// the view distance when the drag started.
    pub fn zoom_drag(&mut self, v3d: &View3D, ui_scale: f32, region_ymax: i32, y_init: i32, y_curr: i32, init_dist: f32) {
        let dist_min = v3d.grid * 0.001;
        let dist_max = v3d.clip_end * 10.0;

        let len_new = 5.0 * ui_scale + (region_ymax - y_curr) as f32 / ui_scale;
        let len_old = 5.0 * ui_scale + (region_ymax - y_init) as f32 / ui_scale;
        let mut zfac = init_dist * (2.0 * ((len_new / len_old.max(1.0)) - 1.0) + 1.0) / self.dist;

        if zfac != 1.0 {
            /* `CLAMP(zfac, zfac_min, zfac_max)`. */
            let (zfac_min, zfac_max) = (dist_min / self.dist, dist_max / self.dist);
            if zfac < zfac_min {
                zfac = zfac_min;
            } else if zfac > zfac_max {
                zfac = zfac_max;
            }
            self.dist *= zfac;
        }

        /* These limits were in old code too. */
        if self.dist < dist_min {
            self.dist = dist_min;
        } else if self.dist > dist_max {
            self.dist = dist_max;
        }
    }
    /// `ED_view3d_calc_zfac()` (editors/space_view3d/view3d_project.cc:287-313): the clip space
    /// `w` of `co`, i.e. `mul_project_m4_v3_zfac(persmat, co)`, made positive and non-zero.
    pub fn calc_zfac(&self, co: [f32; 3]) -> f32 {
        let p = &self.persmat;
        let mut zfac = p[0][3] * co[0] + p[1][3] * co[1] + p[2][3] * co[2] + p[3][3];
        if zfac < 1.0e-6 && zfac > -1.0e-6 {
            zfac = 1.0;
        }
        if zfac < 0.0 {
            zfac = -zfac;
        }
        zfac
    }

    /// `viewmove_apply()` for a perspective view without a camera object or locked offset
    /// (editors/space_view3d/view3d_navigate.cc:1058-1077) with `ED_view3d_win_to_delta(...,
    /// precise = true)` (view3d_project.cc:653-670).
    ///
    /// `winx` / `winy` are the region size, `zfac` is `ViewOpsData::init.zfac`
    /// (`calc_zfac(-ofs)` when the drag started) and `dx` / `dy` the cursor movement in pixels
    /// since the previous event (y up). The matrices must be up to date (`update()`).
    pub fn pan(&mut self, winx: i32, winy: i32, zfac: f32, dx: i32, dy: i32) {
        /* `event_ofs` is previous minus current. */
        let (ex, ey) = (-(dx as f32), -(dy as f32));
        let ndx = 2.0 * ex * zfac / winx as f32;
        let ndy = 2.0 * ey * zfac / winy as f32;

        let wininv = invert_m4_m4(&self.winmat);
        let r = [
            wininv[0][0] * ndx + wininv[1][0] * ndy,
            wininv[0][1] * ndx + wininv[1][1] * ndy,
            wininv[0][2] * ndx + wininv[1][2] * ndy,
        ];
        /* `mul_mat3_m4_v3(viewinv, r)`. */
        for i in 0..3 {
            let d = self.viewinv[0][i] * r[0] + self.viewinv[1][i] * r[1] + self.viewinv[2][i] * r[2];
            self.ofs[i] -= d;
        }
    }

    /// `ED_view3d_win_to_delta()` with `precise = false` (editors/space_view3d/view3d_project.cc:
    /// 653-676), used by `convertViewVec()` of `transform.translate` (editors/transform/transform.cc:192):
    /// the world space vector for a cursor movement of `dx` / `dy` pixels (y up) in a region of
    /// `winx` x `winy` pixels, with `zfac` from `calc_zfac()`. The matrices must be up to date.
    pub fn win_to_delta(&self, winx: i32, winy: i32, zfac: f32, dx: f32, dy: f32) -> [f32; 3] {
        let ndx = 2.0 * dx * zfac / winx as f32;
        let ndy = 2.0 * dy * zfac / winy as f32;
        let p = &self.persinv;
        std::array::from_fn(|i| p[0][i] * ndx + p[1][i] * ndy)
    }
    /// `viewrotate_apply()` in turntable mode (the default, `USER_TRACKBALL` off) without a
    /// dynamic pivot offset (editors/space_view3d/view3d_navigate_view_rotate.cc:205-276).
    ///
    /// `dx` / `dy` are the cursor movement in window pixels (y up) since the previous event,
    /// `sensitivity` is `U.view_rotate_sensitivity_turntable / UI_SCALE_FAC` and `reverse` is
    /// -1 when the view is upside down (`persmat[2][1] < 0` when the drag started, see
    /// `ViewOpsData::init()`), else 1. The inverse of the rotation matrix is its transpose.
    pub fn orbit_turntable(&mut self, reverse: f32, sensitivity: f32, dx: i32, dy: i32) {
        /* `viewrotate_apply()`: reset every time because of view snapping. */
        self.view = View::User;
        let m = quat_to_mat4(self.viewquat);
        /* `m_inv[col]`, with `m_inv = transpose(m)`. */
        let m_inv = |col: usize| [m[0][col], m[1][col], m[2][col]];
        let (inv0, inv2) = (m_inv(0), m_inv(2));
        let zvec = [0.0f32, 0.0, 1.0];

        /* Horizon axis, blended with the un-rotated one against gimbal lock. */
        let d = [zvec[0] - inv2[0], zvec[1] - inv2[1], zvec[2] - inv2[2]];
        let xaxis = if dot_v3(d, d) > 0.001 {
            let mut x = cross_v3(zvec, inv2);
            if dot_v3(x, inv0) < 0.0 {
                x = [-x[0], -x[1], -x[2]];
            }
            let mut fac = angle_normalized_v3v3(zvec, inv2) / std::f32::consts::PI;
            fac = (fac - 0.5).abs() * 2.0;
            fac *= fac;
            [
                x[0] * (1.0 - fac) + inv0[0] * fac,
                x[1] * (1.0 - fac) + inv0[1] * fac,
                x[2] * (1.0 - fac) + inv0[2] * fac,
            ]
        } else {
            inv0
        };

        /* Up/down rotation about the view X axis, then the orbital rotation about global Z. */
        let quat_local_x = axis_angle_to_quat(xaxis, sensitivity * -(dy as f32));
        let quat_local_x = mul_qt_qtqt(self.viewquat, quat_local_x);
        let half = 0.5 * sensitivity * reverse * dx as f32;
        let quat_global_z = [half.cos(), 0.0, 0.0, half.sin()];
        self.viewquat = normalize_qt(mul_qt_qtqt(quat_local_x, quat_global_z));
    }
    /// `vieworbit_exec()` for a user perspective view without a camera object, locked rotation or
    /// dynamic orbit centre (editors/space_view3d/view3d_navigate_view_orbit.cc:44-142), applied
    /// at once (`smooth_viewtx` animation is not modelled).
    ///
    /// `about_z` selects the global Z axis (`ORBITLEFT` / `ORBITRIGHT`) instead of the horizontal
    /// view axis `viewinv[0]` (`ORBITUP` / `ORBITDOWN`); `angle` is signed, in radians.
    pub fn orbit_step(&mut self, about_z: bool, angle: f32) {
        let quat_mul = if about_z {
            axis_angle_to_quat([0.0, 0.0, 1.0], angle)
        } else {
            /* `viewinv[0]` is the first column of the transposed rotation, i.e. the first row. */
            let m = quat_to_mat4(self.viewquat);
            axis_angle_to_quat([m[0][0], m[1][0], m[2][0]], angle)
        };
        let quat_new = normalize_qt(mul_qt_qtqt(self.viewquat, quat_mul));
        /* Support for switching to the opposite view: a turn by exactly 180 degrees. */
        let opposite = if angle.abs() == std::f32::consts::PI { self.view.opposite() } else { View::User };
        match quat_from_axis_view(opposite, self.view_axis_roll) {
            Some(quat) => {
                self.view = opposite;
                self.viewquat = quat;
            }
            None => {
                self.view = View::User;
                self.viewquat = quat_new;
            }
        }
    }
    /// `viewroll_exec()` with a stepped angle for a user perspective view without a camera object
    /// or dynamic orbit centre (editors/space_view3d/view3d_navigate_view_roll.cc:177-239), applied
    /// at once. The roll axis is the view direction, `-normalize(viewinv[2])`; `angle` is signed
    /// (`LEFT` is negative), in radians.
    pub fn roll_step(&mut self, angle: f32) {
        /* `viewinv[2]` is the third column of the transposed rotation, i.e. the third row. */
        let m = quat_to_mat4(self.viewquat);
        let axis = [-m[0][2], -m[1][2], -m[2][2]];
        let quat_mul = axis_angle_to_quat(axis, angle);
        self.viewquat = normalize_qt(mul_qt_qtqt(self.viewquat, quat_mul));
        /* `viewroll_apply_quat()`: only a roll of exactly 90 degrees can stay an axis view. */
        self.view = View::User;
    }
    /// `viewrotate_apply_snap()` (editors/space_view3d/view3d_navigate_view_rotate.cc:56-146)
    /// without the dynamic orbit offset: snaps `curr_viewquat` to the nearest of the 26 axis /
    /// diagonal view directions within 15 degrees, then to the best of 8 rolls within 15 degrees,
    /// and locks it to an axis view when it is within 0.01 radians of one (which is then
    /// orthographic with auto perspective). `init_ortho` is `init.persp_with_auto_persp_applied`.
    pub fn rotate_snap(&mut self, curr_viewquat: [f32; 4], init_ortho: bool) {
        /* `DEG2RADF(45 / 3)`: integer division, so 15 degrees. */
        let axis_limit = 15.0f32 * (std::f64::consts::PI / 180.0) as f32;

        let viewquat_inv = invert_qt_normalized(curr_viewquat);
        let mut zaxis = mul_qt_v3(viewquat_inv, [0.0, 0.0, 1.0]);
        normalize_v3(&mut zaxis);

        let mut zaxis_best = None;
        for x in -1..2 {
            for y in -1..2 {
                for z in -1..2 {
                    if x != 0 || y != 0 || z != 0 {
                        let mut zaxis_test = [x as f32, y as f32, z as f32];
                        normalize_v3(&mut zaxis_test);
                        if angle_normalized_v3v3(zaxis_test, zaxis) < axis_limit {
                            zaxis_best = Some(zaxis_test);
                        }
                    }
                }
            }
        }

        let Some(zaxis_best) = zaxis_best else {
            /* `else if (U.uiflag & USER_AUTOPERSP)`. */
            self.ortho = init_ortho;
            return;
        };

        /* `viewquat_align` is the original `viewquat` aligned to `zaxis_best`, for testing roll. */
        let viewquat_align = normalize_qt(rotation_between_vecs_to_quat(zaxis_best, zaxis));
        let viewquat_align = normalize_qt(mul_qt_qtqt(curr_viewquat, viewquat_align));
        let viewquat_align_inv = invert_qt_normalized(viewquat_align);

        let quat_snap = invert_qt_normalized(normalize_qt(vec_to_quat_negz_posy(zaxis_best)));

        /* Find the best roll: the one that turns the view X axis the least. */
        let mut best_angle = axis_limit;
        let mut quat_best = None;
        for j in 0..8 {
            let quat_roll = normalize_qt(axis_angle_to_quat(zaxis_best, j as f32 * (45.0f32 * (std::f64::consts::PI / 180.0) as f32)));
            let quat_final = normalize_qt(mul_qt_qtqt(quat_snap, quat_roll));
            let xaxis1 = mul_qt_v3(viewquat_align_inv, [1.0, 0.0, 0.0]);
            let xaxis2 = mul_qt_v3(invert_qt_normalized(quat_final), [1.0, 0.0, 0.0]);
            let angle = angle_v3v3(xaxis1, xaxis2);
            if angle <= best_angle {
                best_angle = angle;
                quat_best = Some(quat_final);
            }
        }

        self.viewquat = match quat_best {
            Some(quat) => {
                /* Lock to an axis view if we can. */
                let (quat, view, roll) = quat_to_axis_view_and_reset_quat(quat, 0.01);
                self.view = view;
                self.view_axis_roll = roll;
                quat
            }
            None => viewquat_align,
        };

        /* `USER_AUTOPERSP`: an axis view is orthographic. */
        if self.view != View::User {
            self.ortho = true;
        }
    }
    /// `viewrotate_apply()` (view3d_navigate_view_rotate.cc:166-293): the turntable rotation is
    /// applied to the unsnapped `curr` (`ViewOpsData::curr.viewquat`), which is kept for the next
    /// event; with `axis_snap` the view quaternion is then snapped, so the view never jams up.
    pub fn rotate_apply(&mut self, curr: &mut [f32; 4], reverse: f32, sensitivity: f32, dx: i32, dy: i32, axis_snap: bool, init_ortho: bool) {
        self.viewquat = *curr;
        self.orbit_turntable(reverse, sensitivity, dx, dy);
        *curr = self.viewquat;
        if axis_snap {
            self.rotate_snap(*curr, init_ortho);
        }
    }
    /// `view3d_smooth_view_state_backup()` (view3d_navigate_smoothview.cc:152-159).
    pub fn state_backup(&self) -> ViewState {
        ViewState { quat: self.viewquat, ofs: self.ofs, dist: self.dist }
    }
    /// `ED_view3d_smooth_view_ex()` (view3d_navigate_smoothview.cc:208-366) for an operator that
    /// has already applied its change to `self` (the destination) starting from `src`: `None` when
    /// nothing changed, else the view is put back to `src`, `view` is `User` while it runs and the
    /// returned store finishes it. A rotation-only change takes `smooth_viewtx` scaled by the angle
    /// (180 degrees is 1.0), so small rotations don't lag.
    pub fn smooth_view_begin(&mut self, src: ViewState, smooth_viewtx: f64) -> Option<SmoothView> {
        let dst = self.state_backup();
        if src == dst {
            return None;
        }
        let org_view = self.view;
        let org_view_axis_roll = self.view_axis_roll;
        self.view = View::User;

        let mut time_allowed = smooth_viewtx;
        if src.ofs == dst.ofs && src.dist == dst.dist {
            time_allowed *= angle_signed_qtqt(dst.quat, src.quat).abs() as f64 / std::f64::consts::PI;
        }

        self.viewquat = src.quat;
        self.ofs = src.ofs;
        self.dist = src.dist;
        Some(SmoothView { src, dst, org_view, org_view_axis_roll, time_allowed })
    }
    /// `view3d_smoothview_apply_and_finish_ex()` (view3d_navigate_smoothview.cc:455-499): the
    /// destination is applied and the original `view` and roll are restored.
    pub fn smooth_view_finish(&mut self, sms: &SmoothView) {
        self.viewquat = sms.dst.quat;
        self.ofs = sms.dst.ofs;
        self.dist = sms.dst.dist;
        self.view = sms.org_view;
        self.view_axis_roll = sms.org_view_axis_roll;
    }
    /// `view3d_smoothview_apply_from_timer()` (view3d_navigate_smoothview.cc:508-536) with
    /// `view3d_smoothview_apply_with_interp()` (426-450): `elapsed` is the time since the smooth
    /// view started, in seconds. Returns true once it has finished.
    pub fn smooth_view_apply(&mut self, sms: &SmoothView, elapsed: f64) -> bool {
        let factor = if sms.time_allowed != 0.0 { (elapsed / sms.time_allowed) as f32 } else { 1.0 };
        if factor >= 1.0 {
            self.smooth_view_finish(sms);
            return true;
        }
        /* Ease in/out smoothing. */
        let f = 3.0 * factor * factor - 2.0 * factor * factor * factor;
        self.viewquat = interp_qt_qtqt(sms.src.quat, sms.dst.quat, f);
        let s = 1.0 - f;
        self.ofs = std::array::from_fn(|i| s * sms.src.ofs[i] + f * sms.dst.ofs[i]);
        self.dist = f * sms.dst.dist + (1.0 - f) * sms.src.dist;
        false
    }
    /// `viewdolly_exec()` for a user perspective view without zoom to mouse position, camera object
    /// or locked offset (editors/space_view3d/view3d_navigate_view_dolly.cc:191-237, 66-71): the
    /// offset moves along the view direction by `-(1 - dfac)` units, with `dfac` 1.8 for
    /// `delta < 0` and 0.2 otherwise (a fixed length, independent of `dist`).
    pub fn dolly_step(&mut self, delta: i32) {
        let dfac = if delta < 0 { 1.8f32 } else { 0.2f32 };
        /* `mousevec = -normalize(viewinv[2])`; `viewinv[2]` is the third row of the rotation. */
        let m = quat_to_mat4(self.viewquat);
        let v = [m[0][2], m[1][2], m[2][2]];
        let len = dot_v3(v, v).sqrt();
        if len == 0.0 {
            return;
        }
        let scale = -(1.0 - dfac) * (-1.0 / len);
        for i in 0..3 {
            self.ofs[i] += v[i] * scale;
        }
    }
    /// `viewcenter_cursor_exec()` for a user perspective view without a camera object or locked
    /// offset (editors/space_view3d/view3d_navigate_view_center_cursor.cc:25-50): `ofs` becomes the
    /// negated 3D cursor location, applied at once (`smooth_viewtx` animation is not modelled).
    pub fn center_cursor(&mut self, cursor: [f32; 3]) {
        self.ofs = [-cursor[0], -cursor[1], -cursor[2]];
    }
    /// `ED_view3d_radius_to_dist()` (editors/space_view3d/view3d_utils.cc:1331-1402) for a user view
    /// without a camera object, with `use_aspect = true`: the distance at which a sphere of `radius`
    /// fits the region of `winx` x `winy` pixels.
    fn radius_to_dist(v3d: &View3D, ortho: bool, winx: i32, winy: i32, radius: f32) -> f32 {
        let mut dist = if ortho {
            radius / (DEFAULT_SENSOR_WIDTH / v3d.lens)
        } else {
            /* `focallength_to_fov()`, then the angle corrected by the zoom. */
            let angle = 2.0 * ((DEFAULT_SENSOR_WIDTH / 2.0) / v3d.lens).atan();
            let angle = ((angle / 2.0).tan() * CAMERA_PARAM_ZOOM_INIT_PERSP).atan() * 2.0;
            radius * (1.0 / (angle / 2.0).tan())
        };
        if winx != 0 && winy != 0 {
            let mut aspect = winx as f32 / winy as f32;
            if aspect < 1.0 {
                aspect = 1.0 / aspect;
            }
            dist *= aspect;
        }
        dist
    }
    /// `view3d_from_minmax()` with `do_zoom = true` for a user view without a camera object
    /// (editors/space_view3d/view3d_navigate_view_all.cc:77-136), applied at once: `ofs` becomes the
    /// negated centre of the bounds and `dist` fits them. A perspective view is kept from going
    /// closer than the near clipping plane (`ED_view3d_dist_soft_min_get()`); an orthographic view
    /// whose bounds are a single point is not zoomed. The caller runs the smooth view.
    pub fn view_from_minmax(&mut self, v3d: &View3D, winx: i32, winy: i32, min: [f32; 3], max: [f32; 3]) {
        let size = (max[0] - min[0]).max(max[1] - min[1]).max(max[2] - min[2]);
        let dist_new = if !self.ortho {
            let dist = Self::radius_to_dist(v3d, false, winx, winy, (size / 2.0) * VIEW3D_MARGIN);
            Some(dist.max(v3d.clip_start * 1.5))
        } else if size < 0.0001 {
            None
        } else {
            Some(Self::radius_to_dist(v3d, true, winx, winy, (size / 2.0) * VIEW3D_MARGIN))
        };
        self.ofs = std::array::from_fn(|i| -((min[i] + max[i]) * 0.5));
        if let Some(dist) = dist_new {
            self.dist = dist;
        }
    }
    /// `viewdolly_apply()` with the default vertical drag and no inversion, dolly towards the view
    /// centre (editors/space_view3d/view3d_navigate_view_dolly.cc:75-110, 66-71).
    ///
    /// `region_ymax` is `winrct.ymax`, `y_init` / `y_curr` the cursor y at the start of the drag
    /// and now (y up) and `init_ofs` the offset when the drag started; `mousevec` is the view
    /// direction `-normalize(viewinv[2])`.
    pub fn dolly_drag(&mut self, region_ymax: i32, y_init: i32, y_curr: i32, init_ofs: [f32; 3]) {
        let len1 = (region_ymax - y_curr) as f32 + 5.0;
        let len2 = (region_ymax - y_init) as f32 + 5.0;
        let zfac = 1.0 + (len1 - len2) * 0.01 * self.dist;
        if zfac == 1.0 {
            return;
        }
        /* `viewinv[2]` is the third row of the rotation. */
        let m = quat_to_mat4(self.viewquat);
        let v = [m[0][2], m[1][2], m[2][2]];
        let len = dot_v3(v, v).sqrt();
        if len == 0.0 {
            return;
        }
        let k = (1.0 - zfac) / len;
        for i in 0..3 {
            self.ofs[i] = init_ofs[i] + v[i] * k;
        }
    }
    /// `ED_view3d_cursor3d_position()` without `use_depth` for a perspective view
    /// (editors/space_view3d/view3d_edit.cc:1004-1056), with `ED_view3d_calc_zfac_ex()`,
    /// `ED_view3d_win_to_3d_int()` / `ED_view3d_win_to_3d()` and `ED_view3d_win_to_vector()`
    /// (view3d_project.cc:287-308, 480-521, 699-714): the ray through `mval` (region pixels, y up)
    /// is intersected with the plane through the current cursor `cursor` that faces the view. A
    /// cursor behind the camera is first reset to `-ofs`. The matrices must be up to date.
    pub fn cursor3d_position(&self, winx: i32, winy: i32, mval: [i32; 2], cursor: [f32; 3]) -> [f32; 3] {
        let p = &self.persmat;
        let zfac = p[0][3] * cursor[0] + p[1][3] * cursor[1] + p[2][3] * cursor[2] + p[3][3];
        let depth_pt = if zfac < 0.0 { [-self.ofs[0], -self.ofs[1], -self.ofs[2]] } else { cursor };

        /* `ED_view3d_win_to_3d()` of an orthographic view: the ray starts on the view plane
         * through `mval` and runs along `viewinv[2]`; `lambda = ray_point_factor_v3()`. */
        if self.ortho {
            let dx = 2.0 * (mval[0] as f32 / winx as f32) - 1.0;
            let dy = 2.0 * (mval[1] as f32 / winy as f32) - 1.0;
            let pi = &self.persinv;
            let origin: [f32; 3] = std::array::from_fn(|i| pi[0][i] * dx + pi[1][i] * dy + self.viewinv[3][i]);
            let dir = [self.viewinv[2][0], self.viewinv[2][1], self.viewinv[2][2]];
            let len_sq = dot_v3(dir, dir);
            let lambda = if len_sq == 0.0 {
                0.0
            } else {
                dot_v3([depth_pt[0] - origin[0], depth_pt[1] - origin[1], depth_pt[2] - origin[2]], dir) / len_sq
            };
            return std::array::from_fn(|i| origin[i] + dir[i] * lambda);
        }

        let origin = [self.viewinv[3][0], self.viewinv[3][1], self.viewinv[3][2]];
        let v = [2.0 * (mval[0] as f32 / winx as f32) - 1.0, 2.0 * (mval[1] as f32 / winy as f32) - 1.0, -0.5];
        let pi = &self.persinv;
        let ew = pi[0][3] * v[0] + pi[1][3] * v[1] + pi[2][3] * v[2] + pi[3][3];
        let mut dir = [0.0f32; 3];
        for i in 0..3 {
            let c = pi[0][i] * v[0] + pi[1][i] * v[1] + pi[2][i] * v[2] + pi[3][i];
            dir[i] = (if ew != 0.0 { c / ew } else { c }) - origin[i];
        }
        let len = dot_v3(dir, dir).sqrt();
        if len != 0.0 {
            dir = [dir[0] / len, dir[1] / len, dir[2] / len];
        }

        /* `isect_ray_plane_v3_factor()` against the plane facing the view, unsigned. */
        let n = [self.viewinv[2][0], self.viewinv[2][1], self.viewinv[2][2]];
        let dot = dot_v3(n, dir);
        let lambda = if dot == 0.0 {
            0.0
        } else {
            let h = [origin[0] - depth_pt[0], origin[1] - depth_pt[1], origin[2] - depth_pt[2]];
            (-dot_v3(n, h) / dot).abs()
        };
        [origin[0] + dir[0] * lambda, origin[1] + dir[1] * lambda, origin[2] + dir[2] * lambda]
    }
    /// `AntiAliasingPass::setup_view()` (draw/engines/workbench/workbench_effect_antialiasing.cc) with
    /// `window_translate_m4()` (blenlib/intern/math_geom.cc:4782): a copy whose `winmat` is moved by the
    /// TAA sample `offset` (in [-1, 1)) divided by the region size, and `persmat` updated. Only the
    /// matrices the workbench pass reads are updated; the matrices must be up to date.
    pub fn taa_jittered(&self, offset: [f32; 2], winx: i32, winy: i32) -> RegionView3D {
        let mut r = *self;
        let (x, y) = (offset[0] / winx as f32, offset[1] / winy as f32);
        if r.winmat[2][3] == -1.0 {
            let p = &self.persmat;
            let len1 = 1.0 / dot_v3([p[0][0], p[1][0], p[2][0]], [p[0][0], p[1][0], p[2][0]]).sqrt();
            let len2 = 1.0 / dot_v3([p[0][1], p[1][1], p[2][1]], [p[0][1], p[1][1], p[2][1]]).sqrt();
            r.winmat[2][0] -= len1 * r.winmat[0][0] * x;
            r.winmat[2][1] -= len2 * r.winmat[1][1] * y;
        } else {
            r.winmat[3][0] += x;
            r.winmat[3][1] += y;
        }
        r.persmat = mul_m4_m4m4(&r.winmat, &r.viewmat);
        r
    }
    /// `ED_view3d_pixel_size_no_ui_scale()` (editors/space_view3d/view3d_project.cc:282-285):
    /// `mul_project_m4_v3_zfac(persmat, co) * pixsize`, the world size of one pixel at `co`.
    pub fn pixel_size_no_ui_scale(&self, co: [f32; 3]) -> f32 {
        let p = &self.persmat;
        (p[0][3] * co[0] + p[1][3] * co[1] + p[2][3] * co[2] + p[3][3]) * self.pixsize
    }
    /// `view_zoom_apply_step()` for a perspective view without zoom to mouse position
    /// (`USER_ZOOM_TO_MOUSEPOS` is off by default) and without a camera object
    /// (editors/space_view3d/view3d_navigate_view_zoom.cc:419-457, 151-187).
    /// `delta < 0` zooms out (`WHEELOUTMOUSE`), otherwise in (`WHEELINMOUSE`); the soft distance
    /// range is `ED_view3d_dist_soft_range_get(v3d, false)` (view3d_utils.cc:157-168).
    pub fn zoom_step(&mut self, v3d: &View3D, delta: i32) {
        let dist_min = v3d.grid * 0.001;
        let dist_max = v3d.clip_end * 10.0;
        if delta < 0 {
            if self.dist < dist_max {
                self.dist *= 1.2;
            }
        } else if self.dist > dist_min {
            self.dist *= 1.0 / 1.2;
        }
    }
    /// `ED_view3d_persp_ensure()` (editors/space_view3d/view3d_utils.cc:553-579) without a camera
    /// object: with auto perspective (on by default) an axis view switches back to perspective.
    pub fn persp_ensure(&mut self) {
        if self.ortho && self.view != View::User {
            self.ortho = false;
        }
    }
    /// `view_axis_exec()` and `axis_set_view()` for a preset axis view, without `align_active`,
    /// `relative`, a camera object or locked rotation (editors/space_view3d/view3d_navigate_view_axis.cc:47-170,
    /// view3d_navigate.cc:950-1060), applied at once. With auto perspective an axis view is
    /// orthographic.
    pub fn view_axis(&mut self, view: View) {
        let Some(quat) = quat_from_axis_view(view, 0) else { return };
        self.view = view;
        self.view_axis_roll = 0;
        self.ortho = true;
        self.viewquat = normalize_qt(quat);
    }
    /// `view_axis_exec()` with `relative = true` and no `align_active`
    /// (editors/space_view3d/view3d_navigate_view_axis.cc:76-141): the view is turned by 90 / 180
    /// degrees about its own up / right axis (`viewinv[1]` / `viewinv[0]`, the second / first rows
    /// of the rotation), then snapped to the nearest axis view and roll. Turntable mode only allows
    /// a roll for the top and bottom views. Then `axis_set_view()` with auto perspective: an axis
    /// view is orthographic. Applied at once (`smooth_viewtx` animation is not modelled).
    pub fn view_axis_relative(&mut self, view: View) {
        let m = quat_to_mat4(self.viewquat);
        let right = [m[0][0], m[1][0], m[2][0]];
        let up = [m[0][1], m[1][1], m[2][1]];
        let half_pi = std::f32::consts::FRAC_PI_2;
        let quat_rotate = match view {
            View::Left => axis_angle_to_quat(up, -half_pi),
            View::Right => axis_angle_to_quat(up, half_pi),
            View::Top => axis_angle_to_quat(right, -half_pi),
            View::Bottom => axis_angle_to_quat(right, half_pi),
            View::Front => [1.0, 0.0, 0.0, 0.0],
            View::Back => axis_angle_to_quat(right, std::f32::consts::PI),
            View::User => return,
        };
        let quat_test = mul_qt_qtqt(self.viewquat, quat_rotate);

        const VIEWS: [View; 6] = [View::Front, View::Back, View::Left, View::Right, View::Top, View::Bottom];
        let mut angle_best = f32::MAX;
        let (mut view_best, mut roll_best) = (View::Front, 0);
        for candidate in VIEWS {
            for roll in 0..4 {
                let quat_axis = quat_from_axis_view(candidate, roll).unwrap_or([1.0, 0.0, 0.0, 0.0]);
                let angle_test = angle_signed_qtqt(quat_axis, quat_test).abs();
                if angle_best > angle_test {
                    angle_best = angle_test;
                    view_best = candidate;
                    roll_best = roll;
                }
            }
        }
        if !matches!(view_best, View::Top | View::Bottom) {
            roll_best = 0;
        }

        self.view = view_best;
        self.view_axis_roll = roll_best;
        self.ortho = true;
        self.viewquat = normalize_qt(quat_from_axis_view(view_best, roll_best).unwrap_or(self.viewquat));
    }
    /// `viewpersportho_exec()` (editors/space_view3d/view3d_edit.cc:620-645) for a user view without
    /// a camera object or locked transform: toggles between `RV3D_PERSP` and `RV3D_ORTHO`.
    pub fn toggle_persp_ortho(&mut self) {
        self.ortho = !self.ortho;
    }
    /// `viewcenter_pick_invoke()` (editors/space_view3d/view3d_navigate_view_center_pick.cc) with
    /// `ED_view3d_autodist()`: the depth under `mval` (region pixels, y up) is searched in rectangles
    /// of radius 0, 2 and 4 pixels and unprojected at the pixel centre; without depth the view plane
    /// through the current view centre is used (`ED_view3d_win_to_3d_int()`). `ofs` becomes the
    /// negated point; the caller runs the smooth view. The matrices must be up to date.
    pub fn center_pick(&mut self, winx: i32, winy: i32, mval: [i32; 2], depth: &[f32]) {
        let p = self
            .autodist(winx, winy, mval, depth)
            .unwrap_or_else(|| self.cursor3d_position(winx, winy, mval, [-self.ofs[0], -self.ofs[1], -self.ofs[2]]));
        self.ofs = [-p[0], -p[1], -p[2]];
    }
    /// `ED_view3d_autodist()` without a fallback point (editors/space_view3d/view3d_utils.cc:1211-1241):
    /// the depth under `mval` (region pixels, y up) is searched in rectangles of radius 0, 2 and 4
    /// pixels and unprojected at the pixel centre. `None` when there is no depth. The matrices must
    /// be up to date.
    pub fn autodist(&self, winx: i32, winy: i32, mval: [i32; 2], depth: &[f32]) -> Option<[f32; 3]> {
        for margin in [0, 2, 4] {
            let rect = [mval[0] - margin, mval[0] + margin, mval[1] - margin, mval[1] + margin];
            if let Some(d) = Self::depth_near(depth, winx, winy, rect) {
                if let Some(p) = self.unproject_v3(winx, winy, mval[0] as f32 + 0.5, mval[1] as f32 + 0.5, d) {
                    return Some(p);
                }
            }
        }
        None
    }
    /// `ED_view3d_cursor3d_position()` with `use_depth` (editors/space_view3d/view3d_edit.cc:1004-1047):
    /// the point under `mval` found by `autodist()` in the depth buffer `depth` of the region, else
    /// the view plane fallback of `cursor3d_position()`.
    pub fn cursor3d_position_depth(&self, winx: i32, winy: i32, mval: [i32; 2], cursor: [f32; 3], depth: &[f32]) -> [f32; 3] {
        match self.autodist(winx, winy, mval, depth) {
            Some(p) => p,
            None => self.cursor3d_position(winx, winy, mval, cursor),
        }
    }
    /// `ED_view3d_unproject_v3()`: region pixel `x`, `y` and window depth `z` in [0, 1] to world space.
    pub fn unproject_v3(&self, winx: i32, winy: i32, x: f32, y: f32, z: f32) -> Option<[f32; 3]> {
        let v = [2.0 * x / winx as f32 - 1.0, 2.0 * y / winy as f32 - 1.0, 2.0 * z - 1.0];
        let pi = &self.persinv;
        let w = pi[0][3] * v[0] + pi[1][3] * v[1] + pi[2][3] * v[2] + pi[3][3];
        if w == 0.0 {
            return None;
        }
        Some(std::array::from_fn(|i| (pi[0][i] * v[0] + pi[1][i] * v[1] + pi[2][i] * v[2] + pi[3][i]) / w))
    }
    /// `view3d_depths_rect_create()` + `view3d_depth_near()`: the closest depth inside `rect`
    /// (xmin, xmax, ymin, ymax; region pixels, y up) of a `winx` x `winy` depth buffer, ignoring
    /// the far plane (nothing drawn). `None` when there is no depth in the rectangle.
    pub fn depth_near(depth: &[f32], winx: i32, winy: i32, rect: [i32; 4]) -> Option<f32> {
        if winx <= 0 || winy <= 0 || depth.len() != (winx * winy) as usize {
            return None;
        }
        let (x0, x1) = (rect[0].max(0), rect[1].min(winx - 1));
        let (y0, y1) = (rect[2].max(0), rect[3].min(winy - 1));
        let mut near = f32::MAX;
        for y in y0..=y1 {
            for x in x0..=x1 {
                let d = depth[(y * winx + x) as usize];
                if d < near && d < 1.0 {
                    near = d;
                }
            }
        }
        if near == f32::MAX {
            None
        } else {
            Some(near)
        }
    }
    /// `view3d_zoom_border_exec()` without a camera object or locked offset (editors/space_view3d/
    /// view3d_navigate_view_zoom_border.cc): `rect` is the border (xmin, xmax, ymin, ymax; region
    /// pixels, y up) of a region of `winx` x `winy` pixels and `depth_close` the nearest depth in it.
    /// The border is resized to the region aspect, then `ofs` and `dist` are set; the caller runs the
    /// smooth view. Returns false (view unchanged) where the operator is cancelled.
    pub fn zoom_border(&mut self, v3d: &View3D, winx: i32, winy: i32, rect: [i32; 4], depth_close: Option<f32>, zoom_in: bool) -> bool {
        let dist_min = v3d.grid * 0.001;
        let dist_max = v3d.clip_end * 10.0;
        let [mut xmin, mut xmax, mut ymin, mut ymax] = rect;
        let (size_x, size_y) = ((xmax - xmin) as f32, (ymax - ymin) as f32);
        if size_x <= 0.0 || size_y <= 0.0 || winx <= 0 || winy <= 0 {
            return false;
        }

        /* Resize border to the same ratio as the window (`BLI_rcti_resize_x()` / `_y()`). */
        let region_aspect = winx as f32 / winy as f32;
        if size_x / size_y < region_aspect {
            let new_x = (size_y * region_aspect) as i32;
            xmin = (xmin + (xmax - xmin) / 2) - new_x / 2;
            xmax = xmin + new_x;
        } else {
            let new_y = (size_x / region_aspect) as i32;
            ymin = (ymin + (ymax - ymin) / 2) - new_y / 2;
            ymax = ymin + new_y;
        }

        let cent = [(xmin as f32 + xmax as f32) / 2.0, (ymin as f32 + ymax as f32) / 2.0];
        let mut ofs_new;
        let mut dist_new;

        if !self.ortho {
            /* No depths to use, we can't do anything. */
            let Some(depth) = depth_close else { return false };
            let (Some(p), Some(p_corner)) = (
                self.unproject_v3(winx, winy, cent[0], cent[1], depth),
                self.unproject_v3(winx, winy, xmin as f32, ymin as f32, depth),
            ) else {
                return false;
            };
            let dvec = [p[0] - p_corner[0], p[1] - p_corner[1], p[2] - p_corner[2]];
            ofs_new = [-p[0], -p[1], -p[2]];
            dist_new = dot_v3(dvec, dvec).sqrt();
            /* Account for the lens, without this a narrow lens zooms in too close. */
            dist_new *= v3d.lens / DEFAULT_SENSOR_WIDTH;
        } else {
            dist_new = self.dist;
            let center = depth_close.and_then(|d| self.unproject_v3(winx, winy, cent[0], cent[1], d));
            if let Some(p) = center {
                ofs_new = [-p[0], -p[1], -p[2]];
            } else {
                /* Fall back to the old way that doesn't set the center depth. */
                ofs_new = self.ofs;
                let zfac = self.calc_zfac([-ofs_new[0], -ofs_new[1], -ofs_new[2]]);
                let dx = ((xmin + xmax) as f32 - winx as f32) / 2.0;
                let dy = ((ymin + ymax) as f32 - winy as f32) / 2.0;
                let dvec = self.win_to_delta(winx, winy, zfac, dx, dy);
                for i in 0..3 {
                    ofs_new[i] -= dvec[i];
                }
            }
            /* Work out the ratios, so that everything selected fits when we zoom. */
            let xscale = (xmax - xmin) as f32 / winx as f32;
            let yscale = (ymax - ymin) as f32 / winy as f32;
            dist_new *= xscale.max(yscale);
        }

        if !zoom_in {
            let dvec = [ofs_new[0] - self.ofs[0], ofs_new[1] - self.ofs[1], ofs_new[2] - self.ofs[2]];
            dist_new = self.dist * (self.dist / dist_new);
            ofs_new = [self.ofs[0] + dvec[0], self.ofs[1] + dvec[1], self.ofs[2] + dvec[2]];
        }

        /* Clamp after because we may have been zooming out. */
        self.ofs = ofs_new;
        self.dist = dist_new.clamp(dist_min, dist_max);
        true
    }
    /// `view3d_winmatrix_set()` (perspective, no selection rect) and `view3d_viewmatrix_set()`
    /// for a region of `winx` x `winy` pixels; sets `winmat` and `viewmat`.
    pub fn update(&mut self, v3d: &View3D, winx: i32, winy: i32) {
        self.winx = winx;
        let (vp, clipsta, clipend) = viewplane_get(v3d, self.ortho, self.dist, winx, winy);
        self.winmat = if self.ortho {
            ortho_m4(vp.xmin, vp.xmax, vp.ymin, vp.ymax, clipsta, clipend)
        } else {
            frustum_m4(vp.xmin, vp.xmax, vp.ymin, vp.ymax, clipsta, clipend)
        };

        let mut viewmat = quat_to_mat4(self.viewquat);
        if !self.ortho {
            viewmat[3][2] -= self.dist;
        }
        translate_m4(&mut viewmat, self.ofs[0], self.ofs[1], self.ofs[2]);
        self.viewmat = viewmat;

        /* ED_view3d_update_viewmat(): update utility matrices. */
        self.persmat = mul_m4_m4m4(&self.winmat, &self.viewmat);
        self.persinv = invert_m4_m4(&self.persmat);
        self.viewinv = invert_m4_m4(&self.viewmat);

        /* Pixel-size factor (no selection rect, so the scale is the larger region side). */
        let p = &self.persmat;
        let len_sq_x = p[0][0] * p[0][0] + p[1][0] * p[1][0] + p[2][0] * p[2][0];
        let len_sq_y = p[0][1] * p[0][1] + p[1][1] * p[1][1] + p[2][1] * p[2][1];
        let len_px = 2.0 / len_sq_x.min(len_sq_y).sqrt();
        let len_sc = winx.max(winy) as f32;
        self.pixsize = len_px / len_sc;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() <= eps
    }

    /* Default view, 200x100 region, border x 50..150, y 25..75 around the centre: the origin is inside
     * all four planes, a point 10 units to the right (pixel x = 238) or above (pixel y = 189) is outside. */
    #[test]
    fn clipping_from_rect_planes() {
        let mut rv3d = RegionView3D::default();
        rv3d.update(&View3D::default(), 200, 100);
        let planes = rv3d.clipping_from_rect(200, 100, [50, 150, 25, 75]);
        let dist = |p: &[f32; 4], c: [f32; 3]| p[0] * c[0] + p[1] * c[1] + p[2] * c[2] + p[3];
        assert!(planes.iter().all(|p| dist(p, [0.0; 3]) > 0.0));
        assert!(planes.iter().any(|p| dist(p, [10.0, 0.0, 0.0]) < 0.0));
        assert!(planes.iter().any(|p| dist(p, [0.0, 10.0, 0.0]) < 0.0));
    }

    /* Without a clipping border the bounds are the cube's; with a border that keeps x and y within 0.5
     * (planes keep `dot(plane.xyz, co) + plane.w >= 0`) and does not limit z, the box shrinks to
     * 0.5 x 0.5 x 1 on each side of the origin. */
    #[test]
    fn visible_bounds_follow_clipping() {
        let mut rv3d = RegionView3D::default();
        assert_eq!(rv3d.visible_bounds(), (DEFAULT_CUBE_MIN, DEFAULT_CUBE_MAX));

        rv3d.clip = Some([[1.0, 0.0, 0.0, 0.5], [-1.0, 0.0, 0.0, 0.5], [0.0, 1.0, 0.0, 0.5], [0.0, -1.0, 0.0, 0.5]]);
        let (min, max) = rv3d.visible_bounds();
        for i in 0..3 {
            let e = if i == 2 { 1.0 } else { 0.5 };
            assert!(close(min[i], -e, 1e-5) && close(max[i], e, 1e-5), "{:?} {:?}", min, max);
        }
    }
    /* 1000x500 region: frame_aspect 0.5, horizontal fit, viewfac 1000.
     * pixsize = (36 * 0.01 / 50) / 1000 * 2 = 1.44e-5, so the view plane is
     * +-0.0072 x +-0.0036 and the frustum scales are 2 * 0.01 / 0.0144 and 2 * 0.01 / 0.0072. */
    #[test]
    fn default_projection_1000x500() {
        let mut rv3d = RegionView3D::default();
        rv3d.update(&View3D::default(), 1000, 500);
        let w = rv3d.winmat;
        assert!(close(w[0][0], 1.388_889, 1e-4), "{}", w[0][0]);
        assert!(close(w[1][1], 2.777_778, 1e-4), "{}", w[1][1]);
        assert!(close(w[2][0], 0.0, 1e-6));
        assert!(close(w[2][1], 0.0, 1e-6));
        assert!(close(w[2][2], -1.000_02, 1e-4), "{}", w[2][2]);
        assert!(close(w[3][2], -0.020_000_2, 1e-5), "{}", w[3][2]);
        assert_eq!(w[2][3], -1.0);
        assert_eq!(w[3][3], 0.0);
    }

    /* Identity rotation, perspective: the camera sits `dist` back along +Z. */
    #[test]
    fn default_view_matrix() {
        let mut rv3d = RegionView3D::default();
        rv3d.update(&View3D::default(), 1000, 500);
        let mut expected = IDENTITY;
        expected[3][2] = -10.0;
        assert_eq!(rv3d.viewmat, expected);
    }

    /* viewinv moves the camera to +10 on Z, persinv undoes persmat, and for the 1000x500
     * region pixsize = (2 / 1.388889) / 1000. */
    #[test]
    fn derived_matrices_1000x500() {
        let mut rv3d = RegionView3D::default();
        rv3d.update(&View3D::default(), 1000, 500);

        assert!(close(rv3d.viewinv[3][2], 10.0, 1e-5), "{}", rv3d.viewinv[3][2]);
        assert!(close(rv3d.viewinv[2][2], 1.0, 1e-6));

        let id = mul_m4_m4m4(&rv3d.persinv, &rv3d.persmat);
        for j in 0..4 {
            for k in 0..4 {
                let want = if j == k { 1.0 } else { 0.0 };
                assert!(close(id[j][k], want, 1e-3), "[{j}][{k}] = {}", id[j][k]);
            }
        }

        assert!(close(rv3d.pixsize, 0.00144, 1e-6), "{}", rv3d.pixsize);
    }
    /* A 90 degree rotation about X: q = (cos 45, sin 45, 0, 0). */
    #[test]
    fn quat_rotation_about_x() {
        let h = std::f32::consts::FRAC_1_SQRT_2;
        let m = quat_to_mat4([h, h, 0.0, 0.0]);
        assert!(close(m[1][1], 0.0, 1e-6) && close(m[1][2], 1.0, 1e-6));
        assert!(close(m[2][1], -1.0, 1e-6) && close(m[2][2], 0.0, 1e-6));
        assert!(close(m[0][0], 1.0, 1e-6));
    }

    /* The startup rotation is a unit quaternion; at dist 10 the camera is `dist` along the third
     * row of the rotation: +X, -Y and above the floor (z = 1 - 2 (x^2 + y^2) = 0.4458). */
    #[test]
    fn startup_view_camera_position() {
        let q = STARTUP_VIEWQUAT;
        assert!(close(q.iter().map(|v| v * v).sum::<f32>(), 1.0, 1e-6));

        let mut rv3d = RegionView3D::default();
        rv3d.viewquat = q;
        rv3d.update(&View3D::default(), 1000, 500);
        let cam = rv3d.viewinv[3];
        assert!(close(cam[0], 8.188_282, 1e-3), "{:?}", cam);
        assert!(close(cam[1], -3.616_649, 1e-3), "{:?}", cam);
        assert!(close(cam[2], 4.457_789, 1e-3), "{:?}", cam);
    }

    /* Each step scales the distance by 1.2 (out) or 1 / 1.2 (in); zooming out stops at the soft
     * maximum `clip_end * 10` and zooming in at `grid * 0.001`. */
    #[test]
    fn zoom_step_scales_distance() {
        let v3d = View3D::default();
        let mut rv3d = RegionView3D::default();
        rv3d.zoom_step(&v3d, 1);
        assert!(close(rv3d.dist, 10.0 / 1.2, 1e-5), "{}", rv3d.dist);
        rv3d.dist = 10.0;
        rv3d.zoom_step(&v3d, -1);
        rv3d.zoom_step(&v3d, -1);
        assert!(close(rv3d.dist, 14.4, 1e-4), "{}", rv3d.dist);

        rv3d.dist = 10000.0;
        rv3d.zoom_step(&v3d, -1);
        assert_eq!(rv3d.dist, 10000.0);
        rv3d.dist = 0.001;
        rv3d.zoom_step(&v3d, 1);
        assert_eq!(rv3d.dist, 0.001);
    }

    /* With 1 degree per pixel and the identity view, 90 px right is a 90 degree turn about Z
     * (-90 when upside down) and 90 px down is a 90 degree turn about the view X axis. */
    #[test]
    fn orbit_turntable_axes() {
        let s = std::f32::consts::PI / 180.0;
        let h = std::f32::consts::FRAC_1_SQRT_2;
        let check = |q: [f32; 4], e: [f32; 4]| {
            for i in 0..4 {
                assert!(close(q[i], e[i], 1e-6), "{:?} vs {:?}", q, e);
            }
        };

        let mut rv3d = RegionView3D::default();
        rv3d.orbit_turntable(1.0, s, 90, 0);
        check(rv3d.viewquat, [h, 0.0, 0.0, h]);

        let mut rv3d = RegionView3D::default();
        rv3d.orbit_turntable(-1.0, s, 90, 0);
        check(rv3d.viewquat, [h, 0.0, 0.0, -h]);

        let mut rv3d = RegionView3D::default();
        rv3d.orbit_turntable(1.0, s, 0, -90);
        check(rv3d.viewquat, [h, h, 0.0, 0.0]);
    }

    /* Default view, 1000x500 region: at distance 10 the view is 14.4 units wide, so dragging 100 px
     * to the right moves the scene 1.44 units right, i.e. `ofs` (the negated view centre) +1.44. */
    #[test]
    fn pan_moves_offset() {
        let mut rv3d = RegionView3D::default();
        rv3d.update(&View3D::default(), 1000, 500);
        let zfac = rv3d.calc_zfac([0.0; 3]);
        assert!(close(zfac, 10.0, 1e-4), "{zfac}");

        rv3d.pan(1000, 500, zfac, 100, 0);
        assert!(close(rv3d.ofs[0], 1.44, 1e-3), "{:?}", rv3d.ofs);
        assert!(close(rv3d.ofs[1], 0.0, 1e-4) && close(rv3d.ofs[2], 0.0, 1e-4), "{:?}", rv3d.ofs);
    }

    /* The origin is in the middle of the region; a point far behind the camera is near-clipped. */
    #[test]
    fn project_int_clip_near() {
        let mut rv3d = RegionView3D::default();
        rv3d.update(&View3D::default(), 200, 100);
        assert_eq!(rv3d.project_int_clip_near(200, 100, [0.0; 3]), Some([100, 50]));
        assert_eq!(rv3d.project_int_clip_near(200, 100, [0.0, 0.0, 1000.0]), None);
    }
    /* Region top at y = 499, drag from y = 250 up to y = 350: len_old = 254, len_new = 154, so
     * dist = 10 * (2 * (154 / 254 - 1) + 1); a huge drag down hits the soft maximum 10000. */
    #[test]
    fn zoom_drag_dolly() {
        let v3d = View3D::default();
        let mut rv3d = RegionView3D::default();
        rv3d.zoom_drag(&v3d, 1.0, 499, 250, 350, 10.0);
        assert!(close(rv3d.dist, 2.125_984, 1e-4), "{}", rv3d.dist);

        let mut rv3d = RegionView3D::default();
        rv3d.zoom_drag(&v3d, 1.0, 499, 250, -1_000_000, 10.0);
        assert_eq!(rv3d.dist, 10000.0);
    }

    /* `view_axis_exec()` with `relative`: the axis view reached by turning the view about its own
     * up / right axis, like dragging the scene (Front + Top gives the Bottom view, Front + Right
     * the Left view); only the top and bottom views keep a roll. */
    #[test]
    fn view_axis_relative_from_front_and_right() {
        let from = |view: View, to: View| {
            let mut rv3d = RegionView3D::default();
            rv3d.view_axis(view);
            rv3d.view_axis_relative(to);
            (rv3d.view, rv3d.view_axis_roll, rv3d.ortho)
        };
        assert_eq!(from(View::Front, View::Top), (View::Bottom, 0, true));
        assert_eq!(from(View::Front, View::Right), (View::Left, 0, true));
        assert_eq!(from(View::Right, View::Right), (View::Front, 0, true));
        assert_eq!(from(View::Right, View::Top), (View::Bottom, 1, true));
        assert_eq!(from(View::Top, View::Front), (View::Top, 0, true));
    }
    /* Top to Front is a 90 degree rotation, so it takes half of `smooth_viewtx` (0.1 s); the view is
     * a user view that starts at the old rotation, is eased half way at 0.05 s (a 45 degree turn
     * about X) and ends on the Front view with its axis view restored. */
    #[test]
    fn smooth_view_rotation_only() {
        let mut rv3d = RegionView3D::default();
        let src = rv3d.state_backup();
        rv3d.view_axis(View::Front);
        let sms = rv3d.smooth_view_begin(src, SMOOTH_VIEWTX).unwrap();
        assert!(close(sms.time_allowed as f32, 0.1, 1e-6), "{}", sms.time_allowed);
        assert_eq!((rv3d.view, rv3d.ortho, rv3d.viewquat), (View::User, true, [1.0, 0.0, 0.0, 0.0]));

        let a = 22.5f32.to_radians();
        assert!(!rv3d.smooth_view_apply(&sms, 0.05));
        let want = [a.cos(), -a.sin(), 0.0, 0.0];
        for i in 0..4 {
            assert!(close(rv3d.viewquat[i], want[i], 1e-5), "{:?}", rv3d.viewquat);
        }

        assert!(rv3d.smooth_view_apply(&sms, 0.1));
        assert_eq!((rv3d.view, rv3d.view_axis_roll), (View::Front, 0));
        let front = quat_from_axis_view(View::Front, 0).unwrap();
        for i in 0..4 {
            assert!(close(rv3d.viewquat[i], front[i], 1e-5), "{:?}", rv3d.viewquat);
        }
    }

    /* A change of the offset takes the whole `smooth_viewtx`; no change gives no smooth view. */
    #[test]
    fn smooth_view_offset_and_unchanged() {
        let mut rv3d = RegionView3D::default();
        let src = rv3d.state_backup();
        assert!(rv3d.smooth_view_begin(src, SMOOTH_VIEWTX).is_none());

        rv3d.center_cursor([1.0, 2.0, 3.0]);
        let sms = rv3d.smooth_view_begin(src, SMOOTH_VIEWTX).unwrap();
        assert_eq!(sms.time_allowed, SMOOTH_VIEWTX);
        assert!(!rv3d.smooth_view_apply(&sms, 0.1));
        assert!(close(rv3d.ofs[0], -0.5, 1e-5) && close(rv3d.ofs[1], -1.0, 1e-5) && close(rv3d.ofs[2], -1.5, 1e-5), "{:?}", rv3d.ofs);
        assert!(rv3d.smooth_view_apply(&sms, 0.25));
        assert_eq!(rv3d.ofs, [-1.0, -2.0, -3.0]);
    }
    /* The cursor at (1, 2, 3) is moved to the view centre by `ofs = -cursor`: it then projects to
     * the middle of the region. */
    #[test]
    fn center_cursor_sets_offset() {
        let mut rv3d = RegionView3D::default();
        rv3d.center_cursor([1.0, 2.0, 3.0]);
        assert_eq!(rv3d.ofs, [-1.0, -2.0, -3.0]);
        rv3d.update(&View3D::default(), 200, 100);
        assert_eq!(rv3d.project_int_clip_near(200, 100, [1.0, 2.0, 3.0]), Some([100, 50]));
    }
    /* 1000x500 region, orthographic, distance 10: `ortho_scale = 10 * 36 / 50 = 7.2`, so the view
     * plane is +-7.2 x +-3.6 and the clip range is +-500; the view matrix has no distance offset. */
    #[test]
    fn ortho_projection_1000x500() {
        let mut rv3d = RegionView3D::default();
        rv3d.toggle_persp_ortho();
        rv3d.update(&View3D::default(), 1000, 500);
        let w = rv3d.winmat;
        assert!(close(w[0][0], 1.0 / 7.2, 1e-5), "{}", w[0][0]);
        assert!(close(w[1][1], 1.0 / 3.6, 1e-5), "{}", w[1][1]);
        assert!(close(w[2][2], -0.002, 1e-6), "{}", w[2][2]);
        assert!(close(w[3][2], 0.0, 1e-6) && w[3][3] == 1.0 && w[2][3] == 0.0);
        assert_eq!(rv3d.viewmat[3][2], 0.0);
        assert!(close(rv3d.pixsize, 0.0144, 1e-6), "{}", rv3d.pixsize);
    }

    /* Numpad 7 snaps to the table quaternion and goes orthographic; a mouse orbit start switches back
     * to perspective and the drag leaves the axis view; a 180 degree step from the front view
     * lands on the back view. */
    #[test]
    fn view_axis_and_opposite() {
        let mut rv3d = RegionView3D::default();
        rv3d.view_axis(View::Top);
        assert_eq!((rv3d.view, rv3d.ortho, rv3d.viewquat), (View::Top, true, [1.0, 0.0, 0.0, 0.0]));
        rv3d.persp_ensure();
        assert!(!rv3d.ortho && rv3d.view == View::Top);
        rv3d.orbit_turntable(1.0, 0.01, 1, 0);
        assert_eq!(rv3d.view, View::User);

        rv3d.view_axis(View::Front);
        assert_eq!(rv3d.viewquat, [SQRT1_2, -SQRT1_2, 0.0, 0.0]);
        rv3d.orbit_step(true, -std::f32::consts::PI);
        assert_eq!((rv3d.view, rv3d.viewquat), (View::Back, [0.0, 0.0, -SQRT1_2, -SQRT1_2]));
    }
    /* The Front view turned 5 degrees about Z snaps back to exactly the Front table quaternion and
     * becomes orthographic; without snap the turned rotation stays a user view. A view far from
     * every snap direction gives back the perspective the drag started with. */
    #[test]
    fn rotate_snap_to_front() {
        let front = quat_from_axis_view(View::Front, 0).unwrap();
        let turned = normalize_qt(mul_qt_qtqt(front, axis_angle_to_quat([0.0, 0.0, 1.0], 5.0f32.to_radians())));

        let mut rv3d = RegionView3D::default();
        let mut curr = turned;
        rv3d.rotate_apply(&mut curr, 1.0, 0.01, 0, 0, true, false);
        assert_eq!((rv3d.view, rv3d.ortho, rv3d.viewquat), (View::Front, true, front));

        rv3d.rotate_apply(&mut curr, 1.0, 0.01, 0, 0, false, false);
        assert_eq!(rv3d.view, View::User);
        for i in 0..4 {
            assert!(close(rv3d.viewquat[i], turned[i], 1e-5), "{:?} vs {:?}", rv3d.viewquat, turned);
        }

        let mut rv3d = RegionView3D::default();
        rv3d.ortho = true;
        rv3d.rotate_snap(STARTUP_VIEWQUAT, false);
        assert!(!rv3d.ortho && rv3d.view == View::User);
    }
    /* Orthographic, 200x100 region: 50 px right of the centre is a quarter of the 14.4 unit wide
     * view, i.e. 3.6 units, whatever the depth. */
    #[test]
    fn cursor3d_position_ortho() {
        let mut rv3d = RegionView3D::default();
        rv3d.toggle_persp_ortho();
        rv3d.update(&View3D::default(), 200, 100);
        let c = rv3d.cursor3d_position(200, 100, [150, 50], [0.0; 3]);
        assert!(close(c[0], 3.6, 1e-3) && close(c[1], 0.0, 1e-4) && close(c[2], 0.0, 1e-4), "{:?}", c);
    }
    /* Default view, 200x100 region: the region centre hits the cursor plane at the origin; 50 px
     * right of it is a quarter of the 14.4 unit wide view at distance 10, i.e. 3.6 units. */
    #[test]
    fn cursor3d_position_on_view_plane() {
        let mut rv3d = RegionView3D::default();
        rv3d.update(&View3D::default(), 200, 100);
        let c = rv3d.cursor3d_position(200, 100, [100, 50], [0.0; 3]);
        assert!(close(c[0], 0.0, 1e-4) && close(c[1], 0.0, 1e-4) && close(c[2], 0.0, 1e-4), "{:?}", c);
        let c = rv3d.cursor3d_position(200, 100, [150, 50], [0.0; 3]);
        assert!(close(c[0], 3.6, 1e-3) && close(c[1], 0.0, 1e-4) && close(c[2], 0.0, 1e-4), "{:?}", c);
    }
}
