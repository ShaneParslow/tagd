use std::os::fd::{AsRawFd, BorrowedFd};
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use anyhow::{Context, Result};
use nix::fcntl::AT_FDCWD;
use nix::sys::fanotify::{EventFFlags, Fanotify, InitFlags, MarkFlags, MaskFlags};

/// Initialize nix Fanotify group and apply mark for entire filesystem
pub fn fan_provider_init() -> Result<Fanotify> {
    let flags = InitFlags::FAN_UNLIMITED_QUEUE | InitFlags::FAN_CLOEXEC;
    let event_f_flags = EventFFlags::O_CLOEXEC;

    let fa = Fanotify::init(flags, event_f_flags).context("Failed to init fanotify group")?;

    let mark_flags = MarkFlags::FAN_MARK_ADD | MarkFlags::FAN_MARK_FILESYSTEM;
    let mask_flags = MaskFlags::FAN_CLOSE_WRITE;

    fa.mark(mark_flags, mask_flags, AT_FDCWD, Some(Path::new(".")))
        .context("Failed to apply fanotify mark")?;

    Ok(fa)
}

/// Loop for fanotify event provider thread
pub fn fan_provider_exec(fa: Fanotify, tx: mpsc::Sender<PathBuf>) {
    let mut retry = 0;
    loop {
        let events = match fa.read_events() {
            Ok(events) => events,
            Err(e) => {
                retry += 1;
                eprintln!("WARN: Fanotify read_events() error: {e}");
                if retry > 10 {
                    panic!("Fanotify read_events() keeps erroring!")
                }
                continue;
            }
        };

        retry = 0;

        for event in events {
            let Some(fd) = event.fd() else {
                eprintln!("WARN: fanotify queue overflow - events have been dropped");
                continue;
            };

            let Some(path) = get_path(fd) else { continue };
            tx.send(path).expect("Fanotify -> queue channel died!");
        }
    }
}

/// Get path from fd via proc
fn get_path(fd: BorrowedFd) -> Option<PathBuf> {
    let path = std::fs::read_link(format!("/proc/self/fd/{}", fd.as_raw_fd()))
        .inspect_err(|e| eprintln!("WARN: Fanotify failed to get path from fd: {e}"))
        .ok()?;

    // HACK: A path ending with " (deleted)" is actually valid, but this is how proc conveys
    // that a backing file was deleted, and it will only result in the file not being indexed.
    match path.to_str() {
        Some(p) if p.ends_with(" (deleted)") => None,
        Some(_) => Some(path),
        None => {
            eprintln!("WARN: Fanotify found non-utf8 path, skipping...");
            None
        }
    }
}
