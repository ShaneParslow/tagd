use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

use tagd_core::tagger::{TagRequest, TaggerResponse};

/// Runs a tagger binary against a single request and returns its parsed
/// response. The request is written as JSON to the tagger's stdin.
pub fn run_tagger(exec: &Path, request: &TagRequest) -> Result<TaggerResponse> {
    let mut child = Command::new(exec)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to spawn tagger {exec:?}"))?;

    let request_json = serde_json::to_vec(request).context("Failed to serialize tag request")?;
    child
        .stdin
        .take()
        .context("Tagger stdin was not captured")?
        .write_all(&request_json)
        .with_context(|| format!("Failed to write request to tagger {exec:?}"))?;
    // The taken stdin is dropped here, closing the pipe so the tagger reads EOF.

    let output = child
        .wait_with_output()
        .with_context(|| format!("Failed to wait on tagger {exec:?}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "Tagger {exec:?} exited with {}: {}",
            output.status,
            stderr.trim()
        );
    }

    serde_json::from_slice(&output.stdout)
        .with_context(|| format!("Failed to parse response from tagger {exec:?}"))
}
