//! The air's rail capture on the course's own rail grid (board row G20c2c1b1; TRICKY_DISC for gari's `.pbd`, `.ltg`
//! and `.ssf`, skipped without it): `Rider::step` runs `Rail_TryCapture` (`ground_tick::rail_try_capture`) on
//! `CourseGround::rails` from the board bone's frame. A rider in the air just over one of gari's rails, falling onto it as it flies along
//! it, is caught (kind 1, along); turned across it, kind 3 or 4; turned round, kind 2; 40 cm over it, or with the
//! rail switched off, not. Then the slide on the game's rail tick.

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
    // the board bone sits at the rider's place here: `over` cm over the rail
    let pos = port([p[0], p[1], p[2] + over, 1.0]);
    let mut r = Rider::new(pos, tricky_game::rider::wrap(heading + turn - std::f32::consts::FRAC_PI_2));
    r.grounded = false;
    r.air_time = 0.5;
    r.vel = port([t[0] * 1500.0, t[1] * 1500.0, t[2] * 1500.0 - 300.0, 0.0]);
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
    let along = fly(&world, &rails, &seg, 10.0, 0.0);
    let across = fly(&world, &rails, &seg, 10.0, pi / 2.0);
    let back = fly(&world, &rails, &seg, 10.0, pi);
    let high = fly(&world, &rails, &seg, 40.0, 0.0);
    println!("spline {spline}: along {along:?}, across {across:?}, turned round {back:?}, 40 cm over {high:?}");
    assert_eq!(along, Some(1));
    assert!(matches!(across, Some(3) | Some(4)), "{across:?}");
    assert_eq!(back, Some(2));
    assert_eq!(high, None);
    // the velocity: along the tangent at the speed (at most the speed along it, or 555.55554 if that is more), its z
    // × 0.1
    let (_, v, t) = fly_vel(&world, &rails, &seg, 10.0, 0.0).unwrap();
    let speed = { let v = [t[0] * 1500.0, t[1] * 1500.0, t[2] * 1500.0 - 300.0]; (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() };
    let along = 1500.0 * (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]) - 300.0 * t[2];
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
    assert_eq!(fly(&world, &rails, &seg, 10.0, 0.0), None);
}

/// The slide on the grid (row G20c2c3): after the capture the rail tick is `RailSlideMotion_Update` itself. With
/// nothing held the rider stays on gari's rail for 20 ticks, moving along it, the board bone held on the rail and the
/// normal up; held steer pushes it off the rail.
#[test]
fn the_slide_runs_on_the_game_rail_tick() {
    let Some((cg, pbd)) = course() else { return };
    let (_, seg, rails) = a_rail(&pbd);
    let mut world = CollisionWorld::default();
    world.min_y = -1e9;
    world.course = Some(Arc::new(cg));
    let (p0, d0) = cubic(&seg, 0.3);
    let n0 = (d0[0] * d0[0] + d0[1] * d0[1] + d0[2] * d0[2]).sqrt();
    let t0 = [d0[0] / n0, d0[1] / n0, d0[2] / n0, 0.0];
    let ride = |steer: f32, ticks: usize| -> (Rider, usize, Vec3) {
        let heading = tricky_game::jump::atan2_inline(t0[1], t0[0]);
        let mut r = Rider::new(port([p0[0], p0[1], p0[2] + 10.0, 1.0]), tricky_game::rider::wrap(heading - std::f32::consts::FRAC_PI_2));
        r.grounded = false;
        r.air_time = 0.5;
        r.vel = port([t0[0] * 1200.0, t0[1] * 1200.0, t0[2] * 1200.0 - 300.0, 0.0]);
        r.step(&world, &rails, Input::default(), 1.0 / 60.0);
        assert!(r.rail.is_some() && r.rail_grid, "caught on the grid");
        let start = r.pos;
        let mut on = 0;
        for _ in 0..ticks {
            r.step(&world, &rails, Input { steer, ..Input::default() }, 1.0 / 60.0);
            if r.rail.is_none() {
                break;
            }
            on += 1;
        }
        (r, on, start)
    };
    let (r, on, start) = ride(0.0, 20);
    assert_eq!(on, 20, "stayed on the rail");
    // the motion's own state: the blend 1 for the first 0.6 s, the time on the rail counted
    assert_eq!(r.rail_blend, 1.0);
    assert!((r.rail_time - 20.0 / 60.0).abs() < 1e-4, "time on the rail {}", r.rail_time);
    let rw = world.course.as_ref().unwrap().rails.lock().unwrap();
    let f = r.board_bone_frame();
    let hit = rw.closest_point(f[3]).expect("the rail");
    let dz = f[3][2] - hit.point[2];
    let flat = ((f[3][0] - hit.point[0]).powi(2) + (f[3][1] - hit.point[1]).powi(2)).sqrt();
    // the board's attitude turned to the rail: its forward (the bone's x row) up the rail's slope
    let fwd = f[0];
    let travelled = (r.pos - start).length() * 100.0;
    println!("after 20 ticks: board {dz:.2} cm over the rail, {flat:.2} cm off it sideways, speed {:.0} cm/s, {travelled:.0} cm travelled, forward z {:.3} (the rail's {:.3}), normal {:?}", r.vel.length() * 100.0, fwd[2], hit.tangent[2], r.normal);
    assert!(dz.abs() < 1.0, "the board bone held on the rail: {dz}");
    assert!(flat < 2.0, "beside the rail: {flat}");
    assert!((fwd[2] - hit.tangent[2]).abs() < 0.02, "the attitude along the rail: forward z {} against {}", fwd[2], hit.tangent[2]);
    assert!(r.normal.y > 0.9, "the normal up: {:?}", r.normal);
    // a tick is 1/60 s: about 20/60 s of the speed along the rail
    let expect = r.vel.length() * 100.0 * 20.0 / 60.0;
    assert!((travelled - expect).abs() < 0.08 * expect, "travelled {travelled} cm, about {expect}");
    drop(rw);
    let (_, held, _) = ride(1.0, 600);
    let (_, free, _) = ride(0.0, 600);
    println!("held steer: off after {held} ticks; nothing held: {free}");
    assert!(held + 10 < free, "held steer takes it off sooner");
}
