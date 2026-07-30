use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

use tagd_core::tagger::TaggerResponse;

/// Runs a tagger binary against a single file and returns its parsed response.
pub fn run_tagger(exec: &Path, path: &Path) -> Result<TaggerResponse> {
    let output = Command::new(exec)
        .arg(path)
        .output()
        .with_context(|| format!("Failed to spawn tagger {exec:?}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Tagger {exec:?} exited with {}: {}", output.status, stderr.trim());
    }

    serde_json::from_slice(&output.stdout)
        .with_context(|| format!("Failed to parse response from tagger {exec:?}"))
}
