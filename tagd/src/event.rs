mod fanotify;

use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

/// Filesystem event that gets added to retag queue
#[derive(Debug)]
pub struct Event {
    pub path: PathBuf,
    pub mtime: i64,
}

/// Create threads for all event providers
pub fn spawn_event_providers(tx: mpsc::Sender<Event>) {
    let fa = fanotify::fan_provider_init();
    thread::spawn(
        move || fanotify::fan_provider_exec(fa, tx)
    );
}
