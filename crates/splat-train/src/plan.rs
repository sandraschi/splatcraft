//! Pure planning: turn user parameters into an ordered list of stages.
//!
//! Tool versions differ (Brush renamed its iteration flag between releases),
//! so planning reads each tool's `--help` through a [`Probe`] and only emits
//! flags it saw. A tool that lacks a flag we need makes planning fail with a
//! clear message instead of running a guessed command line.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::error::{Result, TrainError};
use crate::probe::Probe;

pub const TIMEOUT_FFMPEG: u64 = 3600;
pub const TIMEOUT_COLMAP: u64 = 7200;
pub const TIMEOUT_BRUSH: u64 = 4 * 3600;
pub const TIMEOUT_LOCAL: u64 = 900;

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Mkdir(PathBuf),
    Exec {
        program: PathBuf,
        args: Vec<String>,
    },
    CopyImages {
        from: PathBuf,
        to: PathBuf,
        max: usize,
    },
    /// Fail with `hint` unless one of the paths exists.
    Require {
        any_of: Vec<PathBuf>,
        hint: String,
    },
    /// Validate the newest `.ply` in `from_dir` and write `output`.
    Finalize {
        from_dir: PathBuf,
        output: PathBuf,
        overwrite: bool,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stage {
    pub name: String,
    pub kind: Kind,
    pub timeout_secs: u64,
}

fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn bad(m: &str) -> TrainError {
    TrainError::Invalid(format!("corrupt job.json: {m}"))
}

fn text<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    v.get(k).and_then(Value::as_str).ok_or_else(|| bad(k))
}

impl Stage {
    pub fn to_json(&self) -> Value {
        let mut v = match &self.kind {
            Kind::Mkdir(p) => json!({"kind": "mkdir", "path": s(p)}),
            Kind::Exec { program, args } => {
                json!({"kind": "exec", "program": s(program), "args": args})
            }
            Kind::CopyImages { from, to, max } => {
                json!({"kind": "copy_images", "from": s(from), "to": s(to), "max": max})
            }
            Kind::Require { any_of, hint } => json!({
                "kind": "require",
                "any_of": any_of.iter().map(|p| s(p)).collect::<Vec<_>>(),
                "hint": hint
            }),
            Kind::Finalize {
                from_dir,
                output,
                overwrite,
            } => json!({
                "kind": "finalize",
                "from_dir": s(from_dir),
                "output": s(output),
                "overwrite": overwrite
            }),
        };
        v["name"] = json!(self.name);
        v["timeout_secs"] = json!(self.timeout_secs);
        v
    }

    pub fn from_json(v: &Value) -> Result<Stage> {
        let kind = match text(v, "kind")? {
            "mkdir" => Kind::Mkdir(PathBuf::from(text(v, "path")?)),
            "exec" => Kind::Exec {
                program: PathBuf::from(text(v, "program")?),
                args: v
                    .get("args")
                    .and_then(Value::as_array)
                    .ok_or_else(|| bad("args"))?
                    .iter()
                    .filter_map(|a| a.as_str().map(str::to_string))
                    .collect(),
            },
            "copy_images" => Kind::CopyImages {
                from: PathBuf::from(text(v, "from")?),
                to: PathBuf::from(text(v, "to")?),
                max: v
                    .get("max")
                    .and_then(Value::as_u64)
                    .and_then(|m| usize::try_from(m).ok())
                    .ok_or_else(|| bad("max"))?,
            },
            "require" => Kind::Require {
                any_of: v
                    .get("any_of")
                    .and_then(Value::as_array)
                    .ok_or_else(|| bad("any_of"))?
                    .iter()
                    .filter_map(|a| a.as_str().map(PathBuf::from))
                    .collect(),
                hint: text(v, "hint")?.to_string(),
            },
            "finalize" => Kind::Finalize {
                from_dir: PathBuf::from(text(v, "from_dir")?),
                output: PathBuf::from(text(v, "output")?),
                overwrite: v.get("overwrite").and_then(Value::as_bool).unwrap_or(false),
            },
            other => return Err(bad(&format!("unknown stage kind '{other}'"))),
        };
        Ok(Stage {
            name: text(v, "name")?.to_string(),
            kind,
            timeout_secs: v
                .get("timeout_secs")
                .and_then(Value::as_u64)
                .ok_or_else(|| bad("timeout_secs"))?,
        })
    }
}

