//! Deterministic synthetic splats for tests, smoke checks and recipes.

use crate::model::{rest_coeffs, Coords, Splats};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform in [lo, hi).
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        let u = (self.next() >> 40) as f32 / (1u64 << 24) as f32;
        lo + (hi - lo) * u
    }
}

/// `n` plausible splats with SH degree `degree`, reproducible from `seed`.
pub fn synthetic(n: usize, degree: u8, seed: u64) -> Splats {
    let degree = degree.min(3);
    let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let rest = 3 * rest_coeffs(degree);
    let mut s = Splats::empty(degree);
    s.coords = Coords::Rdf;
    for _ in 0..n {
        s.positions.push([
            rng.range(-5.0, 5.0),
            rng.range(-5.0, 5.0),
            rng.range(-5.0, 5.0),
        ]);
        s.scales.push([
            rng.range(-6.0, -2.0),
            rng.range(-6.0, -2.0),
            rng.range(-6.0, -2.0),
        ]);
        let mut q = [
            rng.range(-1.0, 1.0),
            rng.range(-1.0, 1.0),
            rng.range(-1.0, 1.0),
            rng.range(-1.0, 1.0),
        ];
        let norm = q.iter().map(|v| v * v).sum::<f32>().sqrt();
        if norm < 1e-3 {
            q = [1.0, 0.0, 0.0, 0.0];
        } else {
            for v in &mut q {
                *v /= norm;
            }
        }
        s.rotations.push(q);
        s.opacities.push(rng.range(-2.0, 4.0));
        s.sh_dc.push([
            rng.range(-1.5, 1.5),
            rng.range(-1.5, 1.5),
            rng.range(-1.5, 1.5),
        ]);
        for _ in 0..rest {
            s.sh_rest.push(rng.range(-0.5, 0.5));
        }
    }
    s
}
