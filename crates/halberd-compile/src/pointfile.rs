//! The leak path vbsp writes when a map leaks (`<map>.lin`, a "pointfile"):
//! one point per line, `x y z`, from an entity inside the map out to the
//! void outside it.

/// Most points read (real leak paths have a few dozen).
const MAX_POINTS: usize = 100_000;
/// Largest pointfile read.
const MAX_BYTES: usize = 16 * 1024 * 1024;

/// The points of a pointfile's text, in order. Lines that aren't three
/// numbers are skipped.
pub fn parse_pointfile(text: &str) -> Vec<[f32; 3]> {
    let text = text.get(..MAX_BYTES.min(text.len())).unwrap_or("");
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
    let path = vmf.with_extension("lin");
    let size = std::fs::metadata(&path).ok()?.len();
    if size > MAX_BYTES as u64 {
        return None;
    }
    let text = std::fs::read_to_string(path).ok()?;
    let points = parse_pointfile(&text);
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
    fn fuzz_odd_pointfiles_never_crash() {
        let mut seed: u64 = 0x1EA4;
        for _ in 0..2000 {
            let text: String = (0..64)
                .map(|_| {
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    b"0123456789 .-e\nx"[(seed % 16) as usize] as char
                })
                .collect();
            for p in parse_pointfile(&text) {
                assert!(p.iter().all(|v| v.is_finite()));
            }
        }
    }
}
