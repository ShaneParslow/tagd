use std::path::PathBuf;
use std::sync::mpsc;

use anyhow::{Context, Result};

use crate::tagger::Tagger;
use crate::subprocess;
use crate::db::Db;

pub struct Queue {
    taggers: Vec<Tagger>,
    rx: mpsc::Receiver<PathBuf>,
    db: Db,
}

impl Queue {
    pub fn new(taggers: Vec<Tagger>, rx: mpsc::Receiver<PathBuf>) -> Result<Self> {
        let db = Db::open().context("Failed to open database")?;
        Ok(Queue {
            taggers,
            rx,
            db,
        })
    }

    // Loop forever recieving events from event threads and running all taggers on each event
    pub fn run(&self) {
        while let Ok(event) = self.rx.recv() {
            let Some(path) = event.to_str() else { continue };
            for tagger in &self.taggers {
                let query = subprocess::Query::new(event.clone());
                let Ok(response) = subprocess::run_tagger(&tagger.path, query) else { continue };
                if let Err(e) = self.db.set_tags(path, &response) {
                    eprintln!("ERR: Could not set tags ({})", e);
                }
            }
        }
    }
}
