//! The riding physics on its own, without Bevy or a level (row F11a): the numbers the port notes give.

use glam::Vec3;
use tricky_game::collide::CollisionWorld;
use tricky_game::rails::{Rail, Rails};
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

/// A long rail along −z at y = 0 and a rider in the air just above it, flying down it at 18 m/s (row E8m).
fn rail_and_flyer() -> (Rails, Rider) {
    let rail = Rail::new(vec![Vec3::new(0.0, 0.0, 5.0), Vec3::new(0.0, 0.0, -40.0)]).unwrap();
    let rails = Rails(vec![rail], Default::default());
    let mut r = Rider::new(Vec3::new(0.0, 0.3, -9.5), 0.0);
    r.grounded = false;
    r.air_time = 0.5;
    r.vel = Vec3::new(0.0, 0.0, -18.0);
    (rails, r)
}

#[test]
fn a_rider_put_down_in_the_air_is_not_captured_by_a_rail() {
    // row E8m: `Rail_TryCapture` 0x125cb8 has two callers, `AirMotion_Update` 0x1089f0 and `GroundMotion_Update`
    // 0x10ae78. `Boarder_Wipeout` 0x11d830 switches the motion to 5 at once (`Boarder_SetMotion` 0x11f610 exits
    // the old motion and enters the new), and `WipeoutMotion_Update` 0x10c2f0 and `GetUpMotion_Update` 0x1102c8
    // never call it: a downed rider is not captured, and a capture never gets it up.
    let mut world = CollisionWorld::default();
    world.min_y = -1.0e6;
    // control: with nothing in the way the rider is locked onto the rail on the first tick
    let (rails, mut r) = rail_and_flyer();
    r.step(&world, &rails, Input::default(), 1.0 / 60.0);
    assert!(r.rail.is_some() && r.crashed <= 0.0, "the control rider was not captured: rail {:?} crashed {}", r.rail, r.crashed);

    // a wall across the line: the rider hits it in the air on the same tick that it is over the rail (an air wall:
    // `Boarder_WallImpactCheck`, a crash), so it is down and the rail must not take it
    let mut wall = CollisionWorld::default();
    wall.min_y = -1.0e6;
    let q = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
    wall.add_ex([q(-5.0, -5.0, -10.0), q(5.0, 10.0, -10.0), q(5.0, -5.0, -10.0)], None, false, 1, false);
    wall.add_ex([q(-5.0, -5.0, -10.0), q(-5.0, 10.0, -10.0), q(5.0, 10.0, -10.0)], None, false, 1, false);
    let (rails, mut r) = rail_and_flyer();
    r.step(&wall, &rails, Input::default(), 1.0 / 60.0);
    assert!(r.crashed > 0.0, "the wall did not put the rider down (cause {:?}, pos {:?})", r.cause, r.pos);
    assert!(r.rail.is_none(), "a downed rider was captured by the rail");
    // and it stays down: no rail, not up, over the next half second
    for i in 0..30 {
        r.step(&wall, &rails, Input::default(), 1.0 / 60.0);
        assert!(r.rail.is_none(), "tick {i}: captured while down");
        assert!(r.crashed > 0.0, "tick {i}: up again after {:.2} s down", r.down_t);
    }
}

/// The wall across the flyer's line (row E8o): a vertical wall at z = −10, 10 m wide, from −5 to 10 m high.
fn wall_across() -> CollisionWorld {
    let mut wall = CollisionWorld::default();
    wall.min_y = -1.0e6;
    let q = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
    wall.add_ex([q(-5.0, -5.0, -10.0), q(5.0, 10.0, -10.0), q(5.0, -5.0, -10.0)], None, false, 1, false);
    wall.add_ex([q(-5.0, -5.0, -10.0), q(-5.0, 10.0, -10.0), q(5.0, 10.0, -10.0)], None, false, 1, false);
    wall
}

