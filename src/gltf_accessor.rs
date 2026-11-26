//! glTF accessor decoding (io_scene_gltf2/io/imp/gltf2_io_binary.py `decode_accessor_obj`, :112-212; component types and
//! element counts from io/com/constants.py:9-83). Works on already-sliced buffer view bytes; the buffer/URI/GLB loading and
//! meshopt are not ported. Values are returned as `f64` rows (exact for every glTF component type).

#![allow(dead_code)]

/// `ComponentType` (constants.py:9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComponentType {
    Byte = 5120,
    UnsignedByte = 5121,
    Short = 5122,
    UnsignedShort = 5123,
    UnsignedInt = 5125,
    Float = 5126,
}

impl ComponentType {
    pub fn from_code(code: u32) -> Option<Self> {
        Some(match code {
            5120 => Self::Byte,
            5121 => Self::UnsignedByte,
            5122 => Self::Short,
            5123 => Self::UnsignedShort,
            5125 => Self::UnsignedInt,
            5126 => Self::Float,
            _ => return None,
        })
    }

    pub fn size(self) -> usize {
        match self {
            Self::Byte | Self::UnsignedByte => 1,
            Self::Short | Self::UnsignedShort => 2,
            Self::UnsignedInt | Self::Float => 4,
        }
    }

    fn read(self, b: &[u8]) -> f64 {
        match self {
            Self::Byte => b[0] as i8 as f64,
            Self::UnsignedByte => b[0] as f64,
            Self::Short => i16::from_le_bytes([b[0], b[1]]) as f64,
            Self::UnsignedShort => u16::from_le_bytes([b[0], b[1]]) as f64,
            Self::UnsignedInt => u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64,
            Self::Float => f32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64,
        }
    }
}

/// `DataType.num_elements` (constants.py:75).
pub fn num_elements(type_name: &str) -> Option<usize> {
    Some(match type_name {
        "SCALAR" => 1,
        "VEC2" => 2,
        "VEC3" => 3,
        "VEC4" => 4,
        "MAT2" => 4,
        "MAT3" => 9,
        "MAT4" => 16,
        _ => return None,
    })
}

/// Bytes of a buffer view plus its `byteStride`.
#[derive(Clone, Copy, Debug)]
pub struct View<'a> {
    pub data: &'a [u8],
    pub byte_stride: Option<usize>,
}

#[derive(Clone, Copy, Debug)]
pub struct SparseIndices<'a> {
    pub count: usize,
    pub view: View<'a>,
    pub byte_offset: usize,
    pub component_type: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct SparseValues<'a> {
    pub view: View<'a>,
    pub byte_offset: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct Sparse<'a> {
    pub indices: SparseIndices<'a>,
    pub values: SparseValues<'a>,
}

#[derive(Clone, Copy, Debug)]
pub struct Accessor<'a> {
    /// `None`: no buffer view, the array starts as zeros.
    pub view: Option<View<'a>>,
    pub byte_offset: usize,
    pub component_type: u32,
    pub count: usize,
    pub type_name: &'a str,
    pub normalized: bool,
    pub sparse: Option<Sparse<'a>>,
}

/// `count x components` values, row-major.
#[derive(Clone, Debug, PartialEq)]
pub struct Decoded {
    pub components: usize,
    pub data: Vec<f64>,
}

impl Decoded {
    pub fn rows(&self) -> impl Iterator<Item = &[f64]> {
        self.data.chunks(self.components.max(1))
    }
}

fn read_dense(view: Option<View>, byte_offset: usize, ct: ComponentType, comps: usize, count: usize) -> Result<Vec<f64>, String> {
    let Some(view) = view else { return Ok(vec![0.0; count * comps]) };
    let data = view.data.get(byte_offset..).ok_or("accessor byteOffset past the buffer view")?;
    let size = ct.size();
    let default_stride = size * comps;
    let stride = view.byte_stride.filter(|&s| s != 0).unwrap_or(default_stride);
    if stride % size != 0 {
        return Err("byteStride is not a multiple of the component size".into());
    }
    let mut out = Vec::with_capacity(count * comps);
    for i in 0..count {
        for c in 0..comps {
            let at = i * stride + c * size;
            let b = data.get(at..at + size).ok_or("accessor reads past the end of its buffer view")?;
            out.push(ct.read(b));
        }
    }
    Ok(out)
}

