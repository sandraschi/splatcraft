//! Extract and download the assets of a finished Marble world.

use std::path::Path;

use serde_json::{json, Value};
use splat_formats::{spz, FormatError};

use crate::client::Transport;
use crate::error::{Result, WorldsError};

pub const MAX_ASSET_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    Splat100k,
    Splat500k,
    SplatFull,
    Mesh,
    Pano,
    Thumbnail,
}

impl Which {
    pub fn parse(s: &str) -> Option<Which> {
        match s {
            "100k" => Some(Which::Splat100k),
            "500k" => Some(Which::Splat500k),
            "full" => Some(Which::SplatFull),
            "mesh" => Some(Which::Mesh),
            "pano" => Some(Which::Pano),
            "thumbnail" => Some(Which::Thumbnail),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Which::Splat100k => "100k",
            Which::Splat500k => "500k",
            Which::SplatFull => "full",
            Which::Mesh => "mesh",
            Which::Pano => "pano",
            Which::Thumbnail => "thumbnail",
        }
    }

    fn is_splat(self) -> bool {
        matches!(self, Which::Splat100k | Which::Splat500k | Which::SplatFull)
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct AssetUrls {
    pub splat_100k: Option<String>,
    pub splat_500k: Option<String>,
    pub splat_full: Option<String>,
    pub mesh: Option<String>,
    pub pano: Option<String>,
    pub thumbnail: Option<String>,
    pub caption: Option<String>,
}

fn s(v: Option<&Value>) -> Option<String> {
    v.and_then(Value::as_str).map(str::to_string)
}

/// Pull URLs out of a world object (`response` of a finished operation).
pub fn extract(world: &Value) -> AssetUrls {
    let a = world.get("assets");
    let p = |ptr: &str| a.and_then(|a| a.pointer(ptr));
    AssetUrls {
        splat_100k: s(p("/splats/spz_urls/100k")),
        splat_500k: s(p("/splats/spz_urls/500k")),
        splat_full: s(p("/splats/spz_urls/full_res")),
        mesh: s(p("/mesh/collider_mesh_url")),
        pano: s(p("/imagery/pano_url")),
        thumbnail: s(p("/thumbnail_url")),
        caption: s(p("/caption")),
    }
}

impl AssetUrls {
    pub fn url(&self, w: Which) -> Option<&str> {
        match w {
            Which::Splat100k => self.splat_100k.as_deref(),
            Which::Splat500k => self.splat_500k.as_deref(),
            Which::SplatFull => self.splat_full.as_deref(),
            Which::Mesh => self.mesh.as_deref(),
            Which::Pano => self.pano.as_deref(),
            Which::Thumbnail => self.thumbnail.as_deref(),
        }
    }
}

fn clean_tag(tag: &str) -> String {
    let t: String = tag
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(64)
        .collect();
    if t.is_empty() {
        "world".into()
    } else {
        t
    }
}

fn extension(w: Which, url: &str) -> &'static str {
    if w.is_splat() {
        return "spz";
    }
    if w == Which::Mesh {
        return "glb";
    }
    let path = url.split(['?', '#']).next().unwrap_or("");
    match path
        .rsplit('.')
        .next()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "png",
        Some("webp") => "webp",
        _ => "jpg",
    }
}

pub fn file_name(tag: &str, w: Which, url: &str) -> String {
    format!("{}_{}.{}", clean_tag(tag), w.as_str(), extension(w, url))
}

