use anyhow::{Result, bail};
use std::path::PathBuf;

use tagd_core::Response;

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

pub fn run_tagger(exec: &PathBuf, query: Query) -> Result<Response> {
    let output = std::process::Command::new(exec)
        .arg(query.path)
        .output()?;
    if !output.status.success() { bail!("Tagger did not return success") };
    let out = String::from_utf8(output.stdout)?;
    println!("{out}");
    Ok(serde_json::from_str(&out)?)
}
