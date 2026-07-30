use std::path::PathBuf;
use std::sync::mpsc;

use anyhow::{Context, Result};

use crate::db::Db;
use crate::registry::TaggerRegistry;
use crate::subprocess;

pub struct Queue {
    registry: TaggerRegistry,
    rx: mpsc::Receiver<PathBuf>,
    db: Db,
}

impl Queue {
    pub fn new(registry: TaggerRegistry, rx: mpsc::Receiver<PathBuf>) -> Result<Self> {
        let db = Db::open().context("Failed to open database")?;
        Ok(Queue { registry, rx, db })
    }

    /// Receives file events forever, running the tagger plan against each path
    /// and storing the results. Returns when the event channel closes.
    pub fn run(&self) -> Result<()> {
        // The tagger set is fixed for the daemon's lifetime, so plan once.
        let plan = self
            .registry
            .run_plan()
            .context("Failed to build tagger run plan")?;

        while let Ok(path) = self.rx.recv() {
            let Some(path_str) = path.to_str() else {
                eprintln!("WARN: skipping non-UTF-8 path {path:?}");
                continue;
            };

            // Tagger deps graph is flattened to "stages" where a stage is a
            // set of taggers that can be run concurrently.
            // TODO: pass upstream tag values into dependent taggers. honor
            // the dependency value filter (glob) to skip invocations.
            // concurrent tagger runs potentially with tokio for long-running
            // taggers.
            for stage in &plan {
                for tagger in stage {
                    let response = match subprocess::run_tagger(&tagger.path, &path) {
                        Ok(response) => response,
                        Err(e) => {
                            eprintln!("ERR: tagger {} failed on {path_str}: {e:#}", tagger.info.name);
                            continue;
                        }
                    };
                    if let Err(e) = self.db.set_tags(path_str, &response) {
                        eprintln!(
                            "ERR: failed to store tags for {path_str} from {}: {e:#}",
                            tagger.info.name
                        );
                    }
                }
            }
        }

        Ok(())
    }
}
