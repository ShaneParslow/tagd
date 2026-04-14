use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::os::fd::{AsRawFd, BorrowedFd};
use std::sync::mpsc;

use anyhow::Result;
use nix::fcntl::AT_FDCWD;
use nix::sys::fanotify::{Fanotify, InitFlags, EventFFlags, MarkFlags, MaskFlags};

use crate::event::Event;

/// Initialize nix Fanotify group and apply mark for entire filesystem
pub fn fan_provider_init() -> Fanotify {
    let flags = InitFlags::FAN_UNLIMITED_QUEUE | InitFlags::FAN_CLOEXEC;
    let event_f_flags = EventFFlags::O_CLOEXEC;

    let fa = Fanotify::init(flags, event_f_flags).expect("Failed to init fanotify group");

    let mark_flags = MarkFlags::FAN_MARK_ADD | MarkFlags::FAN_MARK_FILESYSTEM;
    let mask_flags = MaskFlags::FAN_CLOSE_WRITE;

    fa.mark(mark_flags, mask_flags, AT_FDCWD, Some(Path::new("."))).expect("Failed to apply fanotify mark");
    
    fa
}

/// Loop for fanotify event provider thread
pub fn fan_provider_exec(fa: Fanotify, tx: mpsc::Sender<Event>) {
    loop {
        let events = fa.read_events().expect("Failed to read fanotify events");

        for event in events {
            let Some(fd) = event.fd() else {
                eprintln!("WARN: fanotify queue overflow - events have been dropped");
                continue;
            };

            let Some(path) = get_path(fd) else { continue };
            let Ok(mtime) = get_mtime(&path) else { continue };
            tx.send(Event { path, mtime }).expect("Failed to add event to queue");
        }
    }
}

/// Get path from fd via proc
fn get_path(fd: BorrowedFd) -> Option<PathBuf> {
    let path = std::fs::read_link(format!("/proc/self/fd/{}", fd.as_raw_fd())).expect("Failed to read fd path");

    // HACK: A path ending with " (deleted)" is actually valid, but this is how proc conveys
    // that a backing file was deleted, and it will only result in the file not being indexed.
    match path.to_str().expect("Failed to convert path to str").ends_with(" (deleted)") {
        true => None,
        false => Some(path),
    }
}

fn get_mtime(path: &Path) -> Result<i64> {
    Ok(fs::metadata(path)?.mtime())
}
