//! Camera metadata from Exif: what shot the photo, with what settings, when,
//! and whether it carries a location.
//!
//! Runs alongside `tagger-image` on the same `mime: image/*` gate rather than
//! downstream of it. That's deliberate — `kamadak-exif` parses HEIF and AVIF,
//! which the `image` crate build next door does not, and HEIC is exactly the
//! kind of file people want camera tags on.

use std::fs::File;
use std::io::BufReader;

use anyhow::Result;
use exif::{Exif, In, Tag, Value};

use tagd_core::tagger::{Dependency, TagRequest, Tagger, TaggerInfo};

struct ExifTagger;

impl Tagger for ExifTagger {
    fn info() -> TaggerInfo {
        TaggerInfo {
            name: "exif".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            dependencies: vec![Dependency::matching("mime", "image/*")],
            keys: [
                "exif",
                "camera-make",
                "camera-model",
                "lens",
                "iso",
                "aperture",
                "shutter",
                "focal-length",
                "focal-length-35mm",
                "date-taken",
                "year-taken",
                "software",
                "gps",
            ]
            .iter()
            .map(|k| k.to_string())
            .collect(),
        }
    }

    fn new() -> Result<Self> {
        Ok(ExifTagger)
    }

    fn tag(&mut self, req: &TagRequest) -> Result<Vec<(String, String)>> {
        let mut reader = BufReader::new(File::open(&req.path)?);

        // Most PNGs and every screenshot have no Exif at all, and a container
        // this parser doesn't know looks the same from here. Both are ordinary
        // outcomes, not failures — record the absence and move on, so `exif:false`
        // is a queryable answer rather than a silent gap.
        let Ok(exif) = exif::Reader::new().read_from_container(&mut reader) else {
            return Ok(vec![("exif".to_string(), false.to_string())]);
        };

        let mut tags = vec![("exif".to_string(), true.to_string())];

        // Exif strings are byte arrays that are only Ascii by convention;
        // `ascii` strips the quoting `display_value` would add.
        push(&mut tags, "camera-make", ascii(&exif, Tag::Make));
        push(&mut tags, "camera-model", ascii(&exif, Tag::Model));
        push(&mut tags, "lens", ascii(&exif, Tag::LensModel));
        push(&mut tags, "software", ascii(&exif, Tag::Software));

        // These carry units the display impl knows how to render: `f/2.8`,
        // `1/125 s`, `50 mm`. Keeping that formatting means the tag reads the
        // way the value is written on a lens barrel.
        push(
            &mut tags,
            "iso",
            display(&exif, Tag::PhotographicSensitivity),
        );
        push(&mut tags, "aperture", display(&exif, Tag::FNumber));
        push(&mut tags, "shutter", display(&exif, Tag::ExposureTime));
        push(&mut tags, "focal-length", display(&exif, Tag::FocalLength));
        push(
            &mut tags,
            "focal-length-35mm",
            display(&exif, Tag::FocalLengthIn35mmFilm),
        );

        // "2023:07:14 10:23:45" -> "2023-07-14" plus the bare year, since exact
        // match is the only query the store offers and nobody searches for a
        // timestamp to the second.
        if let Some((date, year)) =
            ascii(&exif, Tag::DateTimeOriginal).and_then(|taken| split_datetime(&taken))
        {
            tags.push(("date-taken".to_string(), date));
            tags.push(("year-taken".to_string(), year));
        }

        // The coordinates themselves are useless to an equality-only query, but
        // "which of my photos are geotagged" is a question people genuinely ask
        // before sharing a folder.
        tags.push((
            "gps".to_string(),
            exif.get_field(Tag::GPSLatitude, In::PRIMARY)
                .is_some()
                .to_string(),
        ));

        Ok(tags)
    }
}

fn push(tags: &mut Vec<(String, String)>, key: &str, value: Option<String>) {
    if let Some(value) = value {
        tags.push((key.to_string(), value));
    }
}

/// An Ascii-typed field as a plain string, or `None` if absent or empty.
/// Cameras pad these generously, hence the trim.
fn ascii(exif: &Exif, tag: Tag) -> Option<String> {
    let field = exif.get_field(tag, In::PRIMARY)?;
    let Value::Ascii(ref parts) = field.value else {
        return None;
    };
    let text = parts
        .iter()
        .map(|part| String::from_utf8_lossy(part))
        .collect::<Vec<_>>()
        .join(" ");
    let text = text.trim().trim_matches('\0').trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// A field rendered the way Exif tooling renders it, units included.
fn display(exif: &Exif, tag: Tag) -> Option<String> {
    let field = exif.get_field(tag, In::PRIMARY)?;
    let text = field.display_value().with_unit(exif).to_string();
    let text = text.trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// Splits an Exif `YYYY:MM:DD HH:MM:SS` timestamp into an ISO date and its
/// year. Returns `None` for anything that doesn't have that shape.
fn split_datetime(value: &str) -> Option<(String, String)> {
    let date = value.split_whitespace().next()?;
    let mut parts = date.split(':');
    let (year, month, day) = (parts.next()?, parts.next()?, parts.next()?);
    if year.len() != 4 || month.len() != 2 || day.len() != 2 {
        return None;
    }
    if !date.chars().all(|c| c.is_ascii_digit() || c == ':') {
        return None;
    }
    Some((format!("{year}-{month}-{day}"), year.to_string()))
}

fn main() {
    tagd_core::tagger::run::<ExifTagger>()
}
