use super::*;

fn v(points: &[(f32, f32)]) -> Vec<Vec2> {
    points.iter().map(|&(x, y)| Vec2::new(x, y)).collect()
}

/// Area, worked out in double precision (far from the middle, single
/// precision loses whole square units).
fn area(part: &[Vec2]) -> f32 {
    let n = part.len();
    let total = (0..n)
        .map(|i| part[i].as_dvec2().perp_dot(part[(i + 1) % n].as_dvec2()))
        .sum::<f64>()
        * 0.5;
    total as f32
}

#[test]
fn a_square_is_one_part_and_an_l_is_two() {
    let square = v(&[(0.0, 0.0), (64.0, 0.0), (64.0, 64.0), (0.0, 64.0)]);
    assert_eq!(convex_parts(&square).unwrap().len(), 1);
    let l = v(&[
        (0.0, 0.0),
        (128.0, 0.0),
        (128.0, 64.0),
        (64.0, 64.0),
        (64.0, 128.0),
        (0.0, 128.0),
    ]);
    let parts = convex_parts(&l).unwrap();
    assert_eq!(parts.len(), 2);
    let total: f32 = parts.iter().map(|p| area(p)).sum();
    assert_eq!(total, 128.0 * 64.0 + 64.0 * 64.0);
    assert!(parts.iter().all(|p| area(p) > 0.0), "counter-clockwise");
}

#[test]
fn clockwise_repeated_and_straight_corners_are_cleaned() {
    let messy = v(&[
        (0.0, 0.0),
        (0.0, 64.0),
        (0.0, 64.0),
        (32.0, 64.0),
        (64.0, 64.0),
        (64.0, 0.0),
        (0.0, 0.0),
    ]);
    let clean = clean_outline(&messy).unwrap();
    assert_eq!(clean.len(), 4);
    assert!(area(&clean) > 0.0);
}

#[test]
fn bad_outlines_are_refused() {
    let line = v(&[(0.0, 0.0), (64.0, 0.0), (128.0, 0.0)]);
    assert!(clean_outline(&line).is_err());
    let bow_tie = v(&[(0.0, 0.0), (64.0, 64.0), (64.0, 0.0), (0.0, 64.0)]);
    assert_eq!(clean_outline(&bow_tie), Err(GeomError::CrossesItself));
    assert_eq!(
        clean_outline(&v(&[(0.0, 0.0), (f32::NAN, 1.0), (1.0, 1.0)])),
        Err(GeomError::NotFinite)
    );
    let too_many: Vec<Vec2> = (0..300)
        .map(|i| {
            let a = i as f32 / 300.0 * std::f32::consts::TAU;
            Vec2::new(a.cos(), a.sin()) * 1000.0
        })
        .collect();
    assert_eq!(clean_outline(&too_many), Err(GeomError::TooManyFaces));
}

#[test]
fn prisms_fill_the_outline_between_two_heights() {
    let u = v(&[
        (0.0, 0.0),
        (192.0, 0.0),
        (192.0, 128.0),
        (128.0, 128.0),
        (128.0, 64.0),
        (64.0, 64.0),
        (64.0, 128.0),
        (0.0, 128.0),
    ]);
    let brushes = prism_brushes(&u, 0.0, 96.0).unwrap();
    assert!(brushes.len() <= 3, "{}", brushes.len());
    let volume: f32 = brushes.iter().map(Brush::volume).sum();
    assert!((volume - (192.0 * 128.0 - 64.0 * 64.0) * 96.0).abs() < 1.0);
    for (i, a) in brushes.iter().enumerate() {
        for b in &brushes[i + 1..] {
            assert!(!a.overlaps(b));
        }
        for p in a.points() {
            assert_eq!(p, p.round());
        }
    }
    assert!(prism_brushes(&u, 0.0, 0.5).is_err());
}

#[test]
fn random_star_outlines_split_into_convex_parts_that_fill_them() {
    let mut seed: u64 = 0x5747_0000_0000_0001;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % 10_001) as f32 / 10_000.0
    };
    for _ in 0..200 {
        let n = 3 + (next() * 20.0) as usize;
        let outline: Vec<Vec2> = (0..n)
            .map(|i| {
                let a = i as f32 / n as f32 * std::f32::consts::TAU;
                let r = 64.0 + next() * 256.0;
                (Vec2::new(a.cos(), a.sin()) * r).round()
            })
            .collect();
        let Ok(clean) = clean_outline(&outline) else {
            continue;
        };
        let parts = convex_parts(&clean).unwrap();
        let total: f32 = parts.iter().map(|p| area(p)).sum();
        assert!((total - area(&clean)).abs() < 1.0);
        for part in &parts {
            let k = part.len();
            for i in 0..k {
                let (a, b, c) = (part[(i + k - 1) % k], part[i], part[(i + 1) % k]);
                assert!((b - a).perp_dot(c - b) >= -1e-3, "convex");
            }
        }
        assert!(parts.len() <= n - 2);
    }
}

#[test]
fn sharp_or_nearly_straight_corners_never_make_drifting_brushes() {
    // Found by review: these made brushes up to 2 units off the outline.
    let sliver = v(&[(4032.0, 7208.0), (3424.0, 7808.0), (5384.0, 5872.0)]);
    assert_eq!(prism_brushes(&sliver, 0.0, 64.0), Err(GeomError::TooSharp));
    let bent = v(&[
        (7672.0, 408.0),
        (7784.0, 3696.0),
        (7816.0, 4640.0),
        (6112.0, 4032.0),
        (5552.0, 2920.0),
    ]);
    // The nearly straight corner is dropped; the rest makes clean brushes.
    let clean = clean_outline(&bent).unwrap();
    assert_eq!(clean.len(), 4);
    let brushes = prism_brushes(&bent, 0.0, 64.0).unwrap();
    let volume: f32 = brushes.iter().map(|b| b.volume()).sum();
    assert!((volume - area(&clean) * 64.0).abs() < 64.0, "{volume}");
}

