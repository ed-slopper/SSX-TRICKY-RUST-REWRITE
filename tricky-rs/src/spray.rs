//! Snow thrown up by the boards, as the original's board effects: spray off the edge when carving
//! (per surface), a thin trail in the air and on rails, a fan when braking, a splash on hard landings.

use crate::rider::Rider;
use crate::{ui, Mode, Opponents, RiderRes, SmoothDt};
use bevy::prelude::*;

const COUNT: usize = 900;

/// Per-surface spray (`SurfaceTable_Init` 0x256188 rows 13..16): rate factor, flake size (cm), life (s).
fn surf(kind: u8) -> (f32, f32, f32, f32) {
    match kind {
        1 => (0.075, 3.0, 0.136, 0.243),
        2 => (0.159, 4.3, 0.30, 0.45),
        3 => (0.505, 7.85, 0.36, 0.58),
        5 => (0.200, 1.2, 0.10, 0.27),
        18 => (1.854, 2.56, 0.10, 0.27),
        6 | 10 | 17 => (1.0, 2.0, 0.4, 0.6),
        _ => (0.0, 2.0, 0.4, 0.6),
    }
}

#[derive(Clone, Copy, Default)]
struct Flake { pos: Vec3, vel: Vec3, life: f32, max: f32, size: f32, grav: f32, drag: f32 }
#[derive(Resource)]
pub struct Spray { flakes: Vec<Flake>, next: usize, seed: u32, owed: Vec<f32>, was_air: Vec<f32>, air_rate: Vec<f32>, fan: Vec<f32>, fall: Vec<f32> }
#[derive(Component)]
pub struct FlakeVisual(usize);

