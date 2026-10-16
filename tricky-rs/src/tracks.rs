//! Board tracks in the snow on the game's own code (board row G7f3b): each rider's strip is `Fx_BoardTrack` 0x132648
//! (`tricky_game::boardtrack::board_track`: up to 100 points on snow, powder and ice, a point every 110 cm or sooner
//! when the board turns or its width across the track changes, the strip's fade-in and the oldest points' fade-out,
//! the side the board is lit from) and is drawn as `Fx_BoardTrack_Draw` 0x1331a8 draws it (`board_track_draw`: the
//! strip twice, the shade on the side away from the light at −0x40 in blend state 4, the lit side at 0x80 in state
//! 8), here a darkening mesh and an additive one, the strips' weights as their alpha.
//!
//! What the strip reads of the rider: the surface, motion and hit kind, the board's corners (`pose::board_corners`
//! on `Rider::board_bone_frame`), the velocity, the ground's normal and the brake and steer. STANDIN: the ground point
//! is the rider's place on the snow, the motions 5 and 6's two skeleton vectors are the board's middle corners (the
//! skeleton, row G5), the light set is a fixed sun (the rider's lights, row G7b), and the strip is untextured (the
//! track texture's u, v are not used; the lit pass is added at a fifth for it).

use crate::rider::Rider;
use crate::{ui, Mode, Opponents, RiderRes};
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use tricky_game::boardtrack::{self, DrawEnv, Lights, TrackIn, Trail, Vertex};

type V4 = [f32; 4];

#[derive(Component)]
pub struct TrackMesh;

#[derive(Resource)]
pub struct Tracks { trails: Vec<Trail>, dark: Handle<Mesh>, light: Handle<Mesh> }

pub fn setup_tracks(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let (dark, light) = (meshes.add(empty()), meshes.add(empty()));
    let base = StandardMaterial { base_color: Color::WHITE, unlit: true, double_sided: true, cull_mode: None, ..default() };
    // the shaded side: black at the strip's weight; the lit side: added on
    let dark_mat = materials.add(StandardMaterial { alpha_mode: AlphaMode::Blend, ..base.clone() });
    let light_mat = materials.add(StandardMaterial { alpha_mode: AlphaMode::Add, ..base });
    for (mesh, mat) in [(dark.clone(), dark_mat), (light.clone(), light_mat)] {
        commands.spawn((Mesh3d(mesh), MeshMaterial3d(mat), Transform::IDENTITY, bevy::render::view::NoFrustumCulling, TrackMesh));
    }
    commands.insert_resource(Tracks { trails: Vec::new(), dark, light });
}

fn empty() -> Mesh {
    let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; 3]);
    m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; 3]);
    m.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.0f32; 4]; 3]);
    m.insert_indices(Indices::U32(vec![0, 1, 2]));
    m
}

/// A port position (m, y up) in the game's (cm, z up)
fn game(v: Vec3, w: f32) -> V4 {
    [v.x * 100.0, -v.z * 100.0, v.y * 100.0, w]
}
fn game_dir(v: Vec3) -> V4 {
    [v.x, -v.z, v.y, 0.0]
}

/// STANDIN: a fixed sun for the rider's light set (+0x5ae0): an ambient and one light from up and to one side
fn sun() -> Lights {
    let d = { let v = [0.45f32, 0.3, 0.84]; let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt(); [v[0] / l, v[1] / l, v[2] / l, 0.0] };
    Lights { ambient: [0.25; 3], weights: [[0.5; 3], [0.0; 3], [0.0; 3]], dirs: [d, [0.0; 4], [0.0; 4]] }
}

/// What `Fx_BoardTrack` reads of a rider
fn track_in(r: &Rider) -> TrackIn {
    let motion = if r.crashed > 0.0 { 5 } else if r.rail.is_some() { 3 } else if r.grounded { 2 } else { 1 };
    let m = tricky_game::pose::board_corners(r.board_bone_frame(), r.stats.kind as u32, r.data.goofy as u32, r.stats.scale);
    TrackIn {
        surface: r.surface as u32,
        motion,
        hit: r.hit_kind,
        m,
        table: [m[1], m[2]],
        pos: game(r.vel, 0.0),
        up: game_dir(r.normal),
        ground: game(r.pos, 1.0),
        f1fc: r.brake_v,
        f214: r.steer,
        lights: sun(),
    }
}

/// The renderer `Fx_BoardTrack_Draw` calls: the blend state it sets picks the mesh, the strips go into it
struct Collect<'a> { origin: V4, pass: u32, dark: &'a mut Strips, light: &'a mut Strips }

