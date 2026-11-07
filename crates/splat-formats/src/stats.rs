//! Honest summary numbers for a splat set.

use crate::model::Splats;

#[derive(Clone, Debug, PartialEq)]
pub struct Stats {
    pub count: usize,
    pub sh_degree: u8,
    pub antialiased: bool,
    pub bbox_min: [f32; 3],
    pub bbox_max: [f32; 3],
    /// Mean of sigmoid(opacity logit) over finite splats.
    pub mean_opacity: f32,
    /// Smallest and largest linear scale (exp of the stored log scale).
    pub scale_min: f32,
    pub scale_max: f32,
    /// Splats with any NaN or infinite value.
    pub non_finite: usize,
}

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

pub fn compute(s: &Splats) -> Stats {
    let mut bmin = [f32::INFINITY; 3];
    let mut bmax = [f32::NEG_INFINITY; 3];
    let mut smin = f32::INFINITY;
    let mut smax = f32::NEG_INFINITY;
    let mut op_sum = 0.0f64;
    let mut finite = 0usize;
    let mut bad = 0usize;
    for i in 0..s.len() {
        if !s.is_finite_at(i) {
            bad += 1;
            continue;
        }
        finite += 1;
        if let Some(p) = s.positions.get(i) {
            for k in 0..3 {
                bmin[k] = bmin[k].min(p[k]);
                bmax[k] = bmax[k].max(p[k]);
            }
        }
        if let Some(sc) = s.scales.get(i) {
            for v in sc {
                let e = v.exp();
                smin = smin.min(e);
                smax = smax.max(e);
            }
        }
        if let Some(o) = s.opacities.get(i) {
            op_sum += f64::from(sigmoid(*o));
        }
    }
    if finite == 0 {
        bmin = [0.0; 3];
        bmax = [0.0; 3];
        smin = 0.0;
        smax = 0.0;
    }
    Stats {
        count: s.len(),
        sh_degree: s.sh_degree,
        antialiased: s.antialiased,
        bbox_min: bmin,
        bbox_max: bmax,
        mean_opacity: if finite == 0 {
            0.0
        } else {
            (op_sum / finite as f64) as f32
        },
        scale_min: smin,
        scale_max: smax,
        non_finite: bad,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::synth::synthetic;

    #[test]
    fn stats_of_synthetic() {
        let s = synthetic(500, 0, 9);
        let st = compute(&s);
        assert_eq!(st.count, 500);
        assert_eq!(st.non_finite, 0);
        assert!(st.bbox_min[0] >= -5.0 && st.bbox_max[0] < 5.0);
        assert!(st.mean_opacity > 0.0 && st.mean_opacity < 1.0);
        assert!(st.scale_min > 0.0 && st.scale_min <= st.scale_max);
    }

    #[test]
    fn stats_of_empty() {
        let st = compute(&Splats::empty(0));
        assert_eq!(st.count, 0);
        assert_eq!(st.bbox_min, [0.0; 3]);
    }

    #[test]
    fn stats_counts_non_finite() {
        let mut s = synthetic(10, 0, 1);
        s.opacities[3] = f32::INFINITY;
        assert_eq!(compute(&s).non_finite, 1);
    }
}
