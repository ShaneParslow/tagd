use std::path::Path;

use anyhow::{Result, anyhow, bail};
use magic::{
    Cookie,
    cookie::{Flags, Load},
};

use tagd_core::tagger::{Tagger, TaggerInfo};

struct StdMime {
    cookie: Cookie<Load>,
}

impl Tagger for StdMime {
    fn info() -> TaggerInfo {
        TaggerInfo {
            name: "std-mime".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            keys: vec!["mime".to_string()],
        }
    }

    fn new() -> Result<Self> {
        // libmagic's error types aren't Send/Sync, so flatten them to strings.
        let cookie = Cookie::open(Flags::MIME_TYPE)
            .map_err(|e| anyhow!("failed to initialize libmagic: {e}"))?
            .load(&Default::default())
            .map_err(|e| anyhow!("failed to load magic database: {e}"))?;
        Ok(StdMime { cookie })
    }

    fn tag(&mut self, path: &Path) -> Result<Vec<(String, String)>> {
        let mime = self
            .cookie
            .file(path)
            .map_err(|e| anyhow!("failed to determine MIME type: {e}"))?;

        // HACK: .file will output "cannot open `path` (No such file or directory)" without returning an error
        if mime.starts_with("cannot open") {
            bail!("File does not exist");
        }

        Ok(vec![("mime".to_string(), mime)])
    }
}

fn main() {
    tagd_core::tagger::run::<StdMime>()
}
