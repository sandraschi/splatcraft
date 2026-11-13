//! Pure edit operations on [`Splats`]. No I/O, no globals.
//!
//! All operations assume the data is in the frame the caller chose (the core
//! crate converts to RDF before calling). Every function returns a new set.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::unimplemented)]
#![deny(clippy::todo)]

pub mod mesh;
pub mod recolor;

use splat_formats::{rest_coeffs, FormatError, Result, Splats};

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

/// Keep the splats at `idx`, in that order.
pub fn take(s: &Splats, idx: &[usize]) -> Splats {
    let rest = s.rest_per_splat();
    let mut out = Splats::empty(s.sh_degree);
    out.antialiased = s.antialiased;
    out.coords = s.coords;
    for &i in idx {
        if i >= s.len() {
            continue;
        }
        out.positions.push(s.positions[i]);
        out.scales.push(s.scales[i]);
        out.rotations.push(s.rotations[i]);
        out.opacities.push(s.opacities[i]);
        out.sh_dc.push(s.sh_dc[i]);
        out.sh_rest
            .extend_from_slice(&s.sh_rest[i * rest..(i + 1) * rest]);
    }
    out
}

/// Keep splats whose position is inside (or, with `invert`, outside) the box.
/// Splats with a non-finite position are always dropped.
pub fn crop_aabb(s: &Splats, min: [f32; 3], max: [f32; 3], invert: bool) -> Result<Splats> {
    for k in 0..3 {
        if !(min[k].is_finite() && max[k].is_finite()) || min[k] > max[k] {
            return Err(FormatError::Invalid(
                "crop box needs finite min <= max on every axis".into(),
            ));
        }
    }
    let idx: Vec<usize> = s
        .positions
        .iter()
        .enumerate()
        .filter(|(_, p)| {
            if !p.iter().all(|v| v.is_finite()) {
                return false;
            }
            let inside = (0..3).all(|k| p[k] >= min[k] && p[k] <= max[k]);
            inside != invert
        })
        .map(|(i, _)| i)
        .collect();
    Ok(take(s, &idx))
}

#[derive(Clone, Copy, Debug)]
pub struct Prune {
    /// Drop splats whose opacity (0..1) is below this.
    pub min_opacity: Option<f32>,
    /// Drop splats whose largest linear scale exceeds this.
    pub max_scale: Option<f32>,
    pub drop_non_finite: bool,
}

pub fn prune(s: &Splats, p: &Prune) -> Result<Splats> {
    if let Some(m) = p.min_opacity {
        if !(0.0..1.0).contains(&m) {
            return Err(FormatError::Invalid("min_opacity must be in [0, 1)".into()));
        }
    }
    if let Some(m) = p.max_scale {
        if !(m.is_finite() && m > 0.0) {
            return Err(FormatError::Invalid("max_scale must be positive".into()));
        }
    }
    let idx: Vec<usize> = (0..s.len())
        .filter(|&i| {
            if !s.is_finite_at(i) {
                return !p.drop_non_finite;
            }
            if let Some(m) = p.min_opacity {
                if sigmoid(s.opacities[i]) < m {
                    return false;
                }
            }
            if let Some(m) = p.max_scale {
                let big = s.scales[i].iter().map(|v| v.exp()).fold(0.0f32, f32::max);
                if big > m {
                    return false;
                }
            }
            true
        })
        .collect();
    Ok(take(s, &idx))
}

#[derive(Clone, Copy, Debug)]
pub struct Transform {
    pub translate: [f32; 3],
    /// Uniform scale, applied before rotation and translation.
    pub scale: f32,
    /// Rotation as (w, x, y, z); need not be normalised.
    pub rotate: Option<[f32; 4]>,
}

impl Default for Transform {
    fn default() -> Self {
        Transform {
            translate: [0.0; 3],
            scale: 1.0,
            rotate: None,
        }
    }
}

/// Quaternion from an axis and an angle in degrees.
pub fn quat_axis_angle(axis: [f32; 3], degrees: f32) -> Result<[f32; 4]> {
    let n = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    if !(n.is_finite() && n > 1e-6) || !degrees.is_finite() {
        return Err(FormatError::Invalid(
            "rotation needs a non-zero finite axis and finite angle".into(),
        ));
    }
    let h = degrees.to_radians() * 0.5;
    let s = h.sin() / n;
    Ok([h.cos(), axis[0] * s, axis[1] * s, axis[2] * s])
}

fn qmul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[0] * b[0] - a[1] * b[1] - a[2] * b[2] - a[3] * b[3],
        a[0] * b[1] + a[1] * b[0] + a[2] * b[3] - a[3] * b[2],
        a[0] * b[2] - a[1] * b[3] + a[2] * b[0] + a[3] * b[1],
        a[0] * b[3] + a[1] * b[2] - a[2] * b[1] + a[3] * b[0],
    ]
}

fn qrot(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let conj = [q[0], -q[1], -q[2], -q[3]];
    let r = qmul(qmul(q, [0.0, v[0], v[1], v[2]]), conj);
    [r[1], r[2], r[3]]
}

