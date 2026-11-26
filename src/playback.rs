//! Timeline playback at the scene frame rate. Ports `screen_animation_step_invoke()`
//! (editors/screen/screen_ops.cc:6554, the non-audio-sync path), `Scene::frames_per_second()`
//! (blenkernel/intern/scene.cc:1706) and the timer scheduling of `wm_window_timers_process()`
//! (windowmanager/intern/wm_window.cc:2160-2196). Time comes from the caller, so a fake clock
//! drives it in tests. Audio sync, scene strips and sound handling are not modelled.

/// `eScenePlaybackLoopMode` (DNA_scene_types.h:2888); `Infinite` is the default.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LoopMode {
    #[default]
    Infinite,
    StopStartFrame,
    StopEndFrame,
    Restore,
    Bounce,
}

/// `Scene::frames_per_second()`: `frs_sec / frs_sec_base`.
pub fn frames_per_second(frs_sec: i32, frs_sec_base: f32) -> f64 {
    frs_sec as f64 / frs_sec_base as f64
}

/// The `wmTimer` fields used by playback.
#[derive(Clone, Debug)]
struct Timer {
    step: f64,
    start: f64,
    last: f64,
    next: f64,
    duration: f64,
}

/// `ScreenAnimData` + its `wmTimer`, created by `ED_screen_animation_play()`.
#[derive(Clone, Debug)]
pub struct Playback {
    timer: Timer,
    fps: f64,
    pub reverse: bool,
    /// `sync` = `SCE_FRAME_DROP`: step by the elapsed time instead of one frame.
    pub frame_drop: bool,
    pub loop_mode: LoopMode,
    lagging: f64,
    /// `sad->sfra`: the frame playback started on (`LoopMode::Restore`).
    start_frame: i32,
    pub playing: bool,
}

impl Playback {
    /// Starts at time `now` on frame `cfra`; the timer step is `1 / fps` (screen_ops.cc:6845).
    pub fn start(now: f64, fps: f64, cfra: i32, reverse: bool, frame_drop: bool, loop_mode: LoopMode) -> Self {
        let step = 1.0 / fps;
        Playback {
            timer: Timer { step, start: now, last: now, next: now + step, duration: 0.0 },
            fps,
            reverse,
            frame_drop,
            loop_mode,
            lagging: 0.0,
            start_frame: cfra,
            playing: true,
        }
    }

    /// Time of the next timer event.
    pub fn next_time(&self) -> f64 {
        self.timer.next
    }

    /// One main-loop pass at time `now`: when the timer is due, steps `cfra` within
    /// `start..=end` and returns the new frame.
    pub fn poll(&mut self, now: f64, cfra: &mut i32, start: i32, end: i32) -> Option<i32> {
        let t = &mut self.timer;
        if !self.playing || t.next >= now {
            return None;
        }
        let delta = now - t.last;
        t.duration += delta;
        t.last = now;
        t.next = t.start + t.step * (t.duration / t.step).ceil();
        self.step(delta, cfra, start, end);
        Some(*cfra)
    }

