use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct TaggerResponse {
    pub tagger: String,
    pub tags: Vec<(String, String)>,
    pub mtime_at_tag: i64,
}

#[derive(Serialize, Deserialize)]
pub struct TaggerInfo {
    pub name: String,
    pub version: String,
    pub keys: Vec<String>,
}
