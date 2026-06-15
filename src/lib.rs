//! Read embedded preview images from Affinity files.
//!
//! Affinity Photo (`.afphoto`), Designer (`.afdesign`) and Publisher (`.afpub`)
//! save in a proprietary, zstd-compressed binary format with no public spec.
//! The object graph is effectively a black box, but the apps embed a flattened
//! PNG preview *uncompressed, in the clear*. This crate scans the raw bytes for
//! that preview and returns it , no format reverse engineering required.
//!
//! ```no_run
//! let bytes = std::fs::read("document.afphoto").unwrap();
//! if let Ok(preview) = afphoto::extract_preview(&bytes) {
//!     println!("{}x{} PNG preview", preview.width, preview.height);
//!     std::fs::write("preview.png", &preview.data).unwrap();
//! }
//! ```
//!
//! # Scope
//!
//! v0.1 extracts the largest embedded **PNG** preview and its dimensions.
//! Affinity sometimes also embeds a larger JPEG preview; JPEG support is a
//! deliberate extension point (see the README). Deeper parsing (layers, colour
//! mode) needs the zstd object graph and is out of scope.

#![forbid(unsafe_code)]

/// Magic bytes every Affinity file starts with: `0x00 0xFF 'K' 'A'`.
const MAGIC: [u8; 4] = [0x00, 0xFF, 0x4B, 0x41];

/// PNG 8-byte signature.
const PNG_SIG: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

/// `IEND` chunk type, marking the end of a PNG stream.
const PNG_IEND: [u8; 4] = [0x49, 0x45, 0x4E, 0x44];

/// An extracted preview image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    /// Pixel width, read from the PNG `IHDR`.
    pub width: u32,
    /// Pixel height, read from the PNG `IHDR`.
    pub height: u32,
    /// The complete PNG byte stream, from signature through `IEND` CRC.
    pub data: Vec<u8>,
}

/// Why preview extraction failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The bytes do not start with the Affinity magic.
    NotAffinity,
    /// No embedded PNG preview was found.
    NoPreview,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotAffinity => f.write_str("not an Affinity file (missing magic)"),
            Error::NoPreview => f.write_str("no embedded PNG preview found"),
        }
    }
}

impl std::error::Error for Error {}

/// Returns `true` if `bytes` begins with the Affinity file magic.
///
/// A cheap header check, suitable for content-sniffing before extraction.
pub fn is_affinity(bytes: &[u8]) -> bool {
    bytes.starts_with(&MAGIC)
}

/// Extract the largest embedded PNG preview from an Affinity file's bytes.
///
/// "Largest" is by pixel area, so multi-resolution files yield the richest
/// preview rather than a small layer thumbnail.
///
/// # Errors
///
/// Returns [`Error::NotAffinity`] if the magic is absent, or [`Error::NoPreview`]
/// if no complete PNG stream is present.
pub fn extract_preview(bytes: &[u8]) -> Result<Preview, Error> {
    if !is_affinity(bytes) {
        return Err(Error::NotAffinity);
    }
    scan_pngs(bytes)
        .into_iter()
        .max_by_key(|p| (p.width as u64) * (p.height as u64))
        .ok_or(Error::NoPreview)
}

/// Find every complete, self-consistent PNG stream embedded in `bytes`.
///
/// Each candidate must have a valid signature, a leading `IHDR`, and a trailing
/// `IEND`. The scan is byte-exact, not a decode , callers that need a decoded
/// image should hand `data` to an image library.
fn scan_pngs(bytes: &[u8]) -> Vec<Preview> {
    let mut out = Vec::new();
    let mut search = 0;
    while let Some(rel) = find(&bytes[search..], &PNG_SIG) {
        let start = search + rel;
        // Advance past this signature regardless of whether it parses, so a
        // malformed candidate never wedges the scan.
        search = start + PNG_SIG.len();

        // IHDR must directly follow: length(4) + "IHDR"(4) + width(4) + height(4).
        // width/height sit at signature + 16 and + 20.
        let dims_at = start + 16;
        if dims_at + 8 > bytes.len() {
            continue;
        }
        let width = read_u32_be(bytes, start + 16);
        let height = read_u32_be(bytes, start + 20);

        // The PNG ends 8 bytes past the IEND type marker (4-byte type + 4-byte CRC).
        let Some(iend_rel) = find(&bytes[start..], &PNG_IEND) else {
            continue;
        };
        let end = start + iend_rel + PNG_IEND.len() + 4;
        if end > bytes.len() {
            continue;
        }

        out.push(Preview {
            width,
            height,
            data: bytes[start..end].to_vec(),
        });
    }
    out
}