pub fn transform(s: &Splats, t: &Transform) -> Result<Splats> {
    if !(t.scale.is_finite() && t.scale > 0.0) || !t.translate.iter().all(|v| v.is_finite()) {
        return Err(FormatError::Invalid(
            "transform needs finite translate and scale > 0".into(),
        ));
    }
    let rot = match t.rotate {
        None => None,
        Some(q) => {
            let n = q.iter().map(|v| v * v).sum::<f32>().sqrt();
            if !(n.is_finite() && n > 1e-6) {
                return Err(FormatError::Invalid("rotation quaternion is zero".into()));
            }
            let q = [q[0] / n, q[1] / n, q[2] / n, q[3] / n];
            // Identity (either sign of w) needs no rotation.
            if q[0].abs() > 0.999_999 {
                None
            } else {
                Some(q)
            }
        }
    };
    if rot.is_some() && s.sh_degree > 0 {
        return Err(FormatError::Unsupported(
            "rotating splats with SH degree > 0 needs SH rotation, which is not built yet; \
             run reduce_sh to degree 0 first"
                .into(),
        ));
    }
    let mut out = s.clone();
    let ln_s = t.scale.ln();
    for p in &mut out.positions {
        let mut v = [p[0] * t.scale, p[1] * t.scale, p[2] * t.scale];
        if let Some(q) = rot {
            v = qrot(q, v);
        }
        *p = [
            v[0] + t.translate[0],
            v[1] + t.translate[1],
            v[2] + t.translate[2],
        ];
    }
    for sc in &mut out.scales {
        for v in sc.iter_mut() {
            *v += ln_s;
        }
    }
    if let Some(q) = rot {
        for r in &mut out.rotations {
            let mut m = qmul(q, *r);
            let n = m.iter().map(|v| v * v).sum::<f32>().sqrt();
            if n > 0.0 && n.is_finite() {
                for v in &mut m {
                    *v /= n;
                }
            }
            *r = m;
        }
    }
    Ok(out)
}

/// Truncate spherical harmonics to `degree` (0..=current).
pub fn reduce_sh(s: &Splats, degree: u8) -> Result<Splats> {
    if degree > s.sh_degree {
        return Err(FormatError::Invalid(format!(
            "cannot raise SH degree from {} to {degree}",
            s.sh_degree
        )));
    }
    if degree == s.sh_degree {
        return Ok(s.clone());
    }
    let old = rest_coeffs(s.sh_degree);
    let new = rest_coeffs(degree);
    let mut out = s.clone();
    out.sh_degree = degree;
    out.sh_rest = Vec::with_capacity(s.len() * 3 * new);
    for chunk in s.sh_rest.chunks_exact(3 * old.max(1)) {
        for c in 0..3 {
            out.sh_rest
                .extend_from_slice(&chunk[c * old..c * old + new]);
        }
    }
    if old == 0 {
        out.sh_rest.clear();
    }
    Ok(out)
}