/// `BinaryData.decode_accessor_obj`: strided read, sparse substitution, then normalization of integer types to f32.
pub fn decode_accessor(acc: &Accessor) -> Result<Decoded, String> {
    let ct = ComponentType::from_code(acc.component_type).ok_or("unknown componentType")?;
    let comps = num_elements(acc.type_name).ok_or("unknown accessor type")?;
    if matches!(acc.type_name, "MAT2" | "MAT3") {
        return Err("MAT2/MAT3 accessors are not handled".into());
    }
    let mut data = read_dense(acc.view, acc.byte_offset, ct, comps, acc.count)?;

    if let Some(sp) = &acc.sparse {
        let ict = ComponentType::from_code(sp.indices.component_type).ok_or("unknown sparse index componentType")?;
        let indices = read_dense(Some(sp.indices.view), sp.indices.byte_offset, ict, 1, sp.indices.count)?;
        let values = read_dense(Some(sp.values.view), sp.values.byte_offset, ct, comps, sp.indices.count)?;
        for (k, &idx) in indices.iter().enumerate() {
            let i = idx as usize;
            if i >= acc.count {
                return Err("sparse index out of range".into());
            }
            data[i * comps..(i + 1) * comps].copy_from_slice(&values[k * comps..(k + 1) * comps]);
        }
    }

    if acc.normalized {
        let f = |v: f64| -> f64 {
            let n = match ct {
                ComponentType::Byte => (v / 127.0).max(-1.0),
                ComponentType::UnsignedByte => v / 255.0,
                ComponentType::Short => (v / 32767.0).max(-1.0),
                ComponentType::UnsignedShort => v / 65535.0,
                _ => v,
            };
            n as f32 as f64 /* `astype(np.float32)` */
        };
        data.iter_mut().for_each(|v| *v = f(*v));
    }
    Ok(Decoded { components: comps, data })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f32s(v: &[f32]) -> Vec<u8> {
        v.iter().flat_map(|x| x.to_le_bytes()).collect()
    }

    fn acc<'a>(view: Option<View<'a>>, ct: u32, count: usize, ty: &'a str) -> Accessor<'a> {
        Accessor { view, byte_offset: 0, component_type: ct, count, type_name: ty, normalized: false, sparse: None }
    }

    #[test]
    fn reads_tightly_packed_vec3_floats() {
        let b = f32s(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let d = decode_accessor(&acc(Some(View { data: &b, byte_stride: None }), 5126, 2, "VEC3")).unwrap();
        assert_eq!((d.components, d.data), (3, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]));
    }

    #[test]
    fn honours_byte_offset_and_stride_padding() {
        /* 4 bytes skipped, then VEC2 floats with 4 bytes of padding between elements. */
        let mut b = vec![9, 9, 9, 9];
        b.extend(f32s(&[1.0, 2.0, 0.0, 3.0, 4.0]));
        let mut a = acc(Some(View { data: &b, byte_stride: Some(12) }), 5126, 2, "VEC2");
        a.byte_offset = 4;
        assert_eq!(decode_accessor(&a).unwrap().data, [1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn reads_integer_indices() {
        let b: Vec<u8> = [0u16, 1, 2, 65535].iter().flat_map(|x| x.to_le_bytes()).collect();
        let d = decode_accessor(&acc(Some(View { data: &b, byte_stride: None }), 5123, 4, "SCALAR")).unwrap();
        assert_eq!(d.data, [0.0, 1.0, 2.0, 65535.0]);
        let b = 4_000_000_000u32.to_le_bytes();
        assert_eq!(decode_accessor(&acc(Some(View { data: &b, byte_stride: None }), 5125, 1, "SCALAR")).unwrap().data, [4e9]);
    }

    #[test]
    fn missing_view_is_zeros_and_sparse_overrides() {
        let idx: Vec<u8> = vec![1, 3];
        let vals = f32s(&[7.0, 8.0]);
        let mut a = acc(None, 5126, 4, "SCALAR");
        a.sparse = Some(Sparse {
            indices: SparseIndices { count: 2, view: View { data: &idx, byte_stride: None }, byte_offset: 0, component_type: 5121 },
            values: SparseValues { view: View { data: &vals, byte_stride: None }, byte_offset: 0 },
        });
        assert_eq!(decode_accessor(&a).unwrap().data, [0.0, 7.0, 0.0, 8.0]);
    }

    #[test]
    fn normalizes_integer_types() {
        let b: Vec<u8> = vec![(-128i8) as u8, 127, 0];
        let mut a = acc(Some(View { data: &b, byte_stride: None }), 5120, 3, "SCALAR");
        a.normalized = true;
        assert_eq!(decode_accessor(&a).unwrap().data, [-1.0, 1.0, 0.0]);
        let b: Vec<u8> = [0u16, 65535].iter().flat_map(|x| x.to_le_bytes()).collect();
        let mut a = acc(Some(View { data: &b, byte_stride: None }), 5123, 2, "SCALAR");
        a.normalized = true;
        assert_eq!(decode_accessor(&a).unwrap().data, [0.0, 1.0]);
    }

    #[test]
    fn rejects_short_buffers_and_bad_types() {
        let b = f32s(&[1.0, 2.0]);
        assert!(decode_accessor(&acc(Some(View { data: &b, byte_stride: None }), 5126, 2, "VEC3")).is_err());
        assert!(decode_accessor(&acc(None, 5126, 1, "MAT3")).is_err());
        assert!(decode_accessor(&acc(None, 1234, 1, "SCALAR")).is_err());
    }
}