/// Read a big-endian `u32` at `offset`. Caller guarantees `offset + 4 <= len`.
fn read_u32_be(bytes: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

/// Index of the first occurrence of `needle` in `haystack`, or `None`.
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build PNG-shaped bytes: signature, an `IHDR` carrying `w`/`h`, a filler
    /// chunk, then `IEND`. Byte-exact but not a decodable image , enough to
    /// exercise the scanner, which never decodes.
    fn fake_png(w: u32, h: u32) -> Vec<u8> {
        let mut p = Vec::new();
        p.extend_from_slice(&PNG_SIG);
        p.extend_from_slice(&13u32.to_be_bytes()); // IHDR length
        p.extend_from_slice(b"IHDR");
        p.extend_from_slice(&w.to_be_bytes());
        p.extend_from_slice(&h.to_be_bytes());
        p.extend_from_slice(&[0x08, 0x06, 0x00, 0x00, 0x00]); // depth/colour/etc
        p.extend_from_slice(&[0xAA, 0xBB, 0xCC, 0xDD]); // fake IHDR CRC
        p.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]); // filler payload byte run
        p.extend_from_slice(&0u32.to_be_bytes()); // IEND length
        p.extend_from_slice(&PNG_IEND);
        p.extend_from_slice(&[0xAE, 0x42, 0x60, 0x82]); // IEND CRC
        p
    }

    /// Wrap payloads in a minimal Affinity-looking container (magic + filler).
    fn wrap(payloads: &[&[u8]]) -> Vec<u8> {
        let mut f = Vec::new();
        f.extend_from_slice(&MAGIC);
        f.extend_from_slice(b"\x0b\x00\x00\x00nsrP#InfG"); // header-ish filler
        for p in payloads {
            f.extend_from_slice(&[0x11, 0x22, 0x33, 0x44]); // inter-chunk noise
            f.extend_from_slice(p);
        }
        f.extend_from_slice(b"trailing zstd noise here");
        f
    }

    #[test]
    fn rejects_non_affinity() {
        assert!(!is_affinity(b"not affinity"));
        assert_eq!(extract_preview(b"not affinity"), Err(Error::NotAffinity));
    }

    #[test]
    fn detects_magic() {
        let f = wrap(&[]);
        assert!(is_affinity(&f));
    }

    #[test]
    fn no_png_yields_no_preview() {
        let f = wrap(&[]);
        assert_eq!(extract_preview(&f), Err(Error::NoPreview));
    }

    #[test]
    fn extracts_single_png_with_dimensions() {
        let png = fake_png(512, 384);
        let f = wrap(&[&png]);
        let preview = extract_preview(&f).unwrap();
        assert_eq!(preview.width, 512);
        assert_eq!(preview.height, 384);
        // Carved stream is a complete, standalone PNG.
        assert!(preview.data.starts_with(&PNG_SIG));
        assert!(preview.data.ends_with(&[0xAE, 0x42, 0x60, 0x82]));
    }

    #[test]
    fn picks_largest_png_by_area() {
        let small = fake_png(64, 64);
        let large = fake_png(1024, 768);
        // Order shouldn't matter: small first, large second.
        let f = wrap(&[&small, &large]);
        let preview = extract_preview(&f).unwrap();
        assert_eq!((preview.width, preview.height), (1024, 768));
    }

    #[test]
    fn truncated_png_after_ihdr_is_skipped() {
        // A signature + partial IHDR with no IEND must not panic or match.
        let mut f = wrap(&[]);
        f.extend_from_slice(&PNG_SIG);
        f.extend_from_slice(&[0x00, 0x00, 0x00]); // not enough for dims
        assert_eq!(extract_preview(&f), Err(Error::NoPreview));
    }
}
