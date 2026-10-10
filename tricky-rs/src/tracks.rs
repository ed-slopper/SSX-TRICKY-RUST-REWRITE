//! Board tracks in the snow (`Fx_BoardTrack` 0x132648, drawn by 0x1331a8): each rider leaves a
//! strip of up to 100 points behind it on snow, powder and ice. A point is laid every 110 cm, or
//! sooner when the board turns (5.1 degrees) or its width across the track changes (10 cm) at speed.
//! A strip fades in over its first points; past 90 points the oldest fade out. Tracks fade by count,
//! not by time.

use crate::rider::Rider;
use crate::{ui, Mode, Opponents, RiderRes};
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};

const POINTS: usize = 100;

#[derive(Component)]
pub struct TrackMesh;

#[derive(Clone, Copy)]
struct Point { pos: Vec3, side: Vec3, width: f32, alpha: f32, strip: u32 }

#[derive(Default)]
struct Trail { pts: std::collections::VecDeque<Point>, last_heading: Vec3, strip: u32, on: bool, run: u32 }

#[derive(Resource)]
pub struct Tracks { trails: Vec<Trail>, mesh: Handle<Mesh> }

/// How strongly each surface takes a track (row 21 of the surface table), as an opacity.
fn depth(surface: u8) -> Option<f32> {
    match surface {
        1 | 2 | 8 | 15 | 16 => Some(0.0286),
        3 | 4 => Some(0.0167),
        5 => Some(0.1997),
        _ => None,
    }
}

pub fn setup_tracks(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let mesh = meshes.add(empty());
    let mat = materials.add(StandardMaterial {
        base_color: Color::WHITE, unlit: true, alpha_mode: AlphaMode::Blend, double_sided: true, cull_mode: None,
        ..default()
    });
    commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(mat), Transform::IDENTITY, bevy::render::view::NoFrustumCulling, TrackMesh));
    commands.insert_resource(Tracks { trails: Vec::new(), mesh });
}

fn empty() -> Mesh {
    let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; 3]);
    m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; 3]);
    m.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.0f32; 4]; 3]);
    m.insert_indices(Indices::U32(vec![0, 1, 2]));
    m
}

impl Trail {
    fn update(&mut self, r: &Rider) {
        let k = depth(r.surface).filter(|_| r.grounded && r.rail.is_none());
        let Some(k) = k else { self.on = false; return };
        let s = r.vel.length();
        let fwd = r.forward();
        let travel = if s > 0.3 { (r.vel - r.normal * r.vel.dot(r.normal)).normalize_or(fwd) } else { fwd };
        let side = r.normal.cross(travel).normalize_or_zero();
        // the board's span across the way it is going (1.5 m long, 0.28 m wide); carving narrows it
        let slip = fwd.dot(side).abs();
        let mut width = 1.5 * slip + 0.28 * (1.0 - slip);
        if !r.input.brake { width *= 1.0 - 0.85 * r.input.steer.abs() * 0.5; }
        let at = r.pos + r.normal * 0.04;
        if !self.on { self.strip += 1; self.run = 0; self.on = true; }
        let need = match self.pts.back() {
            Some(p) if p.strip == self.strip => {
                let moved = (at - p.pos).length();
                moved > 1.1 || (s > 1.389 && ((width - p.width).abs() > 0.101 || travel.dot(self.last_heading) < 5.1f32.to_radians().cos()))
            }
            _ => true,
        };
        if !need { return; }
        self.run += 1;
        let alpha = (k * 16.0).min(0.7) * (0.3 * self.run as f32).min(1.0);
        self.pts.push_back(Point { pos: at, side, width, alpha, strip: self.strip });
        self.last_heading = travel;
        while self.pts.len() > POINTS { self.pts.pop_front(); }
    }
}

pub fn tracks(
    rider: Res<RiderRes>, opponents: Res<Opponents>, game: Res<ui::Game>, mode: Res<Mode>,
    mut tracks: ResMut<Tracks>, mut meshes: ResMut<Assets<Mesh>>, mut place: Query<&mut Transform, With<TrackMesh>>,
) {
    if game.screen == ui::Screen::Paused || *mode != Mode::Ride { return; }
    let n = 1 + opponents.0.len();
    if tracks.trails.len() != n { tracks.trails = (0..n).map(|_| Trail::default()).collect(); }
    if game.screen == ui::Screen::Menu { for t in tracks.trails.iter_mut() { t.pts.clear(); } }
    else {
        tracks.trails[0].update(&rider.0);
        for (i, o) in opponents.0.iter().enumerate() { tracks.trails[i + 1].update(&o.rider); }
    }
    // the mesh sits at the player (vertices relative to it) so it sorts after the see-through
    // terrain it lies on
    let origin = rider.0.pos;
    for mut tf in place.iter_mut() { tf.translation = origin; }
    let mut pos: Vec<[f32; 3]> = Vec::new();
    let mut col: Vec<[f32; 4]> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    for t in &tracks.trails {
        let len = t.pts.len();
        for (i, w) in t.pts.iter().collect::<Vec<_>>().windows(2).enumerate() {
            let (a, b) = (w[0], w[1]);
            if a.strip != b.strip { continue; }
            // the oldest ten fade out; the last few taper into the board
            let fade = |j: usize| -> f32 {
                let old = (j as f32 / 10.0).min(1.0);
                let tail = len - 1 - j;
                let taper = [0.0, 0.3, 0.6, 0.9].get(tail).copied().unwrap_or(1.0);
                if len < POINTS { taper } else { old * taper }
            };
            let base = pos.len() as u32;
            for (p, j) in [(a, i), (b, i + 1)] {
                let f = fade(j);
                let half = p.side * p.width * 0.5;
                // the groove: the side away from the light is a little darker
                pos.push((p.pos - half - origin).into());
                pos.push((p.pos + half - origin).into());
                col.push([0.06, 0.08, 0.16, p.alpha * f]);
                col.push([0.14, 0.17, 0.28, p.alpha * f]);
            }
            idx.extend_from_slice(&[base, base + 1, base + 2, base + 1, base + 3, base + 2]);
        }
    }
    if let Some(m) = meshes.get_mut(&tracks.mesh) {
        if idx.is_empty() { pos = vec![[0.0; 3]; 3]; col = vec![[0.0; 4]; 3]; idx = vec![0, 1, 2]; }
        m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; pos.len()]);
        m.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
        m.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
        m.insert_indices(Indices::U32(idx));
    }
}
