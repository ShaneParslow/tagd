use std::path::Path;

use anyhow::Result;
use magika::Session;

use tagd_core::tagger::{TagRequest, Tagger, TaggerInfo};

// The heavy ONNX model is loaded once in `new()` and reused across every `tag`
// call. The `run()` driver currently invokes `tag` once per process, but the
// `Session` is built to be reused across many identifications — so when the
// daemon's protocol grows a long-running "stream paths on stdin" mode, this
// tagger already amortizes the model load correctly, with no change here.
struct Magika {
    session: Session,
}

impl Tagger for Magika {
    fn info() -> TaggerInfo {
        TaggerInfo {
            name: "magika".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            dependencies: vec![],
            keys: vec![
                "mime".to_string(),
                "label".to_string(),
                "group".to_string(),
                "is-text".to_string(),
                "ext-match".to_string(),
                "magika-score".to_string(),
            ],
        }
    }

    fn new() -> Result<Self> {
        let session = Session::new()?;
        Ok(Magika { session })
    }

    fn tag(&mut self, req: &TagRequest) -> Result<Vec<(String, String)>> {
        let file_type = self.session.identify_file_sync(&req.path)?;
        let info = file_type.info();

        Ok(vec![
            ("mime".to_string(), info.mime_type.to_string()),
            ("label".to_string(), info.label.to_string()),
            ("group".to_string(), info.group.to_string()),
            ("is-text".to_string(), info.is_text.to_string()),
            ("ext-match".to_string(), ext_match(&req.path, info.extensions)),
            ("magika-score".to_string(), file_type.score().to_string()),
        ])
    }
}

/// Whether the file's extension is one magika expects for the content it
/// actually found: `yes`, `no`, or `none` when the file has no extension or
/// magika lists none for the type.
fn ext_match(path: &Path, expected: &[&str]) -> String {
    let Some(ext) = path.extension() else {
        return "none".to_string();
    };
    if expected.is_empty() {
        return "none".to_string();
    }
    let ext = ext.to_string_lossy().to_lowercase();
    expected.iter().any(|e| e.eq_ignore_ascii_case(&ext)).to_string()
}

fn main() {
    tagd_core::tagger::run::<Magika>()
}
