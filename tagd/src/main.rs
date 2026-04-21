mod event;
mod queue;
mod subprocess;
mod tagger;
mod db;

use std::sync::mpsc;

use crate::db::Db;

fn main() {
    let (tx, rx) = mpsc::channel();

    event::spawn_event_providers(tx);
    let taggers = tagger::scan_taggers();
    let db = Db::new().expect("Failed to initialize database");

    let queue = queue::Queue::new(taggers, rx, db);
    queue.run();
}
