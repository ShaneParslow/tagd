use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use wildmatch::WildMatch;

/// Metadata a tagger reports for `--tagd-info`: the keys it provides and the
/// keys it depends on.
#[derive(Serialize, Deserialize)]
pub struct TaggerInfo {
    pub name: String,
    pub version: String,
    /// Keys that must be produced before this tagger runs, each optionally
    /// filtered by a glob on the upstream value.
    pub dependencies: Vec<Dependency>,
    /// Keys that this tagger provides.
    pub keys: Vec<String>,
}

/// A tag key a tagger depends on. The key must be produced by an earlier
/// tagger, and if `matches` is set the upstream value must match that glob for
/// this tagger to run — e.g. `Dependency::matching("mime", "image/*")` on an
/// image classifier so it only runs on images.
#[derive(Serialize, Deserialize)]
pub struct Dependency {
    pub key: String,
    /// Glob (`*`, `?`) the upstream value must match. `None` accepts any value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matches: Option<String>,
}

impl Dependency {
    /// Depend on a key with no value filter.
    pub fn on(key: impl Into<String>) -> Dependency {
        Dependency {
            key: key.into(),
            matches: None,
        }
    }

    /// Depend on a key whose value must match `pattern` (glob: `*`, `?`).
    pub fn matching(key: impl Into<String>, pattern: impl Into<String>) -> Dependency {
        Dependency {
            key: key.into(),
            matches: Some(pattern.into()),
        }
    }

    /// Whether an upstream `value` satisfies this dependency's filter. A
    /// dependency with no filter accepts any value.
    pub fn accepts(&self, value: &str) -> bool {
        match &self.matches {
            Some(pattern) => WildMatch::new(pattern).matches(value),
            None => true,
        }
    }
}

/// The unit of work sent to a tagger: the file to tag and the resolved values
/// of the keys it declared as dependencies. This is the message the daemon
/// writes to a tagger's stdin, and the same shape a long-running tagger will
/// read one-per-line.
#[derive(Serialize, Deserialize)]
pub struct TagRequest {
    pub path: PathBuf,
    /// Resolved values of the tagger's declared dependency keys, one entry per
    /// key. Empty when the tagger has no dependencies. Same `(key, value)`
    /// shape as [`TaggerResponse::tags`].
    #[serde(default)]
    pub deps: Vec<(String, String)>,
}

impl TagRequest {
    /// The resolved value of a dependency key, if present.
    pub fn dep(&self, key: &str) -> Option<&str> {
        self.deps
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

/// The result of tagging one file, emitted as one JSON line.
#[derive(Serialize, Deserialize)]
pub struct TaggerResponse {
    pub tagger: String, // can remove now that taggerinfo has?
    pub tags: Vec<(String, String)>,
    pub mtime_at_tag: i64,
}

#[cfg(feature = "runtime")]
mod runtime {
    use std::env;
    use std::io::Read;
    use std::os::unix::fs::MetadataExt;
    use std::path::PathBuf;
    use std::process;

    use anyhow::{Result, bail};

    use super::{TagRequest, TaggerInfo, TaggerResponse};

    /// A tagger's unique logic. The [`run`] driver handles everything else:
    /// argument parsing, `--tagd-info`, request decoding, the mtime consistency
    /// check, response framing, and serialization.
    ///
    /// `info` is static and cheap; `new` builds heavy, reusable state (a
    /// libmagic cookie, an ONNX session) exactly once; `tag` runs per file. It
    /// receives the [`TagRequest`], so a tagger can read `req.path` and the
    /// resolved values of its declared dependencies via `req.deps`.
    ///
    /// This split is what lets the driver evolve from the current one-shot
    /// protocol to a long-running "stream requests on stdin" protocol without
    /// any tagger changing.
    pub trait Tagger {
        fn info() -> TaggerInfo
        where
            Self: Sized;

        fn new() -> Result<Self>
        where
            Self: Sized;

        fn tag(&mut self, req: &TagRequest) -> Result<Vec<(String, String)>>;
    }

    /// Drives a tagger as a binary. Call from `main`:
    ///
    /// ```ignore
    /// fn main() { tagd_core::tagger::run::<MyTagger>() }
    /// ```
    ///
    /// Input is either a `TagRequest` JSON object on stdin (how the daemon
    /// invokes it) or a bare path argument (`tagger <path>`) for manual runs,
    /// which is treated as a request with no dependency values.
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

        let request = match args.len() {
            // Manual one-shot: a path with no dependency values.
            2 => TagRequest {
                path: PathBuf::from(&args[1]),
                deps: Vec::new(),
            },
            // Daemon protocol: a TagRequest JSON object on stdin.
            1 => match read_request() {
                Ok(req) => req,
                Err(e) => {
                    eprintln!("Failed to read tag request: {e:#}");
                    process::exit(1);
                }
            },
            _ => {
                let prog = args.first().map(String::as_str).unwrap_or("tagger");
                eprintln!("Usage: {prog} [<file_path>]  (or a TagRequest JSON on stdin)");
                process::exit(1);
            }
        };

        let mut tagger = match T::new() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("Failed to initialize tagger: {e:#}");
                process::exit(1);
            }
        };

        // TODO: long-running mode — loop reading one TagRequest per line from
        // stdin and emitting one response per line, instead of handling exactly
        // one request. See the note on the `Tagger` trait.
        match tag_file(&mut tagger, &T::info().name, &request) {
            Ok(response) => {
                println!(
                    "{}",
                    serde_json::to_string(&response).expect("serialize TaggerResponse")
                );
                process::exit(0);
            }
            Err(e) => {
                eprintln!("{e:#}");
                process::exit(1);
            }
        }
    }

    /// Reads a single `TagRequest` JSON object from stdin.
    fn read_request() -> Result<TagRequest> {
        let mut input = String::new();
        std::io::stdin().read_to_string(&mut input)?;
        Ok(serde_json::from_str(&input)?)
    }

    /// Tag one file with before/after mtime consistency checking.
    fn tag_file<T: Tagger>(tagger: &mut T, name: &str, req: &TagRequest) -> Result<TaggerResponse> {
        let mtime_before = std::fs::metadata(&req.path)?.mtime();
        let tags = tagger.tag(req)?; // Jump to tagger logic
        let mtime_after = std::fs::metadata(&req.path)?.mtime();
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
