//! Animated splash artwork: decodes an embedded GIF once into RGB frames plus per frame delays and answers "which
//! frame is showing `elapsed` ms after the splash opened". Frames are composited onto a canvas honouring the GIF
//! disposal methods. Blender itself shows a still (`wm_block_splash_image`); the animation is an addition.

use std::time::Duration;

pub struct Anim {
    pub w: i32,
    pub h: i32,
    /// `w * h * 3` bytes per frame, top row first.
    pub frames: Vec<Vec<u8>>,
    /// Display time of each frame in milliseconds (never below 20, as browsers clamp zero delays).
    pub delays: Vec<u32>,
    total: u64,
}

impl Anim {
    pub fn still(w: i32, h: i32, rgb: Vec<u8>) -> Anim {
        Anim { w, h, frames: vec![rgb], delays: vec![1000], total: 1000 }
    }

    pub fn is_animated(&self) -> bool {
        self.frames.len() > 1
    }

    /// Index of the frame shown `elapsed_ms` after the start, looping forever.
    pub fn frame_at(&self, elapsed_ms: u64) -> usize {
        let mut t = elapsed_ms % self.total.max(1);
        for (i, d) in self.delays.iter().enumerate() {
            if t < *d as u64 {
                return i;
            }
            t -= *d as u64;
        }
        0
    }

    /// Time until the frame shown at `elapsed_ms` is replaced.
    pub fn next_change(&self, elapsed_ms: u64) -> Duration {
        let mut t = elapsed_ms % self.total.max(1);
        for d in &self.delays {
            if t < *d as u64 {
                return Duration::from_millis(*d as u64 - t);
            }
            t -= *d as u64;
        }
        Duration::from_millis(20)
    }
}

pub fn decode(bytes: &[u8]) -> Option<Anim> {
    let mut opts = gif::DecodeOptions::new();
    opts.set_color_output(gif::ColorOutput::RGBA);
    let mut dec = opts.read_info(bytes).ok()?;
    let (w, h) = (dec.width() as usize, dec.height() as usize);
    let mut canvas = vec![0u8; w * h * 4];
    let (mut frames, mut delays) = (Vec::new(), Vec::new());
    while let Some(f) = dec.read_next_frame().ok()? {
        let backup = (f.dispose == gif::DisposalMethod::Previous).then(|| canvas.clone());
        let (fx, fy, fw, fh) = (f.left as usize, f.top as usize, f.width as usize, f.height as usize);
        for y in 0..fh.min(h.saturating_sub(fy)) {
            for x in 0..fw.min(w.saturating_sub(fx)) {
                let s = (y * fw + x) * 4;
                if f.buffer[s + 3] != 0 {
                    let d = ((fy + y) * w + fx + x) * 4;
                    canvas[d..d + 4].copy_from_slice(&f.buffer[s..s + 4]);
                }
            }
        }
        frames.push(canvas.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect::<Vec<u8>>());
        delays.push((f.delay as u32 * 10).max(20));
        match f.dispose {
            gif::DisposalMethod::Background => {
                for y in fy..(fy + fh).min(h) {
                    let (a, b) = ((y * w + fx) * 4, (y * w + (fx + fw).min(w)) * 4);
                    canvas[a..b].fill(0);
                }
            }
            gif::DisposalMethod::Previous => canvas = backup.unwrap(),
            _ => {}
        }
    }
    if frames.is_empty() {
        return None;
    }
    let total = delays.iter().map(|d| *d as u64).sum();
    Some(Anim { w: w as i32, h: h as i32, frames, delays, total })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anim() -> Anim {
        Anim { w: 1, h: 1, frames: vec![vec![0; 3]; 3], delays: vec![100, 50, 200], total: 350 }
    }

    #[test]
    fn frame_at_walks_the_delays_and_loops() {
        let a = anim();
        let got: Vec<usize> = [0, 99, 100, 149, 150, 349, 350, 450].iter().map(|t| a.frame_at(*t)).collect();
        assert_eq!(got, [0, 0, 1, 1, 2, 2, 0, 1]);
    }

    #[test]
    fn next_change_counts_down_inside_a_frame() {
        let a = anim();
        assert_eq!(a.next_change(30), Duration::from_millis(70));
        assert_eq!(a.next_change(100), Duration::from_millis(50));
    }

    #[test]
    fn the_embedded_gif_decodes_to_the_still_size_with_many_frames() {
        let a = decode(include_bytes!("../assets/splash.gif")).expect("gif");
        assert_eq!((a.w, a.h), (495, 247));
        assert!(a.is_animated() && a.frames.len() == a.delays.len());
        assert!(a.frames.iter().all(|f| f.len() == 495 * 247 * 3));
    }

    #[test]
    fn garbage_is_rejected() {
        assert!(decode(b"not a gif").is_none());
    }
}
