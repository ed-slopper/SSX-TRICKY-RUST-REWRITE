//! The landing as the game decides it (board row G20c3; TRICKY_DISC for gari's `.pbd`, `.ltg` and `.ssf`, skipped
//! without it): a rider in the air over gari's snow with its attitude kept lands through `Landing_CheckAngles` and
//! `Landing_ChooseState` (`Rider::land_as_game`). Level and forwards: clean, no hard landing. Turned round: clean too
//! (soft and slow enough), riding switch. Nose up 50° (the roll under −π/4) or falling 30 m/s into the snow (over
//! 27.77): a hard landing (state 0xb) for its landing clip. An uber not yet done (a clip of type 0xe short of its
//! safe point): refused, down.

use glam::Vec3;
use std::sync::Arc;
use tricky_game::collide::CollisionWorld;
use tricky_game::course_ground::CourseGround;
use tricky_game::rails::Rails;
use tricky_game::rider::{Input, Rider};

type V4 = [f32; 4];

fn course() -> Option<(CourseGround, tricky_data::pbd::Pbd)> {
    let disc = std::env::var("TRICKY_DISC").ok()?;
    let mut files = tricky_data::big::Files::new(tricky_data::disc::Disc::open(&disc).ok()?);
    files.mount("data/models/gari.big").ok()?;
    let mut get = |ext: &str| files.load(&format!("|data/models/gari.{ext}")).ok().flatten();
    let (pbd, ltg, ssf) = (get("pbd")?, get("ltg")?, get("ssf")?);
    Some((CourseGround::new(&pbd, &ltg, &ssf)?, tricky_data::pbd::Pbd::parse(&pbd)?))
}
fn port(v: V4) -> Vec3 {
    Vec3::new(v[0], v[2], -v[1]) * 0.01
}

/// Flat snow (normal z over 0.97, a skiable surface) under one of the rails' points, 3 m down at most
fn flat_snow(cg: &CourseGround, pbd: &tricky_data::pbd::Pbd) -> V4 {
    for seg in pbd.segments.iter() {
        let f = |i: usize| f32::from_bits(seg[i]);
        for dx in [0.0f32, 400.0, -400.0] {
            let p = [f(12) + dx, f(13) + dx, f(14) + 100.0, 1.0];
            let mut q = tricky_game::worldline::WorldLine::new(0.0, p, [p[0], p[1], p[2] - 600.0, 1.0], 0);
            let (t, h) = cg.ray_cast(&mut q, false, None, &|_| false);
            if let Some(h) = h.filter(|_| 0.0 <= t) {
                if 0.97 < h.normal[2] && !matches!(h.surface, 0 | 6 | 10) {
                    return h.pos;
                }
            }
        }
    }
    panic!("no flat snow");
}

/// A rider 40 cm over the snow at `at`, flying along +x at 1000 cm/s and down at `fall`, its board turned `turn`
/// about the up axis from the travel and pitched nose down by `nose`: ticks until it is down on the snow or put down
fn drop(world: &CollisionWorld, at: V4, turn: f32, nose: f32, fall: f32) -> Rider {
    drop_full(world, at, turn, nose, fall, &|_| {})
}

/// [`drop`] level and forwards, with `set` done to the rider first
fn drop_with(world: &CollisionWorld, at: V4, set: impl Fn(&mut Rider)) -> Rider {
    drop_full(world, at, 0.0, 0.0, 200.0, &set)
}

fn drop_full(world: &CollisionWorld, at: V4, turn: f32, nose: f32, fall: f32, set: &dyn Fn(&mut Rider)) -> Rider {
    let heading = turn;
    let mut r = Rider::new(port([at[0], at[1], at[2] + 40.0, 1.0]), tricky_game::rider::wrap(heading - std::f32::consts::FRAC_PI_2));
    let (fx, fy) = (heading.cos(), heading.sin());
    let up = [nose.sin() * fx, nose.sin() * fy, nose.cos(), 0.0];
    r.att = Some((tricky_game::orient::placed_attitude(heading, Some(up)), r.yaw));
    r.grounded = false;
    r.air_time = 0.5;
    r.vel = port([1000.0, 0.0, -fall, 0.0]);
    set(&mut r);
    for _ in 0..30 {
        r.step(world, &Rails::default(), Input::default(), 1.0 / 60.0);
        if r.grounded || r.crashed > 0.0 {
            break;
        }
    }
    r
}

#[test]
fn landings_are_the_games() {
    let Some((cg, pbd)) = course() else { return };
    let at = flat_snow(&cg, &pbd);
    let mut world = CollisionWorld::default();
    world.min_y = -1e9;
    world.course = Some(Arc::new(cg));
    let pi = std::f32::consts::PI;
    let clean = drop(&world, at, 0.0, 0.0, 200.0);
    println!("clean: grounded {} crashed {} hard {:.2} switch {}", clean.grounded, clean.crashed, clean.hard_land, clean.switch);
    assert!(clean.grounded && clean.crashed <= 0.0 && clean.hard_land == 0.0, "a clean landing");
    let back = drop(&world, at, pi, 0.0, 200.0);
    println!("turned round: grounded {} crashed {} hard {:.2} switch {}", back.grounded, back.crashed, back.hard_land, back.switch);
    assert!(back.grounded && back.crashed <= 0.0 && back.switch != clean.switch && back.hard_land == 0.0, "backwards: switch, clean");
    let nose_up = drop(&world, at, 0.0, -50f32.to_radians(), 200.0);
    println!("nose up 50°: grounded {} crashed {} cause {} hard {:.2}", nose_up.grounded, nose_up.crashed, nose_up.cause, nose_up.hard_land);
    // (`Landing_ChooseState`: forwards, the roll under −π/4, firm: clip 0x242; held for its length)
    let len = |clip: u16| tricky_game::rider::wipeout::clip_frames(clip, nose_up.stats.kind as u8) / 30.0;
    assert!(nose_up.crashed <= 0.0 && nose_up.hard_land == len(0x242), "nose up: hard, clip 0x242's length ({})", len(0x242));
    let fast = drop(&world, at, 0.0, 0.0, 3000.0);
    println!("falling 30 m/s: grounded {} crashed {} hard {:.2}", fast.grounded, fast.crashed, fast.hard_land);
    // (forwards, soft, not firm: clip 0x23e)
    assert!(fast.crashed <= 0.0 && fast.hard_land == len(0x23e), "fast into the snow: hard, clip 0x23e's length ({})", len(0x23e));
    // an uber not yet done (a clip of type 0xe short of its safe point): the check refuses the landing
    let uber = drop_with(&world, at, |r| {
        r.uber = 1.0;
        r.uber_done = false;
    });
    println!("uber cut short: crashed {} cause {}", uber.crashed, uber.cause);
    assert!(uber.crashed > 0.0 && uber.cause == "grab", "an uber cut short: down");
}