    fn step(&mut self, time_delta: f64, cfra: &mut i32, start: i32, end: i32) {
        let dir = if self.reverse { -1 } else { 1 };
        if self.frame_drop {
            let mut delta_frames = time_delta * self.fps + self.lagging;
            if delta_frames < 1.0 {
                delta_frames = 1.0;
                self.lagging = 0.0;
            } else {
                self.lagging = delta_frames - delta_frames.trunc();
            }
            *cfra += dir * delta_frames as i32;
        } else {
            *cfra += dir;
        }
        let forward = !self.reverse;
        let extreme = if forward { *cfra > end } else { *cfra < start };
        if !extreme {
            return;
        }
        match self.loop_mode {
            LoopMode::StopStartFrame => {
                self.playing = false;
                *cfra = if forward { start } else { end };
            }
            LoopMode::Infinite => *cfra = if forward { start } else { end },
            LoopMode::StopEndFrame => {
                *cfra = (*cfra).clamp(start, end);
                self.playing = false;
            }
            LoopMode::Restore => {
                *cfra = self.start_frame;
                self.playing = false;
            }
            LoopMode::Bounce => {
                if forward {
                    self.reverse = true;
                    *cfra = end - 1;
                } else {
                    self.reverse = false;
                    *cfra = start + 1;
                }
                *cfra = (*cfra).clamp(start, end);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Advances a fake clock in `dt` ticks for `secs` seconds; returns the final frame.
    fn run(p: &mut Playback, cfra: &mut i32, dt: f64, secs: f64, range: (i32, i32)) {
        let mut now = 0.0;
        while now < secs {
            now += dt;
            p.poll(now, cfra, range.0, range.1);
        }
    }

    #[test]
    fn fps_is_sec_over_base() {
        assert_eq!(frames_per_second(24, 1.0), 24.0);
        assert!((frames_per_second(30, 1.001) - 29.97003).abs() < 1e-4);
    }

    #[test]
    fn advances_at_the_scene_rate_with_a_fine_clock() {
        let mut cfra = 1;
        let mut p = Playback::start(0.0, 24.0, cfra, false, false, LoopMode::Infinite);
        run(&mut p, &mut cfra, 0.001, 1.0, (1, 250));
        // One second at 24 fps is 24 frames (the last tick may land just before or after 1.0 s).
        assert!((24..=25).contains(&(cfra - 1)), "{cfra}");
    }

    #[test]
    fn no_timer_event_before_the_step_elapses() {
        let mut cfra = 1;
        let mut p = Playback::start(0.0, 24.0, cfra, false, false, LoopMode::Infinite);
        assert_eq!(p.poll(0.04, &mut cfra, 1, 250), None);
        assert_eq!(p.poll(0.0417, &mut cfra, 1, 250), Some(2));
    }

    #[test]
    fn slow_clock_without_frame_drop_steps_one_frame() {
        let mut cfra = 1;
        let mut p = Playback::start(0.0, 24.0, cfra, false, false, LoopMode::Infinite);
        p.poll(0.25, &mut cfra, 1, 250);
        assert_eq!(cfra, 2);
    }

    #[test]
    fn frame_drop_skips_frames_and_carries_the_remainder() {
        let mut cfra = 1;
        let mut p = Playback::start(0.0, 10.0, cfra, false, true, LoopMode::Infinite);
        p.poll(0.25, &mut cfra, 1, 250); // 2.5 frames: +2, lag 0.5
        assert_eq!(cfra, 3);
        p.poll(0.45, &mut cfra, 1, 250); // 2.0 + 0.5 = 2.5: +2
        assert_eq!(cfra, 5);
        p.poll(0.5001, &mut cfra, 1, 250); // under a frame: min one
        assert_eq!(cfra, 6);
    }

    #[test]
    fn infinite_loop_wraps_past_the_end_and_start() {
        let mut cfra = 10;
        let mut p = Playback::start(0.0, 1.0, cfra, false, false, LoopMode::Infinite);
        p.poll(2.0, &mut cfra, 1, 10);
        assert_eq!(cfra, 1);
        let mut cfra = 1;
        let mut p = Playback::start(0.0, 1.0, cfra, true, false, LoopMode::Infinite);
        p.poll(2.0, &mut cfra, 1, 10);
        assert_eq!(cfra, 10);
    }

    #[test]
    fn stop_modes() {
        let go = |mode, from| {
            let mut cfra = from;
            let mut p = Playback::start(0.0, 1.0, 5, false, false, mode);
            p.poll(2.0, &mut cfra, 1, 10);
            (cfra, p.playing)
        };
        assert_eq!(go(LoopMode::StopStartFrame, 10), (1, false));
        assert_eq!(go(LoopMode::StopEndFrame, 10), (10, false));
        assert_eq!(go(LoopMode::Restore, 10), (5, false));
    }

    #[test]
    fn bounce_reverses_at_both_ends() {
        let mut cfra = 10;
        let mut p = Playback::start(0.0, 1.0, cfra, false, false, LoopMode::Bounce);
        p.poll(2.0, &mut cfra, 1, 10);
        assert_eq!((cfra, p.reverse), (9, true));
        let mut cfra = 1;
        p.poll(4.0, &mut cfra, 1, 10);
        assert_eq!((cfra, p.reverse), (2, false));
    }
}
