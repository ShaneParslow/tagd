use magic::{Cookie, cookie::Flags};
use std::env;
use std::os::unix::fs::MetadataExt;
use std::process;

use tagd_core::{TaggerInfo, TaggerResponse};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() == 2 && args[1] == "--tagd-info" {
        let info = TaggerInfo {
            name: "std-mime".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            keys: vec!["mime".to_string()],
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

    let cookie = Cookie::open(Flags::MIME_TYPE).unwrap_or_else(|err| {
        eprintln!("Failed to initialize libmagic: {}", err);
        process::exit(1);
    });

    let database = Default::default();
    let loaded_cookie = cookie.load(&database).unwrap_or_else(|err| {
        eprintln!("Failed to load magic database: {}", err);
        process::exit(1);
    });

    let mime_type = loaded_cookie.file(file_path).unwrap_or_else(|err| {
        eprintln!("Failed to determine MIME type: {}", err);
        process::exit(1);
    });

    let mime = mime_type.to_string();

    // HACK: .file will output "cannot open `path` (No such file or directory)" without returning an error
    if mime.starts_with("cannot open") {
        eprintln!("File does not exist");
        process::exit(1);
    }

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
        tagger: "std-mime".to_string(),
        tags: vec![("mime".to_string(), mime)],
        mtime_at_tag: mtime_before,
    };

    let json = serde_json::to_string(&response).unwrap_or_else(|err| {
        eprintln!("Failed to serialize JSON: {}", err);
        process::exit(1);
    });

    println!("{}", json);
}