#[derive(Clone, Debug)]
pub enum Source {
    Video(PathBuf),
    Images(PathBuf),
    /// An existing COLMAP or Nerfstudio dataset folder (skips ffmpeg, COLMAP).
    Dataset(PathBuf),
}

#[derive(Clone, Debug)]
pub struct Params {
    pub source: Source,
    pub work_dir: PathBuf,
    pub output: PathBuf,
    pub overwrite: bool,
    pub target_frames: u32,
    pub max_resolution: u32,
    pub iters: u32,
    pub max_splats: u32,
    pub sh_degree: u32,
    pub quality: String,
}

impl Params {
    pub fn validate(&self) -> Result<()> {
        let range = |name: &str, v: u32, lo: u32, hi: u32| -> Result<()> {
            if (lo..=hi).contains(&v) {
                Ok(())
            } else {
                Err(TrainError::Invalid(format!("{name} must be {lo}..={hi}")))
            }
        };
        range("target_frames", self.target_frames, 20, 1500)?;
        range("max_resolution", self.max_resolution, 320, 4096)?;
        range("iters", self.iters, 100, 200_000)?;
        range("max_splats", self.max_splats, 10_000, 20_000_000)?;
        range("sh_degree", self.sh_degree, 0, 3)?;
        if !["low", "medium", "high", "extreme"].contains(&self.quality.as_str()) {
            return Err(TrainError::Invalid(
                "quality must be low, medium, high or extreme".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct Tools {
    pub ffmpeg: Option<PathBuf>,
    pub ffprobe: Option<PathBuf>,
    pub colmap: Option<PathBuf>,
    pub brush: Option<PathBuf>,
}

#[derive(Debug)]
pub struct Plan {
    pub stages: Vec<Stage>,
    pub notes: Vec<String>,
}

/// True when `flag` appears in `help` as a whole token (`--max-splats` does
/// not match `--max-splats-x`).
pub fn flag_present(help: &str, flag: &str) -> bool {
    help.match_indices(flag).any(|(i, _)| {
        help[i + flag.len()..]
            .chars()
            .next()
            .is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
    })
}

#[derive(Debug, PartialEq)]
pub struct BrushFlags {
    pub subcommand: bool,
    pub iters: &'static str,
    pub max_splats: bool,
    pub sh_degree: bool,
    pub max_res: bool,
    pub export_name: bool,
    pub export_every: bool,
}

const ITER_FLAGS: [&str; 2] = ["--total-train-iters", "--total-steps"];

fn has_train_command(help: &str) -> bool {
    help.lines().any(|l| {
        let t = l.trim();
        t == "train" || t.starts_with("train ")
    })
}

fn flags_from(help: &str, subcommand: bool) -> Result<BrushFlags> {
    let iters = ITER_FLAGS
        .iter()
        .copied()
        .find(|f| flag_present(help, f))
        .ok_or_else(|| {
            TrainError::Missing(
                "this Brush version lists neither --total-train-iters nor --total-steps; \
                 it is not supported yet"
                    .into(),
            )
        })?;
    if !flag_present(help, "--export-path") {
        return Err(TrainError::Missing(
            "this Brush version has no --export-path; it is not supported yet".into(),
        ));
    }
    Ok(BrushFlags {
        subcommand,
        iters,
        max_splats: flag_present(help, "--max-splats"),
        sh_degree: flag_present(help, "--sh-degree"),
        max_res: flag_present(help, "--max-resolution"),
        export_name: flag_present(help, "--export-name"),
        export_every: flag_present(help, "--export-every"),
    })
}

/// Work out Brush's calling convention from `--help` (and `train --help` for
/// builds that put training behind a subcommand).
pub fn detect_brush(brush: &Path, probe: &dyn Probe) -> Result<BrushFlags> {
    let top = probe.run_capture(brush, &["--help"], 30)?;
    if ITER_FLAGS.iter().any(|f| flag_present(&top, f)) {
        return flags_from(&top, false);
    }
    if has_train_command(&top) {
        let sub = probe.run_capture(brush, &["train", "--help"], 30)?;
        return flags_from(&sub, true);
    }
    flags_from(&top, false)
}

#[derive(Debug, PartialEq)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    pub duration: f64,
}

/// Parse ffprobe `-of default=noprint_wrappers=1` output (`key=value` lines).
pub fn parse_video_info(text: &str) -> Result<VideoInfo> {
    let get = |key: &str| -> Option<&str> {
        text.lines()
            .find_map(|l| l.trim().strip_prefix(key).and_then(|r| r.strip_prefix('=')))
    };
    let num = |key: &str| -> Result<f64> {
        get(key)
            .and_then(|v| v.trim().parse::<f64>().ok())
            .filter(|v| v.is_finite() && *v > 0.0)
            .ok_or_else(|| TrainError::Invalid(format!("ffprobe gave no usable {key}")))
    };
    Ok(VideoInfo {
        width: num("width")? as u32,
        height: num("height")? as u32,
        duration: num("duration")?,
    })
}

/// Sampling rate that yields about `target` frames from `duration` seconds.
pub fn sample_fps(duration: f64, target: u32) -> f64 {
    (f64::from(target) / duration).clamp(0.05, 12.0)
}

fn need<'a>(t: &'a Option<PathBuf>, name: &str, how: &str) -> Result<&'a PathBuf> {
    t.as_ref().ok_or_else(|| {
        TrainError::Missing(format!(
            "{name} was not found. {how} (or set SPLATCRAFT_{} to the executable)",
            name.to_ascii_uppercase()
        ))
    })
}

fn colmap_stage(
    colmap: &Path,
    ws: &Path,
    quality: &str,
    video: bool,
    probe: &dyn Probe,
    notes: &mut Vec<String>,
) -> Result<Vec<Stage>> {
    let help = probe.run_capture(colmap, &["automatic_reconstructor", "--help"], 60)?;
    for f in ["--workspace_path", "--image_path"] {
        if !flag_present(&help, f) {
            return Err(TrainError::Missing(format!(
                "COLMAP automatic_reconstructor has no {f}; this version is not supported yet"
            )));
        }
    }
    let mut a = vec![
        "automatic_reconstructor".to_string(),
        "--workspace_path".into(),
        s(ws),
        "--image_path".into(),
        s(&ws.join("images")),
    ];
    let mut opt = |flag: &str, val: &str| {
        if flag_present(&help, flag) {
            a.push(flag.into());
            a.push(val.into());
        } else {
            notes.push(format!("COLMAP has no {flag}; left at its default"));
        }
    };
    opt("--quality", quality);
    opt("--data_type", if video { "video" } else { "individual" });
    opt("--single_camera", "1");
    opt("--dense", "0");
    Ok(vec![
        Stage {
            name: "colmap".into(),
            kind: Kind::Exec {
                program: colmap.to_path_buf(),
                args: a,
            },
            timeout_secs: TIMEOUT_COLMAP,
        },
        Stage {
            name: "check_sparse".into(),
            kind: Kind::Require {
                any_of: vec![
                    ws.join("sparse").join("0").join("images.bin"),
                    ws.join("sparse").join("0").join("images.txt"),
                ],
                hint: "COLMAP produced no sparse model. The frames probably do not overlap \
                       enough, are too blurry, or the camera barely moved; try a slower, \
                       steadier orbit around the subject."
                    .into(),
            },
            timeout_secs: TIMEOUT_LOCAL,
        },
    ])
}

fn brush_stage(
    brush: &Path,
    flags: &BrushFlags,
    p: &Params,
    dataset: &Path,
    out: &Path,
) -> Vec<Stage> {
    let mut a: Vec<String> = Vec::new();
    if flags.subcommand {
        a.push("train".into());
    }
    a.push(s(dataset));
    a.push(flags.iters.into());
    a.push(p.iters.to_string());
    if flags.max_splats {
        a.push("--max-splats".into());
        a.push(p.max_splats.to_string());
    }
    if flags.sh_degree {
        a.push("--sh-degree".into());
        a.push(p.sh_degree.to_string());
    }
    if flags.max_res {
        a.push("--max-resolution".into());
        a.push(p.max_resolution.to_string());
    }
    a.push("--export-path".into());
    a.push(s(out));
    if flags.export_every {
        a.push("--export-every".into());
        a.push(p.iters.to_string());
    }
    if flags.export_name {
        a.push("--export-name".into());
        a.push("splat.ply".into());
    }
    vec![
        Stage {
            name: "brush".into(),
            kind: Kind::Exec {
                program: brush.to_path_buf(),
                args: a,
            },
            timeout_secs: TIMEOUT_BRUSH,
        },
        Stage {
            name: "finalize".into(),
            kind: Kind::Finalize {
                from_dir: out.to_path_buf(),
                output: p.output.clone(),
                overwrite: p.overwrite,
            },
            timeout_secs: TIMEOUT_LOCAL,
        },
    ]
}

pub fn plan(p: &Params, tools: &Tools, probe: &dyn Probe) -> Result<Plan> {
    p.validate()?;
    let brush = need(
        &tools.brush,
        "brush",
        "Download the Windows zip from https://github.com/ArthurBrussee/brush/releases and \
         unpack it into the tools directory",
    )?;
    let flags = detect_brush(brush, probe)?;
    let ws = &p.work_dir;
    let out = ws.join("out");
    let mut notes = vec![format!(
        "brush: {}{} flag set detected from --help",
        if flags.subcommand {
            "train subcommand, "
        } else {
            ""
        },
        flags.iters
    )];
    let mut stages = Vec::new();
    let dataset: PathBuf = match &p.source {
        Source::Dataset(d) => d.clone(),
        Source::Images(dir) => {
            let colmap = need(
                &tools.colmap,
                "colmap",
                "Install COLMAP from https://github.com/colmap/colmap/releases",
            )?;
            stages.push(Stage {
                name: "copy_images".into(),
                kind: Kind::CopyImages {
                    from: dir.clone(),
                    to: ws.join("images"),
                    max: 1500,
                },
                timeout_secs: TIMEOUT_LOCAL,
            });
            stages.extend(colmap_stage(
                colmap, ws, &p.quality, false, probe, &mut notes,
            )?);
            ws.clone()
        }
        Source::Video(video) => {
            let ffmpeg = need(
                &tools.ffmpeg,
                "ffmpeg",
                "Install ffmpeg (scoop install ffmpeg)",
            )?;
            let ffprobe = need(&tools.ffprobe, "ffprobe", "It ships with ffmpeg")?;
            let colmap = need(
                &tools.colmap,
                "colmap",
                "Install COLMAP from https://github.com/colmap/colmap/releases",
            )?;
            let info = parse_video_info(&probe.run_capture(
                ffprobe,
                &[
                    "-v",
                    "error",
                    "-select_streams",
                    "v:0",
                    "-show_entries",
                    "stream=width,height",
                    "-show_entries",
                    "format=duration",
                    "-of",
                    "default=noprint_wrappers=1",
                    &s(video),
                ],
                60,
            )?)?;
            if info.duration < 2.0 {
                return Err(TrainError::Invalid(
                    "video is shorter than 2 seconds; not enough views".into(),
                ));
            }
            let fps = sample_fps(info.duration, p.target_frames);
            let mut vf = format!("fps={fps:.3}");
            if info.width > p.max_resolution {
                vf.push_str(&format!(",scale={}:-2", p.max_resolution));
            }
            notes.push(format!(
                "video {}x{}, {:.1}s: sampling {fps:.3} fps for about {} frames",
                info.width, info.height, info.duration, p.target_frames
            ));
            stages.push(Stage {
                name: "mkdir_images".into(),
                kind: Kind::Mkdir(ws.join("images")),
                timeout_secs: TIMEOUT_LOCAL,
            });
            stages.push(Stage {
                name: "ffmpeg".into(),
                kind: Kind::Exec {
                    program: ffmpeg.clone(),
                    args: [
                        "-hide_banner",
                        "-nostdin",
                        "-y",
                        "-i",
                        &s(video),
                        "-vf",
                        &vf,
                        "-q:v",
                        "2",
                        "-frames:v",
                        &p.target_frames.to_string(),
                        &s(&ws.join("images").join("frame_%05d.jpg")),
                    ]
                    .iter()
                    .map(|a| (*a).to_string())
                    .collect(),
                },
                timeout_secs: TIMEOUT_FFMPEG,
            });
            stages.extend(colmap_stage(
                colmap, ws, &p.quality, true, probe, &mut notes,
            )?);
            ws.clone()
        }
    };
    stages.extend(brush_stage(brush, &flags, p, &dataset, &out));
    Ok(Plan { stages, notes })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::probe::fake::FakeProbe;

    const BRUSH_NEW: &str = "Usage: brush-cli [OPTIONS] [SOURCE]\n  --total-train-iters <N>\n  \
        --max-splats <N>\n  --sh-degree <N>\n  --max-resolution <N>\n  --export-path <P>\n  \
        --export-name <S>\n  --export-every <N>\n";
    const BRUSH_OLD: &str = "Usage: brush [OPTIONS]\n  --total-steps <N>\n  --export-path <P>\n";
    const BRUSH_SUB: &str = "Commands:\n  train   Train a model\n  view    View\n";
    const COLMAP_HELP: &str = "--workspace_path arg\n--image_path arg\n--quality arg\n\
        --data_type arg\n--single_camera arg\n--dense arg\n";
    const FFPROBE: &str = "width=1920\nheight=1080\nduration=40.000000\n";

    fn params(source: Source) -> Params {
        Params {
            source,
            work_dir: PathBuf::from("/w"),
            output: PathBuf::from("/o/out.spz"),
            overwrite: false,
            target_frames: 120,
            max_resolution: 1280,
            iters: 7000,
            max_splats: 1_000_000,
            sh_degree: 3,
            quality: "medium".into(),
        }
    }

    fn tools() -> Tools {
        Tools {
            ffmpeg: Some("/t/ffmpeg".into()),
            ffprobe: Some("/t/ffprobe".into()),
            colmap: Some("/t/colmap".into()),
            brush: Some("/t/brush".into()),
        }
    }

    fn probe(brush_help: &str) -> FakeProbe {
        FakeProbe::default()
            .reply("brush", "--help", brush_help)
            .reply("colmap", "automatic_reconstructor", COLMAP_HELP)
            .reply("ffprobe", "*", FFPROBE)
    }

    #[test]
    fn flag_matching_is_whole_token() {
        assert!(flag_present("a --max-splats <N>", "--max-splats"));
        assert!(!flag_present("a --max-splats-x", "--max-splats"));
        assert!(flag_present("--x\n--max-splats", "--max-splats"));
        assert!(!flag_present("", "--x"));
    }

    #[test]
    fn brush_detection_handles_three_shapes() {
        let b = Path::new("/t/brush");
        let f = detect_brush(b, &probe(BRUSH_NEW)).unwrap();
        assert_eq!(f.iters, "--total-train-iters");
        assert!(!f.subcommand && f.max_splats && f.export_name);
        let f = detect_brush(b, &probe(BRUSH_OLD)).unwrap();
        assert_eq!(f.iters, "--total-steps");
        assert!(!f.max_splats && !f.export_name);
        let p = probe(BRUSH_SUB).reply("brush", "train", BRUSH_NEW);
        let f = detect_brush(b, &p).unwrap();
        assert!(f.subcommand);
        let e = detect_brush(b, &probe("nothing useful")).unwrap_err();
        assert!(matches!(e, TrainError::Missing(_)));
    }

    #[test]
    fn video_info_parsing() {
        let v = parse_video_info(FFPROBE).unwrap();
        assert_eq!((v.width, v.height), (1920, 1080));
        assert!((v.duration - 40.0).abs() < 1e-9);
        assert!(parse_video_info("width=1920\n").is_err());
        assert!(parse_video_info("width=0\nheight=1\nduration=3").is_err());
        assert!((sample_fps(40.0, 120) - 3.0).abs() < 1e-9);
        assert!((sample_fps(1000.0, 20) - 0.05).abs() < 1e-9);
    }

    #[test]
    fn video_plan_has_the_expected_stages_and_flags() {
        let pl = plan(
            &params(Source::Video("/v/clip.mp4".into())),
            &tools(),
            &probe(BRUSH_NEW),
        )
        .unwrap();
        let names: Vec<&str> = pl.stages.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "mkdir_images",
                "ffmpeg",
                "colmap",
                "check_sparse",
                "brush",
                "finalize"
            ]
        );
        let Kind::Exec { args, .. } = &pl.stages[1].kind else {
            unreachable!("ffmpeg stage is exec")
        };
        assert!(args.contains(&"fps=3.000,scale=1280:-2".to_string()));
        assert!(args.contains(&"120".to_string()));
        let Kind::Exec { args, .. } = &pl.stages[2].kind else {
            unreachable!("colmap stage is exec")
        };
        assert!(args.windows(2).any(|w| w == ["--data_type", "video"]));
        assert!(args.windows(2).any(|w| w == ["--dense", "0"]));
        let Kind::Exec { args, .. } = &pl.stages[4].kind else {
            unreachable!("brush stage is exec")
        };
        assert!(args
            .windows(2)
            .any(|w| w == ["--total-train-iters", "7000"]));
        assert!(args.windows(2).any(|w| w == ["--export-every", "7000"]));
        assert_eq!(args[0], "/w");
    }