#[test]
fn the_air_tick_captures_on_a_rail_before_the_wall_puts_the_rider_down() {
    // row E8o, proven in `AirMotion_Update` 0x108378: with no landing the tick runs `Rail_TryCapture` (call at 0x1089f0:
    // `Score_LandTrick` for the rail, `Boarder_SetMotion`(3), `Boarder_SetState`(0xf)) and does NOT return: next come
    // `Boarder_InstanceCollisions` (0x1089f8) and the wall ray (0x108a2c), whose `Boarder_Wipeout` 0x11d830 switches the
    // motion to 5 at once. So a rider that hits a wall on the tick it is over a rail was locked onto the rail (the
    // capture's `rail_kind` stands, the rail trick was scored) and is then down: no rail, crashed, "air wall".
    let wall = wall_across();
    let (rails, mut r) = rail_and_flyer();
    assert_eq!(r.rail_kind, 0);
    r.step(&wall, &rails, Input::default(), 1.0 / 60.0);
    assert!(r.rail_kind != 0, "the rail did not capture before the wall (rail_kind {})", r.rail_kind);
    assert!(r.crashed > 0.0 && r.cause == "air wall", "the wall did not put the captured rider down (cause {:?}, crashed {})", r.cause, r.crashed);
    assert!(r.rail.is_none(), "a rider put down by the wall is still on the rail");
    assert!(!r.grounded);
}

#[test]
fn a_landing_on_the_tick_is_before_both_the_rail_and_the_wall() {
    // row E8o: the down ray (0x108554) decides the landing first; a landed tick (`s6` cleared at 0x1089bc) skips
    // `Rail_TryCapture`, `Boarder_InstanceCollisions` and the wall ray altogether (branch at 0x1089e4 to 0x108e58). The
    // rider here is over the rail, has snow at its feet and a wall in front: it lands, is not captured, is not put down.
    let mut world = wall_across();
    let q = |x: f32, z: f32| Vec3::new(x, 0.0, z);
    world.add_ex([q(-10.0, 5.0), q(10.0, 5.0), q(-10.0, -40.0)], None, true, 1, false);
    world.add_ex([q(10.0, 5.0), q(10.0, -40.0), q(-10.0, -40.0)], None, true, 1, false);
    let (rails, mut r) = rail_and_flyer();
    // feet on the snow (the port's landing takes snow at or within 2 cm under the feet)
    r.pos.y = 0.0;
    r.step(&world, &rails, Input::default(), 1.0 / 60.0);
    assert!(r.grounded, "the rider did not land (pos {:?})", r.pos);
    assert!(r.rail.is_none() && r.rail_kind == 0, "a landed rider was captured by the rail");
    assert!(r.crashed <= 0.0, "a landed rider was put down by the air wall (cause {:?})", r.cause);
}

#[test]
fn a_grab_short_of_its_fourth_marker_refuses_the_rail_capture() {
    // row E8n: `Rail_TryCapture` 0x125cb8 refuses while the layer-0 clip is of type 0xe, 0xf or 0x10 (the grab, tweak and
    // uber clips, ids 0x278..=0x2db of the descriptor table 0x325fa8) and clip flag 4 is clear; the fall-recover clip
    // 0x224 is type 0 and never gates. The port's form of the test is `Rider::capture_gated` (a grab held, or let go
    // short of its fourth marker, or an uber trick not done): the rider flies on over the rail.
    let mut world = CollisionWorld::default();
    world.min_y = -1.0e6;
    // a grab still playing out, far short of its fourth marker (25 frames): not captured
    let (rails, mut r) = rail_and_flyer();
    r.grab_out = Some((1, 0.0));
    r.step(&world, &rails, Input::default(), 1.0 / 60.0);
    assert!(r.capture_gated(), "a grab short of its marker did not gate");
    assert!(r.rail.is_none() && r.rail_kind == 0, "captured with a grab clip short of flag 4");
    assert!(r.crashed <= 0.0 && !r.grounded);
    // an uber trick not done: not captured
    let (rails, mut r) = rail_and_flyer();
    r.uber = 1.0;
    r.uber_done = false;
    r.step(&world, &rails, Input::default(), 1.0 / 60.0);
    assert!(r.rail.is_none() && r.rail_kind == 0, "captured in an uber trick that is not done");
    // the grab past its fourth marker (flag 4 set): captured as without a grab
    let (rails, mut r) = rail_and_flyer();
    r.grab_out = Some((1, 1.0e3));
    assert!(!r.capture_gated());
    r.step(&world, &rails, Input::default(), 1.0 / 60.0);
    assert!(r.rail.is_some() && r.rail_kind != 0, "not captured with flag 4 set");
}

