//! Binary little-endian 3DGS PLY.
//!
//! Reads the standard property names written by 3DGS trainers
//! (`x y z f_dc_* f_rest_* opacity scale_* rot_*`). Extra properties such as
//! normals are skipped. ASCII and big-endian PLY are rejected.

use std::collections::HashMap;

use crate::error::{FormatError, Result};
use crate::model::{degree_for_rest_floats, rest_coeffs, Coords, Splats, MAX_SPLATS};

const MAX_HEADER: usize = 64 * 1024;

fn type_size(t: &str) -> Option<usize> {
    match t {
        "char" | "int8" | "uchar" | "uint8" => Some(1),
        "short" | "int16" | "ushort" | "uint16" => Some(2),
        "int" | "int32" | "uint" | "uint32" | "float" | "float32" => Some(4),
        "double" | "float64" => Some(8),
        _ => None,
    }
}

fn is_float(t: &str) -> bool {
    t == "float" || t == "float32"
}

struct Header {
    count: usize,
    stride: usize,
    /// Byte offset of each float32 property within a row.
    offsets: HashMap<String, usize>,
    data_start: usize,
}

fn parse_header(bytes: &[u8]) -> Result<Header> {
    let limit = bytes.len().min(MAX_HEADER);
    let marker = b"end_header";
    let pos = bytes
        .get(..limit)
        .and_then(|b| b.windows(marker.len()).position(|w| w == marker))
        .ok_or_else(|| FormatError::Invalid("PLY header not found".into()))?;
    let mut data_start = pos + marker.len();
    if bytes.get(data_start) == Some(&b'\r') {
        data_start += 1;
    }
    if bytes.get(data_start) != Some(&b'\n') {
        return Err(FormatError::Invalid("malformed end_header".into()));
    }
    data_start += 1;
    let text = String::from_utf8_lossy(bytes.get(..pos).unwrap_or(&[])).into_owned();
    let mut lines = text.lines().map(str::trim);
    if lines.next() != Some("ply") {
        return Err(FormatError::Invalid("missing ply magic".into()));
    }
    let mut format_ok = false;
    let mut count: Option<usize> = None;
    let mut in_vertex = false;
    let mut seen_vertex = false;
    let mut stride = 0usize;
    let mut offsets = HashMap::new();
    for line in lines {
        let parts: Vec<&str> = line.split_whitespace().collect();
        match parts.as_slice() {
            ["format", fmt, _] => {
                if *fmt != "binary_little_endian" {
                    return Err(FormatError::Unsupported(format!(
                        "PLY format '{fmt}' (only binary_little_endian)"
                    )));
                }
                format_ok = true;
            }
            ["element", name, n] => {
                if *name == "vertex" && !seen_vertex {
                    let n: usize = n
                        .parse()
                        .map_err(|_| FormatError::Invalid("bad vertex count".into()))?;
                    if n > MAX_SPLATS {
                        return Err(FormatError::Limit(format!(
                            "{n} vertices exceeds the {MAX_SPLATS} cap"
                        )));
                    }
                    count = Some(n);
                    in_vertex = true;
                    seen_vertex = true;
                } else {
                    in_vertex = false;
                }
            }
            ["property", "list", ..] => {
                if in_vertex {
                    return Err(FormatError::Unsupported(
                        "list property in vertex element".into(),
                    ));
                }
            }
            ["property", ty, name] if in_vertex => {
                let size = type_size(ty)
                    .ok_or_else(|| FormatError::Unsupported(format!("PLY property type '{ty}'")))?;
                if is_float(ty) {
                    offsets.insert((*name).to_string(), stride);
                }
                stride += size;
            }
            _ => {}
        }
    }
    if !format_ok {
        return Err(FormatError::Invalid("missing PLY format line".into()));
    }
    let count = count.ok_or_else(|| FormatError::Invalid("no vertex element".into()))?;
    Ok(Header {
        count,
        stride,
        offsets,
        data_start,
    })
}

fn f32_at(row: &[u8], off: usize) -> f32 {
    let b = row.get(off..off + 4).unwrap_or(&[0, 0, 0, 0]);
    f32::from_le_bytes([
        b.first().copied().unwrap_or(0),
        b.get(1).copied().unwrap_or(0),
        b.get(2).copied().unwrap_or(0),
        b.get(3).copied().unwrap_or(0),
    ])
}

