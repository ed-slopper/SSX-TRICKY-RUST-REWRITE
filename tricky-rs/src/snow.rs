//! The falling snow drawn (row E1b2g5): the game's `cSnowFallMan` (`tricky_game::snowfall`, row G7c1) made from
//! `cGame::Init` 0x17e148's draws, stepped each 60 Hz tick (vf1 0x1c92b8) and drawn (vf2 0x1c9740): its walk's box
//! tests are `CullBox`'s test (`tricky_game::cull::box_test`) and each layer it draws (the renderer's +0x260,
//! 0x1e2e60) runs VU1 program 7 (`tricky_game::particlevu::run7`), whose sprites we draw.
//!
//! The VU's sprites come out on the screen; we put each back in the world through the inverse of the matrix we gave
//! the program (the same pixels, with the depth test the GS would make).

use crate::{ui, CharLib, SmoothDt};
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use tricky_game::fxrand::GameSnow;
use tricky_game::pf::Mem;
use tricky_game::ps2f::F;
use tricky_game::snowfall::{self, SnowEnv, GAPP};
use tricky_game::vu::Vu;

// Our memory for the manager and what its draw reads (gApp's renderer, game, camera and the camera's rider).
const APP: u32 = 0x0100_0000;
const REND: u32 = 0x0101_0000;
const VT: u32 = 0x0102_0000;
const GAME: u32 = 0x0103_0000;
/// The camera's rider (cGame+0xa4): the draw reads its weather (+0x18)
const RIDER: u32 = GAME + 0x180;
const MGR: u32 = 0x0104_0000;
const MAT: u32 = 0x0105_0000;
const BOX: u32 = MAT + 0x40;
/// The screen we ask program 7 for: x and y scale, z scale (ndc to ftoi4 integers, exactly invertible)
const S: f32 = 4096.0;
const Z: f32 = 1_048_576.0;
/// Where the block goes in VU1 memory (TOP), past the frame's words 0..6 and before the first buffer at 25
const TOP: u32 = 7;

#[derive(Resource)]
pub struct Snow {
    m: Mem,
    vu: Vu,
    on: bool,
    tick: f32,
    /// cGame+0x78, the density `cGame::Init` drew
    density: F,
    /// the camera's rider's weather (rider+0x18, `snowfall::rider_weather`)
    weather: F,
}

#[derive(Component)]
pub struct SnowMesh;

fn wf(m: &mut Mem, a: u32, v: f32) {
    m.w32(a, v.to_bits());
}

impl Snow {
    /// The manager as `cGame::Init` leaves it: constructed (`cSnowFallMan` 0x1c8db8, its 32 layer draws the ones
    /// `game_init_snow` made, in the same order), `SnowFall_SetDensity` 0x1c9010 with +0x78 and
    /// `SnowFall_SetRange` 0x1c91c0 with the two fall values; +0x74 whether it snows.
    pub fn new(g: &GameSnow) -> Snow {
        let mut m = Mem::default();
        for (base, len) in [(GAPP, 4u32), (APP, 0x800), (REND, 0x20), (VT, 0x300), (GAME, 0x300), (MGR, 0x500), (MAT, 0x60)] {
            m.regions.push((base, vec![0; len as usize]));
        }
        m.w32(GAPP, APP);
        m.w32(APP + 0x30, APP + 0x700);
        m.w32(APP + 0x724, REND);
        m.w32(APP + 0x730, GAME);
        m.w32(REND + 0x10, VT);
        for off in [0x1c0, 0x1c8, 0x1e0, 0x1f0, 0x260, 0x280] {
            m.w32(VT + off + 4, off);
        }
        // STANDIN: our TRICKY_SNOW=1 forces the snow on (a race never draws it), for looking at it
        let on = g.on || std::env::var("TRICKY_SNOW").is_ok_and(|v| v == "1");
        m.w32(GAME + 0x74, on as u32);
        m.w32(GAME + 0xa4, RIDER);
        // the rider's reset (`cBoarder::vf8` 0x116ab8) starts its weather at cGame+0x78
        wf(&mut m, RIDER + 0x18, g.density);
        let words: Vec<u32> = g.layers.iter().flat_map(|l| [l[0], l[2], l[3], l[1]]).collect();
        let mut env = Env { draws: words.into_iter(), vu: None, sprites: Vec::new() };
        snowfall::construct(&mut m, &mut env, MGR, 0);
        snowfall::set_density(&mut m, MGR, F(g.density));
        snowfall::set_range(&mut m, MGR, F(g.fall.0), F(g.fall.1));
        let mut vu = Vu::new();
        // the two GIF tags program 7's chain 0x31c790 loads at 25 and 521 (its UNPACK V4-32s at 0x31ca88 / 0x31caa8):
        // EOP, PRE with PRIM 0x56 (sprite, textured, blended), five PACKED registers ST, RGBAQ, XYZ2, ST, XYZ2 (the
        // program puts NLOOP | EOP in)
        for at in [25, 521] {
            vu.mem[at] = [0x8000, 0x502b_4000, 0x52512, 0];
        }
        Snow { m, vu, on, tick: 0.0, density: F(g.density), weather: F(g.density) }
    }
}

