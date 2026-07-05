use serde::{Deserialize, Serialize};

/// Metadata a tagger reports for `--tagd-info`. Must be cheap to produce — the
/// daemon queries it during discovery, so it must not load heavy models.
#[derive(Serialize, Deserialize)]
pub struct TaggerInfo {
    pub name: String,
    pub version: String,
    pub keys: Vec<String>,
}

/// The result of tagging one file, emitted as one JSON line.
#[derive(Serialize, Deserialize)]
pub struct TaggerResponse {
    pub tagger: String,
    pub tags: Vec<(String, String)>,
    pub mtime_at_tag: i64,
}
