//! Headless still rendering of splats on the GPU.
//!
//! Built on `wgpu-3dgs-viewer`. Scenes are handed over as an in-memory PLY
//! produced by our own codec, so editing and rendering share one data path.
//! The background is opaque black (the viewer clears to black).
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::unimplemented)]
#![deny(clippy::todo)]

use std::fmt;
use std::io::Cursor;
use std::sync::mpsc;

use splat_formats::{ply, Coords, Splats};
use wgpu_3dgs_viewer as gs;
use wgpu_3dgs_viewer::core::glam::{UVec2, Vec3};
use wgpu_3dgs_viewer::core::{
    GaussianDisplayMode, GaussianMaxStdDev, GaussianShDegree, Gaussians, GaussiansSource,
};

#[derive(Debug)]
pub enum ViewError {
    /// No usable GPU adapter. Callers (and tests) may treat this as "skip".
    NoGpu(String),
    Gpu(String),
    Input(String),
    Encode(String),
}

impl fmt::Display for ViewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ViewError::NoGpu(m) => write!(f, "no GPU available: {m}"),
            ViewError::Gpu(m) => write!(f, "gpu error: {m}"),
            ViewError::Input(m) => write!(f, "invalid render input: {m}"),
            ViewError::Encode(m) => write!(f, "png encode error: {m}"),
        }
    }
}

impl std::error::Error for ViewError {}

#[derive(Clone, Debug)]
pub struct RenderOptions {
    pub width: u32,
    pub height: u32,
    /// Orbit angle around the scene centre; 0 looks along +z (RDF front).
    pub yaw_deg: f32,
    pub pitch_deg: f32,
    pub fov_deg: f32,
    /// Multiplies the auto-framed camera distance.
    pub distance_scale: f32,
    /// Splat size multiplier (1.0 = true size).
    pub gaussian_scale: f32,
}

impl Default for RenderOptions {
    fn default() -> Self {
        RenderOptions {
            width: 960,
            height: 540,
            yaw_deg: 0.0,
            pitch_deg: 0.0,
            fov_deg: 60.0,
            distance_scale: 1.0,
            gaussian_scale: 1.0,
        }
    }
}

pub struct Image {
    pub width: u32,
    pub height: u32,
    /// Tightly packed RGBA8.
    pub rgba: Vec<u8>,
}

impl Image {
    /// Share of pixels whose RGB is not black (the viewer clears to opaque black).
    pub fn covered_fraction(&self) -> f32 {
        let total = self.rgba.len() / 4;
        if total == 0 {
            return 0.0;
        }
        let lit = self
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[..3].iter().any(|v| *v != 0))
            .count();
        lit as f32 / total as f32
    }

    pub fn to_png(&self) -> Result<Vec<u8>, ViewError> {
        let mut out = Vec::new();
        let mut enc = png::Encoder::new(&mut out, self.width, self.height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc
            .write_header()
            .map_err(|e| ViewError::Encode(e.to_string()))?;
        w.write_image_data(&self.rgba)
            .map_err(|e| ViewError::Encode(e.to_string()))?;
        w.finish().map_err(|e| ViewError::Encode(e.to_string()))?;
        Ok(out)
    }
}

/// Centre and robust radius (98th percentile distance) of finite positions,
/// in the viewer frame (RDF turned 180 degrees about Z: x and y negated).
fn frame_scene(s: &Splats) -> Option<(Vec3, f32)> {
    let pts: Vec<Vec3> = s
        .positions
        .iter()
        .filter(|p| p.iter().all(|v| v.is_finite()))
        .map(|p| Vec3::new(-p[0], -p[1], p[2]))
        .collect();
    if pts.is_empty() {
        return None;
    }
    let step = (pts.len() / 200_000).max(1);
    let sample: Vec<Vec3> = pts.iter().step_by(step).copied().collect();
    let centre = sample.iter().fold(Vec3::ZERO, |a, p| a + *p) / sample.len() as f32;
    let mut d: Vec<f32> = sample.iter().map(|p| p.distance(centre)).collect();
    d.sort_by(|a, b| a.total_cmp(b));
    let idx = ((d.len() as f32 * 0.98) as usize).min(d.len() - 1);
    Some((centre, d[idx].max(1e-3)))
}

