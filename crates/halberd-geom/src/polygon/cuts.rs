//! Splitting an outline with straight cuts: from the corner of each dent,
//! one of its two walls is continued straight across until it meets the
//! outline again. An L becomes two rectangles this way, the split a person
//! would make, instead of two pieces meeting on a slanted line.
//!
//! A cut is only made where it ends on whole units, so the pieces stay on
//! the grid. Of the possible cuts, the shortest goes first. A piece left
//! with a dent that no such cut can fix is split the general way.

use super::{STRAIGHT, cross, is_convex};
use glam::DVec2;

/// Most parts this split may make before giving up (the general split is
/// used instead).
const MAX_PARTS: usize = 1024;

/// `outline` (indices into `points`, counter-clockwise) cut into parts
/// along straight cuts, as far as such cuts go. New corners where cuts end
/// are added to `points`. Returns `None` if it would make too many parts.
pub(super) fn straight_cuts(
    points: &mut Vec<DVec2>,
    outline: Vec<usize>,
) -> Option<Vec<Vec<usize>>> {
    let mut done = Vec::new();
    let mut todo = vec![outline];
    while let Some(part) = todo.pop() {
        if done.len() + todo.len() > MAX_PARTS {
            return None;
        }
        if is_convex(points, &part) {
            done.push(part);
            continue;
        }
        match best_cut(points, &part) {
            Some((a, b)) => todo.extend([a, b]),
            None => done.push(part),
        }
    }
    Some(done)
}

/// A planned cut: from corner `from` (a position in the part) along the
/// part's edge `edge` to `hit`.
struct Cut {
    from: usize,
    edge: usize,
    /// Where along the edge it ends: 0 at its start, 1 at its end.
    along: f64,
    hit: DVec2,
    length: f64,
}

/// The shortest straight cut from a dent's corner, as the two parts it
/// leaves, or `None` if there is none.
fn best_cut(points: &mut Vec<DVec2>, part: &[usize]) -> Option<(Vec<usize>, Vec<usize>)> {
    let n = part.len();
    let at = |i: usize| points[part[i % n]];
    let mut best: Option<Cut> = None;
    for i in 0..n {
        let (a, b, c) = (at(i + n - 1), at(i), at(i + 1));
        if cross(b - a, c - b) >= -STRAIGHT * (b - a).length().max(1.0) {
            continue; // Not a dent.
        }
        // Continue either wall straight on past the corner.
        for direction in [b - a, b - c] {
            let Some(cut) = cast(points, part, i, direction.normalize_or_zero()) else {
                continue;
            };
            if best
                .as_ref()
                .is_none_or(|old| cut.length < old.length - 1e-9)
            {
                best = Some(cut);
            }
        }
    }
    let cut = best?;
    // The corner where the cut ends: an existing one, or a new one.
    let mut ring: Vec<usize> = part.to_vec();
    let end = if cut.along < 1e-9 {
        cut.edge
    } else if cut.along > 1.0 - 1e-9 {
        (cut.edge + 1) % n
    } else {
        points.push(cut.hit);
        ring.insert(cut.edge + 1, points.len() - 1);
        cut.edge + 1
    };
    let from = if end <= cut.from && ring.len() > n {
        cut.from + 1
    } else {
        cut.from
    };
    let m = ring.len();
    let walk = |start: usize, stop: usize| -> Vec<usize> {
        let mut out = vec![ring[start]];
        let mut k = start;
        while k != stop {
            k = (k + 1) % m;
            out.push(ring[k]);
        }
        out
    };
    let (first, second) = (walk(from, end), walk(end, from));
    (first.len() >= 3 && second.len() >= 3).then_some((first, second))
}

/// Where a ray from the part's corner `from` going `direction` first meets
/// the outline, if the cut there is usable (inside, ending on whole units).
fn cast(points: &[DVec2], part: &[usize], from: usize, direction: DVec2) -> Option<Cut> {
    if direction == DVec2::ZERO {
        return None;
    }
    let n = part.len();
    let start = points[part[from]];
    let mut nearest: Option<Cut> = None;
    for edge in 0..n {
        // The corner's own edges.
        if edge == from || (edge + 1) % n == from {
            continue;
        }
        let (p, q) = (points[part[edge]], points[part[(edge + 1) % n]]);
        let span = q - p;
        let denominator = cross(direction, span);
        if denominator.abs() < 1e-12 {
            continue; // Parallel.
        }
        let t = cross(p - start, span) / denominator;
        let along = cross(p - start, direction) / denominator;
        if t <= 1e-9 || !(-1e-9..=1.0 + 1e-9).contains(&along) {
            continue;
        }
        if nearest.as_ref().is_none_or(|old| t < old.length) {
            nearest = Some(Cut {
                from,
                edge,
                along: along.clamp(0.0, 1.0),
                hit: start + direction * t,
                length: t,
            });
        }
    }
    let cut = nearest?;
    let rounded = cut.hit.round();
    if (cut.hit - rounded).abs().max_element() > 1e-6 {
        return None; // Would end between units.
    }
    Some(Cut {
        hit: rounded,
        ..cut
    })
}
