use std::sync::mpsc;

use crate::event::Event;
use crate::tagger::Tagger;
use crate::subprocess;
use crate::db::Db;

pub struct Queue {
    taggers: Vec<Tagger>,
    rx: mpsc::Receiver<Event>,
    db: Db,
}

impl Queue {
    pub fn init(taggers: Vec<Tagger>, rx: mpsc::Receiver<Event>, db: Db) -> Self {
        Queue {
            taggers,
            rx,
            db,
        }
    }

    pub fn run(&self) {
        while let Ok(msg) = self.rx.recv() {
            for tagger in &self.taggers {
                println!("{msg:?}");
                let query = subprocess::Query::init(msg.path.clone());
                // Blocks
                let output = subprocess::run_tagger(&tagger.path, query).expect("Failed to run tagger");
                for tag in output.tags {
                    println!("{tag:?}");
                }
            }
        }
    }
}
