use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::mpsc;

use anyhow::{Context, Result};

use tagd_core::tagger::TagRequest;

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

            // Tags produced so far this run, so dependent taggers can read the
            // values of the keys they depend on. Reset per file.
            let mut resolved: BTreeMap<String, String> = BTreeMap::new();

            // Tagger deps graph is flattened to "stages" where a stage is a
            // set of taggers that can be run concurrently.
            // TODO: run each stage's taggers concurrently (fold their tags into
            // `resolved` after the stage joins), potentially with tokio for
            // long-running taggers.
            for stage in &plan {
                for tagger in stage {
                    // Resolve declared dependencies against upstream tags,
                    // applying each dependency's glob filter. If any is missing
                    // or fails its filter, this tagger doesn't run on this file.
                    let mut deps = Vec::new();
                    let mut satisfied = true;
                    for dep in &tagger.info.dependencies {
                        let Some(value) = resolved.get(&dep.key) else {
                            satisfied = false;
                            break;
                        };
                        if !dep.accepts(value) {
                            satisfied = false;
                            break;
                        }
                        deps.push((dep.key.clone(), value.clone()));
                    }
                    if !satisfied {
                        continue;
                    }

                    let request = TagRequest {
                        path: path.clone(),
                        deps,
                    };
                    let response = match subprocess::run_tagger(&tagger.path, &request)
                        .with_context(|| {
                            format!("ERR: tagger \"{}\" failed ({})", tagger.info.name, path_str)
                        }) {
                        Ok(response) => response,
                        Err(e) => {
                            eprintln!("{e:#}");
                            continue;
                        }
                    };

                    // Make this tagger's tags visible to downstream dependents.
                    // TODO: once taggers have priorities, resolve same-key
                    // conflicts by priority rather than last-writer-wins.
                    for (key, value) in &response.tags {
                        resolved.insert(key.clone(), value.clone());
                    }

                    if let Err(e) = self.db.set_tags(path_str, &response).with_context(|| {
                        format!(
                            "ERR: failed to store tags for \"{}\" from \"{}\"",
                            path_str, tagger.info.name
                        )
                    }) {
                        eprintln!("{e:#}");
                    }
                }
            }
        }

        Ok(())
    }
}
