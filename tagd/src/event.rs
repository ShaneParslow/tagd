mod fanotify;

use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

/// Create threads for all event providers
pub fn spawn_event_providers(tx: mpsc::Sender<PathBuf>) {
    let fa = fanotify::fan_provider_init();
    thread::spawn(
        move || fanotify::fan_provider_exec(fa, tx)
    );
}
