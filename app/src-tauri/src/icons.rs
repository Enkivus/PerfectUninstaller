//! Application icon extraction.
//!
//! Icons are pulled straight out of the app bundle so the UI can show the real
//! artwork: `Contents/Info.plist` names the icon file (`CFBundleIconFile`),
//! which lives in `Contents/Resources` as `.icns`. Modern `.icns` containers
//! embed PNG data for the large sizes, so we parse the container and hand the
//! first PNG payload back as a base64 data URL — no image decoding needed.

use std::path::Path;

/// icns element types that carry PNG data, best-first for our use case.
///
/// Icons are rendered at ~30 px (60 px on Retina), so the small-but-sharp
/// variants are preferred: they are a fraction of the bytes of the 1024 px
/// entries, which run past 1 MB for a single app.
const PNG_TYPES: [&[u8; 4]; 8] = [
    b"ic11", b"ic07", b"ic12", b"ic08", b"ic13", b"ic09", b"ic14", b"ic10",
];

/// Icons above this size are skipped when a smaller variant exists. Keeps the
/// full-list payload in the low hundreds of kilobytes instead of megabytes.
const MAX_ICON_BYTES: usize = 400 * 1024;

/// Pulls the icon for `bundle` as a `data:image/png;base64,…` URL.
///
/// Returns `None` for anything that is not a macOS bundle with a readable
/// icon, which the UI treats as "fall back to the letter avatar".
pub fn icon_data_url(bundle: &Path) -> Option<String> {
    let png = load_icon_png(bundle)?;
    Some(format!("data:image/png;base64,{}", base64_encode(&png)))
}

/// Reads `CFBundleIconFile` and returns the embedded PNG bytes, if any.
fn load_icon_png(bundle: &Path) -> Option<Vec<u8>> {
    let info = bundle.join("Contents/Info.plist");
    let value = plist::Value::from_file(&info).ok()?;
    let dict = value.as_dictionary()?;
    let name = dict
        .get("CFBundleIconFile")
        .and_then(|v| v.as_string())
        .map(str::trim)
        .filter(|s| !s.is_empty())?;

    let resources = bundle.join("Contents/Resources");

    // `CFBundleIconFile` is usually written without the extension, but some
    // bundles include it, so try both spellings.
    let mut candidates = Vec::new();
    let trimmed = name.strip_suffix(".icns").unwrap_or(name);
    for stem in [trimmed, name] {
        candidates.push(resources.join(format!("{stem}.icns")));
        candidates.push(resources.join(stem));
    }

    for path in candidates {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        if let Some(png) = png_from_icns(&bytes) {
            return Some(png);
        }
        // Some bundles ship a plain PNG.
        if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            return Some(bytes);
        }
    }
    None
}

/// Extracts the highest-resolution PNG payload from an `.icns` container.
///
/// The container is a sequence of `[type:4][length:4 BE][payload]` records
/// after the 8-byte `icns` + total-length header.
fn png_from_icns(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.len() < 8 || &bytes[0..4] != b"icns" {
        return None;
    }

    let mut found: Vec<([u8; 4], &[u8])> = Vec::new();
    let mut any_png: Option<&[u8]> = None;
    let mut offset = 8usize;

    while offset + 8 <= bytes.len() {
        let mut ty = [0u8; 4];
        ty.copy_from_slice(&bytes[offset..offset + 4]);
        let len = u32::from_be_bytes([
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]) as usize;
        // Length includes the 8-byte record header.
        if len < 8 || offset + len > bytes.len() {
            break;
        }
        let payload = &bytes[offset + 8..offset + len];

        if payload.starts_with(b"\x89PNG\r\n\x1a\n") {
            found.push((ty, payload));
            if any_png.is_none() {
                any_png = Some(payload);
            }
        }
        offset += len;
    }

    // `PNG_TYPES` is ordered best-first, so take the first acceptable match.
    for wanted in PNG_TYPES {
        if let Some((_, data)) = found.iter().find(|(ty, _)| ty == wanted) {
            if data.len() <= MAX_ICON_BYTES {
                return Some((*data).to_vec());
            }
        }
    }

    // Nothing small enough in a preferred slot: take the smallest PNG we have
    // rather than giving up, but still respect the cap when we can.
    found
        .iter()
        .map(|(_, data)| *data)
        .filter(|d| d.len() <= MAX_ICON_BYTES)
        .min_by_key(|d| d.len())
        .map(<[u8]>::to_vec)
        .or_else(|| any_png.map(<[u8]>::to_vec))
}

