use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct TaggerResponse {
    pub tagger: String,
    pub tags: Vec<(String, String)>,
}
