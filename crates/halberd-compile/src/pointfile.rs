//! The leak path vbsp writes when a map leaks (`<map>.lin`, a "pointfile"):
//! one point per line, `x y z`, from outside the map (first point),
//! through the gap, to the leaking entity (last point).

use std::io::Read;

/// Most points read (real leak paths have a few dozen).
const MAX_POINTS: usize = 100_000;
/// Largest pointfile read.
const MAX_BYTES: usize = 16 * 1024 * 1024;

/// The points of a pointfile's text, in order. Lines that aren't three
/// numbers are skipped.
pub fn parse_pointfile(text: &str) -> Vec<[f32; 3]> {
    let text = &text[..text.floor_char_boundary(MAX_BYTES)];
    text.lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace().map(|p| p.parse::<f32>().ok());
            let point = [parts.next()??, parts.next()??, parts.next()??];
            point.iter().all(|v| v.is_finite()).then_some(point)
        })
        .take(MAX_POINTS)
        .collect()
}

/// Reads the pointfile next to `vmf`, if there is one.
pub fn read_pointfile(vmf: &std::path::Path) -> Option<Vec<[f32; 3]>> {
    let file = std::fs::File::open(vmf.with_extension("lin")).ok()?;
    let mut bytes = Vec::new();
    // One byte past the cap tells a file that is too big (even one still
    // growing) from one that just fits.
    file.take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > MAX_BYTES {
        return None;
    }
    let points = parse_pointfile(&String::from_utf8_lossy(&bytes));
    (points.len() >= 2).then_some(points)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_are_read_in_order_and_junk_is_skipped() {
        let text = "0 0 64\n128.5 -32 64\nnot a point\n1 2\n256 0 inf\n512 0 64\n";
        assert_eq!(
            parse_pointfile(text),
            [[0.0, 0.0, 64.0], [128.5, -32.0, 64.0], [512.0, 0.0, 64.0]]
        );
        assert!(parse_pointfile("").is_empty());
    }

    #[test]
    fn non_numbers_and_overflowing_numbers_are_skipped() {
        let text = "nan 0 0\ninf 0 0\n1e39 0 0\n0 -inf 0\n1 2 3\n4 5 6\n";
        assert_eq!(parse_pointfile(text), [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]);
    }

    #[test]
    fn at_most_the_cap_of_points_is_read() {
        let text = "1 2 3\n".repeat(MAX_POINTS + 1);
        assert_eq!(parse_pointfile(&text).len(), MAX_POINTS);
    }

    #[test]
    fn text_past_the_byte_cap_is_ignored_even_mid_character() {
        // A multi-byte character straddling the cap must not lose it all.
        let mut text = "1 2 3\n".to_string();
        text.push_str(&"é".repeat(MAX_BYTES));
        assert_eq!(parse_pointfile(&text), [[1.0, 2.0, 3.0]]);
    }

    #[test]
    fn pointfiles_too_big_are_not_read_and_short_ones_are_no_path() {
        let folder = tempfile::tempdir().unwrap();
        let vmf = folder.path().join("map.vmf");
        assert_eq!(read_pointfile(&vmf), None, "no file");
        std::fs::write(vmf.with_extension("lin"), "0 0 0\n").unwrap();
        assert_eq!(read_pointfile(&vmf), None, "one point");
        std::fs::write(vmf.with_extension("lin"), b"0 0 0\n\xff\n64 0 0\n").unwrap();
        assert_eq!(
            read_pointfile(&vmf),
            Some(vec![[0.0, 0.0, 0.0], [64.0, 0.0, 0.0]]),
            "bytes that aren't text are skipped"
        );
        let mut big = "0 0 0\n".repeat(MAX_BYTES / 6 + 1).into_bytes();
        big.truncate(MAX_BYTES + 1);
        std::fs::write(vmf.with_extension("lin"), big).unwrap();
        assert_eq!(read_pointfile(&vmf), None, "over the cap");
    }

    #[test]
    fn fuzz_odd_pointfiles_never_crash() {
        let mut seed: u64 = 0x1EA4;
        for _ in 0..2000 {
            let text: String = (0..64)
                .map(|_| {
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    b"0123456789 .-e\nxinfa"[(seed % 20) as usize] as char
                })
                .collect();
            for p in parse_pointfile(&text) {
                assert!(p.iter().all(|v| v.is_finite()));
            }
        }
    }
}
