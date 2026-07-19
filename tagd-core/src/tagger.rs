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

#[cfg(feature = "runtime")]
mod runtime {
    use std::env;
    use std::os::unix::fs::MetadataExt;
    use std::path::Path;
    use std::process;

    use anyhow::{Result, bail};

    use super::{TaggerInfo, TaggerResponse};

    /// A tagger's unique logic. The [`run`] driver handles everything else:
    /// argument parsing, `--tagd-info`, the mtime consistency check, response
    /// framing, and serialization.
    ///
    /// `info` is static and cheap; `new` builds heavy, reusable state (a
    /// libmagic cookie, an ONNX session) exactly once; `tag` runs per file.
    /// This split is what lets the driver evolve from the current one-shot
    /// protocol to a long-running "stream paths on stdin" protocol without any
    /// tagger changing.
    pub trait Tagger {
        fn info() -> TaggerInfo
        where
            Self: Sized;

        fn new() -> Result<Self>
        where
            Self: Sized;

        fn tag(&mut self, path: &Path) -> Result<Vec<(String, String)>>;
    }

    /// Drives a tagger as a binary. Call from `main`:
    ///
    /// ```ignore
    /// fn main() { tagd_core::run::<MyTagger>() }
    /// ```
    ///
    /// Handles argument parsing, `--tagd-info`, mtime consistency, and serialization.
    pub fn run<T: Tagger>() -> ! {
        let args: Vec<String> = env::args().collect();

        if args.len() == 2 && args[1] == "--tagd-info" {
            let info = T::info();
            println!(
                "{}",
                serde_json::to_string(&info).expect("serialize TaggerInfo")
            );
            process::exit(0);
        }

        if args.len() != 2 {
            let prog = args.first().map(String::as_str).unwrap_or("tagger");
            eprintln!("Usage: {prog} <file_path>");
            process::exit(1);
        }

        let mut tagger = match T::new() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("Failed to initialize tagger: {e}");
                process::exit(1);
            }
        };

        // TODO: split logic here for oneshot vs background tagger
        // loop getting path from stdin and running tag_file on it

        let path = Path::new(&args[1]);
        match tag_file(&mut tagger, &T::info().name, path) {
            Ok(response) => {
                println!(
                    "{}",
                    serde_json::to_string(&response).expect("serialize TaggerResponse")
                );
                process::exit(0);
            }
            Err(e) => {
                eprintln!("{e}");
                process::exit(1);
            }
        }
    }

    /// Tag one file with before/after mtime consistency checking.
    fn tag_file<T: Tagger>(tagger: &mut T, name: &str, path: &Path) -> Result<TaggerResponse> {
        let mtime_before = std::fs::metadata(path)?.mtime();
        let tags = tagger.tag(path)?; // Jump to tagger logic
        let mtime_after = std::fs::metadata(path)?.mtime();
        if mtime_before != mtime_after {
            bail!("File was modified during tagging");
        }
        Ok(TaggerResponse {
            tagger: name.to_string(),
            tags,
            mtime_at_tag: mtime_before,
        })
    }
}

#[cfg(feature = "runtime")]
pub use runtime::{Tagger, run};