    #[test]
    fn no_scale_when_video_is_small_and_old_brush_omits_unknown_flags() {
        let mut p = params(Source::Video("/v/clip.mp4".into()));
        p.max_resolution = 4096;
        let pl = plan(&p, &tools(), &probe(BRUSH_OLD)).unwrap();
        let Kind::Exec { args, .. } = &pl.stages[1].kind else {
            unreachable!("ffmpeg stage is exec")
        };
        assert!(args.contains(&"fps=3.000".to_string()));
        let Kind::Exec { args, .. } = &pl.stages[4].kind else {
            unreachable!("brush stage is exec")
        };
        assert!(args.contains(&"--total-steps".to_string()));
        assert!(!args.contains(&"--max-splats".to_string()));
        assert!(!args.contains(&"--export-name".to_string()));
    }

    #[test]
    fn dataset_plan_skips_ffmpeg_and_colmap() {
        let pl = plan(
            &params(Source::Dataset("/d/set".into())),
            &Tools {
                brush: Some("/t/brush".into()),
                ..Tools::default()
            },
            &probe(BRUSH_NEW),
        )
        .unwrap();
        let names: Vec<&str> = pl.stages.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["brush", "finalize"]);
    }

    #[test]
    fn images_plan_copies_then_runs_colmap() {
        let pl = plan(
            &params(Source::Images("/i".into())),
            &tools(),
            &probe(BRUSH_NEW),
        )
        .unwrap();
        assert_eq!(pl.stages[0].name, "copy_images");
        let Kind::Exec { args, .. } = &pl.stages[1].kind else {
            unreachable!("colmap stage is exec")
        };
        assert!(args.windows(2).any(|w| w == ["--data_type", "individual"]));
    }

    #[test]
    fn missing_tools_and_bad_params_fail_clearly() {
        let p = params(Source::Video("/v.mp4".into()));
        let e = plan(&p, &Tools::default(), &probe(BRUSH_NEW)).unwrap_err();
        assert!(e.to_string().contains("brush was not found"), "{e}");
        let mut t = tools();
        t.colmap = None;
        let e = plan(&p, &t, &probe(BRUSH_NEW)).unwrap_err();
        assert!(e.to_string().contains("colmap was not found"), "{e}");
        let mut bad = params(Source::Dataset("/d".into()));
        bad.iters = 5;
        assert!(plan(&bad, &tools(), &probe(BRUSH_NEW)).is_err());
        let mut bad = params(Source::Dataset("/d".into()));
        bad.quality = "ultra".into();
        assert!(bad.validate().is_err());
    }

    #[test]
    fn short_video_is_rejected() {
        let only = FakeProbe::default()
            .reply("brush", "--help", BRUSH_NEW)
            .reply("ffprobe", "*", "width=640\nheight=480\nduration=1.0\n");
        let e = plan(&params(Source::Video("/v.mp4".into())), &tools(), &only).unwrap_err();
        assert!(e.to_string().contains("shorter than 2 seconds"), "{e}");
    }

    #[test]
    fn stage_json_round_trips() {
        let pl = plan(
            &params(Source::Video("/v/clip.mp4".into())),
            &tools(),
            &probe(BRUSH_NEW),
        )
        .unwrap();
        for st in &pl.stages {
            assert_eq!(&Stage::from_json(&st.to_json()).unwrap(), st);
        }
        let copy = Stage {
            name: "c".into(),
            kind: Kind::CopyImages {
                from: "/a".into(),
                to: "/b".into(),
                max: 5,
            },
            timeout_secs: 1,
        };
        assert_eq!(Stage::from_json(&copy.to_json()).unwrap(), copy);
        assert!(Stage::from_json(&json!({"kind": "zzz", "name": "x", "timeout_secs": 1})).is_err());
    }
}
