use std::env;
use std::os::unix::fs::MetadataExt;
use std::process;

use magika::Session;
use tagd_core::{TaggerInfo, TaggerResponse};

// One-shot tagger: load the Magika model, identify a single file, print JSON.
//
// FUTURE (long-running mode): loading and dropping the ONNX model on every
// invocation dominates the cost here. The `Session` is designed to be reused
// across many identifications (and is thread-safe), so the intended evolution
// is to build the `Session` once and then loop reading one file path per line
// from stdin, emitting one JSON `TaggerResponse` per line. That requires a
// matching change in the daemon's tagger protocol (subprocess.rs) to keep the
// process alive and stream paths instead of spawning per file. Until then,
// keep the per-invocation behavior below identical to std-mime.
fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() == 2 && args[1] == "--tagd-info" {
        let info = TaggerInfo {
            name: "magika".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            keys: vec![
                "mime".to_string(),
                "label".to_string(),
                "magika-score".to_string(),
            ],
        };
        println!("{}", serde_json::to_string(&info).unwrap());
        process::exit(0);
    }

    if args.len() != 2 {
        eprintln!("Usage: {} <file_path>", args[0]);
        process::exit(1);
    }

    let file_path = &args[1];

    let mtime_before = std::fs::metadata(file_path)
        .unwrap_or_else(|err| {
            eprintln!("Failed to read file metadata: {}", err);
            process::exit(1);
        })
        .mtime();

    // FUTURE: hoist this out of the per-file path and build it once at startup.
    let mut session = Session::new().unwrap_or_else(|err| {
        eprintln!("Failed to initialize Magika session: {}", err);
        process::exit(1);
    });

    let file_type = session.identify_file_sync(file_path).unwrap_or_else(|err| {
        eprintln!("Failed to identify file: {}", err);
        process::exit(1);
    });

    let info = file_type.info();
    let mime = info.mime_type.to_string();
    let label = info.label.to_string();
    let score = file_type.score().to_string();

    let mtime_after = std::fs::metadata(file_path)
        .unwrap_or_else(|err| {
            eprintln!("Failed to read file metadata: {}", err);
            process::exit(1);
        })
        .mtime();

    if mtime_before != mtime_after {
        eprintln!("File was modified during tagging");
        process::exit(1);
    }

    let response = TaggerResponse {
        tagger: "magika".to_string(),
        tags: vec![
            ("mime".to_string(), mime),
            ("label".to_string(), label),
            ("magika-score".to_string(), score),
        ],
        mtime_at_tag: mtime_before,
    };

    let json = serde_json::to_string(&response).unwrap_or_else(|err| {
        eprintln!("Failed to serialize JSON: {}", err);
        process::exit(1);
    });

    println!("{}", json);
}
