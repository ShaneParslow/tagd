use std::sync::mpsc;

use crate::event::Event;
use crate::subprocess;
use crate::tagger::Tagger;

pub struct Queue {
    taggers: Vec<Tagger>,
    rx: mpsc::Receiver<Event>,
}

impl Queue {
    pub fn init(taggers: Vec<Tagger>, rx: mpsc::Receiver<Event>) -> Self {
        Queue {
            taggers,
            rx,
        }
    }

    pub fn run(&self) {
        while let Ok(msg) = self.rx.recv() {
            for tagger in &self.taggers {
                println!("{msg:?}");
                let query = subprocess::Query::init(msg.path.clone());
                // Blocks
                let output = subprocess::run_tagger(&tagger.path, query).expect("Failed to run tagger");
                println!("{output:?}");
            }
        }
    }
}
