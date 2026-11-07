//! In-memory Gaussian splat model.
//!
//! Storage follows the PLY convention that training tools emit: log-scales,
//! logit opacities, SH DC coefficients, quaternions as (w, x, y, z), and
//! `sh_rest` laid out channel-major per splat (all R, then all G, then all B).
//! The default coordinate system is RDF (right, down, front), which is what
//! 3DGS trainers write. SPZ stores RUB (right, up, back).

use crate::error::{FormatError, Result};

/// Hard cap on splat count accepted anywhere in this crate.
pub const MAX_SPLATS: usize = 150_000_000;

/// Sign flips applied to SH coefficient `k` (bands >= 1) when swapping
/// RDF <-> RUB (y and z negated). Derived from the SPZ reference converter.
const SH_FLIP: [f32; 15] = [
    -1.0, -1.0, 1.0, -1.0, 1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, -1.0, 1.0, -1.0, 1.0,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coords {
    /// Right, down, front: the PLY convention of 3DGS trainers.
    Rdf,
    /// Right, up, back: SPZ internal, three.js.
    Rub,
}

impl Coords {
    pub fn parse(s: &str) -> Option<Coords> {
        match s.to_ascii_lowercase().as_str() {
            "rdf" => Some(Coords::Rdf),
            "rub" => Some(Coords::Rub),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Coords::Rdf => "rdf",
            Coords::Rub => "rub",
        }
    }
}

/// Number of SH coefficients per colour channel beyond DC for a degree.
pub fn rest_coeffs(degree: u8) -> usize {
    match degree {
        1 => 3,
        2 => 8,
        3 => 15,
        _ => 0,
    }
}

/// Inverse of `3 * rest_coeffs`: SH degree from the number of `f_rest_*` floats.
pub fn degree_for_rest_floats(n: usize) -> Option<u8> {
    match n {
        0 => Some(0),
        9 => Some(1),
        24 => Some(2),
        45 => Some(3),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Splats {
    pub positions: Vec<[f32; 3]>,
    /// Log scales (PLY `scale_0..2`).
    pub scales: Vec<[f32; 3]>,
    /// Quaternions as (w, x, y, z) (PLY `rot_0..3`).
    pub rotations: Vec<[f32; 4]>,
    /// Opacity logits (PLY `opacity`).
    pub opacities: Vec<f32>,
    /// SH DC coefficients (PLY `f_dc_0..2`).
    pub sh_dc: Vec<[f32; 3]>,
    /// `3 * rest_coeffs(sh_degree)` floats per splat, channel-major.
    pub sh_rest: Vec<f32>,
    pub sh_degree: u8,
    pub antialiased: bool,
    pub coords: Coords,
}

impl Splats {
    pub fn empty(sh_degree: u8) -> Splats {
        Splats {
            positions: Vec::new(),
            scales: Vec::new(),
            rotations: Vec::new(),
            opacities: Vec::new(),
            sh_dc: Vec::new(),
            sh_rest: Vec::new(),
            sh_degree,
            antialiased: false,
            coords: Coords::Rdf,
        }
    }

    pub fn len(&self) -> usize {
        self.positions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    /// Number of `sh_rest` floats stored per splat.
    pub fn rest_per_splat(&self) -> usize {
        3 * rest_coeffs(self.sh_degree)
    }

    /// Check that all attribute arrays agree. Writers call this first.
    pub fn validate(&self) -> Result<()> {
        let n = self.positions.len();
        if self.sh_degree > 3 {
            return Err(FormatError::Invalid(format!(
                "SH degree {} out of range 0..=3",
                self.sh_degree
            )));
        }
        if n > MAX_SPLATS {
            return Err(FormatError::Limit(format!(
                "{n} splats exceeds the {MAX_SPLATS} cap"
            )));
        }
        let rest = n
            .checked_mul(self.rest_per_splat())
            .ok_or_else(|| FormatError::Limit("SH size overflow".into()))?;
        if self.scales.len() != n
            || self.rotations.len() != n
            || self.opacities.len() != n
            || self.sh_dc.len() != n
            || self.sh_rest.len() != rest
        {
            return Err(FormatError::Invalid(
                "attribute arrays have inconsistent lengths".into(),
            ));
        }
        Ok(())
    }

    /// True when every value of splat `i` is finite.
    pub fn is_finite_at(&self, i: usize) -> bool {
        let rest = self.rest_per_splat();
        let ok3 = |v: Option<&[f32; 3]>| v.is_some_and(|a| a.iter().all(|x| x.is_finite()));
        let sh_ok = rest == 0
            || i.checked_mul(rest)
                .and_then(|s| s.checked_add(rest).map(|e| (s, e)))
                .and_then(|(s, e)| self.sh_rest.get(s..e))
                .is_some_and(|sl| sl.iter().all(|x| x.is_finite()));
        ok3(self.positions.get(i))
            && ok3(self.scales.get(i))
            && ok3(self.sh_dc.get(i))
            && self
                .rotations
                .get(i)
                .is_some_and(|a| a.iter().all(|x| x.is_finite()))
            && self.opacities.get(i).is_some_and(|x| x.is_finite())
            && sh_ok
    }

    /// Index of the first splat containing a NaN or infinity.
    pub fn first_non_finite(&self) -> Option<usize> {
        (0..self.len()).find(|&i| !self.is_finite_at(i))
    }

    /// Convert in place between RDF and RUB (a 180 degree turn about X).
    pub fn convert_coords(&mut self, to: Coords) {
        if self.coords == to {
            return;
        }
        for p in &mut self.positions {
            p[1] = -p[1];
            p[2] = -p[2];
        }
        for r in &mut self.rotations {
            // (w, x, y, z): x unchanged, y and z negated.
            r[2] = -r[2];
            r[3] = -r[3];
        }
        let n = rest_coeffs(self.sh_degree);
        let stride = 3 * n;
        if stride > 0 {
            for chunk in self.sh_rest.chunks_exact_mut(stride) {
                for (c, channel) in chunk.chunks_exact_mut(n).enumerate() {
                    let _ = c;
                    for (k, v) in channel.iter_mut().enumerate() {
                        *v *= SH_FLIP.get(k).copied().unwrap_or(1.0);
                    }
                }
            }
        }
        self.coords = to;
    }

    /// Clone converted to another coordinate system.
    pub fn to_coords(&self, to: Coords) -> Splats {
        let mut s = self.clone();
        s.convert_coords(to);
        s
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::synth::synthetic;

    #[test]
    fn coord_swap_twice_is_identity() {
        let s = synthetic(64, 3, 5);
        let back = s.to_coords(Coords::Rub).to_coords(Coords::Rdf);
        assert_eq!(s, back);
    }

    #[test]
    fn rub_negates_y_and_z() {
        let s = synthetic(8, 0, 1);
        let r = s.to_coords(Coords::Rub);
        for (a, b) in s.positions.iter().zip(&r.positions) {
            assert_eq!(a[0], b[0]);
            assert_eq!(-a[1], b[1]);
            assert_eq!(-a[2], b[2]);
        }
    }

    #[test]
    fn validate_catches_mismatched_arrays() {
        let mut s = synthetic(4, 1, 2);
        s.opacities.pop();
        assert!(s.validate().is_err());
    }

    #[test]
    fn non_finite_is_found() {
        let mut s = synthetic(10, 0, 3);
        s.scales[7][1] = f32::NAN;
        assert_eq!(s.first_non_finite(), Some(7));
    }
}