/// One sprite: two screen corners (x, y as ftoi4 integers), their depth, and the colour (0..128 each)
type Sprite = ([i32; 4], [i32; 4], [u32; 4]);

struct Env<'a> {
    draws: std::vec::IntoIter<u32>,
    vu: Option<&'a mut Vu>,
    sprites: Vec<Sprite>,
}

impl SnowEnv for Env<'_> {
    fn base_construct(&mut self, _: &mut Mem, _: u32, _: u32) {}
    fn rand_fx(&mut self) -> u32 {
        self.draws.next().expect("the snow's 32 layer draws")
    }
    /// +0x260 (0x1e2e60, `particledraw::draw_program7`): the layer's nine quadwords to VU1 at TOP and program 7 run.
    /// The renderer's set-ups (+0x1c0, +0x1c8 fog, +0x1e0, +0x1f0 texture) have nothing to do here.
    fn renderer(&mut self, m: &mut Mem, func: u32, _: u32, args: &[u32]) -> u32 {
        let Some(vu) = self.vu.as_deref_mut() else { return 0 };
        if func != 0x260 {
            return 0;
        }
        for q in 0..9 {
            vu.mem[(TOP + q) as usize] = [0, 1, 2, 3].map(|k| m.r32(args[0] + 0x10 * q + 4 * k));
        }
        vu.top = TOP;
        vu.gs.clear();
        vu.kicks.clear();
        tricky_game::particlevu::run7(vu, 0);
        for &(_, from) in &vu.kicks {
            let n = (vu.gs[from][0] & 0x7fff) as usize;
            for d in vu.gs[from + 1..].chunks(5).take(n) {
                self.sprites.push((d[2].map(|w| w as i32), d[4].map(|w| w as i32), d[1]));
            }
        }
        0
    }
    /// +0x280 (`cPS2GraphicsMan::CullBox` 0x1e1b10): its test on our clip matrix
    fn cull_box(&mut self, m: &mut Mem, _: u32, _: u32, min: [u32; 4], max: [u32; 4]) -> u32 {
        for k in 0..4 {
            m.w32(BOX + 4 * k, min[k as usize]);
            m.w32(BOX + 0x10 + 4 * k, max[k as usize]);
        }
        tricky_game::cull::box_test(m, MAT, BOX, BOX + 0x10)
    }
}

/// Game space (cm, z up) to the app's (m, y up)
fn g2b() -> Mat4 {
    Mat4::from_cols(Vec4::new(0.01, 0.0, 0.0, 0.0), Vec4::new(0.0, 0.0, -0.01, 0.0), Vec4::new(0.0, 0.01, 0.0, 0.0), Vec4::W)
}

pub fn setup_snow(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; 3]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; 3]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32; 2]; 3]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.0f32; 4]; 3]);
    mesh.insert_indices(Indices::U32(vec![0, 1, 2]));
    let mat = materials.add(StandardMaterial { unlit: true, alpha_mode: AlphaMode::Blend, cull_mode: None, ..default() });
    commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(mat), Transform::default(), SnowMesh, bevy::render::view::NoFrustumCulling));
}

