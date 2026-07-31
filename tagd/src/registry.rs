use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use tagd_core::tagger::TaggerInfo;

pub struct Tagger {
    pub path: PathBuf,
    pub info: TaggerInfo,
}

pub struct TaggerRegistry {
    pub taggers: Vec<Tagger>,
}

// needs: get_deps
impl TaggerRegistry {
    /// Scans the tagger directory for taggers and builds the registry.
    ///
    /// Taggers must be executable, must start with "tagger-", and must
    /// return success upon invocation with `--tagd-info`
    pub fn scan_taggers() -> Result<TaggerRegistry> {
        let search_dir = tagger_search_dir();

        let taggers = std::fs::read_dir(&search_dir)
            .with_context(|| format!("Failed to read tagger directory: {:?}", search_dir))?
            .filter_map(|entry| {
                let path = entry
                    .inspect_err(|e| {
                        eprintln!("Failed to get tagger directory entry, skipping. ({})", e)
                    })
                    .ok()?
                    .path();
                if !path.file_name()?.to_string_lossy().starts_with("tagger-") {
                    return None;
                }
                let info = run_tagd_info(&path)
                    .with_context(|| format!("Failed to run --tagd-info on {:?}, skipping...", path))
                    .inspect_err(|e| { eprintln!("{:#}", e) })
                    .ok()?;
                Some(Tagger { path, info })
            })
            .collect();

        Ok(TaggerRegistry { taggers })
    }

    /// Produces the taggers to run for a full retag, grouped into ordered
    /// stages. Every tagger within a stage is independent and may run
    /// concurrently; each stage depends only on keys produced by earlier
    /// stages, so the stages themselves must run in order. Sequential
    /// execution is just a flatten:
    ///
    /// ```ignore
    /// for stage in registry.run_plan()? {
    ///     for tagger in stage { /* run */ }
    /// }
    /// ```
    ///
    /// Ordering is a topological sort over tag-key dependencies: a tagger runs
    /// after every registered tagger that provides one of its dependency keys.
    /// The value filter (glob) in a dependency does *not* affect ordering — it
    /// only gates whether a tagger is actually invoked at run time, which the
    /// queue decides once it holds the upstream value.
    ///
    /// Taggers with a dependency key that no runnable tagger provides are
    /// dropped with a warning, cascading to anything that transitively depends
    /// on them. A dependency cycle is a hard error.
    ///
    /// The returned `&Tagger`s borrow `self`, so they work as-is for sequential
    /// runs and for scoped concurrency (`std::thread::scope`, or a per-stage
    /// `FuturesUnordered`/`join_all` under tokio). Detached `tokio::spawn`
    /// (which needs `'static`) would want the taggers behind an `Arc` instead.
    pub fn run_plan(&self) -> Result<Vec<Vec<&Tagger>>> {
        let n = self.taggers.len();

        // key -> indices of taggers providing it
        let mut providers: HashMap<&str, Vec<usize>> = HashMap::new();
        for (i, t) in self.taggers.iter().enumerate() {
            for key in &t.info.keys {
                providers.entry(key.as_str()).or_default().push(i);
            }
        }

        let provided_by_runnable = |key: &str, runnable: &[bool], skip: usize| {
            providers
                .get(key)
                .is_some_and(|ps| ps.iter().any(|&p| p != skip && runnable[p]))
        };

        // Drop taggers whose dependency keys no runnable tagger provides,
        // cascading to their dependents. Fixpoint over a tiny graph, so the
        // naive repeat-until-stable pass is fine.
        let mut runnable = vec![true; n];
        loop {
            let mut changed = false;
            for (i, t) in self.taggers.iter().enumerate() {
                if !runnable[i] {
                    continue;
                }
                for dep in &t.info.dependencies {
                    if !provided_by_runnable(&dep.key, &runnable, i) {
                        eprintln!(
                            "Tagger {:?} depends on key {:?} which no runnable tagger provides, skipping.",
                            t.info.name, dep.key
                        );
                        runnable[i] = false;
                        changed = true;
                        break;
                    }
                }
            }
            if !changed {
                break;
            }
        }

        // Build the dependency edges over the runnable set: dependents[p] holds
        // every tagger that must wait for p, and in_degree counts each tagger's
        // distinct prerequisites.
        let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut in_degree = vec![0usize; n];
        for (i, t) in self.taggers.iter().enumerate() {
            if !runnable[i] {
                continue;
            }
            let mut prereqs = HashSet::new();
            for dep in &t.info.dependencies {
                for &p in providers.get(dep.key.as_str()).into_iter().flatten() {
                    if p != i && runnable[p] && prereqs.insert(p) {
                        dependents[p].push(i);
                        in_degree[i] += 1;
                    }
                }
            }
        }

        // Kahn's algorithm, emitting one stage per level.
        let mut stages: Vec<Vec<&Tagger>> = Vec::new();
        let mut current: Vec<usize> = (0..n)
            .filter(|&i| runnable[i] && in_degree[i] == 0)
            .collect();
        let mut emitted = 0;
        while !current.is_empty() {
            let mut next = Vec::new();
            let mut stage = Vec::with_capacity(current.len());
            for &i in &current {
                stage.push(&self.taggers[i]);
                emitted += 1;
                for &d in &dependents[i] {
                    in_degree[d] -= 1;
                    if in_degree[d] == 0 {
                        next.push(d);
                    }
                }
            }
            stages.push(stage);
            current = next;
        }

        let runnable_count = runnable.iter().filter(|&&r| r).count();
        if emitted != runnable_count {
            let cycle: Vec<&str> = (0..n)
                .filter(|&i| runnable[i] && in_degree[i] > 0)
                .map(|i| self.taggers[i].info.name.as_str())
                .collect();
            bail!("Dependency cycle among taggers: {:?}", cycle);
        }

        Ok(stages)
    }
}

fn run_tagd_info(path: &Path) -> Result<TaggerInfo> {
    let out = Command::new(path)
        .current_dir(tagger_search_dir())
        .arg("--tagd-info")
        .output()
        .context("Failed to run tagger with --tagd-info")?;
    serde_json::from_slice(&out.stdout).context("Failed to deserialize --tagd-info")
}

fn tagger_search_dir() -> PathBuf {
    // Runtime env override
    if let Ok(dir) = std::env::var("TAGD_TAGGER_DIR") {
        return PathBuf::from(dir);
    }

    // Debug build default search path
    #[cfg(debug_assertions)]
    {
        // All workspace binaries end up here
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap() // workspace root
            .join("target/debug")
    }

    // Release build default search path
    #[cfg(not(debug_assertions))]
    {
        PathBuf::from("/usr/lib/tagd/taggers")
    }
}
