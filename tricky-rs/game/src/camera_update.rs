//! `Game_UpdateCameras` 0x17f390 (board row E11c): for each camera slot of the game object (0x80 bytes from +0xa0), build
//! the view matrix, position and lens from the camera's list entry. Checked in the function runner
//! (`tests/camera_update.rs`).

use crate::camera::{mat_prod, rot9, IDENT3};
use crate::emitter::{cl, ps2_div};

/// What the camera slot's id names (`FUN_00173c18`): the camera's place, its three angles and its lens.
#[derive(Clone, Copy, Debug)]
pub struct CamEntry {
    /// +0x00: the position.
    pub pos: [f32; 3],
    /// +0x0c heading, +0x10 pitch, +0x14 roll (radians).
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    /// +0x18: the field of view (halved and clamped for the lens).
    pub fov: f32,
    /// +0x1c: the near distance (at least 1), +0x20: a word handed on.
    pub near: f32,
    pub w20: u32,
}

pub trait EntryEnv {
    fn entry(&mut self, id: u32) -> CamEntry;
}

/// The vector through a 3×3 `c` (the sum over the three rows of `c`, left to right).
fn through(c: &[f32; 9], v: [f32; 3]) -> [f32; 3] {
    let lane = |i: usize| (c[i] * v[0] + c[3 + i] * v[1]) + c[6 + i] * v[2];
    [lane(0), lane(1), lane(2)]
}

/// The camera's view matrix (four rows of four, row r column c at `[r][c]`) for the basis and position, as the vector
/// unit builds it: the basis rows (`side`, `back`, `v`) times the translation matrix, then transposed.
fn view_matrix(side: [f32; 3], back: [f32; 3], v: [f32; 3], pos: [f32; 3]) -> [[f32; 4]; 4] {
    let basis = [
        [side[0], side[1], side[2], 0.0],
        [back[0], back[1], back[2], 0.0],
        [v[0], v[1], v[2], 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let t = [[1.0, 0.0, 0.0, -pos[0]], [0.0, 1.0, 0.0, -pos[1]], [0.0, 0.0, 1.0, -pos[2]], [0.0, 0.0, 0.0, 1.0]];
    let mut out = [[0.0f32; 4]; 4];
    for (j, b) in basis.iter().enumerate() {
        for c in 0..4 {
            let acc = t[0][c] * b[0];
            let acc = acc + t[1][c] * b[1];
            let acc = acc + t[2][c] * b[2];
            out[j][c] = acc + t[3][c] * b[3];
        }
    }
    let mut m = [[0.0f32; 4]; 4];
    for r in 0..4 {
        for c in 0..4 {
            m[r][c] = out[c][r];
        }
    }
    m
}

/// `Game_UpdateCameras` 0x17f390 on the game object's words: for each of the `obj[0x298/4]` slots (0x80 bytes from
/// +0xa0 the entry id) fills the position (+0xb0, w = 1), the view matrix (+0xc0), the half field of view clamped to
/// [0.054831136, 1.3962634] (+0x100), the near distance (+0x104, at least 1) and the word at +0x108.
pub fn update_cameras(obj: &mut [u32], env: &mut dyn EntryEnv) {
    let count = obj[0x298 / 4] as i32;
    let mut i = 0;
    while i < count {
        let base = i as usize * 0x80 / 4;
        let e = env.entry(obj[base + 0xa0 / 4]);
        // turn (0, -1, 0) and the z axis through heading, pitch and roll
        let v0 = [0.0f32, -1.0, 0.0];
        let c1 = mat_prod(IDENT3, rot9(0.0, 0.0, 1.0, (-e.yaw) - 1.570_796_4));
        let c2 = mat_prod(c1, rot9(1.0, 0.0, 0.0, e.pitch));
        let v = through(&c2, v0);
        let c3 = mat_prod(c2, rot9(0.0, 1.0, 0.0, e.roll));
        let u = through(&c3, [0.0, 0.0, 1.0]);
        // the part of u across v (u less its projection on v), made a unit vector
        let k = (u[0] * v[0] + u[1] * v[1]) + u[2] * v[2];
        let q = [cl(v[0] * k), cl(v[1] * k), cl(v[2] * k)];
        let d = [u[0] - q[0], u[1] - q[1], u[2] - q[2]];
        let len = ((d[0] * d[0] + d[1] * d[1]) + d[2] * d[2]).sqrt();
        let back = if len == 0.0 {
            d
        } else {
            let r = ps2_div(1.0, len);
            [cl(d[0] * r), cl(d[1] * r), cl(d[2] * r)]
        };
        let side = [v[1] * back[2] - v[2] * back[1], v[2] * back[0] - v[0] * back[2], v[0] * back[1] - v[1] * back[0]];
        let m = view_matrix(side, back, v, e.pos);
        for r in 0..4 {
            for c in 0..4 {
                obj[base + 0xc0 / 4 + 4 * r + c] = m[r][c].to_bits();
            }
        }
        let mut fov = e.fov * 0.5;
        if fov < 0.054_831_136 {
            fov = 0.054_831_136;
        } else if 1.396_263_4 < fov {
            fov = 1.396_263_4;
        }
        obj[base + 0x100 / 4] = fov.to_bits();
        for k in 0..3 {
            obj[base + 0xb0 / 4 + k] = e.pos[k].to_bits();
        }
        obj[base + 0xb0 / 4 + 3] = 1.0f32.to_bits();
        obj[base + 0x104 / 4] = (if e.near < 1.0 { 1.0 } else { e.near }).to_bits();
        obj[base + 0x108 / 4] = e.w20;
        i += 1;
    }
}
