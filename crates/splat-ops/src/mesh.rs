//! Axis-aligned box collider as a binary glTF 2.0 (GLB) mesh.
//!
//! The box is the bounds of the finite splat positions. Marble's downloaded
//! collider is a separate fetch asset and is not produced here.

use splat_formats::{FormatError, Result};

/// Bounds of the finite positions, expanded so every axis has some thickness.
pub struct BoxBounds {
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub finite: usize,
}

/// GLB bytes plus the box that was encoded.
pub struct BoxGlb {
    pub bytes: Vec<u8>,
    pub bounds: BoxBounds,
    pub vertices: usize,
    pub triangles: usize,
}

/// Bounds of finite positions. A zero-length axis is padded so the box has volume.
pub fn bounds(positions: &[[f32; 3]]) -> Result<BoxBounds> {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    let mut finite = 0usize;
    for p in positions {
        if p.iter().all(|v| v.is_finite()) {
            finite += 1;
            for k in 0..3 {
                min[k] = min[k].min(p[k]);
                max[k] = max[k].max(p[k]);
            }
        }
    }
    if finite == 0 {
        return Err(FormatError::Invalid(
            "no finite splat positions to build a collider".into(),
        ));
    }
    for k in 0..3 {
        if max[k] - min[k] < 1.0e-3 {
            let mid = (min[k] + max[k]) * 0.5;
            min[k] = mid - 5.0e-3;
            max[k] = mid + 5.0e-3;
        }
    }
    Ok(BoxBounds { min, max, finite })
}

/// 8 corners, 12 triangles, one mesh. Positions are written as given (RDF or RUB).
pub fn box_glb(bounds: &BoxBounds) -> Result<BoxGlb> {
    let min = bounds.min;
    let max = bounds.max;
    let mut bin = Vec::with_capacity(96 + 72);
    let corners = [
        [min[0], min[1], min[2]],
        [max[0], min[1], min[2]],
        [max[0], max[1], min[2]],
        [min[0], max[1], min[2]],
        [min[0], min[1], max[2]],
        [max[0], min[1], max[2]],
        [max[0], max[1], max[2]],
        [min[0], max[1], max[2]],
    ];
    for c in corners {
        for v in c {
            bin.extend_from_slice(&v.to_le_bytes());
        }
    }
    // Two triangles per face, outward when Y is up. Collision code uses the
    // solid, so winding is recorded and not treated as a shading normal.
    const INDEX: [u16; 36] = [
        0, 1, 2, 0, 2, 3, // z min
        5, 4, 7, 5, 7, 6, // z max
        4, 5, 1, 4, 1, 0, // y min
        3, 2, 6, 3, 6, 7, // y max
        4, 0, 3, 4, 3, 7, // x min
        1, 5, 6, 1, 6, 2, // x max
    ];
    for i in INDEX {
        bin.extend_from_slice(&i.to_le_bytes());
    }
    let json = format!(
        concat!(
            "{{\"asset\":{{\"version\":\"2.0\",\"generator\":\"splatcraft\"}},",
            "\"scene\":0,\"scenes\":[{{\"nodes\":[0]}}],\"nodes\":[{{\"mesh\":0}}],",
            "\"meshes\":[{{\"name\":\"collider\",\"primitives\":[{{\"attributes\":{{\"POSITION\":0}},\"indices\":1,\"mode\":4}}]}}],",
            "\"accessors\":[",
            "{{\"bufferView\":0,\"componentType\":5126,\"count\":8,\"type\":\"VEC3\",\"min\":[{min0},{min1},{min2}],\"max\":[{max0},{max1},{max2}]}},",
            "{{\"bufferView\":1,\"componentType\":5123,\"count\":36,\"type\":\"SCALAR\"}}",
            "],",
            "\"bufferViews\":[",
            "{{\"buffer\":0,\"byteOffset\":0,\"byteLength\":96,\"target\":34962}},",
            "{{\"buffer\":0,\"byteOffset\":96,\"byteLength\":72,\"target\":34963}}",
            "],",
            "\"buffers\":[{{\"byteLength\":{bin_len}}}]}}"
        ),
        min0 = num(min[0]),
        min1 = num(min[1]),
        min2 = num(min[2]),
        max0 = num(max[0]),
        max1 = num(max[1]),
        max2 = num(max[2]),
        bin_len = bin.len(),
    );
    let mut json_bytes = json.into_bytes();
    while json_bytes.len() % 4 != 0 {
        json_bytes.push(b' ');
    }
    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    let json_len = u32_len(json_bytes.len())?;
    let bin_len = u32_len(bin.len())?;
    let total = 12u32
        .checked_add(8)
        .and_then(|n| n.checked_add(json_len))
        .and_then(|n| n.checked_add(8))
        .and_then(|n| n.checked_add(bin_len))
        .ok_or_else(|| FormatError::Limit("glb length overflow".into()))?;
    let mut out = Vec::with_capacity(total as usize);
    out.extend_from_slice(&0x4654_6C67u32.to_le_bytes());
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&total.to_le_bytes());
    out.extend_from_slice(&json_len.to_le_bytes());
    out.extend_from_slice(&0x4E4F_534Au32.to_le_bytes());
    out.extend_from_slice(&json_bytes);
    out.extend_from_slice(&bin_len.to_le_bytes());
    out.extend_from_slice(&0x004E_4942u32.to_le_bytes());
    out.extend_from_slice(&bin);
    if out.len() != total as usize {
        return Err(FormatError::Invalid("glb length mismatch".into()));
    }
    Ok(BoxGlb {
        bytes: out,
        bounds: BoxBounds {
            min,
            max,
            finite: bounds.finite,
        },
        vertices: 8,
        triangles: 12,
    })
}

