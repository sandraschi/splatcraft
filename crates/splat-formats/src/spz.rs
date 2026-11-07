//! Niantic SPZ, versions 2 and 3 (gzip container).
//!
//! Layout (little endian): 16 byte header (magic 0x5053474e, version, count,
//! sh degree, fractional bits, flags, reserved), then struct-of-arrays blocks:
//! positions (3 x 24-bit fixed point), alphas, colours, scales, rotations
//! (v3: 4 byte smallest-three, v2: 3 byte xyz), spherical harmonics.
//!
//! SPZ stores RUB coordinates; this module sets `Coords::Rub` on read and
//! converts to RUB on write. SPZ v4 (zstd) and v1 (float16) are rejected.

use std::f32::consts::FRAC_1_SQRT_2;
use std::io::{Read, Write};

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;

use crate::error::{FormatError, Result};
use crate::model::{rest_coeffs, Coords, Splats, MAX_SPLATS};

const MAGIC: u32 = 0x5053_474e;
const HEADER: usize = 16;
const COLOR_SCALE: f32 = 0.15;
const FRACTIONAL_BITS: u8 = 12;
/// Cap on decompressed size (guards against gzip bombs).
const MAX_DECOMPRESSED: u64 = 8 * 1024 * 1024 * 1024;

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

fn inv_sigmoid(p: f32) -> f32 {
    let p = p.clamp(1e-6, 1.0 - 1e-6);
    (p / (1.0 - p)).ln()
}

fn to_u8(v: f32) -> u8 {
    v.round().clamp(0.0, 255.0) as u8
}

fn le_u32(b: &[u8], at: usize) -> u32 {
    let g = |i: usize| b.get(at + i).copied().unwrap_or(0);
    u32::from_le_bytes([g(0), g(1), g(2), g(3)])
}

pub fn write(splats: &Splats) -> Result<Vec<u8>> {
    splats.validate()?;
    let s = splats.to_coords(Coords::Rub);
    let n = s.len();
    let n_i32 = i32::try_from(n).map_err(|_| FormatError::Limit("too many splats".into()))?;
    let nc = rest_coeffs(s.sh_degree);
    let mut raw = Vec::with_capacity(HEADER + n * (9 + 1 + 3 + 3 + 4 + 3 * nc));
    raw.extend_from_slice(&MAGIC.to_le_bytes());
    raw.extend_from_slice(&3i32.to_le_bytes());
    raw.extend_from_slice(&n_i32.to_le_bytes());
    raw.push(s.sh_degree);
    raw.push(FRACTIONAL_BITS);
    raw.push(u8::from(s.antialiased));
    raw.push(0);

    let scale = (1u32 << FRACTIONAL_BITS) as f32;
    for p in &s.positions {
        for v in p {
            let fixed = (v * scale).round().clamp(-8_388_608.0, 8_388_607.0) as i32;
            let b = fixed.to_le_bytes();
            raw.extend_from_slice(&b[..3]);
        }
    }
    for o in &s.opacities {
        raw.push(to_u8(sigmoid(*o) * 255.0));
    }
    for c in &s.sh_dc {
        for v in c {
            raw.push(to_u8(v * (COLOR_SCALE * 255.0) + 127.5));
        }
    }
    for sc in &s.scales {
        for v in sc {
            raw.push(to_u8((v + 10.0) * 16.0));
        }
    }
    for r in &s.rotations {
        raw.extend_from_slice(&pack_rotation(*r).to_le_bytes());
    }
    for i in 0..n {
        for j in 0..nc {
            for c in 0..3 {
                let v = s.sh_rest[i * 3 * nc + c * nc + j];
                let bucket = if j < 3 { 8.0 } else { 16.0 };
                let q = (v * 128.0 + 128.0).round();
                let q = ((q + bucket / 2.0) / bucket).floor() * bucket;
                raw.push(q.clamp(0.0, 255.0) as u8);
            }
        }
    }
    let mut enc = GzEncoder::new(Vec::new(), Compression::default());
    enc.write_all(&raw)?;
    Ok(enc.finish()?)
}

/// `r` is (w, x, y, z); packed as smallest-three of (x, y, z, w).
fn pack_rotation(r: [f32; 4]) -> u32 {
    let mut q = [r[1], r[2], r[3], r[0]];
    let norm = q.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 0.0 && norm.is_finite() {
        for v in &mut q {
            *v /= norm;
        }
    } else {
        q = [0.0, 0.0, 0.0, 1.0];
    }
    let mut largest = 0usize;
    for i in 1..4 {
        if q[i].abs() > q[largest].abs() {
            largest = i;
        }
    }
    if q[largest] < 0.0 {
        for v in &mut q {
            *v = -*v;
        }
    }
    let mut comp = largest as u32;
    for (i, v) in q.iter().enumerate() {
        if i == largest {
            continue;
        }
        let neg = u32::from(*v < 0.0);
        let mag = ((511.0 * (v.abs() / FRAC_1_SQRT_2)).round() as u32).min(511);
        comp = (comp << 10) | (neg << 9) | mag;
    }
    comp
}

