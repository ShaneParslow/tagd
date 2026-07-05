//! The socket query protocol: newline-delimited JSON over the daemon's Unix
//! socket. These Rust types are the canonical binding, but the wire format is
//! language-agnostic — any client that speaks JSONL can talk to the daemon.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A request from a client to the daemon. Serialized as a single JSON line with
/// an `"action"` discriminator, e.g.
/// `{"action":"files_by_qualified_tag","tagger":"std-mime","key":"mime","value":"text/plain"}`.
#[derive(Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Request {
    FilesByQualifiedTag {
        tagger: String,
        key: String,
        value: String,
    },
}

/// A response from the daemon containing one or more files alongside their mtime_at_tag.
#[derive(Serialize, Deserialize)]
pub struct FilesResponse {
    pub files: Vec<FileMatch>,
}

#[derive(Serialize, Deserialize)]
pub struct FileMatch {
    pub path: String,
    pub mtime_at_tag: i64,
}

/// Path to the daemon's Unix socket. Lives here so the daemon and any clients
/// (tagctl) resolve it identically. Override with `TAGD_SOCKET_PATH`.
pub fn socket_path() -> PathBuf {
    if let Ok(path) = std::env::var("TAGD_SOCKET_PATH") {
        return PathBuf::from(path);
    }

    #[cfg(debug_assertions)]
    {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap() // workspace root
            .join("target/debug/tagd.sock")
    }

    #[cfg(not(debug_assertions))]
    {
        PathBuf::from("/run/tagd/tagd.sock")
    }
}
