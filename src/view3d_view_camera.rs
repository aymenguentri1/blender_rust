//! `view3d.view_camera` (Numpad 0), `view_camera_exec()` (editors/space_view3d/view3d_navigate_view_camera.cc:30-110)
//! and `ED_view3d_lastview_store()` (editors/space_view3d/view3d_utils.cc:513-521). Pure state logic: the
//! smooth view animation and the camera object lookup are done by the caller.
//! Deviation: `view_camera_exec()` returns to the last view through `axis_set_view()`; here the stored
//! quaternion / view / roll / persp are returned for the caller to apply.

/// `eRegionView3D_Persp` (makesdna/DNA_view3d_types.h:86-90).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Persp {
    Ortho,
    Persp,
    Camob,
}

/// The persp/last-view fields of `RegionView3D` used by the operator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraViewState {
    pub persp: Persp,
    pub viewquat: [f32; 4],
    pub view_axis_roll: i32,
    /// `lviewquat`, `lview_axis_roll`, `lpersp` (never `Camob`: DNA_view3d_types.h:528).
    pub lviewquat: [f32; 4],
    pub lview_axis_roll: i32,
    pub lpersp: Persp,
}

impl CameraViewState {
    /// `ED_view3d_lastview_store()`.
    pub fn lastview_store(&mut self) {
        self.lviewquat = self.viewquat;
        self.lview_axis_roll = self.view_axis_roll;
        if self.persp != Persp::Camob {
            self.lpersp = self.persp;
        }
    }

    /// `view_camera_exec()` outside a locked transform: from a non-camera view the current view is stored
    /// (unless a smooth view runs) and `persp` becomes `Camob`, `None` when no camera exists
    /// (`OPERATOR_CANCELLED`); from the camera view it restores the last view and returns its quaternion.
    pub fn toggle_camera(&mut self, has_camera: bool, smooth_running: bool) -> Option<[f32; 4]> {
        if self.persp != Persp::Camob {
            if !smooth_running {
                self.lastview_store();
            }
            if !has_camera {
                return None;
            }
            self.persp = Persp::Camob;
            Some(self.viewquat)
        } else {
            self.persp = self.lpersp;
            self.viewquat = self.lviewquat;
            self.view_axis_roll = self.lview_axis_roll;
            Some(self.viewquat)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> CameraViewState {
        CameraViewState {
            persp: Persp::Ortho,
            viewquat: [0.5, 0.5, 0.5, 0.5],
            view_axis_roll: 1,
            lviewquat: [1.0, 0.0, 0.0, 0.0],
            lview_axis_roll: 0,
            lpersp: Persp::Persp,
        }
    }

    #[test]
    fn camera_view_stores_last_view_and_sets_camob() {
        let mut s = state();
        assert!(s.toggle_camera(true, false).is_some());
        assert_eq!(s.persp, Persp::Camob);
        assert_eq!((s.lviewquat, s.lview_axis_roll, s.lpersp), ([0.5; 4], 1, Persp::Ortho));
    }

    #[test]
    fn toggling_back_restores_previous_perspective() {
        let mut s = state();
        s.toggle_camera(true, false);
        s.viewquat = [1.0, 0.0, 0.0, 0.0];
        assert_eq!(s.toggle_camera(true, false), Some([0.5; 4]));
        assert_eq!((s.persp, s.view_axis_roll), (Persp::Ortho, 1));
    }

    #[test]
    fn no_camera_cancels_and_smooth_view_keeps_stored_view() {
        let mut s = state();
        assert_eq!(s.toggle_camera(false, false), None);
        assert_eq!(s.persp, Persp::Ortho);
        let mut s = state();
        s.toggle_camera(true, true);
        assert_eq!((s.lviewquat, s.lpersp), ([1.0, 0.0, 0.0, 0.0], Persp::Persp));
    }

    #[test]
    fn lpersp_never_becomes_camob() {
        let mut s = state();
        s.persp = Persp::Camob;
        s.lastview_store();
        assert_eq!(s.lpersp, Persp::Persp);
    }
}