/// Standard base64 with padding.
fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(TABLE[(triple >> 18) as usize & 0x3f] as char);
        out.push(TABLE[(triple >> 12) as usize & 0x3f] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(triple >> 6) as usize & 0x3f] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[triple as usize & 0x3f] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn icns(records: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
        let mut body = Vec::new();
        for (ty, payload) in records {
            body.extend_from_slice(*ty);
            body.extend_from_slice(&((payload.len() + 8) as u32).to_be_bytes());
            body.extend_from_slice(payload);
        }
        let mut out = b"icns".to_vec();
        out.extend_from_slice(&((body.len() + 8) as u32).to_be_bytes());
        out.extend_from_slice(&body);
        out
    }

    fn png_of(len: usize) -> Vec<u8> {
        let mut v = b"\x89PNG\r\n\x1a\n".to_vec();
        v.resize(len, 0);
        v
    }

    #[test]
    fn picks_the_best_supported_png_type() {
        let a = png_of(300);
        let b = png_of(50);
        let file = icns(&[
            (b"ic07", b.clone()),
            (b"ic09", a.clone()),
            (b"ic04", vec![0u8; 20]), // legacy RLE, no PNG
        ]);
        // ic07 is ranked ahead of ic09 in PNG_TYPES.
        assert_eq!(png_from_icns(&file), Some(b));
    }

    #[test]
    fn prefers_small_sharp_variants_over_huge_ones() {
        let small = png_of(50);
        let huge = png_of(2_000_000);
        // ic09 (512px) is listed after ic07 (128px), so the small one wins even
        // though the container offers a much larger asset.
        let file = icns(&[(b"ic09", huge), (b"ic07", small.clone())]);
        assert_eq!(png_from_icns(&file), Some(small));
    }

    #[test]
    fn falls_back_to_any_png_payload() {
        let only = png_of(16);
        let file = icns(&[(b"icXX", only.clone())]);
        assert_eq!(png_from_icns(&file), Some(only));
    }

    #[test]
    fn rejects_non_icns_and_truncated_files() {
        assert_eq!(png_from_icns(b"not an icns file"), None);
        let file = icns(&[(b"ic09", png_of(100))]);
        assert_eq!(png_from_icns(&file[..10]), None);
        // A record length that runs past the end must not panic.
        let mut bad = b"icns".to_vec();
        bad.extend_from_slice(&9999u32.to_be_bytes());
        bad.extend_from_slice(b"ic09");
        bad.extend_from_slice(&9999u32.to_be_bytes());
        bad.extend_from_slice(b"\x89PNG");
        assert_eq!(png_from_icns(&bad), None);
    }

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    /// Live check against the real `/Applications` on this machine.
    /// Ignored by default because it depends on what is installed.
    #[test]
    #[ignore]
    fn extracts_icons_from_real_bundles() {
        let mut resolved = 0usize;
        let mut total_bytes = 0usize;
        let mut misses = Vec::new();

        for dir in ["/Applications", "/System/Applications"] {
            let Ok(entries) = std::fs::read_dir(dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "app") != Some(true) {
                    continue;
                }
                match icon_data_url(&path) {
                    Some(url) => {
                        resolved += 1;
                        total_bytes += url.len();
                    }
                    // Expected for bundles that ship legacy (non-PNG) `.icns`
                    // data or no icon resource at all; the UI falls back to a
                    // letter avatar.
                    None => misses.push(path.display().to_string()),
                }
            }
        }

        println!("{resolved} icons resolved, {} missing", misses.len());
        assert!(resolved > 10, "expected most bundles to yield an icon");
        assert!(
            total_bytes < 12 * 1024 * 1024,
            "icon payload grew too large: {} KB",
            total_bytes / 1024
        );
    }

    #[test]
    fn missing_bundle_yields_no_icon() {
        assert!(icon_data_url(Path::new("/nonexistent/App.app")).is_none());
    }
}
