//! Snow thrown up by the boards: more when carving or braking, a burst on landing.

use crate::rider::Rider;
use crate::{ui, Mode, Opponents, RiderRes, SmoothDt};
use bevy::prelude::*;

const COUNT: usize = 360;

#[derive(Clone, Copy, Default)]
struct Flake { pos: Vec3, vel: Vec3, life: f32, max: f32, size: f32 }
#[derive(Resource)]
pub struct Spray { flakes: Vec<Flake>, next: usize, seed: u32, owed: Vec<f32>, was_air: Vec<f32> }
#[derive(Component)]
pub struct FlakeVisual(usize);

impl Spray {
    fn rand(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.seed >> 8) as f32 / 16777216.0
    }
    fn emit(&mut self, pos: Vec3, vel: Vec3, size: f32) {
        let life = 0.35 + self.rand() * 0.5;
        let i = self.next;
        self.flakes[i] = Flake { pos, vel, life, max: life, size };
        self.next = (i + 1) % COUNT;
    }
    fn from_rider(&mut self, slot: usize, r: &Rider, dt: f32) {
        if self.owed.len() <= slot { self.owed.resize(slot + 1, 0.0); self.was_air.resize(slot + 1, 0.0); }
        let speed = r.vel.length();
        let fwd = r.forward();
        let side = r.normal.cross(fwd).normalize_or_zero();
        if !r.grounded {
            self.was_air[slot] += dt;
            return;
        }
        // touching down throws a ring of snow
        if self.was_air[slot] > 0.35 {
            for _ in 0..((self.was_air[slot] * 22.0) as usize).min(40) {
                let a = self.rand() * std::f32::consts::TAU;
                let out = (fwd * a.cos() + side * a.sin()) * (1.5 + self.rand() * 3.0);
                let up = r.normal * (1.0 + self.rand() * 2.5);
                let size = 0.07 + self.rand() * 0.06;
                self.emit(r.pos + r.normal * 0.1, r.vel * 0.5 + out + up, size);
            }
        }
        self.was_air[slot] = 0.0;
        if speed < 3.0 { return; }
        let carve = r.input.steer.abs();
        let brake = if r.input.brake { 1.0 } else { 0.0 };
        self.owed[slot] += dt * (speed * 0.5 + speed * 3.5 * carve + speed * 6.0 * brake).min(140.0);
        while self.owed[slot] >= 1.0 {
            self.owed[slot] -= 1.0;
            // out from the edge that is digging in, and back from the tail
            let edge = side * r.input.steer.signum() * if carve > 0.1 { 1.0 } else { self.rand() - 0.5 };
            let at = r.pos - fwd * (0.2 + self.rand() * 0.6) + edge * 0.12 + r.normal * 0.05;
            let throw = edge * speed * (0.10 + 0.15 * carve) * (0.5 + self.rand()) + r.normal * (0.6 + self.rand() * (1.2 + 2.0 * carve + 2.0 * brake)) + fwd * brake * speed * 0.3 * self.rand();
            let size = 0.04 + self.rand() * 0.05 + 0.04 * brake;
            self.emit(at, r.vel * 0.35 + throw, size);
        }
    }
}

pub fn setup_spray(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let mesh = meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap());
    let mat = materials.add(StandardMaterial { base_color: Color::srgba(1.0, 1.0, 1.0, 0.85), unlit: true, alpha_mode: AlphaMode::Blend, ..default() });
    for i in 0..COUNT {
        commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(mat.clone()), Transform::from_scale(Vec3::ZERO), Visibility::Hidden, FlakeVisual(i)));
    }
    commands.insert_resource(Spray { flakes: vec![Flake::default(); COUNT], next: 0, seed: 12345, owed: Vec::new(), was_air: Vec::new() });
}

pub fn spray(
    time: Res<SmoothDt>, rider: Res<RiderRes>, opponents: Res<Opponents>, game: Res<ui::Game>, mode: Res<Mode>,
    mut spray: ResMut<Spray>, mut q: Query<(&FlakeVisual, &mut Transform, &mut Visibility)>,
) {
    if game.screen == ui::Screen::Paused { return; }
    let dt = time.dt.min(0.05);
    if *mode == Mode::Ride && game.screen != ui::Screen::Menu {
        spray.from_rider(0, &rider.0, dt);
        for (i, o) in opponents.0.iter().enumerate() { spray.from_rider(i + 1, &o.rider, dt); }
    }
    if std::env::var("TRICKY_SPRAYTEST").is_ok() {
        // a rider carving hard on the spot, to look at the spray
        let mut r = rider.0.clone();
        r.grounded = true;
        r.normal = Vec3::Y;
        r.vel = crate::rider::heading(r.yaw) * 4.0;
        r.input.steer = 1.0;
        spray.from_rider(0, &r, dt * 5.0);
    }
    for f in spray.flakes.iter_mut() {
        if f.life <= 0.0 { continue; }
        f.life -= dt;
        f.vel.y -= 9.0 * dt;
        f.vel *= 1.0 - (1.5 * dt).min(1.0);
        f.pos += f.vel * dt;
    }
    for (v, mut tf, mut vis) in &mut q {
        let f = &spray.flakes[v.0];
        if f.life > 0.0 {
            tf.translation = f.pos;
            tf.scale = Vec3::splat(f.size * (0.4 + 0.6 * f.life / f.max));
            if *vis != Visibility::Visible { *vis = Visibility::Visible; }
        } else if *vis != Visibility::Hidden { *vis = Visibility::Hidden; }
    }
}
