mod fanotify;

use std::path::{PathBuf};
use std::sync::mpsc;
use std::thread;

use anyhow::Result;

/// Create threads for all event providers.
pub fn spawn_event_providers() -> Result<mpsc::Receiver<PathBuf>> {
    let (tx, rx) = mpsc::channel();
    
    let fa = fanotify::fan_provider_init()?;
    thread::spawn(
        move || fanotify::fan_provider_exec(fa, tx)
    );

    Ok(rx)
}
