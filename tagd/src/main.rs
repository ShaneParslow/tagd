mod event;
mod subprocess;
mod tagger;

use std::sync::mpsc;

fn main() {
    let (tx, rx) = mpsc::channel();

    event::spawn_event_providers(tx);

    while let Ok(msg) = rx.recv() {
        println!("{msg:?}");
        let query = subprocess::Query::init(msg.path);
        let output = subprocess::fork_tagger("./std-mime".into(), query).expect("Failed to run tagger");
        println!("{output:?}");
    }
}