fn num(v: f32) -> String {
    let mut s = format!("{v:.6}");
    while s.contains('.') && (s.ends_with('0') || s.ends_with('.')) {
        s.pop();
    }
    if s.is_empty() || s == "-" {
        s = "0".to_string();
    }
    s
}

fn u32_len(n: usize) -> Result<u32> {
    u32::try_from(n).map_err(|_| FormatError::Limit("glb chunk exceeds 4 GiB".into()))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn box_glb_is_a_gltf_box() {
        let b = bounds(&[
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [0.0, 3.0, 0.0],
            [0.0, 0.0, 4.0],
            [f32::NAN, 1.0, 1.0],
        ])
        .unwrap();
        assert_eq!(b.finite, 4);
        let glb = box_glb(&b).unwrap();
        assert_eq!(&glb.bytes[0..4], b"glTF");
        assert_eq!(u32::from_le_bytes(glb.bytes[4..8].try_into().unwrap()), 2);
        assert_eq!(
            glb.bytes.len(),
            u32::from_le_bytes(glb.bytes[8..12].try_into().unwrap()) as usize
        );
        let json_len = u32::from_le_bytes(glb.bytes[12..16].try_into().unwrap()) as usize;
        let json = std::str::from_utf8(&glb.bytes[20..20 + json_len]).unwrap();
        assert!(json.contains("\"count\":8"));
        assert!(json.contains("\"count\":36"));
        assert!(json.contains("collider"));
        assert_eq!(glb.vertices, 8);
        assert_eq!(glb.triangles, 12);
    }

    #[test]
    fn empty_and_non_finite_are_refused() {
        assert!(bounds(&[]).is_err());
        assert!(bounds(&[[f32::NAN, 0.0, 0.0]]).is_err());
    }

    #[test]
    fn flat_cloud_still_has_volume() {
        let b = bounds(&[[1.0, 2.0, 3.0], [1.0, 2.0, 3.0]]).unwrap();
        assert!(b.max[0] > b.min[0]);
        assert!(b.max[1] > b.min[1]);
        assert!(b.max[2] > b.min[2]);
    }
}
