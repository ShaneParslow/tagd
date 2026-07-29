use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

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
    /// Taggers must be executable, and must
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
                if !path.file_name()?.to_string_lossy().starts_with("tagger-") { return None }
                let info = run_tagd_info(&path)
                    .inspect_err(|e| {
                        eprintln!("Failed to run tagd-info on {:?}, skipping.\n{:?}", path, e)
                    })
                    .ok()?;
                Some(Tagger { path, info })
            })
            .collect();

        Ok(TaggerRegistry { taggers })
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
        let target_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap() // workspace root
            .join("target/debug");
        return target_dir;
    }

    // Release build default search path
    #[cfg(not(debug_assertions))]
    {
        PathBuf::from("/usr/lib/tagd/taggers")
    }
}
