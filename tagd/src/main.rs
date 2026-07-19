mod event;
mod queue;
mod subprocess;
mod tagger;
mod db;
mod socket;

use anyhow::{Context, Result};

fn main() -> Result<()> {
    let taggers = tagger::scan_taggers().context("Failed to scan for taggers")?; // TODO: proper tagger registry tracking keys for each

    socket::spawn_socket_listener()?;
    
    let rx = event::spawn_event_providers().context("Failed to start file event threads")?;
    let queue = queue::Queue::new(taggers, rx).context("Failed to create queue")?;
    
    queue.run();
    Ok(())
}
