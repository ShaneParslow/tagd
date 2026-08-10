//! Source-file facts that need no language parser: size, leftover markers, and
//! the declared license.
//!
//! The only two-level tagger in the tree, and the only one with more than one
//! dependency:
//!
//! - `group: code` (magika, stage 0) — restricts this to source files, not
//!   prose or data.
//! - `text-lines` (tagger-text, stage 1) — no glob, just presence. The text
//!   tagger emits it only for files it read in full, so depending on it is how
//!   this tagger inherits the size cap without knowing what the cap is.
//!
//! The second dependency is what forces this into stage 2 of the run plan.

use anyhow::Result;

use tagd_core::tagger::{Dependency, TagRequest, Tagger, TaggerInfo};

/// Conventional "come back to this" markers. Uppercase-only and matched as
/// substrings: the convention is uppercase, and requiring it keeps prose words
/// like "hack" from counting.
const MARKERS: &[&str] = &["TODO", "FIXME", "XXX", "HACK"];

/// Buckets for `code-size-class`, keyed on non-blank lines (exclusive upper
/// bound), ascending.
const SIZE_CLASSES: &[(usize, &str)] = &[
    (50, "tiny"),
    (200, "small"),
    (600, "medium"),
    (2000, "large"),
];

/// How far into a file to look for an SPDX identifier. License headers are at
/// the top by convention; scanning further would start matching the code that
/// merely talks about licenses.
const LICENSE_SCAN_LINES: usize = 30;

struct Code;

impl Tagger for Code {
    fn info() -> TaggerInfo {
        TaggerInfo {
            name: "code".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            dependencies: vec![
                Dependency::matching("group", "code"),
                Dependency::on("text-lines"),
            ],
            keys: [
                "code-lines",
                "code-size-class",
                "code-markers",
                "code-has-markers",
                "code-license",
            ]
            .iter()
            .map(|k| k.to_string())
            .collect(),
        }
    }

    fn new() -> Result<Self> {
        Ok(Code)
    }

    fn tag(&mut self, req: &TagRequest) -> Result<Vec<(String, String)>> {
        let source = std::fs::read_to_string(&req.path)?;

        let code_lines = source.lines().filter(|l| !l.trim().is_empty()).count();

        // Counted per line, not per occurrence: a line reading "TODO: fix this
        // XXX" is one thing to come back to, not two.
        let markers = source
            .lines()
            .filter(|line| MARKERS.iter().any(|m| line.contains(m)))
            .count();

        Ok(vec![
            ("code-lines".to_string(), code_lines.to_string()),
            (
                "code-size-class".to_string(),
                size_class(code_lines).to_string(),
            ),
            ("code-markers".to_string(), markers.to_string()),
            ("code-has-markers".to_string(), (markers > 0).to_string()),
            ("code-license".to_string(), license(&source)),
        ])
    }
}

fn size_class(lines: usize) -> &'static str {
    SIZE_CLASSES
        .iter()
        .find(|(limit, _)| lines < *limit)
        .map(|(_, name)| *name)
        .unwrap_or("huge")
}

/// The SPDX identifier from a `SPDX-License-Identifier:` header, or `none`.
///
/// Takes only the first whitespace-delimited token after the colon, so a
/// comment closer (`MIT */`) doesn't end up in the tag. Compound expressions
/// like `Apache-2.0 OR MIT` are therefore recorded as their first term — the
/// tag answers "what license is this under" for the overwhelmingly common
/// single-license case and doesn't pretend to be an SPDX expression parser.
fn license(source: &str) -> String {
    source
        .lines()
        .take(LICENSE_SCAN_LINES)
        .find_map(|line| line.split_once("SPDX-License-Identifier:"))
        .and_then(|(_, rest)| rest.split_whitespace().next())
        // Strips a comment closer that ran up against the identifier
        // (`MIT*/`). Only those two characters — trimming `-` as well would
        // turn the real identifier `MIT-0` into `MIT`.
        .map(|id| id.trim_end_matches(['*', '/']).to_string())
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| "none".to_string())
}

fn main() {
    tagd_core::tagger::run::<Code>()
}
