//! Geometry icons (`release/datafiles/icons/*.dat`, the toolbar tool icons): a `VCO\0` file of coloured triangles.
//! Sources: `BKE_icon_geom_from_memory` (icons.cc:609) for the format and `BKE_icon_geom_rasterize`
//! (icons_rasterize.cc:60) for the drawing: a 256x256 canvas (at least; twice the size above that) filled triangle
//! by triangle, flat when the three corner colours are equal and otherwise smooth by barycentric weights, then
//! box-scaled to the icon size. Deviation: the triangle scan is a plain pixel-centre test, not
//! `BLI_bitmap_draw_2d_tri_v2i`; the box scale is the simple average of the covered source pixels.

/// `Icon_Geom`: `coords_len` triangles, three corners each.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IconGeom {
    pub coords_range: [u8; 2],
    /// Three `[x, y]` corners per triangle (0..=255).
    pub coords: Vec<[u8; 2]>,
    /// One RGBA colour per corner, same length as `coords`.
    pub colors: Vec<[u8; 4]>,
}

impl IconGeom {
    pub fn triangles(&self) -> usize {
        self.coords.len() / 3
    }
}

/// `BKE_icon_geom_from_memory`: `None` when the data is not a whole number of triangles or lacks the `VCO\0` header.
pub fn from_memory(data: &[u8]) -> Option<IconGeom> {
    if data.len() <= 8 {
        return None;
    }
    let body = data.len() - 8;
    const DIV: usize = 3 * 2 * 3;
    if body % DIV != 0 || data[..4] != [b'V', b'C', b'O', 0] {
        return None;
    }
    let p = &data[8..];
    let coords = p[..body / 3].chunks_exact(2).map(|c| [c[0], c[1]]).collect();
    let colors = p[body / 3..].chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]).collect();
    Some(IconGeom { coords_range: [data[4], data[5]], coords, colors })
}

/// `BKE_icon_geom_rasterize`: RGBA bytes, `size_x * size_y * 4`, row 0 at the bottom as in an `ImBuf`.
pub fn rasterize(geom: &IconGeom, size_x: usize, size_y: usize) -> Vec<u8> {
    let rw = 256.max(size_x * 2);
    let rh = 256.max(size_y * 2);
    let (sx, sy) = (rw as f32 / 256.0, rh as f32 / 256.0);
    let mut rect = vec![[0u8; 4]; rw * rh];
    for t in 0..geom.triangles() {
        let pt: [[f32; 2]; 3] = std::array::from_fn(|i| {
            let c = geom.coords[3 * t + i];
            [(c[0] as f32 * sx).floor(), (c[1] as f32 * sy).floor()]
        });
        let col = [geom.colors[3 * t], geom.colors[3 * t + 1], geom.colors[3 * t + 2]];
        let flat = col[0] == col[1] && col[0] == col[2];
        let (x0, x1) = (pt.iter().map(|p| p[0]).fold(f32::MAX, f32::min) as usize, pt.iter().map(|p| p[0]).fold(0.0, f32::max) as usize);
        let (y0, y1) = (pt.iter().map(|p| p[1]).fold(f32::MAX, f32::min) as usize, pt.iter().map(|p| p[1]).fold(0.0, f32::max) as usize);
        let area = (pt[1][0] - pt[0][0]) * (pt[2][1] - pt[0][1]) - (pt[2][0] - pt[0][0]) * (pt[1][1] - pt[0][1]);
        if area == 0.0 {
            continue;
        }
        for y in y0..=y1.min(rh - 1) {
            for x in x0..=x1.min(rw - 1) {
                let p = [x as f32 + 0.5, y as f32 + 0.5];
                let w0 = ((pt[1][0] - p[0]) * (pt[2][1] - p[1]) - (pt[2][0] - p[0]) * (pt[1][1] - p[1])) / area;
                let w1 = ((pt[2][0] - p[0]) * (pt[0][1] - p[1]) - (pt[0][0] - p[0]) * (pt[2][1] - p[1])) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                rect[y * rw + x] = if flat {
                    col[0]
                } else {
                    let w = [w0, w1, w2];
                    std::array::from_fn(|ch| (0..3).map(|k| col[k][ch] as f32 * w[k]).sum::<f32>().round().clamp(0.0, 255.0) as u8)
                };
            }
        }
    }
    /* `IMB_scale(.., Box)`: average of the covered pixels. */
    let mut out = vec![0u8; size_x * size_y * 4];
    for y in 0..size_y {
        for x in 0..size_x {
            let (xa, xb) = (x * rw / size_x, ((x + 1) * rw / size_x).max(x * rw / size_x + 1));
            let (ya, yb) = (y * rh / size_y, ((y + 1) * rh / size_y).max(y * rh / size_y + 1));
            let mut acc = [0u32; 4];
            for yy in ya..yb {
                for xx in xa..xb {
                    for (a, v) in acc.iter_mut().zip(rect[yy * rw + xx]) {
                        *a += v as u32;
                    }
                }
            }
            let n = ((xb - xa) * (yb - ya)) as u32;
            for ch in 0..4 {
                out[(y * size_x + x) * 4 + ch] = (acc[ch] / n) as u8;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One triangle covering the lower left half of the 256 canvas, in one flat colour.
    fn file(colors: [[u8; 4]; 3]) -> Vec<u8> {
        let mut d = vec![b'V', b'C', b'O', 0, 255, 255, 0, 0];
        d.extend([0, 0, 255, 0, 0, 255]);
        d.extend(colors.iter().flatten());
        d
    }

    #[test]
    fn parses_the_header_and_whole_triangles_only() {
        let red = [255, 0, 0, 255];
        let g = from_memory(&file([red; 3])).unwrap();
        assert_eq!((g.triangles(), g.coords_range), (1, [255, 255]));
        assert_eq!(g.coords, [[0, 0], [255, 0], [0, 255]]);
        assert_eq!(g.colors, [red; 3]);
        let mut bad = file([red; 3]);
        bad[0] = b'X';
        assert!(from_memory(&bad).is_none(), "header");
        assert!(from_memory(&file([red; 3])[..file([red; 3]).len() - 1]).is_none(), "partial triangle");
        assert!(from_memory(&[b'V', b'C', b'O', 0, 0, 0, 0, 0]).is_none(), "no body");
    }

    #[test]
    fn rasterizes_flat_and_smooth_triangles() {
        let red = [255, 0, 0, 255];
        let img = rasterize(&from_memory(&file([red; 3])).unwrap(), 16, 16);
        assert_eq!(&img[(1 * 16 + 1) * 4..][..4], red, "inside the triangle");
        assert_eq!(&img[(14 * 16 + 14) * 4..][..4], [0, 0, 0, 0], "outside it stays transparent");
        let blue = [0, 0, 255, 255];
        let smooth = rasterize(&from_memory(&file([red, blue, blue])).unwrap(), 16, 16);
        let near_red = smooth[(1 * 16 + 1) * 4];
        let near_blue = smooth[(1 * 16 + 12) * 4];
        assert!(near_red > 200 && near_blue < 60, "red fades towards the blue corners: {near_red} {near_blue}");
    }
}
