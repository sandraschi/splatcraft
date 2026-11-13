//! Recolor near-grey splats by tinting SH DC.
//!
//! A splat is grey when its base colour (DC only) has channel spread at or
//! below `grey_delta`. Tinted splats have their higher SH bands cleared so the
//! recipe colour is what renders. Saturated splats are left alone.

use splat_formats::{FormatError, Result, Splats};

/// 3DGS DC scale: `rgb = 0.5 + SH_C0 * dc`.
const SH_C0: f32 = 0.282_094_8;

/// Named tint. `tint` multiplies linear RGB in 0..=1.
pub struct Recipe {
    pub id: &'static str,
    pub tint: [f32; 3],
}

/// Offline colour recipes. These are the recolor presets, not Marble assets.
pub const RECIPES: &[Recipe] = &[
    Recipe {
        id: "ghibli",
        tint: [0.62, 0.78, 0.55],
    },
    Recipe {
        id: "cyberpunk",
        tint: [0.95, 0.22, 0.78],
    },
    Recipe {
        id: "ova",
        tint: [0.96, 0.68, 0.40],
    },
];

pub struct RecolorStats {
    pub tinted: usize,
    pub skipped_color: usize,
    pub skipped_bad: usize,
}

pub fn find(id: &str) -> Result<&'static Recipe> {
    RECIPES
        .iter()
        .find(|r| r.id.eq_ignore_ascii_case(id))
        .ok_or_else(|| {
            FormatError::Invalid(format!(
                "unknown recolor recipe '{id}' (known: {})",
                RECIPES.iter().map(|r| r.id).collect::<Vec<_>>().join(", ")
            ))
        })
}

/// Returns a new set. `grey_delta` is the maximum RGB channel spread still called grey.
pub fn apply(s: &Splats, recipe: &Recipe, grey_delta: f32) -> Result<(Splats, RecolorStats)> {
    if !(0.0..=1.0).contains(&grey_delta) || !grey_delta.is_finite() {
        return Err(FormatError::Invalid(
            "grey_delta must be a finite number from 0 to 1".into(),
        ));
    }
    let mut out = s.clone();
    let rest = out.rest_per_splat();
    let mut stats = RecolorStats {
        tinted: 0,
        skipped_color: 0,
        skipped_bad: 0,
    };
    for i in 0..out.len() {
        if !out.is_finite_at(i) {
            stats.skipped_bad += 1;
            continue;
        }
        let rgb = dc_to_rgb(out.sh_dc[i]);
        let spread = channel_spread(rgb);
        if spread > grey_delta {
            stats.skipped_color += 1;
            continue;
        }
        let tinted = [
            (rgb[0] * recipe.tint[0]).clamp(0.0, 1.0),
            (rgb[1] * recipe.tint[1]).clamp(0.0, 1.0),
            (rgb[2] * recipe.tint[2]).clamp(0.0, 1.0),
        ];
        out.sh_dc[i] = rgb_to_dc(tinted);
        if rest > 0 {
            let start = i * rest;
            for v in &mut out.sh_rest[start..start + rest] {
                *v = 0.0;
            }
        }
        stats.tinted += 1;
    }
    Ok((out, stats))
}

fn dc_to_rgb(dc: [f32; 3]) -> [f32; 3] {
    [
        (0.5 + SH_C0 * dc[0]).clamp(0.0, 1.0),
        (0.5 + SH_C0 * dc[1]).clamp(0.0, 1.0),
        (0.5 + SH_C0 * dc[2]).clamp(0.0, 1.0),
    ]
}

fn rgb_to_dc(rgb: [f32; 3]) -> [f32; 3] {
    [
        (rgb[0] - 0.5) / SH_C0,
        (rgb[1] - 0.5) / SH_C0,
        (rgb[2] - 0.5) / SH_C0,
    ]
}

fn channel_spread(rgb: [f32; 3]) -> f32 {
    let max = rgb[0].max(rgb[1]).max(rgb[2]);
    let min = rgb[0].min(rgb[1]).min(rgb[2]);
    max - min
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use splat_formats::Splats;

    fn one(dc: [f32; 3], degree: u8) -> Splats {
        let mut s = Splats::empty(degree);
        s.positions.push([0.0, 0.0, 0.0]);
        s.scales.push([-4.0, -4.0, -4.0]);
        s.rotations.push([1.0, 0.0, 0.0, 0.0]);
        s.opacities.push(0.0);
        s.sh_dc.push(dc);
        s.sh_rest.resize(s.rest_per_splat(), 0.25);
        s
    }

    #[test]
    fn grey_is_tinted_and_saturated_is_kept() {
        let mut scene = Splats::empty(0);
        for dc in [[0.0, 0.0, 0.0], [1.8, -1.5, -1.5]] {
            scene.positions.push([0.0, 0.0, 0.0]);
            scene.scales.push([-4.0, -4.0, -4.0]);
            scene.rotations.push([1.0, 0.0, 0.0, 0.0]);
            scene.opacities.push(0.0);
            scene.sh_dc.push(dc);
        }
        let recipe = find("ghibli").unwrap();
        let (out, stats) = apply(&scene, recipe, 0.08).unwrap();
        assert_eq!(stats.tinted, 1);
        assert_eq!(stats.skipped_color, 1);
        assert_ne!(out.sh_dc[0], scene.sh_dc[0]);
        assert_eq!(out.sh_dc[1], scene.sh_dc[1]);
        let rgb = [
            0.5 + SH_C0 * out.sh_dc[0][0],
            0.5 + SH_C0 * out.sh_dc[0][1],
            0.5 + SH_C0 * out.sh_dc[0][2],
        ];
        assert!((rgb[1] - 0.5 * recipe.tint[1]).abs() < 1e-4);
    }

    #[test]
    fn higher_bands_on_a_tinted_splat_are_cleared() {
        let s = one([0.0, 0.0, 0.0], 1);
        let (out, stats) = apply(&s, find("ova").unwrap(), 0.08).unwrap();
        assert_eq!(stats.tinted, 1);
        assert!(out.sh_rest.iter().all(|v| *v == 0.0));
    }

    #[test]
    fn unknown_recipe_and_bad_delta_fail() {
        assert!(find("noir").is_err());
        let s = one([0.0, 0.0, 0.0], 0);
        assert!(apply(&s, find("cyberpunk").unwrap(), 2.0).is_err());
    }
}
