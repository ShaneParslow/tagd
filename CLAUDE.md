# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`tagd` is a Linux file-tagging system. A daemon watches the filesystem for
changes, runs pluggable "taggers" on each changed file, and stores the resulting
key/value tags in SQLite. Clients query tags over a Unix socket. Written in Rust
(edition 2024, `resolver = "3"`).

## Workspace layout

A single Cargo workspace with four kinds of member:

- **`tagd-core`** — shared library and the two wire protocols. No heavy deps by
  default. Depended on by everything else.
- **`tagd`** — the daemon (root-only). Watches files, drives taggers, owns the DB
  and the socket listener.
- **`tagctl`** — a thin CLI client that speaks the socket protocol.
- **`taggers/*`** — independent binaries, one crate each, each producing an
  executable named `tagger-<name>`. Current taggers: `std-mime` (libmagic),
  `magika` (ONNX model).

## Commands

```sh
cargo build                    # build everything; tagger binaries land in target/debug
cargo run -p tagd              # run the daemon — needs root (see below)
cargo run -p tagctl -- files std-mime mime text/plain   # query the daemon
```

There is currently **no test suite** and no linting config beyond stock
`cargo clippy` / `cargo fmt`.

Running the daemon requires `CAP_SYS_ADMIN` (root): the fanotify
`FAN_MARK_FILESYSTEM` mark in [tagd/src/event/fanotify.rs](tagd/src/event/fanotify.rs)
watches the whole filesystem. In a debug build it marks `.` (the cwd), so run it
from a directory you intend to watch, e.g. `sudo -E cargo run -p tagd` from a
scratch dir.

The `magika` tagger links ONNX Runtime via the `ort` crate; the pinned
`ort = "=2.0.0-rc.12"` downloads/links a prebuilt runtime on first build
(see the note in [taggers/magika/Cargo.toml](taggers/magika/Cargo.toml)).

## The two protocols in tagd-core

Everything crossing a process boundary is newline-delimited JSON. `tagd-core`
holds the canonical Rust bindings, but both formats are language-agnostic.

**Socket/query protocol** ([tagd-core/src/query.rs](tagd-core/src/query.rs)):
client → daemon `Request` (a `serde` enum tagged by `"action"`), daemon →
client one response line. `socket_path()` lives here so daemon and client resolve
the same path.

**Tagger protocol** ([tagd-core/src/tagger.rs](tagd-core/src/tagger.rs)): a
tagger is any executable that
1. prints its `TaggerInfo` JSON and exits 0 when run with `--tagd-info`, and
2. given a file path argument, prints one `TaggerResponse` JSON line.

The `runtime` cargo feature on `tagd-core` gates the `Tagger` trait and the
`run::<T>()` driver that implement this protocol (pulling in `anyhow` +
`serde_json`). Tagger crates enable it; `tagctl` and `tagd` do **not**, so pure
clients don't compile those deps. A tagger's `main` is just
`tagd_core::tagger::run::<MyTagger>()`; the driver handles arg parsing,
`--tagd-info`, the before/after mtime consistency check, and serialization.

The `Tagger` trait deliberately splits `info()` (static, cheap) / `new()` (build
heavy reusable state once — a libmagic cookie, an ONNX session) / `tag()`
(per-file). This split anticipates a future switch from one-process-per-file to a
long-running "stream paths on stdin" protocol without touching any tagger. Look
for the `TODO` in `run()` before changing invocation semantics.

## Daemon data flow

[tagd/src/main.rs](tagd/src/main.rs) wires it together:
`event providers (fanotify) → mpsc channel → Queue → taggers (subprocess) → Db`,
with the socket listener running in parallel.

- **Discovery** ([tagd/src/tagger.rs](tagd/src/tagger.rs)): `scan_taggers()`
  scans the tagger dir for executables named `tagger-*` (no `.` in the name) that
  succeed on `--tagd-info`. It does **not** yet track which keys each tagger
  provides (there's a TODO for a real registry).
- **Events** ([tagd/src/event/fanotify.rs](tagd/src/event/fanotify.rs)): watches
  `FAN_CLOSE_WRITE`, resolves each event fd to a path via `/proc/self/fd`, and
  drops paths ending in ` (deleted)`.
- **Queue** ([tagd/src/queue.rs](tagd/src/queue.rs)): for each path, runs every
  tagger as a subprocess and writes the results.
- **Storage** ([tagd/src/db.rs](tagd/src/db.rs) + `tagd/src/schema.sql`): SQLite
  in WAL mode. Each thread opens its own `Db` connection rather than sharing one —
  SQLite handles the locking. The socket handler opens a fresh connection per
  query.

## Tag model (see schema.sql)

`files` and `tags`, one row per (`file_id`, `source_tagger`, `key`). Different
taggers may emit the same `key`; they're disambiguated by `source_tagger` (hence
"qualified tag" queries). Each tag records `mtime_at_tag` and is only considered
valid while the file's mtime still matches. Stale tags are **not** auto-pruned
yet — the reasoning and open questions are in the comments in `schema.sql`.

## Path resolution / env overrides

Debug vs. release builds resolve paths differently via `cfg(debug_assertions)`;
each has an env override. Debug builds put everything under `target/debug/`
(socket, `tags.db`, and the tagger search dir), which is what makes the whole
system runnable from the workspace without installation.

| Purpose        | Env var             | Release default          |
|----------------|---------------------|--------------------------|
| Unix socket    | `TAGD_SOCKET_PATH`  | `/run/tagd/tagd.sock`    |
| Tag database   | `TAGD_DB_PATH`      | `/var/lib/tagd/tags.db`  |
| Tagger dir     | `TAGD_TAGGER_DIR`   | `/usr/lib/tagd/taggers`  |

## Adding a tagger

Create a crate under `taggers/` (workspace picks it up via `members = ["taggers/*"]`),
name its binary `tagger-<name>` in `[[bin]]`, depend on
`tagd-core` with `features = ["runtime"]`, implement `Tagger`, and call
`run::<T>()` in `main`. It's automatically discovered once the built binary is in
the tagger search dir (which in debug is just `target/debug/`).
