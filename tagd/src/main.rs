mod event;
mod queue;
mod subprocess;
mod tagger;
mod db;
mod socket;

use std::sync::mpsc;

fn main() {
    // Event threads to queue channel
    let (tx, rx) = mpsc::channel();

    let taggers = tagger::scan_taggers(); // TODO: proper tagger registry tracking keys for each

    let queue = queue::Queue::new(taggers, rx);
    
    socket::spawn_socket_listener();
    event::spawn_event_providers(tx);
    


    queue.run();
}
