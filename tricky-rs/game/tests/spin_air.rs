//! The spin state on the game's code (board row G1a5b3b1): a jump with a spin wound up on the stick runs
//! `SpinState_Update` from the take-off (`Rider::spin_st`); the spin turns while the stick is held, completes to a
//! whole half turn once it is let go (the game's auto-complete), and the landing ends the state (`SpinControl_Exit`)
//! with the kept attitude.

use glam::Vec3;
use tricky_game::collide::CollisionWorld;
use tricky_game::rails::Rails;
use tricky_game::rider::{Input, Rider};

/// A long gentle slope falling along −z (20 m wide), surface 1 (snow)
fn slope() -> CollisionWorld {
    let mut w = CollisionWorld::default();
    let (a, b, drop) = (10.0, 400.0, 60.0);
    let y = |z: f32| drop * (z / b);
    let q = |x: f32, z: f32| Vec3::new(x, y(z), z);
    w.add_ex([q(-a, 0.0), q(a, 0.0), q(-a, -b)], None, true, 1, false);
    w.add_ex([q(a, 0.0), q(a, -b), q(-a, -b)], None, true, 1, false);
    w.min_y = -1.0e6;
    w
}

#[test]
fn a_spun_jump_runs_the_games_spin_state() {
    let world = slope();
    let rails = Rails::default();
    let mut r = Rider::new(Vec3::new(0.0, 0.05, -2.0), 0.0);
    r.grounded = true;
    r.vel = Vec3::new(0.0, 0.0, -12.0);
    let tick = |r: &mut Rider, i: Input| r.step(&world, &rails, i, 1.0 / 60.0);
    // ride a little, then crouch winding a spin up, then let the jump go
    for _ in 0..20 {
        tick(&mut r, Input::default());
    }
    for _ in 0..40 {
        tick(&mut r, Input { jump: true, wind: Some(1.0), ..Input::default() });
    }
    let mut air = 0;
    let mut spun_held = 0.0f32;
    let mut in_state = 0;
    for i in 0..240 {
        // keep the stick right for a third of a second in the air, then let go
        let steer = if air < 20 { 1.0 } else { 0.0 };
        tick(&mut r, Input { steer, ..Input::default() });
        if !r.grounded {
            air += 1;
            in_state += r.spin_st.is_some() as u32;
            if air == 20 {
                spun_held = r.spin;
            }
            if air == 1 {
                let st = r.spin_st.expect("the spin state");
                println!("take-off: predicted landing in {:.3} s, the spin rate {:.2} rad/s, mode {}", r.land_time, st.w_spin, st.mode);
                assert_eq!(st.mode, 0, "on the stick from the launch");
                assert!(0.7 < r.land_time && r.land_time < 1.2, "the predicted landing");
            }
            if air == 25 {
                assert_eq!(r.spin_st.unwrap().mode, 2, "completing once the stick is let go");
            }
        } else if 0 < air {
            println!("landed after {air} ticks (tick {i})");
            break;
        }
    }
    println!("in the air {air} ticks, the spin state {in_state} of them; spun {spun_held:.3} rad with the stick held; at the landing spin {:.3} flip {:.3}, state {:?}", r.spin, r.flip, r.spin_st.is_some());
    assert!(air > 30, "a jump");
    assert!(in_state as i32 >= air - 1, "the spin state from the take-off");
    // about the launch rate (4.79 rad/s, in its band) for the third of a second the stick was held
    assert!(1.2 < spun_held.abs() && spun_held.abs() < 1.7, "turning while the stick is held: {spun_held}");
    assert!(r.spin_st.is_none() && r.att.is_some(), "the landing ends the state, the attitude kept");
}
