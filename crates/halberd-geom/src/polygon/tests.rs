use super::*;

fn v(points: &[(f32, f32)]) -> Vec<Vec2> {
    points.iter().map(|&(x, y)| Vec2::new(x, y)).collect()
}

fn area(part: &[Vec2]) -> f32 {
    let n = part.len();
    (0..n)
        .map(|i| part[i].perp_dot(part[(i + 1) % n]))
        .sum::<f32>()
        * 0.5
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
