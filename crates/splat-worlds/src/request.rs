//! Pure request building and cost estimation. No network, no files.

use serde_json::{json, Map, Value};

use crate::error::{Result, WorldsError};

pub const MODELS: &[&str] = &["marble-1.1", "marble-1.1-plus"];
pub const DEFAULT_MODEL: &str = "marble-1.1";
/// Credits for one `marble-1.1` world. `marble-1.1-plus` adds 300 per dynamic
/// cube, unknown at submit time, so its estimate is a floor.
pub const BASE_CREDITS: u64 = 1500;
const MAX_PROMPT_CHARS: usize = 4000;
const MAX_NAME_CHARS: usize = 200;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Text,
    Image,
    Video,
}

impl Kind {
    pub fn parse(s: &str) -> Option<Kind> {
        match s {
            "text" => Some(Kind::Text),
            "image" => Some(Kind::Image),
            "video" => Some(Kind::Video),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Text => "text",
            Kind::Image => "image",
            Kind::Video => "video",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    None,
    /// A public https URL.
    Uri(String),
    /// An id from `media-assets:prepare_upload`.
    MediaAsset(String),
}

#[derive(Clone, Debug)]
pub struct GenerateRequest {
    pub kind: Kind,
    pub prompt: Option<String>,
    pub source: Source,
    pub name: String,
    pub model: String,
    pub is_pano: bool,
    pub disable_recaption: bool,
    pub seed: Option<u64>,
    pub tags: Vec<String>,
}

/// Credits a request will cost at minimum.
pub fn estimate_credits(model: &str) -> Result<u64> {
    if MODELS.contains(&model) {
        Ok(BASE_CREDITS)
    } else {
        Err(WorldsError::Invalid(format!(
            "unknown model '{model}' (known: {})",
            MODELS.join(", ")
        )))
    }
}

impl GenerateRequest {
    pub fn validate(&self) -> Result<()> {
        estimate_credits(&self.model)?;
        if self.name.chars().count() > MAX_NAME_CHARS {
            return Err(WorldsError::Invalid(format!(
                "name exceeds {MAX_NAME_CHARS} characters"
            )));
        }
        if let Some(p) = &self.prompt {
            if p.chars().count() > MAX_PROMPT_CHARS {
                return Err(WorldsError::Invalid(format!(
                    "prompt exceeds {MAX_PROMPT_CHARS} characters"
                )));
            }
        }
        match (self.kind, &self.source) {
            (Kind::Text, Source::None) => {
                if self.prompt.as_deref().is_none_or(|p| p.trim().is_empty()) {
                    return Err(WorldsError::Invalid(
                        "text generation needs a prompt".into(),
                    ));
                }
            }
            (Kind::Text, _) => {
                return Err(WorldsError::Invalid(
                    "text generation takes no source".into(),
                ));
            }
            (_, Source::None) => {
                return Err(WorldsError::Invalid(format!(
                    "{} generation needs a url or an uploaded file",
                    self.kind.as_str()
                )));
            }
            (_, Source::Uri(u)) => {
                if !u.starts_with("https://") {
                    return Err(WorldsError::Invalid(
                        "source url must start with https://".into(),
                    ));
                }
            }
            (_, Source::MediaAsset(id)) => {
                if id.is_empty() {
                    return Err(WorldsError::Invalid("empty media asset id".into()));
                }
            }
        }
        if self.is_pano && self.kind != Kind::Image {
            return Err(WorldsError::Invalid(
                "is_pano only applies to images".into(),
            ));
        }
        Ok(())
    }

    /// The JSON body for `POST /worlds:generate`.
    pub fn body(&self) -> Result<Value> {
        self.validate()?;
        let media = |key: &str, extra_pano: bool| -> Value {
            let mut m = Map::new();
            match &self.source {
                Source::Uri(u) => {
                    m.insert("source".into(), json!("uri"));
                    m.insert("uri".into(), json!(u));
                }
                Source::MediaAsset(id) => {
                    m.insert("source".into(), json!("media_asset"));
                    m.insert("media_asset_id".into(), json!(id));
                }
                Source::None => {}
            }
            if extra_pano && self.is_pano {
                m.insert("is_pano".into(), json!(true));
            }
            let _ = key;
            Value::Object(m)
        };
        let mut wp = Map::new();
        wp.insert("type".into(), json!(self.kind.as_str()));
        match self.kind {
            Kind::Text => {
                wp.insert(
                    "text_prompt".into(),
                    json!(self.prompt.clone().unwrap_or_default()),
                );
            }
            Kind::Image => {
                wp.insert("image_prompt".into(), media("image_prompt", true));
            }
            Kind::Video => {
                wp.insert("video_prompt".into(), media("video_prompt", false));
            }
        }
        if self.kind != Kind::Text {
            if let Some(p) = self.prompt.as_ref().filter(|p| !p.trim().is_empty()) {
                wp.insert("text_prompt".into(), json!(p));
            }
            if self.disable_recaption {
                wp.insert("disable_recaption".into(), json!(true));
            }
        }
        let mut body = Map::new();
        body.insert("display_name".into(), json!(self.name));
        body.insert("model".into(), json!(self.model));
        body.insert("world_prompt".into(), Value::Object(wp));
        if let Some(seed) = self.seed {
            body.insert("seed".into(), json!(seed));
        }
        if !self.tags.is_empty() {
            body.insert("tags".into(), json!(self.tags));
        }
        Ok(Value::Object(body))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn text(prompt: &str) -> GenerateRequest {
        GenerateRequest {
            kind: Kind::Text,
            prompt: Some(prompt.into()),
            source: Source::None,
            name: "w".into(),
            model: DEFAULT_MODEL.into(),
            is_pano: false,
            disable_recaption: false,
            seed: None,
            tags: vec![],
        }
    }

    #[test]
    fn text_body_matches_the_python_bridge_shape() {
        let b = text("a mossy cave").body().unwrap();
        assert_eq!(
            b,
            json!({
                "display_name": "w",
                "model": "marble-1.1",
                "world_prompt": {"type": "text", "text_prompt": "a mossy cave"}
            })
        );
    }

    #[test]
    fn image_and_video_bodies() {
        let mut r = text("");
        r.kind = Kind::Image;
        r.prompt = Some("warm light".into());
        r.source = Source::Uri("https://x.test/a.jpg".into());
        r.is_pano = true;
        r.disable_recaption = true;
        r.seed = Some(7);
        r.tags = vec!["t".into()];
        let b = r.body().unwrap();
        assert_eq!(b["world_prompt"]["image_prompt"]["source"], "uri");
        assert_eq!(b["world_prompt"]["image_prompt"]["is_pano"], true);
        assert_eq!(b["world_prompt"]["text_prompt"], "warm light");
        assert_eq!(b["world_prompt"]["disable_recaption"], true);
        assert_eq!(b["seed"], 7);
        assert_eq!(b["tags"], json!(["t"]));
        let mut v = text("");
        v.kind = Kind::Video;
        v.prompt = None;
        v.source = Source::MediaAsset("abc".into());
        let b = v.body().unwrap();
        assert_eq!(b["world_prompt"]["video_prompt"]["media_asset_id"], "abc");
        assert!(b["world_prompt"].get("text_prompt").is_none());
    }

    #[test]
    fn validation_rejects_bad_requests() {
        assert!(text("").body().is_err());
        assert!(text("   ").body().is_err());
        let mut r = text("x");
        r.model = "marble-9".into();
        assert!(r.body().is_err());
        let mut r = text("x");
        r.source = Source::Uri("https://a".into());
        assert!(r.body().is_err());
        let mut r = text("x");
        r.kind = Kind::Image;
        r.source = Source::Uri("http://insecure.test/a.jpg".into());
        assert!(r.body().is_err());
        let mut r = text("x");
        r.kind = Kind::Video;
        r.source = Source::Uri("https://a.test/v.mp4".into());
        r.is_pano = true;
        assert!(r.body().is_err());
        assert!(text(&"x".repeat(MAX_PROMPT_CHARS + 1)).body().is_err());
    }

    #[test]
    fn estimates() {
        assert_eq!(estimate_credits("marble-1.1").unwrap(), 1500);
        assert_eq!(estimate_credits("marble-1.1-plus").unwrap(), 1500);
        assert!(estimate_credits("nope").is_err());
    }
}
