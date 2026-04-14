use serde::{Deserialize, Serialize};

#[derive(Serialize)]
#[derive(Deserialize)]
pub struct Response {
    pub tagger: String,
    pub tags: Vec<(String, String)>,
}
