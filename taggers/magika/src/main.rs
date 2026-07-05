use std::path::Path;

use anyhow::Result;
use magika::Session;

use tagd_core::tagger::{Tagger, TaggerInfo};

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
            keys: vec![
                "mime".to_string(),
                "label".to_string(),
                "magika-score".to_string(),
            ],
        }
    }

    fn new() -> Result<Self> {
        let session = Session::new()?;
        Ok(Magika { session })
    }

    fn tag(&mut self, path: &Path) -> Result<Vec<(String, String)>> {
        let file_type = self.session.identify_file_sync(path)?;
        let info = file_type.info();
        Ok(vec![
            ("mime".to_string(), info.mime_type.to_string()),
            ("label".to_string(), info.label.to_string()),
            ("magika-score".to_string(), file_type.score().to_string()),
        ])
    }
}

fn main() {
    tagd_core::tagger::run::<Magika>()
}
