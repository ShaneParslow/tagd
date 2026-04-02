use std::path::PathBuf;

// Query that gets sent to tagger subprocess
pub struct Query {
    path: PathBuf,
}

impl Query {
    pub fn init(path: PathBuf) -> Query {
        Query {
            path,
        }
    }
}

pub fn run_tagger(exec: &PathBuf, query: Query) -> std::io::Result<std::process::Output> {
    std::process::Command::new(exec)
        .arg(query.path)
        .output()
}
