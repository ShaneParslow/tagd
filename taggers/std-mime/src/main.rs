use magic::{Cookie, cookie::Flags};
use serde::Serialize;
use std::env;
use std::process;

#[derive(Serialize)]
struct Tags {
    mime: String,
}

#[derive(Serialize)]
struct Output {
    tags: Tags,
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() == 2 && args[1] == "--tagd-info" {
        process::exit(0);
    }

    if args.len() != 2 {
        eprintln!("Usage: {} <file_path>", args[0]);
        process::exit(1);
    }

    let file_path = &args[1];

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

    let output = Output {
        tags: Tags {
            mime,
        },
    };

    let json = serde_json::to_string(&output).unwrap_or_else(|err| {
        eprintln!("Failed to serialize JSON: {}", err);
        process::exit(1);
    });

    println!("{}", json);
}
