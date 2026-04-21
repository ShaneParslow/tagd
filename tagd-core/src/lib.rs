use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Response {
    pub tagger: String,
    pub tags: Vec<(String, String)>,
}
