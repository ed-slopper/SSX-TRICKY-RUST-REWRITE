//! The air's rail capture on the course's own rail grid (board row G20c2c1b1; TRICKY_DISC for gari's `.pbd`, `.ltg`
//! and `.ssf`, skipped without it): `Rider::step` runs `Rail_TryCapture` (`ground_tick::rail_try_capture`) on
//! `CourseGround::rails` from the board bone's frame. A rider in the air just over one of gari's rails, flying along
//! it, is caught (kind 1, along); turned across it, kind 3 or 4; turned round, kind 2; a metre over it, or with the
//! rail switched off, not.

use glam::Vec3;
use std::sync::Arc;
use tricky_game::collide::CollisionWorld;
use tricky_game::course_ground::CourseGround;
use tricky_game::rails::{Rail, Rails};
use tricky_game::rider::{Input, Rider};

fn course() -> Option<(CourseGround, tricky_data::pbd::Pbd)> {
    let disc = std::env::var("TRICKY_DISC").ok()?;
    let mut files = tricky_data::big::Files::new(tricky_data::disc::Disc::open(&disc).ok()?);
    files.mount("data/models/gari.big").ok()?;
    let mut get = |ext: &str| files.load(&format!("|data/models/gari.{ext}")).ok().flatten();
    let (pbd, ltg, ssf) = (get("pbd")?, get("ltg")?, get("ssf")?);
    Some((CourseGround::new(&pbd, &ltg, &ssf)?, tricky_data::pbd::Pbd::parse(&pbd)?))
}

type V4 = [f32; 4];
fn cubic(seg: &[u32], t: f32) -> (V4, V4) {
    let f = |i: usize| f32::from_bits(seg[i]);
    let p = std::array::from_fn(|k| f(k) * t * t * t + f(4 + k) * t * t + f(8 + k) * t + f(12 + k));
    let d = std::array::from_fn(|k| 3.0 * f(k) * t * t + 2.0 * f(4 + k) * t + f(8 + k));
    (p, d)
}
fn port(v: V4) -> Vec3 {
    Vec3::new(v[0], v[2], -v[1]) * 0.01
}

/// A long, nearly level rail segment (its spline, the segment) and the port's polyline of it
fn a_rail(pbd: &tricky_data::pbd::Pbd) -> (u32, [u32; 0x20], Rails) {
    for seg in &pbd.segments {
        let s = tricky_data::pbd::SegmentRec(*seg);
        let (a, b) = (cubic(seg, 0.0).0, cubic(seg, 1.0).0);
        let len = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        if 600.0 < len && (b[2] - a[2]).abs() < 0.1 * len {
            let pts: Vec<Vec3> = (0..=40).map(|k| port(cubic(seg, k as f32 / 40.0).0)).collect();
            let mut rail = Rail::new(pts).unwrap();
            rail.spline = s.spline() as usize;
            let mut seg2 = [0u32; 0x20];
            seg2[..seg.len().min(0x20)].copy_from_slice(&seg[..seg.len().min(0x20)]);
            return (s.spline(), seg2, Rails(vec![rail], Default::default()));
        }
    }
    panic!("no long level rail");
}

/// One tick of a rider flying along the rail at its middle, `over` cm above it, the board turned `turn` from the
/// travel: what it was caught as
fn fly(world: &CollisionWorld, rails: &Rails, seg: &[u32], over: f32, turn: f32) -> Option<u32> {
    fly_vel(world, rails, seg, over, turn).map(|c| c.0)
}

/// The same, with the velocity the capture left (cm/s, the game's axes)
fn fly_vel(world: &CollisionWorld, rails: &Rails, seg: &[u32], over: f32, turn: f32) -> Option<(u32, V4, V4)> {
    let (p, d) = cubic(seg, 0.5);
    let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    let t = [d[0] / n, d[1] / n, d[2] / n, 0.0];
    let heading = tricky_game::jump::atan2_inline(t[1], t[0]);
    // the board bone sits at the rider's place here: 30 cm over the rail plus `over`
    let pos = port([p[0], p[1], p[2] + 30.0 + over, 1.0]);
    let mut r = Rider::new(pos, tricky_game::rider::wrap(heading + turn - std::f32::consts::FRAC_PI_2));
    r.grounded = false;
    r.air_time = 0.5;
    r.vel = port([t[0] * 1500.0, t[1] * 1500.0, t[2] * 1500.0 - 50.0, 0.0]);
    r.step(world, rails, Input::default(), 1.0 / 60.0);
    let v = r.vel * 100.0;
    r.rail.map(|_| (r.rail_kind, [v.x, -v.z, v.y, 0.0], t))
}

#[test]
fn the_air_catches_gari_rails_as_the_game_does() {
    let Some((cg, pbd)) = course() else { return };
    let (spline, seg, rails) = a_rail(&pbd);
    let mut world = CollisionWorld::default();
    world.min_y = -1e9;
    world.course = Some(Arc::new(cg));
    let pi = std::f32::consts::PI;
    let along = fly(&world, &rails, &seg, 0.0, 0.0);
    let across = fly(&world, &rails, &seg, 0.0, pi / 2.0);
    let back = fly(&world, &rails, &seg, 0.0, pi);
    let high = fly(&world, &rails, &seg, 100.0, 0.0);
    println!("spline {spline}: along {along:?}, across {across:?}, turned round {back:?}, a metre over {high:?}");
    assert_eq!(along, Some(1));
    assert!(matches!(across, Some(3) | Some(4)), "{across:?}");
    assert_eq!(back, Some(2));
    assert_eq!(high, None);
    // the velocity: along the tangent at the speed (at most the speed along it, or 555.55554 if that is more), its z
    // × 0.1
    let (_, v, t) = fly_vel(&world, &rails, &seg, 0.0, 0.0).unwrap();
    let speed = (1500.0f32 * 1500.0 + 50.0 * 50.0).sqrt();
    let along = 1500.0 * (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]) - 50.0 * t[2];
    let s = speed.min(along.max(555.55554));
    // (the tangent is the closest point's, a tick's travel on: the direction compared loosely, the speed and the z
    // scaling closely)
    let flat = (v[0] * v[0] + v[1] * v[1]).sqrt();
    let want_flat = s * (t[0] * t[0] + t[1] * t[1]).sqrt();
    assert!((flat - want_flat).abs() < 0.01 * want_flat, "speed along {flat}, wanted {want_flat} (a tick of the air's gravity and drag before)");
    assert!((v[2] - t[2] * s * 0.1).abs() < 0.5, "z {}, wanted {}", v[2], t[2] * s * 0.1);
    assert!((v[0] * t[0] + v[1] * t[1]) / flat > 0.999 * (t[0] * t[0] + t[1] * t[1]).sqrt(), "along the rail: {v:?}");
    // switched off: not caught
    world.course.as_ref().unwrap().set_rail_live(spline, false);
    assert_eq!(fly(&world, &rails, &seg, 0.0, 0.0), None);
}
