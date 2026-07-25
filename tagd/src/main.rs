mod db;
mod event;
mod queue;
mod registry;
mod socket;
mod subprocess;

use anyhow::{Context, Result};

fn main() -> Result<()> {
    let registry =
        registry::TaggerRegistry::scan_taggers().context("Failed to scan for taggers")?;

    socket::spawn_socket_listener()?;

    let rx = event::spawn_event_providers().context("Failed to start file event threads")?;
    let queue = queue::Queue::new(registry, rx).context("Failed to create queue")?;

    queue.run();
    Ok(())
}