#[derive(Default)]
struct Strips { pos: Vec<[f32; 3]>, col: Vec<[f32; 4]>, idx: Vec<u32> }

impl DrawEnv for Collect<'_> {
    fn vcall(&mut self, off: u32, a1: Option<u32>, _a2: Option<u32>, _f12: Option<f32>) -> u32 {
        if off == 0x1e0 {
            self.pass = a1.unwrap_or(0);
        }
        0
    }
    fn view_word(&mut self) -> u32 {
        0
    }
    fn origin_quad(&mut self) -> V4 {
        // (the draw takes ten times this from every point: the strip comes out relative to the player)
        self.origin.map(|x| x * 0.1)
    }
    fn draw_strip(&mut self, verts: &[Vertex]) {
        // STANDIN: no track texture (it modulates both passes in the game): the lit pass added at a fifth
        let (s, rgb) = if self.pass == 4 { (&mut *self.dark, 0.0) } else { (&mut *self.light, 0.2) };
        let base = s.pos.len() as u32;
        for v in verts {
            // the game's cm, z up, relative to the player, as the port's metres
            s.pos.push([v.pos[0] * 0.01, v.pos[2] * 0.01, -v.pos[1] * 0.01]);
            s.col.push([rgb, rgb, rgb, (v.w as i32).clamp(0, 0x80) as f32 / 128.0]);
        }
        for i in 0..verts.len().saturating_sub(2) as u32 {
            s.idx.extend_from_slice(&[base + i, base + i + 1, base + i + 2]);
        }
    }
}

/// The board's track in the snow, every rider's (`Fx_BoardTrack` 0x132648, `Fx_BoardTrack_Draw` 0x1331a8).
pub fn tracks(
    rider: Res<RiderRes>, opponents: Res<Opponents>, game_ui: Res<ui::Game>, mode: Res<Mode>,
    mut tracks: ResMut<Tracks>, mut meshes: ResMut<Assets<Mesh>>, mut place: Query<&mut Transform, With<TrackMesh>>,
) {
    if game_ui.screen == ui::Screen::Paused || *mode != Mode::Ride { return; }
    let n = 1 + opponents.0.len();
    if tracks.trails.len() != n { tracks.trails = (0..n).map(|_| Trail::default()).collect(); }
    if game_ui.screen == ui::Screen::Menu { for t in tracks.trails.iter_mut() { *t = Trail::default(); } }
    else {
        boardtrack::board_track(&mut tracks.trails[0], &track_in(&rider.0));
        for (i, o) in opponents.0.iter().enumerate() { boardtrack::board_track(&mut tracks.trails[i + 1], &track_in(&o.rider)); }
    }
    // the meshes sit at the player (vertices relative to it) so they sort after the see-through terrain they lie on
    let origin = rider.0.pos;
    for mut tf in place.iter_mut() { tf.translation = origin; }
    let (mut dark, mut light) = (Strips::default(), Strips::default());
    for t in &tracks.trails {
        let mut env = Collect { origin: game(origin, 0.0), pass: 0, dark: &mut dark, light: &mut light };
        boardtrack::board_track_draw(t, &mut env);
    }
    if std::env::var("TRICKY_TRACKDBG").is_ok() {
        let t = &tracks.trails[0];
        let maxa = |s: &Strips| s.col.iter().map(|c| c[3]).fold(0.0f32, f32::max);
        println!("track: points {} head {} fade {:.2} open {} | dark {} verts max alpha {:.2} | light {} verts max alpha {:.2} | surface {} grounded {}",
            t.w[0x1944 / 4], t.w[0x1940 / 4], f32::from_bits(t.w[0x1948 / 4]), t.w[0x1950 / 4], dark.pos.len(), maxa(&dark), light.pos.len(), maxa(&light), rider.0.surface, rider.0.grounded);
    }
    for (handle, mut s) in [(tracks.dark.clone(), dark), (tracks.light.clone(), light)] {
        if let Some(m) = meshes.get_mut(&handle) {
            if s.idx.is_empty() { s.pos = vec![[0.0; 3]; 3]; s.col = vec![[0.0; 4]; 3]; s.idx = vec![0, 1, 2]; }
            m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; s.pos.len()]);
            m.insert_attribute(Mesh::ATTRIBUTE_POSITION, s.pos);
            m.insert_attribute(Mesh::ATTRIBUTE_COLOR, s.col);
            m.insert_indices(Indices::U32(s.idx));
        }
    }
}