/// Each 60 Hz tick the manager's step (vf1); each frame its draw (vf2) from the app's camera
#[allow(clippy::too_many_arguments)]
pub fn snow(time: Res<SmoothDt>, game: Res<ui::Game>, snow: Option<ResMut<Snow>>, cam: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut holder: Query<(&mut Transform, &Mesh3d, &MeshMaterial3d<StandardMaterial>), With<SnowMesh>>, mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>, mut images: ResMut<Assets<Image>>, lib: Res<CharLib>, mut loaded: Local<bool>) {
    let (Some(mut s), Ok((camera, gt)), Ok((mut tf, mesh, mat))) = (snow, cam.single(), holder.single_mut()) else { return };
    if !*loaded {
        *loaded = true;
        // The draw's texture is gApp+0x30's +0x4c: `EffectsLoader_LoadTextures` 0x1caef8 fills gApp+0x30 with
        // data/textures/particle.ssh's shapes by the names at 0x33edf0, so +0x4c is the 20th, "str2".
        // STANDIN: our tools' PNG export of that shape (chars/particles), else a soft dot, for the game's own
        // .ssh reader
        let name = crate::particles::TEX_NAMES[0x4c / 4];
        let (w, h, px) = image::open(lib.dir.join("particles").join(format!("{name}.png"))).ok().map(|i| {
            let i = i.to_rgba8();
            let (w, h) = i.dimensions();
            (w, h, i.into_raw())
        }).unwrap_or_else(|| {
            let px = (0..32 * 32).flat_map(|k| {
                let (dx, dy) = ((k % 32) as f32 / 15.5 - 1.0, (k / 32) as f32 / 15.5 - 1.0);
                [255, 255, 255, ((1.0 - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0) * 255.0) as u8]
            }).collect();
            (32, 32, px)
        });
        let tex = Some(images.add(Image::new(bevy::render::render_resource::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            bevy::render::render_resource::TextureDimension::D2, px, bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default())));
        if let Some(m) = materials.get_mut(&mat.0) {
            m.base_color_texture = tex;
        }
    }
    let s = &mut *s;
    if game.screen != ui::Screen::Paused {
        s.tick += time.dt.min(0.1);
        while s.tick >= 1.0 / 60.0 {
            s.tick -= 1.0 / 60.0;
            snowfall::step(&mut s.m, MGR);
            // the camera's rider's tick (`cBoarder::vf7` 0x117198) eases its weather toward the density
            // STANDIN: the app keeps no rider event bits (rider+0x388), so event 0xe is never on
            s.weather = snowfall::rider_weather(s.weather, s.density, false);
            wf(&mut s.m, RIDER + 0x18, s.weather.0);
        }
    }
    let view = camera.clip_from_view() * gt.compute_matrix().inverse();
    let clip = view * g2b();
    let eye = gt.translation();
    for (k, c) in clip.to_cols_array_2d().iter().enumerate() {
        for (i, x) in c.iter().enumerate() {
            wf(&mut s.m, MAT + 0x10 * k as u32 + 4 * i as u32, *x);
        }
        s.vu.mem[k] = c.map(f32::to_bits);
    }
    // the frame's words 4..6: the screen's scale and offset, and the sprites' scale (the projection's, per cm)
    let p = camera.clip_from_view();
    s.vu.mem[4] = [S, S, Z, 0.0].map(f32::to_bits);
    s.vu.mem[5] = [0.0f32; 4].map(f32::to_bits);
    s.vu.mem[6] = [p.x_axis.x * 0.01, p.y_axis.y * 0.01, 0.0, 0.0].map(f32::to_bits);
    let at = (eye / 0.01).to_array();
    for (i, x) in [at[0], -at[2], at[1], 1.0].into_iter().enumerate() {
        wf(&mut s.m, GAME + 0x90 + 0x20 + 4 * i as u32, x);
    }
    let mut env = Env { draws: Vec::new().into_iter(), vu: Some(&mut s.vu), sprites: Vec::new() };
    if s.on {
        snowfall::draw(&mut s.m, &mut env, MGR);
    }
    let inv = view.inverse();
    let back = |x: i32, y: i32, z: i32| inv.project_point3(Vec3::new(x as f32 / 16.0 / S, y as f32 / 16.0 / S, z as f32 / 16.0 / Z)) - eye;
    let n = env.sprites.len();
    let (mut pos, mut uv, mut col, mut idx) = (Vec::with_capacity(4 * n), Vec::with_capacity(4 * n), Vec::with_capacity(4 * n), Vec::with_capacity(6 * n));
    for (a, b, c) in env.sprites {
        let k = pos.len() as u32;
        for (x, y, u, v) in [(a[0], a[1], 0.0, 0.0), (b[0], a[1], 1.0, 0.0), (b[0], b[1], 1.0, 1.0), (a[0], b[1], 0.0, 1.0)] {
            pos.push(back(x, y, a[2]).to_array());
            uv.push([u, v]);
            col.push(c.map(|v| (v as f32 / 128.0).min(1.0)));
        }
        idx.extend_from_slice(&[k, k + 1, k + 2, k, k + 2, k + 3]);
    }
    tf.translation = eye;
    let Some(m) = meshes.get_mut(&mesh.0) else { return };
    if pos.is_empty() {
        pos = vec![[0.0; 3]; 3];
        uv = vec![[0.0; 2]; 3];
        col = vec![[0.0; 4]; 3];
        idx = vec![0, 1, 2];
    }
    let nrm = vec![[0.0f32, 1.0, 0.0]; pos.len()];
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nrm);
    m.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);
    m.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    m.insert_indices(Indices::U32(idx));
}
