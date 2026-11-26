//! Wavefront OBJ reader: `v`, `vt`, `vn` and `f` records.
//!
//! Ported from `OBJParser` (io/wavefront_obj/importer/obj_import_file_reader.cc):
//!  - `read_next_obj_line()` (512): a `\` followed only by blanks and a newline joins the next line with a space
//!  - `parse_keyword()` (475): the keyword must be followed by a control character or blank, else it is not matched
//!  - `parse_floats()` (io/common/intern/string_utils.cc:127): numeric prefix per value, `1-2` reads as 1 and -2
//!  - `geom_add_vertex()` (87): 3 floats, missing ones 0 (the `xyzrgb` colour extension is not ported)
//!  - `geom_add_vertex_normal()` (139): 3 floats, normalised; `geom_add_uv_vertex()` (152): 2 floats
//!  - `geom_add_polygon()` (222): `v`, `v/vt`, `v//vn`, `v/vt/vn` corners; indices are 1-based, negative ones count
//!    from the end of what has been read so far; a corner that is out of range invalidates the whole face (it is
//!    dropped); UV / normal indices are ignored when the file has no UVs / normals (#103212, #98782); a `#` ends the
//!    face; text after the first `/`-group of a corner is skipped
//! Objects, groups, materials, smoothing, polylines and curves are not ported.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Corner {
    pub vert: usize,
    pub uv: Option<usize>,
    pub normal: Option<usize>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ObjData {
    pub vertices: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub normals: Vec<[f32; 3]>,
    pub faces: Vec<Vec<Corner>>,
    /// `has_invalid_faces_`: at least one face was dropped.
    pub has_invalid_faces: bool,
}

/// `read_next_obj_line()` applied to a whole buffer.
fn split_lines(text: &str) -> Vec<String> {
    let b = text.as_bytes();
    let mut lines = Vec::new();
    let mut cur: Vec<u8> = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        i += 1;
        if c == b'\\' {
            let mut a = i;
            while a < b.len() && b[a] <= b' ' && b[a] != b'\n' {
                a += 1;
            }
            if a < b.len() && b[a] == b'\n' {
                cur.push(b' ');
                i = a + 1;
            } else {
                cur.push(c);
            }
        } else if c == b'\n' {
            lines.push(String::from_utf8_lossy(&cur).into_owned());
            cur.clear();
        } else {
            cur.push(c);
        }
    }
    if !cur.is_empty() {
        lines.push(String::from_utf8_lossy(&cur).into_owned());
    }
    lines
}

/// `parse_keyword()`: the rest of the line after the keyword and one separator.
fn keyword<'a>(line: &'a str, kw: &str) -> Option<&'a str> {
    let b = line.as_bytes();
    if b.len() < kw.len() + 1 || &b[..kw.len()] != kw.as_bytes() || b[kw.len()] > b' ' {
        return None;
    }
    Some(&line[kw.len() + 1..])
}

/// `parse_float()` (io/common/intern/string_utils.cc:107): skips blanks, then takes the longest numeric prefix
/// (fast_float, general format with a leading `+`): sign, digits, `.`, digits, an exponent that has digits, or
/// `inf` / `infinity` / `nan`. `None` and the cursor unmoved when there is no number or the value overflows.
fn float_prefix(s: &str) -> (Option<f32>, &str) {
    let s = s.trim_start_matches([' ', '\t', '\n', '\x0b', '\x0c', '\r']);
    let b = s.as_bytes();
    let mut i = usize::from(!b.is_empty() && (b[0] == b'-' || b[0] == b'+'));
    let start_digits = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let mut ndigits = i - start_digits;
    if i < b.len() && b[i] == b'.' {
        let f = i + 1;
        let mut j = f;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        ndigits += j - f;
        if ndigits > 0 {
            i = j;
        }
    }
    if ndigits == 0 {
        let low = s[start_digits..].to_ascii_lowercase();
        for word in ["infinity", "inf", "nan"] {
            if low.starts_with(word) {
                let end = start_digits + word.len();
                return match s[..end].trim_start_matches('+').parse::<f32>() {
                    Ok(v) => (Some(v), &s[end..]),
                    Err(_) => (None, s),
                };
            }
        }
        return (None, s);
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'-' || b[j] == b'+') {
            j += 1;
        }
        let e = j;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j > e {
            i = j;
        }
    }
    match s[..i].trim_start_matches('+').parse::<f32>() {
        Ok(v) if v.is_finite() => (Some(v), &s[i..]),
        _ => (None, s),
    }
}

/// `parse_floats()` (string_utils.cc:127): `N` values, each the longest numeric prefix at the cursor, which moves
/// past it; a value that does not parse is `0` and the cursor stays (`require_trailing_space` is false here).
fn floats<const N: usize>(rest: &str) -> [f32; N] {
    let mut out = [0.0; N];
    let mut p = rest;
    for o in out.iter_mut() {
        if let (Some(v), r) = float_prefix(p) {
            *o = v;
            p = r;
        }
    }
    out
}

/// `parse_int()` on a prefix: optional sign and digits. Returns the value (if any) and the rest.
fn int_prefix(s: &str) -> (Option<i32>, &str) {
    let b = s.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'-' || b[i] == b'+') {
        i += 1;
    }
    let d = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    if i == d {
        return (None, s);
    }
    (s[..i].parse::<i32>().ok(), &s[i..])
}

/// Zero based index from a 1-based or negative OBJ index, `None` when out of `[0, n)`.
fn resolve(index: i32, n: usize) -> Option<usize> {
    let i = if index < 0 { index as i64 + n as i64 } else { index as i64 - 1 };
    (i >= 0 && (i as usize) < n).then_some(i as usize)
}

