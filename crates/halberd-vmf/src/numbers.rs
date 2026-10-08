//! Numbers in VMF values: points like `(1 2 3)`, planes made of three
//! points, and vectors like `1 2 3`.

use crate::VmfError;

/// Reads a side's `plane` value: three points, `(x y z) (x y z) (x y z)`.
pub fn parse_plane(text: &str) -> Result<[[f64; 3]; 3], VmfError> {
    let bad = || VmfError::BadNumbers {
        what: "plane",
        found: text.chars().take(120).collect(),
    };
    // Exactly three "( … )" groups, each holding exactly three numbers.
    let mut points = [[0.0; 3]; 3];
    let mut rest = text.trim();
    for point in &mut points {
        let inner = rest.strip_prefix('(').ok_or_else(bad)?;
        let close = inner.find(')').ok_or_else(bad)?;
        *point = parse_vec3(&inner[..close]).ok_or_else(bad)?;
        rest = inner[close + 1..].trim_start();
    }
    if rest.is_empty() {
        Ok(points)
    } else {
        Err(bad())
    }
}

/// Reads three numbers separated by spaces, such as an entity's `origin`.
pub fn parse_vec3(text: &str) -> Option<[f64; 3]> {
    let mut parts = text.split_ascii_whitespace().map(str::parse::<f64>);
    let v = [
        parts.next()?.ok()?,
        parts.next()?.ok()?,
        parts.next()?.ok()?,
    ];
    (parts.next().is_none() && v.iter().all(|n| n.is_finite())).then_some(v)
}

/// A number as Hammer writes it: whole numbers without a decimal point,
/// others with up to six decimals and no trailing zeros.
pub fn format_number(n: f64) -> String {
    let rounded = n.round();
    if (n - rounded).abs() < 1e-6 {
        // Avoid "-0".
        let whole = if rounded == 0.0 { 0.0 } else { rounded };
        return format!("{whole}");
    }
    let text = format!("{n:.6}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Three numbers separated by spaces.
pub fn format_vec3(v: [f64; 3]) -> String {
    format!(
        "{} {} {}",
        format_number(v[0]),
        format_number(v[1]),
        format_number(v[2])
    )
}

/// A `plane` value from three points.
pub fn format_plane(points: [[f64; 3]; 3]) -> String {
    format!(
        "({}) ({}) ({})",
        format_vec3(points[0]),
        format_vec3(points[1]),
        format_vec3(points[2])
    )
}
