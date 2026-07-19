use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::process;

use clap::{Parser, Subcommand};

use tagd_core::query::{Request, socket_path};

/// A small client for the tagd daemon's Unix socket. Each subcommand maps to a
/// single JSONL request/response round-trip.
#[derive(Parser)]
#[command(name = "tagctl", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List files carrying a given tagger/key/value tag.
    Files {
        tagger: String,
        key: String,
        value: String,
    },
}

fn main() {
    let cli = Cli::parse();

    let request = match cli.command {
        Command::Files { tagger, key, value } => {
            Request::FilesByQualifiedTag { tagger, key, value }
        }
    };

    let json = serde_json::to_string(&request).expect("serialize request");
    let response = round_trip(&json);
    print!("{response}");
}

/// Connect, send one request line, read one response line.
fn round_trip(request_json: &str) -> String {
    let path = socket_path();
    let stream = UnixStream::connect(&path).unwrap_or_else(|e| {
        eprintln!(
            "Failed to connect to tagd socket at {}: {e}",
            path.display()
        );
        process::exit(1);
    });

    let mut writer = stream.try_clone().expect("clone socket stream");
    writeln!(writer, "{request_json}").unwrap_or_else(|e| {
        eprintln!("Failed to send request: {e}");
        process::exit(1);
    });

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap_or_else(|e| {
        eprintln!("Failed to read response: {e}");
        process::exit(1);
    });
    line
}