fn add_face(rest: &str, data: &mut ObjData) {
    let mut corners = Vec::new();
    let mut valid = true;
    let mut p = rest.trim_start_matches(|c: char| c <= ' ');
    while !p.is_empty() && valid {
        let (vi, mut q) = int_prefix(p);
        if q.starts_with('#') {
            break;
        }
        let (mut uv_i, mut n_i) = (None, None);
        if q.starts_with('/') {
            q = &q[1..];
            if !q.is_empty() && !q.starts_with('/') {
                let (v, r) = int_prefix(q);
                uv_i = v;
                q = r;
            }
            if q.starts_with('/') {
                let (v, r) = int_prefix(&q[1..]);
                n_i = v;
                q = r;
            }
        }
        let vert = vi.and_then(|v| resolve(v, data.vertices.len()));
        valid &= vert.is_some();
        let mut uv = None;
        if let (Some(i), false) = (uv_i, data.uvs.is_empty()) {
            uv = resolve(i, data.uvs.len());
            valid &= uv.is_some();
        }
        let mut normal = None;
        if let (Some(i), false) = (n_i, data.normals.is_empty()) {
            normal = resolve(i, data.normals.len());
            valid &= normal.is_some();
        }
        if let Some(vert) = vert {
            corners.push(Corner { vert, uv, normal });
        }
        /* Skip the rest of the token (extra stuff per face, #103441) and the blanks after it. */
        let skip = q.find(|c: char| c <= ' ').unwrap_or(q.len());
        p = q[skip..].trim_start_matches(|c: char| c <= ' ');
    }
    if valid {
        data.faces.push(corners);
    } else {
        data.has_invalid_faces = true;
    }
}

/// Parses OBJ text. Malformed lines never panic: unknown records are ignored, bad faces dropped.
pub fn parse(text: &str) -> ObjData {
    let mut data = ObjData::default();
    for line in split_lines(text) {
        let line = line.trim_start_matches(|c: char| c <= ' ');
        if line.is_empty() {
            continue;
        }
        if let Some(r) = keyword(line, "v") {
            data.vertices.push(floats::<3>(r));
        } else if let Some(r) = keyword(line, "vn") {
            let n = floats::<3>(r);
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            data.normals.push(if l > 1e-35 { [n[0] / l, n[1] / l, n[2] / l] } else { [0.0; 3] });
        } else if let Some(r) = keyword(line, "vt") {
            data.uvs.push(floats::<2>(r));
        } else if let Some(r) = keyword(line, "f") {
            add_face(r, &mut data);
        }
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    const CUBE: &str = "# cube\nv -1 -1 -1\nv 1 -1 -1\nv 1 1 -1\nv -1 1 -1\nv -1 -1 1\nv 1 -1 1\nv 1 1 1\nv -1 1 1\n\
vt 0 0\nvt 1 0\nvt 1 1\nvt 0 1\nvn 0 0 -2\nvn 0 0 1\n\
f 1/1/1 4/4/1 3/3/1 2/2/1\nf 5/1/2 6/2/2 7/3/2 8/4/2\nf 1 2 6 5\nf 2//1 3//1 7//1 6//1\nf 3 4 8 7\nf 4 1 5 8\n";

    #[test]
    fn cube_counts() {
        let d = parse(CUBE);
        assert_eq!((d.vertices.len(), d.uvs.len(), d.normals.len(), d.faces.len()), (8, 4, 2, 6));
        assert!(!d.has_invalid_faces);
        assert_eq!(d.normals[0], [0.0, 0.0, -1.0]);
        assert_eq!(d.faces[0][1], Corner { vert: 3, uv: Some(3), normal: Some(0) });
        assert_eq!(d.faces[3][0], Corner { vert: 1, uv: None, normal: Some(0) });
    }

    #[test]
    fn ngon_negative_indices_and_continuation() {
        let d = parse("v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0.5 1.5 0\nv 0 1 0\nf -5 -4 -3 \\\n -2 -1\n");
        assert_eq!(d.faces.len(), 1);
        let v: Vec<usize> = d.faces[0].iter().map(|c| c.vert).collect();
        assert_eq!(v, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn malformed_lines_do_not_panic() {
        let d = parse("v 1 2\nv\nv x y z\nf 1 2 9\nf a b c\nf 1 2 1/9\nvt\nvn 0 0 0\nf\nf 1 2 # 3\n\\\n");
        assert_eq!(d.vertices, vec![[1.0, 2.0, 0.0], [0.0; 3]]);
        assert_eq!(d.normals, vec![[0.0; 3]]);
        assert!(d.has_invalid_faces);
        assert_eq!(d.faces.len(), 2); /* "f 1 2 1/9" (no UVs: ignored) and "f 1 2 # 3" */
    }

    #[test]
    fn float_prefix_parsing() {
        let d = parse("v 1.0abc 2 3\nv 1-2 3\nvt .5e1x +2\nvn 1e 0 0\nv 1e+2 -.5 +3");
        assert_eq!(d.vertices[0], [1.0, 0.0, 0.0]); /* "abc" fails: stays 0 and so does everything after */
        assert_eq!(d.vertices[1], [1.0, -2.0, 3.0]);
        assert_eq!(d.uvs[0], [5.0, 0.0]); /* fails at "x": 0, cursor stays */
        assert_eq!(d.normals[0], [1.0, 0.0, 0.0]);
        assert_eq!(d.vertices[2], [100.0, -0.5, 3.0]);
    }

    #[test]
    fn uv_and_normal_indices_ignored_without_layers() {
        let d = parse("v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1/5/7 2/5/7 3/5/7\n");
        assert_eq!(d.faces[0][0], Corner { vert: 0, uv: None, normal: None });
    }
}
