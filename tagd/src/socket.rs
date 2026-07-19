use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::thread;

use anyhow::{Context, Result};

use tagd_core::query::{socket_path, Request, FilesResponse, FileMatch};

use crate::db::Db;

/// Creates query socket and socket listener thread.
pub fn spawn_socket_listener() -> Result<()> {
    let path = socket_path();
    if path.exists() {
        std::fs::remove_file(&path)
            .with_context(|| format!("Failed to remove stale socket file at {:?}", path))?;
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create socket directory: {:?}", parent))?;
    }
    let listener = UnixListener::bind(&path)
        .with_context(|| format!("Failed to bind Unix socket: {:?}", path))?;

    thread::spawn(move || {
        // Make handler thread for each incoming stream
        // Never returns: UnixListener::incoming blocks.
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => { thread::spawn(move || handle_client(stream)); },
                Err(e) => eprintln!("WARN: Socket accept error: {}", e),
            }
        }
    });

    Ok(())
}

// TODO: persistent connection, open db before loop to avoid overhead.
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
            // TODO: write error to socket
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
                .query_files_by_qualified_tag(&tagger, &key, &value) // TODO: handle error here. make response_json block a funct and use `?`?
                .unwrap_or_default() // this is *definitely* not the right way to handle an error here...
                .into_iter()
                .map(|(path, mtime_at_tag)| FileMatch { path, mtime_at_tag })
                .collect();
            serde_json::to_string(&FilesResponse { files }).unwrap()
        }
    };

    let _ = writeln!(writer, "{response_json}");
}