#[test]
fn a_comb_with_many_teeth_is_still_made_of_boxes() {
    // 63 teeth: 254 corners, many of them on the comb's straight back.
    let mut outline = vec![Vec2::new(0.0, 0.0)];
    for t in 0..63 {
        let x = t as f32 * 32.0;
        outline.extend([
            Vec2::new(x, 64.0),
            Vec2::new(x + 16.0, 64.0),
            Vec2::new(x + 16.0, 32.0),
            Vec2::new(x + 32.0, 32.0),
        ]);
    }
    outline.push(Vec2::new(63.0 * 32.0, 0.0));
    let outline: Vec<Vec2> = outline.into_iter().rev().collect();
    let brushes = prism_brushes(&outline, 0.0, 16.0).unwrap();
    assert!(brushes.iter().all(|b| b.faces().len() <= 8), "small pieces");
}

#[test]
fn random_outlines_far_from_the_middle_fill_exactly_or_are_refused() {
    // Star-shaped outlines with dents, up to 8,000 units out, on an
    // 8-unit grid. Each either makes brushes that exactly fill it, or is
    // refused as too sharp; never brushes with gaps or overlaps.
    let mut seed: u64 = 0x0123_4567_89ab_cdef;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % 10_001) as f32 / 10_000.0
    };
    let (mut made, mut refused) = (0, 0);
    for _ in 0..400 {
        let n = 3 + (next() * 30.0) as usize;
        let middle = Vec2::new(next() * 8000.0, next() * 8000.0);
        let size = 64.0 + next() * 4000.0;
        let outline: Vec<Vec2> = (0..n)
            .map(|i| {
                let a = (i as f32 + next() * 0.8) / n as f32 * std::f32::consts::TAU;
                let r = size * (0.2 + next() * 0.8);
                ((middle + Vec2::new(a.cos(), a.sin()) * r) / 8.0).round() * 8.0
            })
            .collect();
        let Ok(clean) = clean_outline(&outline) else {
            continue;
        };
        match prism_brushes(&outline, 0.0, 64.0) {
            Ok(brushes) => {
                made += 1;
                // Brush::volume sums large numbers far from the middle;
                // the floor's area is exact enough.
                let floor = |b: &Brush| {
                    let face = b
                        .faces()
                        .iter()
                        .find(|f| f.plane().normal.z < -0.5)
                        .unwrap();
                    let corners: Vec<Vec2> = face.vertices().iter().map(|p| p.truncate()).collect();
                    f64::from(area(&corners)).abs()
                };
                let volume: f64 = brushes.iter().map(|b| floor(b) * 64.0).sum();
                let expected = f64::from(area(&clean)) * 64.0;
                assert!(
                    (volume - expected).abs() <= expected * 1e-6 + 64.0,
                    "{volume} vs {expected} for {outline:?}"
                );
            }
            Err(GeomError::TooSharp) => refused += 1,
            Err(e) => panic!("{e} for {outline:?}"),
        }
    }
    assert!(
        made > 300,
        "most outlines work: {made} made, {refused} refused"
    );
}

/// True if every side of `part` runs along X or Y.
fn boxy(part: &[Vec2]) -> bool {
    let n = part.len();
    (0..n).all(|i| {
        let d = part[(i + 1) % n] - part[i];
        d.x == 0.0 || d.y == 0.0
    })
}

#[test]
fn an_l_u_and_plus_split_into_rectangles() {
    let l = v(&[
        (0.0, 0.0),
        (256.0, 0.0),
        (256.0, 128.0),
        (128.0, 128.0),
        (128.0, 256.0),
        (0.0, 256.0),
    ]);
    let u = v(&[
        (0.0, 0.0),
        (192.0, 0.0),
        (192.0, 192.0),
        (128.0, 192.0),
        (128.0, 64.0),
        (64.0, 64.0),
        (64.0, 192.0),
        (0.0, 192.0),
    ]);
    let plus = v(&[
        (64.0, 0.0),
        (128.0, 0.0),
        (128.0, 64.0),
        (192.0, 64.0),
        (192.0, 128.0),
        (128.0, 128.0),
        (128.0, 192.0),
        (64.0, 192.0),
        (64.0, 128.0),
        (0.0, 128.0),
        (0.0, 64.0),
        (64.0, 64.0),
    ]);
    for (outline, pieces) in [(l, 2), (u, 3), (plus, 3)] {
        let parts = convex_parts(&outline).unwrap();
        assert_eq!(parts.len(), pieces, "{outline:?}");
        for part in &parts {
            assert_eq!(part.len(), 4, "a rectangle: {part:?}");
            assert!(boxy(part), "{part:?}");
        }
        let total: f32 = parts.iter().map(|p| area(p)).sum();
        assert_eq!(total, area(&clean_outline(&outline).unwrap()));
    }
}

#[test]
fn a_cut_that_would_end_between_units_is_not_made() {
    // The dent's walls continue onto a slanted side at a point that is not
    // on whole units, so the general split is used, still on whole units.
    let outline = v(&[
        (0.0, 0.0),
        (100.0, 0.0),
        (100.0, 50.0),
        (50.0, 50.0),
        (50.0, 100.0),
        (0.0, 33.0),
    ]);
    for part in convex_parts(&outline).unwrap() {
        assert!(part.iter().all(|p| *p == p.round()), "{part:?}");
    }
}
