//! The rail grid from the course (board row G20c2c1a; TRICKY_DISC, skipped without it): the `.ltg` INodes' spline
//! lists index the `.pbd`'s spline segments (every entry in range, every segment's box meeting its INode's), and on the
//! grid `course_ground::rail_world` builds, `Rail_ClosestPoint` (`RailWorld::closest_point`) finds every switched-on
//! curve's own points, on gari and elysium.

fn course(name: &str) -> Option<(Vec<u8>, Vec<u8>, Vec<u8>)> {
    let disc = std::env::var("TRICKY_DISC").ok()?;
    let mut files = tricky_data::big::Files::new(tricky_data::disc::Disc::open(&disc).ok()?);
    files.mount(&format!("data/models/{name}.big")).ok()?;
    let mut get = |ext: &str| files.load(&format!("|data/models/{name}.{ext}")).ok().flatten();
    Some((get("pbd")?, get("ltg")?, get("ssf")?))
}

#[test]
fn inode_spline_lists_index_the_segments() {
    let Some((pbd_b, ltg_b, _)) = course("gari") else { return };
    let ltg = tricky_data::ltg::Ltg::parse(&ltg_b).unwrap();
    let pbd = tricky_data::pbd::Pbd::parse(&pbd_b).unwrap();
    let (mut n, mut inside) = (0, 0);
    for (off, g) in ltg.tgroups.iter().flatten() {
        for node in &g.inodes {
            let at = *off as usize + node.offsets[2] as usize;
            for k in 0..node.counts[3].max(0) as usize {
                let w = u32::from_le_bytes(ltg_b[at + 4 * k..at + 4 * k + 4].try_into().unwrap());
                n += 1;
                let seg = pbd.segments.get(w as usize).expect("a segment index");
                let f = |i: usize| f32::from_bits(seg[i]);
                let (lo, hi) = ([f(0x17), f(0x18), f(0x19)], [f(0x1a), f(0x1b), f(0x1c)]);
                let b = node.bbox;
                inside += (0..3).all(|i| lo[i] <= b.max[i] + 1.0 && b.min[i] - 1.0 <= hi[i]) as u32;
            }
        }
    }
    assert!(n > 100);
    assert_eq!(inside, n, "every listed segment's box meets its INode's");
}

#[test]
fn the_grid_finds_its_curves_points() {
    for name in ["gari", "elysium"] {
        let Some((pbd_b, ltg_b, ssf_b)) = course(name) else { return };
        let c = tricky_game::course_ground::CourseGround::new(&pbd_b, &ltg_b, &ssf_b).expect("the course");
        let pbd = tricky_data::pbd::Pbd::parse(&pbd_b).unwrap();
        // every rail switched on, as their rail nodes do when they come on
        let mut rails = c.rails.clone();
        for k in 0..pbd.splines.len() as u32 {
            rails.set_live(k, true);
        }
        let (mut tried, mut found) = (0, 0);
        for seg in &pbd.segments {
            let s = tricky_data::pbd::SegmentRec(*seg);
            if pbd.splines.get(s.spline() as usize).is_none() {
                continue;
            }
            for t in [0.1f32, 0.5, 0.9] {
                let f = |i: usize| f32::from_bits(seg[i]);
                let p: [f32; 4] = std::array::from_fn(|k| f(k) * t * t * t + f(4 + k) * t * t + f(8 + k) * t + f(12 + k));
                tried += 1;
                if let Some(h) = rails.closest_point([p[0], p[1], p[2], 1.0]) {
                    let d = ((h.point[0] - p[0]).powi(2) + (h.point[1] - p[1]).powi(2) + (h.point[2] - p[2]).powi(2)).sqrt();
                    if d < 2.0 {
                        found += 1;
                    }
                }
            }
        }
        println!("{name}: {found} of {tried} points on the curves found within 2 cm");
        assert!(tried > 50, "{name}: rails");
        assert_eq!(found, tried, "{name}: every point found");
    }
}
