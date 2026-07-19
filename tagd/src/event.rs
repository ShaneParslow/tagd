mod fanotify;

use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

use anyhow::{Context, Result};

/// Create threads for all event providers.
pub fn spawn_event_providers() -> Result<mpsc::Receiver<PathBuf>> {
    let (tx, rx) = mpsc::channel();

    let fa = fanotify::fan_provider_init().context("Failed to start fanotify provider")?;
    thread::spawn(move || fanotify::fan_provider_exec(fa, tx));

    Ok(rx)
}
