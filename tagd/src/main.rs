mod event;
mod queue;
mod subprocess;
mod tagger;
mod db;
mod socket;

use anyhow::{Context, Result};

fn main() -> Result<()> {
    let taggers = tagger::scan_taggers()?; // TODO: proper tagger registry tracking keys for each

    socket::spawn_socket_listener()?;
    
    let rx = event::spawn_event_providers()?;
    let queue = queue::Queue::new(taggers, rx).context("Failed to create queue")?;
    queue.run();
    Ok(())
}
