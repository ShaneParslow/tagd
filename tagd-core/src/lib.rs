use serde::Serialize;

#[derive(Serialize)]
pub struct Response {
    pub tagger: String,
    pub tags: Vec<(String, String)>,
}
