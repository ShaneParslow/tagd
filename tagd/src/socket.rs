use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::thread;

use tagd_core::query::{socket_path, Request, FilesResponse, FileMatch};

use crate::db::Db;

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
