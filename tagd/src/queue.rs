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
            // Create or update file entry
            let file_id = self.db.upsert_file(event.path.to_str().expect("Invalid path"), event.mtime).expect("Failed to add file to db");
            
            for tagger in &self.taggers {
                // Build query
                let query = subprocess::Query::new(event.path.clone());
                
                // Run tagger - blocks
                let Ok(response) = subprocess::run_tagger(&tagger.path, query) else { continue };
                
                // Set tags
                self.db.set_tags(file_id, &response.tagger, &response.tags).expect("Failed to set tags");
                for tag in response.tags {
                    println!("{tag:?}");
                }
            }
        }
    }
}
