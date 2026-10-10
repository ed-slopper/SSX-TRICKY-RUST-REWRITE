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
