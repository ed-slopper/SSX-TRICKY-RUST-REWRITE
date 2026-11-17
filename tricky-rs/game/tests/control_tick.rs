//! The ground's control states in `Rider::step` (row G1i1): cruise, prewind and jump on the game's pad word.

use glam::Vec3;
use tricky_game::collide::CollisionWorld;
use tricky_game::rails::Rails;
use tricky_game::rider::control_tick::{CRUISE, JUMP, PREWIND, SPIN};
use tricky_game::rider::{Input, Rider};

/// A long slope falling along −z, 20 m wide
fn slope(len: f32, drop: f32) -> CollisionWorld {
    let mut w = CollisionWorld::default();
    let y = |z: f32| drop * (z / len);
    let q = |x: f32, z: f32| Vec3::new(x, y(z), z);
    w.add_ex([q(-10.0, 0.0), q(10.0, 0.0), q(-10.0, -len)], None, true, 1, false);
    w.add_ex([q(10.0, 0.0), q(10.0, -len), q(-10.0, -len)], None, true, 1, false);
    w
}

fn rider() -> Rider {
    let mut r = Rider::new(Vec3::new(0.0, 0.0, -1.0), 0.0);
    r.grounded = true;
    r.surface = 1;
    r.normal = Vec3::Y;
    r.vel = Vec3::new(0.0, 0.0, -10.0);
    r
}

#[test]
fn the_jump_button_winds_up_and_jumps_through_the_games_states() {
    let world = slope(300.0, 40.0);
    let rails = Rails::default();
    let mut r = rider();
    let step = |r: &mut Rider, i: Input| r.step(&world, &rails, i, 1.0 / 60.0);
    for _ in 0..60 {
        step(&mut r, Input::default());
    }
    assert_eq!(r.ctl, CRUISE);
    // the button down: cruise brakes to 0, then the prewind crouches and winds up a spin to the right
    let hold = Input { jump: true, steer: 1.0, ..Input::default() };
    let mut states = vec![];
    for _ in 0..40 {
        step(&mut r, hold);
        states.push(r.ctl);
    }
    assert!(states.contains(&PREWIND), "{states:?}");
    assert!(r.grounded);
    assert!(0.9 < r.crouch_v, "crouched: {}", r.crouch_v);
    assert!(0.5 < r.wind, "wound up: {}", r.wind);
    assert!(0.0 < r.charge);
    // let go: the jump state lets the crouch out (13.3333 a second), then the impulse and the spin state
    let mut seen = vec![];
    for _ in 0..20 {
        step(&mut r, Input::default());
        seen.push(r.ctl);
        if !r.grounded {
            break;
        }
    }
    assert!(seen.contains(&JUMP), "{seen:?}");
    assert!(!r.grounded, "jumped: {seen:?}");
    assert_eq!(r.ctl, SPIN);
    assert!(0.5 < r.takeoff_wind.0, "the take-off's spin wind-up: {:?}", r.takeoff_wind);
    assert!(r.vel.y > 3.0, "the jump's impulse: {:?}", r.vel);
}

#[test]
fn a_tap_jumps_without_a_wind_up() {
    let world = slope(300.0, 40.0);
    let rails = Rails::default();
    let mut r = rider();
    for _ in 0..60 {
        r.step(&world, &rails, Input::default(), 1.0 / 60.0);
    }
    r.step(&world, &rails, Input { jump: true, ..Input::default() }, 1.0 / 60.0);
    let mut left = false;
    for _ in 0..30 {
        r.step(&world, &rails, Input::default(), 1.0 / 60.0);
        left |= !r.grounded;
    }
    assert!(left, "a tap jumps too (the smallest jump, 630.8786 cm/s)");
    assert_eq!(r.takeoff_wind, (0.0, 0.0));
}

#[test]
fn tuck_crouches_and_brake_brakes_in_cruise() {
    let world = slope(300.0, 40.0);
    let rails = Rails::default();
    let mut tuck = rider();
    let mut brake = rider();
    for _ in 0..30 {
        tuck.step(&world, &rails, Input { tuck: true, ..Input::default() }, 1.0 / 60.0);
        brake.step(&world, &rails, Input { brake: true, ..Input::default() }, 1.0 / 60.0);
    }
    assert_eq!((tuck.ctl, brake.ctl), (CRUISE, CRUISE));
    assert!(0.9 < tuck.crouch_v && tuck.brake_v == 0.0, "tuck: crouch {} brake {}", tuck.crouch_v, tuck.brake_v);
    assert!(0.9 < brake.brake_v && brake.crouch_v == 0.0, "brake: crouch {} brake {}", brake.crouch_v, brake.brake_v);
    assert!(tuck.vel.length() > brake.vel.length());
}


#[test]
fn the_button_map_binds_the_riders_trick_slots() {
    // `Pad_SetupVibration` 0x152770: combo k (easyA L2, easyB R1, ...) to the slot of row k at the character and the
    // board's kind
    use tricky_game::controls::{LSHIFT, LSHIFT2, RSHIFT};
    use tricky_game::trickdata::{BOARDS, RIDERS};
    for (ch, d) in RIDERS.iter().enumerate() {
        for b in &BOARDS[ch][..3] {
            let mut r = rider();
            r.data = d;
            r.stats.kind = b.kind;
            let m = r.button_map();
            let rows = &tricky_data::trick_slots::TRICK_SLOTS[ch][b.kind as usize];
            // the last combos bound win a shared slot, so look at the first three only where they differ
            for (k, combo) in [(0, LSHIFT2), (1, RSHIFT), (3, LSHIFT)] {
                let slot = rows[k][0] as usize;
                if rows.iter().skip(k + 1).all(|r| r[0] as usize != slot) {
                    assert_eq!(m.pressed[slot], combo, "{} kind {} row {k}", d.name, b.kind);
                }
            }
        }
    }
}