impl Spray {
    fn rand(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.seed >> 8) as f32 / 16777216.0
    }
    fn u(&mut self) -> f32 { self.rand() - 0.5 }
    fn emit(&mut self, pos: Vec3, vel: Vec3, size: f32, life: f32, grav: f32, drag: f32) {
        let i = self.next;
        self.flakes[i] = Flake { pos, vel, life, max: life, size, grav, drag };
        self.next = (i + 1) % COUNT;
    }
    /// The original's board effects (`Boarder_UpdateEffects` 0x1350c0, `Fx_SurfaceSpray` 0x1311b0,
    /// `Fx_BrakeFan_Update` 0x12d108, `Fx_LandingSplash` 0x12ccc8), in metres.
    fn from_rider(&mut self, slot: usize, r: &Rider, dt: f32) {
        if self.owed.len() <= slot {
            self.owed.resize(slot + 1, 0.0); self.was_air.resize(slot + 1, 0.0); self.air_rate.resize(slot + 1, 70.0);
            self.fan.resize(slot + 1, 0.0); self.fall.resize(slot + 1, 0.0);
        }
        let s = r.vel.length();
        let fwd = r.forward();
        let side = r.normal.cross(fwd).normalize_or_zero();
        let rate;
        let (mut size, mut l0, mut l1) = (3.0, 0.4, 0.6);
        let mut v0 = r.vel;
        let mut spread = (4.0, 0.5, 4.0);
        let mut grav = 1.0;
        if r.rail.is_some() {
            rate = if matches!(r.surface, 13 | 18 | 19) { 0.0 } else { 30.0 };
            self.was_air[slot] = 0.0;
        } else if !r.grounded {
            // in the air: 70 a second, falling off by x0.94667 a tick to 10
            self.was_air[slot] += dt;
            self.fall[slot] = r.vel.dot(r.normal).min(0.0);
            self.air_rate[slot] = (self.air_rate[slot] * 0.94667f32.powf(dt * 60.0)).max(10.0);
            rate = self.air_rate[slot] * (s / 2.778).min(1.0);
        } else {
            self.air_rate[slot] = 70.0;
            // touching down: a splash ring if it came down hard (over 5.5 m/s into the snow)
            if self.was_air[slot] > 0.1 {
                let vn = -self.fall[slot];
                if vn * 0.281 > 1.54 {
                    let k = (vn * 0.281).min(4.9);
                    let tang = (r.vel - r.normal * r.vel.dot(r.normal)) * 0.7;
                    let tang = tang.clamp_length_max(8.33);
                    for i in 0..25 {
                        let a = i as f32 / 25.0 * std::f32::consts::TAU;
                        let out = (fwd * a.cos() + side * a.sin()) * k * (0.6 + 0.4 * self.rand());
                        self.emit(r.pos + r.normal * 0.05, tang + out + r.normal * k * 0.6, 0.09, 1.0, 9.8, 1.0);
                    }
                }
                // and a few puffs around the board
                for _ in 0..((vn * 0.09) as usize).clamp(2, 8) {
                    let at = r.pos + fwd * self.u() * 0.8 + side * self.u() * 0.8;
                    self.emit(at, r.vel * 0.3 + r.normal * 0.4, 0.25, 0.6, 0.0, 2.0);
                }
            }
            self.was_air[slot] = 0.0;
            let (k, sz, a, b) = surf(r.surface);
            (size, l0, l1) = (sz, a, b);
            grav = 1.0;
            // carving throws snow off the edge: 0.006 s (1 + 149 steer^2) per second (s in cm/s)
            let steer = r.input.steer;
            rate = 0.006 * s * 100.0 * (1.0 + 149.0 * steer * steer) * k;
            let v = r.vel + side * steer * 1.1 * s;
            let mut lat = v.dot(side);
            let lat_spread = if lat.abs() < 4.5 { 3.0 } else { let w = lat.abs() - 1.5; lat = (lat + 4.5 * lat.signum()) * 0.5; w };
            let f = v.dot(fwd) * 0.5;
            let up = v.dot(r.normal);
            v0 = side * lat + fwd * f + r.normal * up.max(0.0);
            spread = (lat_spread, 0.0, f.abs());
            // braking hard fans snow out ahead: 12 rays every quarter second or so
            if r.input.brake && k >= 0.02 && s > 1.0 {
                self.fan[slot] -= dt;
                if self.fan[slot] <= 0.0 {
                    let c = (0.54 * s).clamp(0.72, 6.08);
                    self.fan[slot] = 0.25 * (1.0 - 0.0986 * c) + self.rand() * 0.08;
                    for i in 0..12 {
                        let sgn = if i % 2 == 0 { 1.0 } else { -1.0 };
                        let ang = (30.0 + 25.0 * self.rand()).to_radians();
                        let dir = (fwd * ang.cos() + side * sgn * ang.sin()).normalize_or_zero();
                        let u = self.rand() * 2.0 - 1.0;
                        let sp = c * (1.0 - u.powi(4)) * (0.8 + 0.4 * self.rand());
                        self.emit(r.pos + fwd * 0.3, r.vel * 0.5 + dir * sp + r.normal * sp * 0.4, 0.06, 1.0, 1.0, 1.1);
                    }
                }
            }
        }
        if s < 0.5 { return; }
        self.owed[slot] += dt * rate.min(900.0);
        while self.owed[slot] >= 1.0 {
            self.owed[slot] -= 1.0;
            // from a box at the board: 15 cm either side, 90 cm along
            let at = r.pos + side * self.u() * 0.3 + fwd * self.u() * 1.8 + r.normal * 0.03;
            let vel = v0 + side * self.u() * spread.0 + r.normal * self.u() * spread.1 + fwd * self.u() * spread.2;
            let sz = size * 0.02 * (0.8 + 0.4 * self.rand());
            let life = l0 + (l1 - l0) * self.rand();
            self.emit(at, vel, sz, life, grav, 0.0);
        }
    }
}

pub fn setup_spray(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let mesh = meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap());
    let mat = materials.add(StandardMaterial { base_color: Color::srgba(1.0, 1.0, 1.0, 0.85), unlit: true, alpha_mode: AlphaMode::Blend, ..default() });
    for i in 0..COUNT {
        commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(mat.clone()), Transform::from_scale(Vec3::ZERO), Visibility::Hidden, FlakeVisual(i)));
    }
    commands.insert_resource(Spray { flakes: vec![Flake::default(); COUNT], next: 0, seed: 12345, owed: Vec::new(), was_air: Vec::new(), air_rate: Vec::new(), fan: Vec::new(), fall: Vec::new() });
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
        f.vel.y -= f.grav * dt;
        f.vel *= (1.0 - 0.0182f32 * f.drag).powf(dt * 60.0);
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
