//! The riding physics on its own, without Bevy or a level (row F11a): the numbers the port notes give.

use glam::Vec3;
use tricky_game::collide::CollisionWorld;
use tricky_game::rails::Rails;
use tricky_game::rider::{wrap, Input, Rider, GRAVITY_FALLING};

#[test]
fn falls_with_the_games_gravity() {
    // nothing to land on: one second of free fall from rest at 60 ticks a second
    let mut world = CollisionWorld::default();
    world.min_y = -1.0e6;
    let rails = Rails::default();
    let mut r = Rider::new(Vec3::new(0.0, 100.0, 0.0), 0.0);
    r.grounded = false;
    r.vel = Vec3::ZERO;
    for _ in 0..60 {
        r.step(&world, &rails, Input::default(), 1.0 / 60.0);
    }
    // −1900.84 cm/s² falling (Air_IntegrateRK4 0x12b340), metres here
    assert!((r.vel.y + GRAVITY_FALLING).abs() < 0.2, "vy after 1 s: {}", r.vel.y);
    assert!((r.pos.y - (100.0 - GRAVITY_FALLING / 2.0)).abs() < 0.5, "y after 1 s: {}", r.pos.y);
    assert!(!r.grounded);
}

#[test]
fn angles_wrap_to_plus_minus_pi() {
    use std::f32::consts::PI;
    assert!((wrap(3.0 * PI).abs() - PI).abs() < 1e-5);
    assert!((wrap(PI / 2.0) - PI / 2.0).abs() < 1e-6);
    assert!((wrap(-PI / 2.0 - 2.0 * PI) + PI / 2.0).abs() < 1e-5);
}

/// A long slope (a quad of two triangles) falling along −z, which is where a rider at yaw 0 faces: 20 m wide, from
/// z = 0 down to z = −`len`, dropping `drop` over it.
fn slope(len: f32, drop: f32, surface: u8) -> CollisionWorld {
    let mut w = CollisionWorld::default();
    let (a, b) = (10.0, len);
    // y falls as z falls
    let y = |z: f32| drop * (z / b);
    let q = |x: f32, z: f32| Vec3::new(x, y(z), z);
    w.add_ex([q(-a, 0.0), q(a, 0.0), q(-a, -b)], None, true, surface, false);
    w.add_ex([q(a, 0.0), q(a, -b), q(-a, -b)], None, true, surface, false);
    w
}

fn ride(r: &mut Rider, world: &CollisionWorld, ticks: usize, input: Input) {
    let rails = Rails::default();
    for _ in 0..ticks {
        r.step(world, &rails, input, 1.0 / 60.0);
    }
}

#[test]
fn rides_a_slope_on_the_games_ground_tick() {
    // 100 m long, falling 20 m: it stays on the snow, speeds up, and the game-side rider of the exact tick is kept
    let world = slope(100.0, 20.0, 1);
    let mut r = Rider::new(Vec3::new(0.0, 0.0, -1.0), 0.0);
    r.grounded = true;
    r.surface = 1;
    r.normal = Vec3::Y;
    ride(&mut r, &world, 120, Input::default());
    assert!(r.pos.is_finite() && r.vel.is_finite() && r.yaw.is_finite(), "{:?} {:?}", r.pos, r.vel);
    assert!(r.grounded, "left the snow at {:?}", r.pos);
    assert!(r.gm.is_some(), "the ground tick keeps its game-side rider");
    assert!(r.vel.z < -5.0, "no speed gained: {:?}", r.vel);
    assert!(r.vel.length() < 30.0, "the speed limit: {:?}", r.vel);
    // on the snow, along the surface (the slope falls 0.2 per metre)
    let on = 0.2 * r.pos.z;
    assert!((r.pos.y - on).abs() < 0.5, "y {} against the snow {}", r.pos.y, on);
    assert!(r.yaw.abs() < 0.1, "still pointing downhill: yaw {}", r.yaw);
}

#[test]
fn rides_past_the_end_of_the_snow_into_the_air() {
    let world = slope(30.0, 6.0, 1);
    let mut r = Rider::new(Vec3::new(0.0, 0.0, -1.0), 0.0);
    r.grounded = true;
    r.surface = 1;
    r.normal = Vec3::Y;
    r.vel = Vec3::new(0.0, 0.0, -20.0);
    // (a rider that falls out of the world is put back on the snow, so look for the moment it leaves it)
    let rails = Rails::default();
    let mut left = false;
    for _ in 0..120 {
        r.step(&world, &rails, Input::default(), 1.0 / 60.0);
        assert!(r.pos.is_finite() && r.vel.is_finite());
        left |= !r.grounded;
    }
    assert!(left, "never left the snow, now at {:?}", r.pos);
}

#[test]
fn a_crash_surface_puts_the_rider_down() {
    // surface 6: `GroundMotion_Update` calls `Boarder_Wipeout`
    let world = slope(60.0, 8.0, 6);
    let mut r = Rider::new(Vec3::new(0.0, 0.0, -1.0), 0.0);
    r.grounded = true;
    r.surface = 6;
    r.normal = Vec3::Y;
    r.vel = Vec3::new(0.0, 0.0, -10.0);
    ride(&mut r, &world, 30, Input::default());
    assert!(r.crashed > 0.0 || r.wipe.is_some(), "no wipe-out on surface 6");
    assert!(r.crash_spray.is_some(), "the crash sprays from the crash point");
}

