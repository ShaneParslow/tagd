//! Whole-file hygiene facts about text: line endings, indentation, encoding,
//! and the small things that show up in diffs and code review.
//!
//! Depends on magika's `is-text` rather than a `mime: text/*` glob, because
//! magika's mime types don't partition that way — Python is `text/x-python`
//! but Rust is `application/x-rust` and JSON is `application/json`. `is-text`
//! comes from magika's own type table and covers all of them.
//!
//! Answers "which files still have CRLF", "what's tab-indented", "what's
//! missing a trailing newline" across a whole filesystem.

use std::path::Path;

use anyhow::Result;

use tagd_core::tagger::{Dependency, TagRequest, Tagger, TaggerInfo};

/// Files larger than this are reported as `text-truncated: yes` and nothing
/// else. A partial read would give confidently wrong answers — a file whose
/// first 16 MiB happen to be LF-only isn't an LF file — and emitting no
/// `text-lines` also stops `tagger-code` from running on a file nobody read in
/// full. Well above any hand-written source file; a text file past it is a log
/// or a data dump.
const MAX_BYTES: u64 = 16 * 1024 * 1024;

struct Text;

impl Tagger for Text {
    fn info() -> TaggerInfo {
        TaggerInfo {
            name: "text".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            dependencies: vec![Dependency::matching("is-text", "true")],
            keys: [
                "text-truncated",
                "text-lines",
                "text-line-ending",
                "text-indent",
                "text-final-newline",
                "text-trailing-whitespace",
                "text-longest-line",
                "text-encoding",
                "text-bom",
                "text-shebang",
            ]
            .iter()
            .map(|k| k.to_string())
            .collect(),
        }
    }

    fn new() -> Result<Self> {
        Ok(Text)
    }

    fn tag(&mut self, req: &TagRequest) -> Result<Vec<(String, String)>> {
        if std::fs::metadata(&req.path)?.len() > MAX_BYTES {
            return Ok(vec![("text-truncated".to_string(), true.to_string())]);
        }

        let bytes = std::fs::read(&req.path)?;
        Ok(analyze(&bytes))
    }
}

fn analyze(bytes: &[u8]) -> Vec<(String, String)> {
    let bom = bytes.starts_with(&[0xEF, 0xBB, 0xBF]);
    let body = if bom { &bytes[3..] } else { bytes };

    let stats = LineStats::of(body);

    let mut tags = vec![
        ("text-truncated".to_string(), false.to_string()),
        ("text-lines".to_string(), stats.lines.to_string()),
        (
            "text-line-ending".to_string(),
            stats.line_ending().to_string(),
        ),
        ("text-indent".to_string(), stats.indent().to_string()),
        (
            "text-final-newline".to_string(),
            stats.final_newline.to_string(),
        ),
        (
            "text-trailing-whitespace".to_string(),
            stats.trailing_whitespace.to_string(),
        ),
        (
            "text-longest-line".to_string(),
            stats.longest_line.to_string(),
        ),
        ("text-encoding".to_string(), encoding(body).to_string()),
        ("text-bom".to_string(), bom.to_string()),
        ("text-shebang".to_string(), shebang(body)),
    ];

    tags.sort_by(|a, b| a.0.cmp(&b.0));
    tags
}

#[derive(Default)]
struct LineStats {
    lines: usize,
    lf: usize,
    crlf: usize,
    cr: usize,
    tab_indented: usize,
    space_indented: usize,
    longest_line: usize,
    trailing_whitespace: bool,
    final_newline: bool,
}

impl LineStats {
    /// One pass over the bytes. Splits on any of LF, CRLF or lone CR so a
    /// mixed-ending file is still measured line by line rather than read as one
    /// enormous line.
    fn of(body: &[u8]) -> LineStats {
        let mut stats = LineStats::default();
        if body.is_empty() {
            // An empty file isn't missing a trailing newline — it has no lines
            // to end. Reporting `no` here would flag every placeholder file.
            stats.final_newline = true;
            return stats;
        }

        let mut start = 0;
        let mut i = 0;
        while i < body.len() {
            let (terminator_len, is_terminator) = match body[i] {
                b'\n' => {
                    stats.lf += 1;
                    (1, true)
                }
                b'\r' if body.get(i + 1) == Some(&b'\n') => {
                    stats.crlf += 1;
                    (2, true)
                }
                b'\r' => {
                    stats.cr += 1;
                    (1, true)
                }
                _ => (0, false),
            };

            if is_terminator {
                stats.record_line(&body[start..i]);
                i += terminator_len;
                start = i;
            } else {
                i += 1;
            }
        }

        // Trailing bytes with no terminator are still a line — and their
        // absence is exactly what `text-final-newline` reports.
        if start < body.len() {
            stats.record_line(&body[start..]);
        } else {
            stats.final_newline = true;
        }

        stats
    }

    fn record_line(&mut self, line: &[u8]) {
        self.lines += 1;
        self.longest_line = self.longest_line.max(line.len());

        match line.first() {
            // An all-whitespace line isn't evidence of an indentation style,
            // it's just a blank line someone left spaces on.
            Some(b'\t') if line.iter().any(|b| !b.is_ascii_whitespace()) => self.tab_indented += 1,
            Some(b' ') if line.iter().any(|b| !b.is_ascii_whitespace()) => self.space_indented += 1,
            _ => {}
        }

        if matches!(line.last(), Some(b' ' | b'\t')) {
            self.trailing_whitespace = true;
        }
    }

    fn line_ending(&self) -> &'static str {
        match (self.lf > 0, self.crlf > 0, self.cr > 0) {
            (false, false, false) => "none",
            (true, false, false) => "lf",
            (false, true, false) => "crlf",
            (false, false, true) => "cr",
            _ => "mixed",
        }
    }

    fn indent(&self) -> &'static str {
        match (self.tab_indented > 0, self.space_indented > 0) {
            (false, false) => "none",
            (true, false) => "tabs",
            (false, true) => "spaces",
            // Continuation lines and aligned comments make a few of these
            // normal, so "mixed" means "both appear", not "inconsistent".
            (true, true) => "mixed",
        }
    }
}

/// `ascii` is called out separately from `utf-8` because it's the property that
/// actually matters when you're hunting for smart quotes or stray non-breaking
/// spaces that crept into a config file.
fn encoding(body: &[u8]) -> &'static str {
    if body.is_ascii() {
        "ascii"
    } else if std::str::from_utf8(body).is_ok() {
        "utf-8"
    } else {
        "non-utf8"
    }
}

/// The interpreter a `#!` line names, unwrapping `/usr/bin/env foo` to `foo`.
/// `none` when there's no shebang, so the tag is always present and can't go
/// stale after someone removes one.
fn shebang(body: &[u8]) -> String {
    let Some(rest) = body.strip_prefix(b"#!") else {
        return "none".to_string();
    };
    let line = rest
        .split(|b| *b == b'\n' || *b == b'\r')
        .next()
        .unwrap_or(&[]);
    let Ok(line) = std::str::from_utf8(line) else {
        return "none".to_string();
    };

    let mut words = line.split_whitespace();
    let Some(first) = words.next() else {
        return "none".to_string();
    };
    let interpreter = match basename(first) {
        // `env` is a launcher, not the interpreter; the next word is the real
        // answer, unless it's an option like `env -S python3`.
        "env" => words.find(|w| !w.starts_with('-')).unwrap_or(first),
        _ => first,
    };

    basename(interpreter).to_string()
}

fn basename(path: &str) -> &str {
    Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
}

fn main() {
    tagd_core::tagger::run::<Text>()
}