/// Download the requested assets into `dest`. Each result is a JSON entry;
/// per-asset problems are reported, not fatal, except I/O and size errors.
pub fn fetch<T: Transport>(
    t: &T,
    urls: &AssetUrls,
    which: &[Which],
    dest: &Path,
    tag: &str,
    overwrite: bool,
) -> Result<Vec<Value>> {
    std::fs::create_dir_all(dest)?;
    let mut out = Vec::new();
    for &w in which {
        let Some(url) = urls.url(w) else {
            out.push(json!({"asset": w.as_str(), "status": "missing"}));
            continue;
        };
        let path = dest.join(file_name(tag, w, url));
        if path.exists() && !overwrite {
            out.push(
                json!({"asset": w.as_str(), "status": "exists", "path": path.to_string_lossy()}),
            );
            continue;
        }
        let bytes = t.get_bytes(url, MAX_ASSET_BYTES)?;
        let mut tmp = path.as_os_str().to_owned();
        tmp.push(".splatcraft-tmp");
        let tmp = std::path::PathBuf::from(tmp);
        std::fs::write(&tmp, &bytes)?;
        if let Err(e) = std::fs::rename(&tmp, &path) {
            let _ = std::fs::remove_file(&tmp);
            return Err(WorldsError::Io(e.to_string()));
        }
        let mut entry = json!({
            "asset": w.as_str(),
            "status": "downloaded",
            "path": path.to_string_lossy(),
            "bytes": bytes.len(),
        });
        if w.is_splat() {
            match spz::read(&bytes) {
                Ok(s) => {
                    entry["validated"] = json!(true);
                    entry["splats"] = json!(s.len());
                    entry["sh_degree"] = json!(s.sh_degree);
                }
                Err(FormatError::Unsupported(m)) => {
                    entry["validated"] = json!(false);
                    entry["note"] =
                        json!(format!("kept as downloaded, our codec cannot read it: {m}"));
                }
                Err(e) => {
                    entry["validated"] = json!(false);
                    entry["note"] = json!(format!("downloaded file did not parse as SPZ: {e}"));
                }
            }
        }
        out.push(entry);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::client::fake::Fake;
    use splat_formats::synth::synthetic;

    fn world() -> Value {
        json!({"id": "w1", "assets": {
            "splats": {"spz_urls": {"100k": "https://cdn.test/a100.spz", "full_res": "https://cdn.test/full.spz"}},
            "mesh": {"collider_mesh_url": "https://cdn.test/m.glb"},
            "imagery": {"pano_url": "https://cdn.test/p.png?sig=1"},
            "thumbnail_url": "https://cdn.test/t.webp",
            "caption": "a cave"
        }})
    }

    #[test]
    fn extraction() {
        let a = extract(&world());
        assert_eq!(a.splat_100k.as_deref(), Some("https://cdn.test/a100.spz"));
        assert_eq!(a.splat_500k, None);
        assert_eq!(a.mesh.as_deref(), Some("https://cdn.test/m.glb"));
        assert_eq!(a.caption.as_deref(), Some("a cave"));
        assert_eq!(extract(&json!({})), AssetUrls::default());
    }

    #[test]
    fn file_names_are_safe() {
        assert_eq!(file_name("w/1:x", Which::Splat100k, "u"), "w_1_x_100k.spz");
        assert_eq!(file_name("", Which::Mesh, "u"), "world_mesh.glb");
        assert_eq!(
            file_name("a", Which::Pano, "https://x/p.png?s=1"),
            "a_pano.png"
        );
        assert_eq!(
            file_name("a", Which::Thumbnail, "https://x/t"),
            "a_thumbnail.jpg"
        );
    }

    #[test]
    fn fetch_validates_spz_and_reports_missing() {
        let d = tempfile::tempdir().unwrap();
        let f = Fake::default();
        f.bytes.borrow_mut().insert(
            "https://cdn.test/a100.spz".into(),
            splat_formats::spz::write(&synthetic(120, 1, 3)).unwrap(),
        );
        f.bytes.borrow_mut().insert(
            "https://cdn.test/full.spz".into(),
            vec![0x28, 0xb5, 0x2f, 0xfd, 1, 2, 3],
        );
        let urls = extract(&world());
        let r = fetch(
            &f,
            &urls,
            &[Which::Splat100k, Which::Splat500k, Which::SplatFull],
            d.path(),
            "w1",
            false,
        )
        .unwrap();
        assert_eq!(r[0]["status"], "downloaded");
        assert_eq!(r[0]["validated"], true);
        assert_eq!(r[0]["splats"], 120);
        assert_eq!(r[1]["status"], "missing");
        assert_eq!(r[2]["validated"], false);
        assert!(r[2]["note"].as_str().unwrap().contains("v4"));
        assert!(d.path().join("w1_100k.spz").exists());
        // Second run does not overwrite.
        let again = fetch(&f, &urls, &[Which::Splat100k], d.path(), "w1", false).unwrap();
        assert_eq!(again[0]["status"], "exists");
    }
}
