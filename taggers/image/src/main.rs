//! Image geometry and pixel format, read from the header only.
//!
//! Depends on `mime` matching `image/*`, so it never touches a file magika
//! didn't classify as an image. It reads the container header and stops — no
//! pixels are decoded — which keeps it cheap enough for the synchronous queue
//! even on very large images.
//!
//! `image/svg+xml`, `image/x-dwg` and friends pass the glob but aren't raster
//! formats the `image` crate parses; those simply produce no tags.

use anyhow::Result;
use image::{ImageDecoder, ImageReader};

use tagd_core::tagger::{Dependency, TagRequest, Tagger, TaggerInfo};

/// Aspect ratios worth naming, as (long side, short side). Anything not within
/// [`ASPECT_TOLERANCE`] of one of these is tagged `other` rather than reduced
/// to its own ratio — a literal 1997:1333 tag would be noise, not a category.
const ASPECT_RATIOS: &[(u32, u32)] = &[
    (1, 1),
    (5, 4),
    (4, 3),
    (3, 2),
    (16, 10),
    (16, 9),
    (2, 1),
    (21, 9),
    (3, 1),
];

/// Relative tolerance for matching an aspect ratio. Generous enough to absorb
/// the off-by-a-few-pixels crops cameras and screenshot tools produce.
const ASPECT_TOLERANCE: f64 = 0.02;

/// Buckets for `resolution-class`, keyed on the longer edge (exclusive upper
/// bound), ascending.
const RESOLUTION_CLASSES: &[(u32, &str)] = &[
    (65, "icon"),
    (321, "thumbnail"),
    (1025, "small"),
    (2049, "medium"),
    (4097, "large"),
];

struct Image;

impl Tagger for Image {
    fn info() -> TaggerInfo {
        TaggerInfo {
            name: "image".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            dependencies: vec![Dependency::matching("mime", "image/*")],
            keys: [
                "image-format",
                "width",
                "height",
                "dimensions",
                "orientation",
                "aspect",
                "resolution-class",
                "image-color",
                "image-alpha",
            ]
            .iter()
            .map(|k| k.to_string())
            .collect(),
        }
    }

    fn new() -> Result<Self> {
        Ok(Image)
    }

    fn tag(&mut self, req: &TagRequest) -> Result<Vec<(String, String)>> {
        // `with_guessed_format` sniffs the magic bytes rather than trusting the
        // extension, which matters here: magika already told us this is an
        // image, and it may well be one whose name lies about the format.
        let reader = ImageReader::open(&req.path)?.with_guessed_format()?;

        // Not a raster format this build understands (SVG, DWG, a codec left
        // out of the feature list). Nothing to say — let another tagger have it.
        let Some(format) = reader.format() else {
            return Ok(Vec::new());
        };

        let decoder = match reader.into_decoder() {
            Ok(decoder) => decoder,
            // The header claimed a format but doesn't parse. Truncated
            // downloads and half-written files land here; not an error worth
            // failing the whole tagger over.
            Err(_) => return Ok(Vec::new()),
        };

        let (width, height) = decoder.dimensions();
        let color = decoder.original_color_type();
        let alpha = decoder.color_type().has_alpha();

        // A zero dimension means a malformed header; every derived tag below
        // would be meaningless.
        if width == 0 || height == 0 {
            return Ok(Vec::new());
        }

        let format_name = format!("{format:?}").to_lowercase();

        Ok(vec![
            ("image-format".to_string(), format_name),
            ("width".to_string(), width.to_string()),
            ("height".to_string(), height.to_string()),
            ("dimensions".to_string(), format!("{width}x{height}")),
            (
                "orientation".to_string(),
                orientation(width, height).to_string(),
            ),
            ("aspect".to_string(), aspect(width, height)),
            (
                "resolution-class".to_string(),
                resolution_class(width.max(height)).to_string(),
            ),
            (
                "image-color".to_string(),
                format!("{color:?}").to_lowercase(),
            ),
            ("image-alpha".to_string(), alpha.to_string()),
        ])
    }
}

fn orientation(width: u32, height: u32) -> &'static str {
    match width.cmp(&height) {
        std::cmp::Ordering::Greater => "landscape",
        std::cmp::Ordering::Less => "portrait",
        std::cmp::Ordering::Equal => "square",
    }
}

/// The nearest named ratio, always written long-side-first — a 1080x1920 phone
/// screenshot is `16:9`, and `orientation` carries the direction. That keeps
/// "find my 16:9 images" one query instead of two.
fn aspect(width: u32, height: u32) -> String {
    let long = width.max(height) as f64;
    let short = width.min(height) as f64;
    let ratio = long / short;

    ASPECT_RATIOS
        .iter()
        .find(|(w, h)| {
            let target = *w as f64 / *h as f64;
            (ratio - target).abs() / target <= ASPECT_TOLERANCE
        })
        .map(|(w, h)| format!("{w}:{h}"))
        .unwrap_or_else(|| "other".to_string())
}

fn resolution_class(longest_edge: u32) -> &'static str {
    RESOLUTION_CLASSES
        .iter()
        .find(|(limit, _)| longest_edge < *limit)
        .map(|(_, name)| *name)
        .unwrap_or("huge")
}

fn main() {
    tagd_core::tagger::run::<Image>()
}
