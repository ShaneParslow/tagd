use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::thread;

use serde::{Deserialize, Serialize};

use crate::db::Db;

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum Request {
    FilesByQualifiedTag {
        tagger: String,
        key: String,
        value: String,
    },
}

#[derive(Serialize)]
struct FileMatch {
    path: String,
    mtime_at_tag: i64,
}

#[derive(Serialize)]
struct FilesResponse {
    files: Vec<FileMatch>,
}

pub fn spawn_socket_listener() {
    // Check for old socket file, wait for incoming streams and make handler threads for each.
    thread::spawn(move || {
        let path = socket_path();

        if path.exists() {
            std::fs::remove_file(&path).expect("Failed to remove stale socket file");
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("Failed to create socket directory");
        }

        let listener = UnixListener::bind(&path).expect("Failed to bind Unix socket");

        // Make handler thread for each incoming stream
        // Never returns since UnixListener::incoming blocks.
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    thread::spawn(move || handle_client(stream));
                }
                Err(e) => eprintln!("Socket accept error: {e}"),
            }
        }
    });
}

fn handle_client(stream: std::os::unix::net::UnixStream) {
    let mut writer = match stream.try_clone() {
        Ok(w) => w,
        Err(_) => return,
    };
    let mut reader = BufReader::new(stream);

    let mut line = String::new();
    if reader.read_line(&mut line).is_err() || line.is_empty() {
        return;
    }

    let request = match serde_json::from_str::<Request>(line.trim()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Bad socket request: {e}");
            return;
        }
    };

    let response_json = match request {
        Request::FilesByQualifiedTag { tagger, key, value } => {
            let db = match Db::open() {
                Ok(db) => db,
                Err(e) => {
                    eprintln!("Failed to open database for query: {e}");
                    return;
                }
            };
            let files = db
                .query_files_by_qualified_tag(&tagger, &key, &value)
                .unwrap_or_default()
                .into_iter()
                .map(|(path, mtime_at_tag)| FileMatch { path, mtime_at_tag })
                .collect();
            serde_json::to_string(&FilesResponse { files }).unwrap()
        }
    };

    let _ = writeln!(writer, "{response_json}");
}

fn socket_path() -> PathBuf {
    if let Ok(path) = std::env::var("TAGD_SOCKET_PATH") {
        return PathBuf::from(path);
    }

    #[cfg(debug_assertions)]
    {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent().unwrap()
            .join("target/debug/tagd.sock")
    }

    #[cfg(not(debug_assertions))]
    {
        PathBuf::from("/run/tagd/tagd.sock")
    }
}