/// Returns (w, x, y, z).
fn unpack_rotation(comp: u32) -> [f32; 4] {
    let largest = (comp >> 30) as usize;
    let mut rest = comp;
    let mut q = [0.0f32; 4];
    let mut sum_sq = 0.0f32;
    for i in (0..4).rev() {
        if i == largest {
            continue;
        }
        let mag = (rest & 511) as f32;
        let neg = (rest >> 9) & 1 == 1;
        rest >>= 10;
        let mut v = FRAC_1_SQRT_2 * mag / 511.0;
        if neg {
            v = -v;
        }
        q[i] = v;
        sum_sq += v * v;
    }
    q[largest] = (1.0 - sum_sq).max(0.0).sqrt();
    [q[3], q[0], q[1], q[2]]
}

pub fn read(bytes: &[u8]) -> Result<Splats> {
    if bytes.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]) {
        return Err(FormatError::Unsupported(
            "SPZ v4 (zstd container) is not supported yet; re-export as v3 or PLY".into(),
        ));
    }
    let mut raw = Vec::new();
    GzDecoder::new(bytes)
        .take(MAX_DECOMPRESSED)
        .read_to_end(&mut raw)
        .map_err(|e| FormatError::Invalid(format!("not a gzip SPZ file: {e}")))?;
    if raw.len() < HEADER {
        return Err(FormatError::Invalid("SPZ shorter than header".into()));
    }
    if le_u32(&raw, 0) != MAGIC {
        return Err(FormatError::Invalid("bad SPZ magic".into()));
    }
    let version = le_u32(&raw, 4);
    if version != 2 && version != 3 {
        return Err(FormatError::Unsupported(format!(
            "SPZ version {version} (only 2 and 3)"
        )));
    }
    let count = le_u32(&raw, 8) as usize;
    let degree = raw.get(12).copied().unwrap_or(0);
    let frac_bits = raw.get(13).copied().unwrap_or(0);
    let flags = raw.get(14).copied().unwrap_or(0);
    if count > MAX_SPLATS {
        return Err(FormatError::Limit(format!(
            "{count} splats exceeds the {MAX_SPLATS} cap"
        )));
    }
    if degree > 3 || frac_bits > 24 {
        return Err(FormatError::Invalid(
            "SPZ header fields out of range".into(),
        ));
    }
    let nc = rest_coeffs(degree);
    let rot_bytes = if version == 3 { 4 } else { 3 };
    let per = 9 + 1 + 3 + 3 + rot_bytes + 3 * nc;
    let expected = count
        .checked_mul(per)
        .and_then(|v| v.checked_add(HEADER))
        .ok_or_else(|| FormatError::Limit("size overflow".into()))?;
    if raw.len() < expected {
        return Err(FormatError::Invalid(
            "SPZ data shorter than the header claims".into(),
        ));
    }
    let mut at = HEADER;
    let mut s = Splats::empty(degree);
    s.coords = Coords::Rub;
    s.antialiased = flags & 1 == 1;
    let inv = 1.0 / (1u32 << frac_bits) as f32;
    for i in 0..count {
        let mut p = [0.0f32; 3];
        for (k, slot) in p.iter_mut().enumerate() {
            let o = at + (i * 3 + k) * 3;
            let mut v =
                i32::from(raw[o]) | (i32::from(raw[o + 1]) << 8) | (i32::from(raw[o + 2]) << 16);
            if v & 0x80_0000 != 0 {
                v |= -0x100_0000;
            }
            *slot = v as f32 * inv;
        }
        s.positions.push(p);
    }
    at += count * 9;
    for i in 0..count {
        s.opacities
            .push(inv_sigmoid(f32::from(raw[at + i]) / 255.0));
    }
    at += count;
    for i in 0..count {
        let mut c = [0.0f32; 3];
        for (k, slot) in c.iter_mut().enumerate() {
            *slot = (f32::from(raw[at + i * 3 + k]) / 255.0 - 0.5) / COLOR_SCALE;
        }
        s.sh_dc.push(c);
    }
    at += count * 3;
    for i in 0..count {
        let mut c = [0.0f32; 3];
        for (k, slot) in c.iter_mut().enumerate() {
            *slot = f32::from(raw[at + i * 3 + k]) / 16.0 - 10.0;
        }
        s.scales.push(c);
    }
    at += count * 3;
    for i in 0..count {
        if version == 3 {
            s.rotations.push(unpack_rotation(le_u32(&raw, at + i * 4)));
        } else {
            let o = at + i * 3;
            let x = f32::from(raw[o]) / 127.5 - 1.0;
            let y = f32::from(raw[o + 1]) / 127.5 - 1.0;
            let z = f32::from(raw[o + 2]) / 127.5 - 1.0;
            let w = (1.0 - (x * x + y * y + z * z)).max(0.0).sqrt();
            s.rotations.push([w, x, y, z]);
        }
    }
    at += count * rot_bytes;
    s.sh_rest = vec![0.0; count * 3 * nc];
    for i in 0..count {
        for j in 0..nc {
            for c in 0..3 {
                let b = raw[at + i * 3 * nc + j * 3 + c];
                s.sh_rest[i * 3 * nc + c * nc + j] = (f32::from(b) - 128.0) / 128.0;
            }
        }
    }
    s.validate()?;
    Ok(s)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::synth::synthetic;

    fn sig(x: f32) -> f32 {
        sigmoid(x)
    }

    fn assert_close(a: &Splats, b: &Splats, degree: u8) {
        assert_eq!(a.len(), b.len());
        for i in 0..a.len() {
            for k in 0..3 {
                assert!((a.positions[i][k] - b.positions[i][k]).abs() < 2.5e-4);
                assert!((a.scales[i][k] - b.scales[i][k]).abs() < 0.04);
                assert!((a.sh_dc[i][k] - b.sh_dc[i][k]).abs() < 0.02);
            }
            assert!((sig(a.opacities[i]) - sig(b.opacities[i])).abs() < 0.004);
            let dot: f32 = (0..4).map(|k| a.rotations[i][k] * b.rotations[i][k]).sum();
            assert!(dot.abs() > 0.999, "rotation dot {dot}");
        }
        if degree > 0 {
            for (x, y) in a.sh_rest.iter().zip(&b.sh_rest) {
                assert!((x - y).abs() < 0.07, "sh {x} vs {y}");
            }
        }
    }

    #[test]
    fn roundtrip_all_degrees() {
        for d in 0..=3u8 {
            let s = synthetic(300, d, 21 + u64::from(d));
            let bytes = write(&s).unwrap();
            let back = read(&bytes).unwrap();
            assert_eq!(back.coords, Coords::Rub);
            assert_eq!(back.sh_degree, d);
            assert_close(&s.to_coords(Coords::Rub), &back, d);
        }
    }

    #[test]
    fn header_is_correct() {
        let s = synthetic(5, 1, 3);
        let bytes = write(&s).unwrap();
        let mut raw = Vec::new();
        GzDecoder::new(&bytes[..]).read_to_end(&mut raw).unwrap();
        assert_eq!(&raw[..4], b"NGSP");
        assert_eq!(le_u32(&raw, 4), 3);
        assert_eq!(le_u32(&raw, 8), 5);
        assert_eq!(raw[12], 1);
        assert_eq!(raw[13], FRACTIONAL_BITS);
        assert_eq!(raw.len(), HEADER + 5 * (9 + 1 + 3 + 3 + 4 + 9));
    }

    #[test]
    fn rotation_pack_roundtrip() {
        let s = synthetic(1000, 0, 77);
        for r in &s.rotations {
            let back = unpack_rotation(pack_rotation(*r));
            let dot: f32 = (0..4).map(|k| r[k] * back[k]).sum();
            assert!(dot.abs() > 0.9995);
        }
    }

    #[test]
    fn negative_positions_survive() {
        let mut s = synthetic(1, 0, 5);
        s.positions[0] = [-3.25, 0.0, 100.5];
        let back = read(&write(&s).unwrap()).unwrap().to_coords(Coords::Rdf);
        assert!((back.positions[0][0] + 3.25).abs() < 3e-4);
        assert!((back.positions[0][2] - 100.5).abs() < 3e-4);
    }

    #[test]
    fn zstd_v4_is_honestly_unsupported() {
        let e = read(&[0x28, 0xb5, 0x2f, 0xfd, 0, 0, 0, 0]).unwrap_err();
        assert!(matches!(e, FormatError::Unsupported(_)));
    }

    #[test]
    fn garbage_and_truncation_rejected() {
        assert!(read(b"hello world").is_err());
        assert!(read(&[]).is_err());
        let bytes = write(&synthetic(10, 0, 1)).unwrap();
        assert!(read(&bytes[..bytes.len() / 2]).is_err());
    }

    #[test]
    fn lying_count_rejected() {
        let mut raw = Vec::new();
        raw.extend_from_slice(&MAGIC.to_le_bytes());
        raw.extend_from_slice(&3i32.to_le_bytes());
        raw.extend_from_slice(&100_000_000i32.to_le_bytes());
        raw.extend_from_slice(&[0, 12, 0, 0]);
        let mut enc = GzEncoder::new(Vec::new(), Compression::fast());
        enc.write_all(&raw).unwrap();
        assert!(read(&enc.finish().unwrap()).is_err());
    }
}
