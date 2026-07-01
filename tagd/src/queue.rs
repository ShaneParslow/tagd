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
    pub fn new(taggers: Vec<Tagger>, rx: mpsc::Receiver<Event>, db: Db) -> Self {
        Queue {
            taggers,
            rx,
            db,
        }
    }

    pub fn run(&self) {
        while let Ok(event) = self.rx.recv() {
            let path = event.path.to_str().expect("Invalid path");
            for tagger in &self.taggers {
                let query = subprocess::Query::new(event.path.clone());
                let Ok(response) = subprocess::run_tagger(&tagger.path, query) else { continue };
                self.db.set_tags(path, &response).expect("Failed to set tags");
            }
        }
    }
}
