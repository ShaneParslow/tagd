mod event;

use std::sync::mpsc;

fn main() {
    let (tx, rx) = mpsc::channel();

    event::spawn_event_providers(tx);

    while let Ok(msg) = rx.recv() {
        println!("{msg:?}");
    }
}