pub fn read(bytes: &[u8]) -> Result<Splats> {
    let h = parse_header(bytes)?;
    let need = |name: &str| -> Result<usize> {
        h.offsets
            .get(name)
            .copied()
            .ok_or_else(|| FormatError::Invalid(format!("missing PLY property '{name}'")))
    };
    let pos = [need("x")?, need("y")?, need("z")?];
    let dc = [need("f_dc_0")?, need("f_dc_1")?, need("f_dc_2")?];
    let scale = [need("scale_0")?, need("scale_1")?, need("scale_2")?];
    let rot = [
        need("rot_0")?,
        need("rot_1")?,
        need("rot_2")?,
        need("rot_3")?,
    ];
    let opacity = need("opacity")?;
    let mut rest_offsets = Vec::new();
    while let Some(o) = h.offsets.get(&format!("f_rest_{}", rest_offsets.len())) {
        rest_offsets.push(*o);
    }
    let degree = degree_for_rest_floats(rest_offsets.len()).ok_or_else(|| {
        FormatError::Unsupported(format!(
            "{} f_rest properties (expected 0, 9, 24 or 45)",
            rest_offsets.len()
        ))
    })?;
    let total = h
        .count
        .checked_mul(h.stride)
        .ok_or_else(|| FormatError::Limit("size overflow".into()))?;
    let data = bytes
        .get(h.data_start..)
        .filter(|d| d.len() >= total)
        .ok_or_else(|| FormatError::Invalid("PLY data shorter than the header claims".into()))?;
    let mut s = Splats::empty(degree);
    s.coords = Coords::Rdf;
    if h.stride == 0 {
        return Ok(s);
    }
    for row in data.chunks_exact(h.stride).take(h.count) {
        s.positions.push([
            f32_at(row, pos[0]),
            f32_at(row, pos[1]),
            f32_at(row, pos[2]),
        ]);
        s.sh_dc
            .push([f32_at(row, dc[0]), f32_at(row, dc[1]), f32_at(row, dc[2])]);
        s.scales.push([
            f32_at(row, scale[0]),
            f32_at(row, scale[1]),
            f32_at(row, scale[2]),
        ]);
        s.rotations.push([
            f32_at(row, rot[0]),
            f32_at(row, rot[1]),
            f32_at(row, rot[2]),
            f32_at(row, rot[3]),
        ]);
        s.opacities.push(f32_at(row, opacity));
        for o in &rest_offsets {
            s.sh_rest.push(f32_at(row, *o));
        }
    }
    s.validate()?;
    Ok(s)
}

/// Serialise as 3DGS PLY (always in RDF, the trainer convention).
pub fn write(splats: &Splats) -> Result<Vec<u8>> {
    splats.validate()?;
    let s = splats.to_coords(Coords::Rdf);
    let rest = 3 * rest_coeffs(s.sh_degree);
    let mut head = String::new();
    head.push_str("ply\nformat binary_little_endian 1.0\n");
    head.push_str(&format!("element vertex {}\n", s.len()));
    for n in ["x", "y", "z", "nx", "ny", "nz"] {
        head.push_str(&format!("property float {n}\n"));
    }
    for i in 0..3 {
        head.push_str(&format!("property float f_dc_{i}\n"));
    }
    for i in 0..rest {
        head.push_str(&format!("property float f_rest_{i}\n"));
    }
    head.push_str("property float opacity\n");
    for i in 0..3 {
        head.push_str(&format!("property float scale_{i}\n"));
    }
    for i in 0..4 {
        head.push_str(&format!("property float rot_{i}\n"));
    }
    head.push_str("end_header\n");
    let row_floats = 6 + 3 + rest + 1 + 3 + 4;
    let mut out = Vec::with_capacity(head.len() + s.len() * row_floats * 4);
    out.extend_from_slice(head.as_bytes());
    let mut put = |v: f32| out.extend_from_slice(&v.to_le_bytes());
    for i in 0..s.len() {
        for v in s.positions[i] {
            put(v);
        }
        for _ in 0..3 {
            put(0.0);
        }
        for v in s.sh_dc[i] {
            put(v);
        }
        for v in &s.sh_rest[i * rest..(i + 1) * rest] {
            put(*v);
        }
        put(s.opacities[i]);
        for v in s.scales[i] {
            put(v);
        }
        for v in s.rotations[i] {
            put(v);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::synth::synthetic;

    #[test]
    fn roundtrip_all_degrees_exact() {
        for d in 0..=3u8 {
            let s = synthetic(200, d, 11 + u64::from(d));
            let bytes = write(&s).unwrap();
            let back = read(&bytes).unwrap();
            assert_eq!(s, back, "degree {d}");
        }
    }

    #[test]
    fn rub_input_is_written_as_rdf() {
        let s = synthetic(50, 2, 4);
        let rub = s.to_coords(Coords::Rub);
        let back = read(&write(&rub).unwrap()).unwrap();
        assert_eq!(back.coords, Coords::Rdf);
        assert_eq!(back.positions, s.positions);
    }

    #[test]
    fn truncated_data_is_rejected() {
        let s = synthetic(20, 0, 2);
        let mut bytes = write(&s).unwrap();
        bytes.truncate(bytes.len() - 7);
        assert!(read(&bytes).is_err());
    }

    #[test]
    fn huge_count_in_tiny_file_is_rejected() {
        let hdr = "ply\nformat binary_little_endian 1.0\nelement vertex 100000000\nproperty float x\nend_header\n";
        assert!(read(hdr.as_bytes()).is_err());
    }

    #[test]
    fn ascii_is_unsupported() {
        let hdr = "ply\nformat ascii 1.0\nelement vertex 1\nproperty float x\nend_header\n0\n";
        assert!(matches!(
            read(hdr.as_bytes()),
            Err(FormatError::Unsupported(_))
        ));
    }

    #[test]
    fn garbage_is_rejected() {
        assert!(read(b"not a ply file at all").is_err());
        assert!(read(&[]).is_err());
    }
}