#[test]
fn steering_turns_the_board_on_the_snow() {
    let world = slope(200.0, 30.0, 1);
    let mut left = Rider::new(Vec3::new(0.0, 0.0, -1.0), 0.0);
    left.grounded = true;
    left.surface = 1;
    left.normal = Vec3::Y;
    left.vel = Vec3::new(0.0, 0.0, -10.0);
    let mut right = left.clone();
    ride(&mut left, &world, 90, Input { steer: -1.0, ..Input::default() });
    ride(&mut right, &world, 90, Input { steer: 1.0, ..Input::default() });
    // port steer: +1 right. Facing −z, right is +x
    assert!(right.pos.x > left.pos.x + 0.5, "left x {} right x {}", left.pos.x, right.pos.x);
}

#[test]
fn riders_carry_the_games_stat_bytes() {
    // row G20a: `Stats::of` leaves the 16 bytes `Career_SetupPlayerRider` 0x16a9d8 would, from the rider's base bytes,
    // the board's bonus and a level per group (ours: the training fraction between the start and the cap)
    use tricky_game::rider::Stats;
    use tricky_game::stats::{self, BASE, CAPS, CUBIC, EDGING, GRIP, JUMP, SPEED};
    use tricky_game::trickdata::{BOARDS, RIDERS};
    for (i, d) in RIDERS.iter().enumerate() {
        let board = &BOARDS[i][0];
        for t in [0.0f32, 1.0] {
            let b = Stats::of(d, t, board).bytes;
            let level = |v: (u8, u8)| if t == 0.0 { v.0 } else { v.1 };
            let want = stats::player_bytes(&BASE[i], &CAPS[i], board.bonus, [level(d.edging), level(d.speed), level(d.stability), level(d.tricks)], false);
            assert_eq!(b, want, "{} at training {t}", d.name);
            // grip is the edging group's second byte and the cubic term the speed group's: one value each
            assert_eq!((b[GRIP], b[CUBIC]), (b[EDGING], b[SPEED]), "{}", d.name);
            // the jump is the rider's base byte in the game's integer scaling: 70 → 178, not our rounded 179
            assert_eq!(b[JUMP] as i32, d.jump as i32 * 255 / 100, "{}", d.name);
        }
    }
}

/// A strip of snow (two triangles) from z = `z0` to `z1` (`z0 > z1`), 20 m wide, at heights `y0` and `y1`.
fn strip(w: &mut CollisionWorld, z0: f32, y0: f32, z1: f32, y1: f32, surface: u8) {
    let q = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
    w.add_ex([q(-10.0, y0, z0), q(10.0, y0, z0), q(-10.0, y1, z1)], None, true, surface, false);
    w.add_ex([q(10.0, y0, z0), q(10.0, y1, z1), q(-10.0, y1, z1)], None, true, surface, false);
}

#[test]
fn a_lip_takes_the_rider_off_without_a_crash_and_the_landing_is_back_on_the_snow() {
    // row E8l: the exact ground tick on the game's own take-off (`Jump_ApplyImpulse`'s lip branch runs on every
    // take-off): a downhill run-in, an 8 m gap with the snow 8 m lower, then the landing slope. The rider leaves
    // the snow at the lip, is not put down there, lands on the lower snow and keeps riding.
    let mut world = CollisionWorld::default();
    world.min_y = -1.0e6;
    strip(&mut world, 0.0, 0.0, -40.0, -8.0, 1); // run-in, falling 0.2 per metre
    strip(&mut world, -48.0, -16.0, -140.0, -34.4, 1); // landing, falling 0.2 per metre
    let mut r = Rider::new(Vec3::new(0.0, 0.0, -1.0), 0.0);
    r.grounded = true;
    r.surface = 1;
    r.normal = Vec3::Y;
    r.vel = Vec3::new(0.0, 0.0, -18.0);
    let rails = Rails::default();
    let (mut left_at, mut landed_at, mut crashed_at_lip) = (None, None, false);
    for i in 0..360 {
        r.step(&world, &rails, Input::default(), 1.0 / 60.0);
        assert!(r.pos.is_finite() && r.vel.is_finite(), "tick {i}: {:?} {:?}", r.pos, r.vel);
        if left_at.is_none() && !r.grounded {
            left_at = Some(i);
            crashed_at_lip = r.crashed > 0.0;
        }
        if left_at.is_some() && landed_at.is_none() && r.grounded { landed_at = Some(i); }
    }
    assert!(left_at.is_some(), "never left the snow, now at {:?}", r.pos);
    assert!(!crashed_at_lip, "put down at the lip: cause {}", r.cause);
    assert!(landed_at.is_some(), "never landed, now at {:?}", r.pos);
    assert!(r.crashed <= 0.0, "down after the landing: cause {}, at {:?}", r.cause, r.pos);
    assert!(r.pos.z < -48.0 && (r.pos.y - (-16.0 + 0.2 * (r.pos.z + 48.0))).abs() < 1.0, "not on the landing snow: {:?}", r.pos);
    assert!(r.vel.length() > 5.0, "stalled after the landing: {:?}", r.vel);
}