pub fn render(splats: &Splats, opts: &RenderOptions) -> Result<Image, ViewError> {
    if splats.is_empty() {
        return Err(ViewError::Input("no splats to render".into()));
    }
    if !(16..=4096).contains(&opts.width) || !(16..=4096).contains(&opts.height) {
        return Err(ViewError::Input(
            "width and height must be 16..=4096".into(),
        ));
    }
    if !(opts.fov_deg > 5.0 && opts.fov_deg < 170.0)
        || !(opts.distance_scale.is_finite() && opts.distance_scale > 0.0)
        || !(opts.gaussian_scale.is_finite() && opts.gaussian_scale > 0.0)
        || !opts.yaw_deg.is_finite()
        || !opts.pitch_deg.is_finite()
    {
        return Err(ViewError::Input("camera parameters out of range".into()));
    }
    let splats = splats.to_coords(Coords::Rdf);
    let (centre, radius) =
        frame_scene(&splats).ok_or_else(|| ViewError::Input("no finite splat positions".into()))?;
    let ply_bytes = ply::write(&splats).map_err(|e| ViewError::Input(e.to_string()))?;
    let gaussians = Gaussians::read_from(&mut Cursor::new(ply_bytes), GaussiansSource::Ply)
        .map_err(|e| ViewError::Input(format!("viewer could not read the scene: {e}")))?;

    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
        apply_limit_buckets: false,
    }))
    .map_err(|e| ViewError::NoGpu(e.to_string()))?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("splatcraft render"),
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .map_err(|e| ViewError::Gpu(e.to_string()))?;

    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut viewer: gs::Viewer = gs::Viewer::new(&device, format, &gaussians)
        .map_err(|e| ViewError::Gpu(format!("viewer: {e}")))?;
    viewer.update_model_transform(
        &queue,
        Vec3::ZERO,
        gs::core::glam::Quat::from_axis_angle(Vec3::Z, std::f32::consts::PI),
        Vec3::ONE,
    );
    let sh = GaussianShDegree::new(splats.sh_degree)
        .into_iter()
        .next()
        .ok_or_else(|| ViewError::Input("bad SH degree".into()))?;
    let sd = GaussianMaxStdDev::new(3.0)
        .into_iter()
        .next()
        .ok_or_else(|| ViewError::Gpu("bad max std dev".into()))?;
    viewer.update_gaussian_transform(
        &queue,
        opts.gaussian_scale,
        GaussianDisplayMode::Splat,
        sh,
        false,
        sd,
    );

    let fov = opts.fov_deg.to_radians();
    let dist = radius / (fov * 0.5).tan() * 1.1 * opts.distance_scale;
    let mut camera = gs::Camera::new((dist * 0.01).max(0.01)..(dist * 20.0 + radius * 10.0), fov);
    camera.yaw_by(opts.yaw_deg.to_radians());
    camera.pitch_by(opts.pitch_deg.to_radians());
    camera.pos = centre - camera.get_forward() * dist;
    viewer.update_camera(&queue, &camera, UVec2::new(opts.width, opts.height));

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("target"),
        size: wgpu::Extent3d {
            width: opts.width,
            height: opts.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let row_bytes = opts.width * 4;
    let padded = row_bytes.div_ceil(256) * 256;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(padded) * u64::from(opts.height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("render"),
    });
    viewer.render(&mut encoder, &view);
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(opts.height),
            },
        },
        wgpu::Extent3d {
            width: opts.width,
            height: opts.height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(std::iter::once(encoder.finish()));

    let (tx, rx) = mpsc::channel();
    readback.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|e| ViewError::Gpu(format!("poll: {e:?}")))?;
    rx.recv()
        .map_err(|e| ViewError::Gpu(e.to_string()))?
        .map_err(|e| ViewError::Gpu(format!("map: {e:?}")))?;
    let data = readback
        .slice(..)
        .get_mapped_range()
        .map_err(|e| ViewError::Gpu(format!("map range: {e:?}")))?;
    let mut rgba = Vec::with_capacity((row_bytes * opts.height) as usize);
    for row in data.chunks(padded as usize).take(opts.height as usize) {
        rgba.extend_from_slice(row.get(..row_bytes as usize).unwrap_or(row));
    }
    drop(data);
    readback.unmap();
    Ok(Image {
        width: opts.width,
        height: opts.height,
        rgba,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use splat_formats::synth::synthetic;

    /// A dense cluster of opaque, reasonably large splats.
    fn blob() -> Splats {
        let mut s = synthetic(3000, 0, 5);
        for p in &mut s.positions {
            for v in p.iter_mut() {
                *v *= 0.2;
            }
        }
        for sc in &mut s.scales {
            *sc = [-3.0; 3];
        }
        s.opacities.fill(4.0);
        s
    }

    #[test]
    fn framing_is_sane() {
        let (c, r) = frame_scene(&blob()).unwrap();
        assert!(c.length() < 0.5 && r > 0.05 && r < 3.0);
        assert!(frame_scene(&Splats::empty(0)).is_none());
    }

    #[test]
    fn rejects_bad_input() {
        let o = RenderOptions::default();
        assert!(matches!(
            render(&Splats::empty(0), &o),
            Err(ViewError::Input(_))
        ));
        let bad = RenderOptions {
            width: 5,
            ..RenderOptions::default()
        };
        assert!(matches!(render(&blob(), &bad), Err(ViewError::Input(_))));
    }

    #[test]
    fn renders_something_or_skips_without_gpu() {
        let opts = RenderOptions {
            width: 320,
            height: 240,
            ..RenderOptions::default()
        };
        match render(&blob(), &opts) {
            Ok(img) => {
                assert_eq!(img.rgba.len(), 320 * 240 * 4);
                let cov = img.covered_fraction();
                assert!(cov > 0.02, "scene should cover some pixels, got {cov}");
                assert!(cov < 0.999, "scene should not fill every pixel, got {cov}");
                let png = img.to_png().unwrap();
                assert_eq!(&png[1..4], b"PNG");
            }
            Err(ViewError::NoGpu(m)) => eprintln!("skipped, no GPU: {m}"),
            Err(e) => panic!("render failed: {e}"),
        }
    }
}
