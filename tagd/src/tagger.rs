use std::path::{Path, PathBuf};

pub struct Tagger {
    pub path: PathBuf,
}

impl Tagger {
    fn new(path: PathBuf) -> Option<Tagger> {
        match is_tagger(&path) {
            true => Some(
                Tagger {
                    path,
                }),
            false => None,
        }

    }
}

// Scan the tagger directory for taggers.
// Taggers must begin with "tagger-", must not contain '.', must be executable, and must
// return success upon invocation with "--tagd-info"
pub fn scan_taggers() -> Vec<Tagger> {
    std::fs::read_dir(tagger_search_dir())
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            name.starts_with("tagger-") && !name.contains('.')
        })
        .map(|e| e.path())
        .filter_map(|e| Tagger::new(e))
        .collect()
}

// Get directory of taggers
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
            .parent().unwrap()  // workspace root
            .join("target/debug");
        return target_dir;
    }

    // Release build default search path
    #[cfg(not(debug_assertions))]
    {
        PathBuf::from("/usr/lib/tagd/taggers")
    }
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

fn has_tagd_info(path: &Path) -> bool {
    use std::process::Command;
    let out = Command::new(path).arg("--tagd-info").output();
    out.is_ok_and(|o| o.status.success())
}

fn is_tagger(path: &Path) -> bool {
    is_executable(path) && has_tagd_info(path)
}
