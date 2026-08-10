//! Embedded audio metadata and stream properties.
//!
//! Depends on `mime` matching `audio/*`. Reads tag frames and the stream header
//! only — no audio is decoded, so cost is roughly one seek and a few KiB read
//! regardless of how long the track is.

use anyhow::Result;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::prelude::{Accessor, ItemKey};
use lofty::probe::Probe;

use tagd_core::tagger::{Dependency, TagRequest, Tagger, TaggerInfo};

/// Buckets for `duration-class`, in seconds (exclusive upper bound), ascending.
/// Named generically because this tagger sees podcasts and audiobooks as often
/// as it sees songs.
const DURATION_CLASSES: &[(u64, &str)] = &[
    (60, "clip"),
    (5 * 60, "short"),
    (20 * 60, "medium"),
    (90 * 60, "long"),
];

/// Formats that store samples without loss. `lofty`'s `FileType` doesn't expose
/// this, so it's a list — and it must stay one, since e.g. WavPack and Opus sit
/// on opposite sides of a line their container names don't reveal.
const LOSSLESS_FORMATS: &[&str] = &["flac", "wav", "aiff", "ape", "wavpack"];

struct Audio;

impl Tagger for Audio {
    fn info() -> TaggerInfo {
        TaggerInfo {
            name: "audio".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            dependencies: vec![Dependency::matching("mime", "audio/*")],
            keys: [
                "audio-format",
                "duration",
                "duration-class",
                "bitrate",
                "sample-rate",
                "bit-depth",
                "channels",
                "channel-mode",
                "lossless",
                "tagged",
                "artist",
                "album-artist",
                "album",
                "title",
                "track",
                "year",
                "genre",
                "has-cover",
            ]
            .iter()
            .map(|k| k.to_string())
            .collect(),
        }
    }

    fn new() -> Result<Self> {
        Ok(Audio)
    }

    fn tag(&mut self, req: &TagRequest) -> Result<Vec<(String, String)>> {
        let tagged_file = Probe::open(&req.path)?.read()?;

        let format = format!("{:?}", tagged_file.file_type()).to_lowercase();
        let properties = tagged_file.properties();
        let seconds = properties.duration().as_secs();

        let mut tags = vec![
            ("duration".to_string(), seconds.to_string()),
            (
                "duration-class".to_string(),
                duration_class(seconds).to_string(),
            ),
            (
                "lossless".to_string(),
                LOSSLESS_FORMATS.contains(&format.as_str()).to_string(),
            ),
            ("audio-format".to_string(), format),
        ];

        // `overall_bitrate` includes the container and tag overhead, which is
        // what a player displays and what people mean by "is this the 320k rip".
        if let Some(bitrate) = properties.overall_bitrate() {
            tags.push(("bitrate".to_string(), bitrate.to_string()));
        }
        if let Some(sample_rate) = properties.sample_rate() {
            tags.push(("sample-rate".to_string(), sample_rate.to_string()));
        }
        if let Some(bit_depth) = properties.bit_depth() {
            tags.push(("bit-depth".to_string(), bit_depth.to_string()));
        }
        if let Some(channels) = properties.channels() {
            tags.push(("channels".to_string(), channels.to_string()));
            tags.push((
                "channel-mode".to_string(),
                channel_mode(channels).to_string(),
            ));
        }

        // A file can carry several tag formats at once (an MP3 with both ID3v2
        // and APE); `primary_tag` picks the one the format prefers, and
        // `first_tag` covers files that only have a secondary one.
        let tag = tagged_file
            .primary_tag()
            .or_else(|| tagged_file.first_tag());
        tags.push(("tagged".to_string(), tag.is_some().to_string()));

        if let Some(tag) = tag {
            push(&mut tags, "artist", tag.artist().map(clean));
            push(&mut tags, "album", tag.album().map(clean));
            push(&mut tags, "title", tag.title().map(clean));
            push(&mut tags, "genre", tag.genre().map(clean));
            push(
                &mut tags,
                "album-artist",
                tag.get_string(ItemKey::AlbumArtist)
                    .map(|s| clean(s.into())),
            );
            push(&mut tags, "track", tag.track().map(|n| n.to_string()));
            // Recording dates come through as a full timestamp, but the year is
            // the part anyone queries — nobody looks for an album by the day it
            // was mastered.
            push(
                &mut tags,
                "year",
                tag.date().map(|date| date.year.to_string()),
            );
            tags.push((
                "has-cover".to_string(),
                (!tag.pictures().is_empty()).to_string(),
            ));
        }

        Ok(tags)
    }
}

fn push(tags: &mut Vec<(String, String)>, key: &str, value: Option<String>) {
    if let Some(value) = value.filter(|v| !v.is_empty()) {
        tags.push((key.to_string(), value));
    }
}

/// Tag frames are full of padding and stray whitespace, and an equality-only
/// query has no way to forgive it.
fn clean(value: std::borrow::Cow<'_, str>) -> String {
    value.trim().trim_matches('\0').trim().to_string()
}

fn duration_class(seconds: u64) -> &'static str {
    DURATION_CLASSES
        .iter()
        .find(|(limit, _)| seconds < *limit)
        .map(|(_, name)| *name)
        .unwrap_or("very-long")
}

fn channel_mode(channels: u8) -> &'static str {
    match channels {
        0 => "unknown",
        1 => "mono",
        2 => "stereo",
        _ => "multichannel",
    }
}

fn main() {
    tagd_core::tagger::run::<Audio>()
}