/// Concatenate sets. Frames are unified to the first set's frame. If SH
/// degrees differ, all are reduced to the lowest. Returns the merged set.
pub fn merge(parts: &[Splats]) -> Result<Splats> {
    let first = parts
        .first()
        .ok_or_else(|| FormatError::Invalid("merge needs at least one input".into()))?;
    if parts.iter().any(|p| p.antialiased != first.antialiased) {
        return Err(FormatError::Invalid(
            "cannot merge antialiased and non-antialiased splats".into(),
        ));
    }
    let degree = parts.iter().map(|p| p.sh_degree).min().unwrap_or(0);
    let mut out = Splats::empty(degree);
    out.antialiased = first.antialiased;
    out.coords = first.coords;
    for p in parts {
        let p = reduce_sh(&p.to_coords(first.coords), degree)?;
        out.positions.extend(p.positions);
        out.scales.extend(p.scales);
        out.rotations.extend(p.rotations);
        out.opacities.extend(p.opacities);
        out.sh_dc.extend(p.sh_dc);
        out.sh_rest.extend(p.sh_rest);
    }
    out.validate()?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use splat_formats::synth::synthetic;
    use splat_formats::Coords;

    #[test]
    fn crop_inside_and_inverse_partition() {
        let s = synthetic(1000, 1, 3);
        let a = crop_aabb(&s, [-2.0; 3], [2.0; 3], false).unwrap();
        let b = crop_aabb(&s, [-2.0; 3], [2.0; 3], true).unwrap();
        assert_eq!(a.len() + b.len(), 1000);
        assert!(!a.is_empty() && !b.is_empty());
        assert!(a.positions.iter().all(|p| p.iter().all(|v| v.abs() <= 2.0)));
        a.validate().unwrap();
        assert!(crop_aabb(&s, [1.0; 3], [0.0; 3], false).is_err());
    }

    #[test]
    fn prune_by_opacity_and_scale() {
        let s = synthetic(1000, 0, 4);
        let p = prune(
            &s,
            &Prune {
                min_opacity: Some(0.5),
                max_scale: None,
                drop_non_finite: true,
            },
        )
        .unwrap();
        assert!(p.len() < 1000 && !p.is_empty());
        assert!(p.opacities.iter().all(|o| sigmoid(*o) >= 0.5));
        let q = prune(
            &s,
            &Prune {
                min_opacity: None,
                max_scale: Some(0.01),
                drop_non_finite: true,
            },
        )
        .unwrap();
        assert!(q.scales.iter().all(|sc| sc.iter().all(|v| v.exp() <= 0.01)));
    }

    #[test]
    fn prune_drops_non_finite() {
        let mut s = synthetic(10, 0, 1);
        s.positions[2][0] = f32::NAN;
        let p = Prune {
            min_opacity: None,
            max_scale: None,
            drop_non_finite: true,
        };
        assert_eq!(prune(&s, &p).unwrap().len(), 9);
    }

    #[test]
    fn translate_and_scale() {
        let s = synthetic(50, 3, 6);
        let t = Transform {
            translate: [1.0, 2.0, 3.0],
            scale: 2.0,
            rotate: None,
        };
        let o = transform(&s, &t).unwrap();
        for i in 0..50 {
            assert!((o.positions[i][0] - (s.positions[i][0] * 2.0 + 1.0)).abs() < 1e-5);
            assert!((o.scales[i][1] - (s.scales[i][1] + 2.0f32.ln())).abs() < 1e-5);
        }
        assert_eq!(o.sh_rest, s.sh_rest);
        assert!(transform(&s, &Transform { scale: 0.0, ..t }).is_err());
    }

    #[test]
    fn rotate_then_inverse_restores() {
        let s = synthetic(200, 0, 8);
        let q = quat_axis_angle([0.3, 1.0, -0.2], 73.0).unwrap();
        let qi = quat_axis_angle([0.3, 1.0, -0.2], -73.0).unwrap();
        let a = transform(
            &s,
            &Transform {
                rotate: Some(q),
                ..Transform::default()
            },
        )
        .unwrap();
        let b = transform(
            &a,
            &Transform {
                rotate: Some(qi),
                ..Transform::default()
            },
        )
        .unwrap();
        for i in 0..200 {
            for k in 0..3 {
                assert!((b.positions[i][k] - s.positions[i][k]).abs() < 1e-4);
            }
            let dot: f32 = (0..4).map(|k| b.rotations[i][k] * s.rotations[i][k]).sum();
            assert!(dot.abs() > 0.9999);
        }
    }

    #[test]
    fn ninety_degrees_about_z_maps_x_to_y() {
        let mut s = synthetic(1, 0, 1);
        s.positions[0] = [1.0, 0.0, 0.0];
        let q = quat_axis_angle([0.0, 0.0, 1.0], 90.0).unwrap();
        let o = transform(
            &s,
            &Transform {
                rotate: Some(q),
                ..Transform::default()
            },
        )
        .unwrap();
        assert!((o.positions[0][1] - 1.0).abs() < 1e-5 && o.positions[0][0].abs() < 1e-5);
    }

    #[test]
    fn rotation_with_sh_is_refused_not_faked() {
        let s = synthetic(5, 1, 2);
        let q = quat_axis_angle([0.0, 1.0, 0.0], 30.0).unwrap();
        let e = transform(
            &s,
            &Transform {
                rotate: Some(q),
                ..Transform::default()
            },
        )
        .unwrap_err();
        assert!(matches!(e, FormatError::Unsupported(_)));
        let r = reduce_sh(&s, 0).unwrap();
        assert!(transform(
            &r,
            &Transform {
                rotate: Some(q),
                ..Transform::default()
            }
        )
        .is_ok());
    }

    #[test]
    fn reduce_sh_keeps_leading_coefficients_per_channel() {
        let s = synthetic(20, 3, 12);
        let r = reduce_sh(&s, 1).unwrap();
        assert_eq!(r.sh_degree, 1);
        r.validate().unwrap();
        for i in 0..20 {
            for c in 0..3 {
                for j in 0..3 {
                    assert_eq!(r.sh_rest[i * 9 + c * 3 + j], s.sh_rest[i * 45 + c * 15 + j]);
                }
            }
        }
        assert!(reduce_sh(&r, 2).is_err());
        assert_eq!(reduce_sh(&s, 0).unwrap().sh_rest.len(), 0);
    }

    #[test]
    fn merge_unifies_degree_and_frame() {
        let a = synthetic(30, 3, 1);
        let b = synthetic(20, 1, 2).to_coords(Coords::Rub);
        let m = merge(&[a.clone(), b.clone()]).unwrap();
        assert_eq!(m.len(), 50);
        assert_eq!(m.sh_degree, 1);
        assert_eq!(m.coords, Coords::Rdf);
        // b's positions were converted back into the first set's frame
        assert_eq!(m.positions[30], synthetic(20, 1, 2).positions[0]);
        assert!(merge(&[]).is_err());
    }
}
