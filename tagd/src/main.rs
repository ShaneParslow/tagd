mod event;
mod queue;
mod subprocess;
mod tagger;
mod db;

use std::sync::mpsc;

fn main() {
    let (tx, rx) = mpsc::channel();

    event::spawn_event_providers(tx);
    let taggers = tagger::scan_taggers();

    let queue = queue::Queue::init(taggers, rx);
    queue.run();
}
