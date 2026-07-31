use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::thread;

use anyhow::{Context, Result};

use tagd_core::query::{Request, socket_path};

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
                Ok(stream) => {
                    thread::spawn(move || handle_client(stream));
                }
                Err(e) => eprintln!("WARN: Socket accept error: {}", e),
            }
        }
    });

    Ok(())
}

fn handle_client(stream: UnixStream) {
    let db = match Db::open().context("ERR: Failed to open database for socket query") {
        Ok(db) => db,
        Err(e) => {
            eprintln!("{e:#}");
            return;
        }
    };

    let mut writer = match stream
        .try_clone()
        .context("ERR: Failed to clone stream for socket query")
    {
        Ok(w) => w,
        Err(e) => {
            eprintln!("{e:#}");
            return;
        }
    };
    let mut reader = BufReader::new(stream);

    let mut line = String::new();
    loop {
        match reader
            .read_line(&mut line)
            .context("WARN: Failed to read from socket")
        {
            Ok(s) => {
                if s == 0 {
                    eprintln!("INFO: Socket EOF");
                    return;
                }
                if let Err(e) = handle_query(&db, &mut writer, &line) {
                    eprintln!("{e:#}");
                }
            }
            Err(e) => {
                eprintln!("{e:#}");
                return;
            }
        }
    }
}

fn handle_query(db: &Db, writer: &mut UnixStream, line: &str) -> Result<()> {
    let request: Request = serde_json::from_str(line.trim())
        .context("Failed to deserialize query")?;

    let response =
        generate_response(db, request).context("Failed to generate response for query")?;

    // TODO: Not atomic! If query modifies, not rolled back even though sending response failed.
    writeln!(writer, "{response}").context("Failed to write query response")
}

fn generate_response(db: &Db, request: Request) -> Result<String> {
    match request {
        Request::FilesByQualifiedTag { tagger, key, value } => {
            let files = db.query_files_by_qualified_tag(&tagger, &key, &value)?;
            serde_json::to_string(&files).context("Failed to serialize response")
        }
    }
}
