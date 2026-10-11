//! The rider scores through the score object (board row G2d): grab names reach the game's clip ids, and a jump
//! with a spin and a grab, landed, is named and paid by the game's own functions.

use glam::Vec3;
use tricky_game::collide::CollisionWorld;
use tricky_game::rails::Rails;
use tricky_game::rider::{grab_clip, Input, Rider};
use tricky_game::score::{grab_of_clip, grab_rate};
use tricky_game::trickdata::RIDERS;

#[test]
fn every_grab_tweak_and_uber_has_a_clip() {
    assert_eq!(grab_clip("Indy"), 0x27e);
    assert_eq!(grab_rate(grab_clip("Indy")), Some(1.0));
    assert_eq!(grab_rate(grab_clip("Nosegrab")), Some(1.75));
    for d in RIDERS.iter() {
        for row in d.rows.iter() {
            for name in [row.grab, row.tweak] {
                let c = grab_clip(name);
                assert!(c != 0 && !grab_of_clip(c).1, "{}: {name}", d.name);
            }
            if let Some(u) = tricky_game::trickdata::uber_name(d.name, row.grab) {
                let c = grab_clip(u);
                assert!(c != 0 && grab_of_clip(c).1, "{}: uber {u}", d.name);
            }
        }
    }
}

#[test]
fn a_spin_and_a_grab_landed_score() {
    let mut world = CollisionWorld::default();
    world.min_y = -1.0e6;
    let rails = Rails::default();
    let mut r = Rider::new(Vec3::new(0.0, 500.0, 0.0), 0.0);
    r.grounded = false;
    r.vel = Vec3::ZERO;
    // a chain begins as for a jump (`Score_StartTrickChain` from snow)
    let (share, _) = r.sc.start_trick_chain(0, 0, 0, 0, true);
    assert_eq!(share, 0.0);
    r.spin_ax = tricky_game::rider::Axis::start(11.517 * r.stats.spin_scale());
    for k in 0..150 {
        let input = Input { steer: if k < 40 { 1.0 } else { 0.0 }, grab: if (30..90).contains(&k) { 1 } else { 0 }, ..Input::default() };
        r.step(&world, &rails, input, 1.0 / 60.0);
    }
    assert!(r.sc.spin_scored > 0, "spin scored: {:?}", r.sc);
    assert_ne!(r.sc.grabs[0], 0, "a grab in the list: {:?}", r.sc.grabs);
    let units = r.sc.units;
    r.flip = 0.0;
    r.land(None);
    assert!(r.score > 0, "points for {units} units: {}", r.score);
    assert!(r.last_trick.contains('+'), "shown: {:?}", r.last_trick);
    println!("{} ({} points, meter {:.3})", r.last_trick, r.score, r.boost);
}
