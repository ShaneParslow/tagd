use std::path::PathBuf;
use std::sync::mpsc;

use crate::tagger::Tagger;
use crate::subprocess;
use crate::db::Db;

pub struct Queue {
    taggers: Vec<Tagger>,
    rx: mpsc::Receiver<PathBuf>,
    db: Db,
}

impl Queue {
    pub fn new(taggers: Vec<Tagger>, rx: mpsc::Receiver<PathBuf>) -> Self {
        let db = Db::open().expect("Failed to initialize database");
        Queue {
            taggers,
            rx,
            db,
        }
    }

    // Loop forever recieving events from event threads and running all taggers on each event
    pub fn run(&self) {
        while let Ok(path) = self.rx.recv() {
            let path_s = path.to_str().expect("Invalid path");
            for tagger in &self.taggers {
                let query = subprocess::Query::new(path.clone());
                let Ok(response) = subprocess::run_tagger(&tagger.path, query) else { continue };
                self.db.set_tags(path_s, &response).expect("Failed to set tags");
            }
        }
    }
}