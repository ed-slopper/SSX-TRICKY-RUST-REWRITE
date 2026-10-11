//! The board's frame exactly as the original builds it (board row G16): `Boarder_UpdatePose` 0x129628 up to its
//! call of `Boarder_UpdateSkeleton` (the rest poses the skeleton: animation, row G5). The frame (rider+0x4aa0..,
//! four rows: the board's x, y and z axes and the position it is drawn at) is what the air tick's down ray, the
//! rail code and the drawing read. Checked bit for bit in the function runner (`tests/pose.rs`).

type V4 = [f32; 4];

/// What `Boarder_UpdatePose` reads to build the frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PoseIn {
    /// +0x170: the attitude quaternion (w, x, y, z)
    pub quat: V4,
    /// +0x214 the steer, +0x424 the motion (3: the rail, no steering tilt), +0x420 the board class
    pub steer: f32,
    pub motion: u32,
    pub class: u32,
    /// +0x140 the position, +0x2c0 the rider's up, +0x308 the body's height above the snow
    pub pos: V4,
    pub up: V4,
    pub height: f32,
    /// +0x330 the board's side on the ground, +0x244 the lean value, stats+0x60 its scale
    pub side: V4,
    pub lean: f32,
    pub lean_scale: f32,
    /// +0x1d0, +0x1d4: the trick's spin and flip; the pivot bone's height (rider + +0x44d8 × 0x394 + 0x8d0)
    pub spin: f32,
    pub flip: f32,
    pub pivot_z: f32,
}

/// The frame times a matrix given by rows (vmulabc / vmaddabc / vmaddbc: each new row is the frame's rows weighted by
/// the matrix row's four lanes, summed in order).
fn mul(f: [V4; 4], m: [V4; 4]) -> [V4; 4] {
    let mut out = [[0.0f32; 4]; 4];
    for (o, r) in out.iter_mut().zip(m.iter()) {
        *o = apply(f, *r);
    }
    out
}

/// The frame applied to one vector (x, y, z, w).
fn apply(f: [V4; 4], v: V4) -> V4 {
    let mut o = [0.0f32; 4];
    for i in 0..4 {
        o[i] = ((f[0][i] * v[0] + f[1][i] * v[1]) + f[2][i] * v[2]) + f[3][i] * v[3];
    }
    o
}

/// `Boarder_UpdatePose` 0x129628 to its `Boarder_UpdateSkeleton` call: the frame from the quaternion; tilted by the
/// steer about the board's y axis (angle steer × 1.134464 on an alpine board, else × 0.87266463; not on a rail, nor
/// with no steer); its position the rider's position + height · up + (lean scale · lean) · side; with a spin or
/// flip, turned by them (spin about z, then flip about x) about the pivot bone's height.
pub fn board_frame(p: &PoseIn) -> [V4; 4] {
    let (w, x, y, z) = (p.quat[0], p.quat[1], p.quat[2], p.quat[3]);
    let y2 = y + y;
    let z2 = z + z;
    let wx2 = w * (x + x);
    let xx = 1.0 - x * (x + x);
    let mut f = [
        [(1.0 - y * y2) - z * z2, x * y2 + w * z2, x * z2 - w * y2, 0.0],
        [x * y2 - w * z2, xx - z * z2, y * z2 + wx2, 0.0],
        [x * z2 + w * y2, y * z2 - wx2, xx - y * y2, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let z0 = f[3][0];
    if p.steer != z0 && p.motion != 3 {
        let k = if p.class == 2 { f32::from_bits(0x3f91_361e) } else { f32::from_bits(0x3f5f_66f3) };
        let (s, c) = crate::math::sin_cos(p.steer * k);
        let t = 1.0 - c;
        let (f23, f31, f14) = (t * z0, t * z0, t * 1.0);
        let r = [
            [f31 * z0 + c, f14 * z0 - s * z0, f23 * z0 + s * 1.0, z0],
            [f31 * 1.0 + s * z0, f14 * 1.0 + c, f23 * 1.0 - s * z0, z0],
            [f31 * z0 - s * 1.0, f14 * z0 + s * z0, f23 * z0 + c, z0],
            [z0, z0, z0, 1.0],
        ];
        f = mul(f, r);
    }
    let a = [p.up[0] * p.height, p.up[1] * p.height, p.up[2] * p.height, p.up[3] * p.height];
    let pos = [p.pos[0] + a[0], p.pos[1] + a[1], p.pos[2] + a[2], p.pos[3] + a[3]];
    let k = p.lean_scale * p.lean;
    let b = [p.side[0] * k, p.side[1] * k, p.side[2] * k, p.side[3] * k];
    f[3] = [pos[0] + b[0], pos[1] + b[1], pos[2] + b[2], pos[3] + b[3]];
    if p.spin != 0.0 || p.flip != 0.0 {
        let bz = p.pivot_z;
        f[3] = apply(f, [0.0, 0.0, bz, 1.0]);
        let (s, c) = crate::math::sin_cos(p.spin);
        let t = 1.0 - c;
        let (f16, f23, f14) = (t * 1.0, t * 0.0, t * 0.0);
        let r = [
            [f23 * 0.0 + c, f14 * 0.0 - s * 1.0, f16 * 0.0 + s * 0.0, 0.0],
            [f23 * 0.0 + s * 1.0, f14 * 0.0 + c, f16 * 0.0 - s * 0.0, 0.0],
            [f23 * 1.0 - s * 0.0, f14 * 1.0 + s * 0.0, f16 * 1.0 + c, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        f = mul(f, r);
        let (s, c) = crate::math::sin_cos(p.flip);
        let t = 1.0 - c;
        let (f16, f23, f14) = (t * 0.0, t * 1.0, t * 0.0);
        let r = [
            [f23 * 1.0 + c, f14 * 1.0 - s * 0.0, f16 * 1.0 + s * 0.0, 0.0],
            [f23 * 0.0 + s * 0.0, f14 * 0.0 + c, f16 * 0.0 - s * 1.0, 0.0],
            [f23 * 0.0 - s * 0.0, f14 * 0.0 + s * 1.0, f16 * 0.0 + c, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        f = mul(f, r);
        f[3] = apply(f, [0.0, 0.0, -bz, 1.0]);
    }
    f
}

/// The body's height above the snow one tick on (rider+0x308; `cBoarder::vf7` 0x117198): toward the surface table's
/// word 10 in ground motion (rider+0x424 = 2), toward 0 otherwise, at most 1.6666666 cm a tick.
pub fn ease_body_height(height: f32, motion: u32, surface_word10: f32) -> f32 {
    let target = if motion == 2 { surface_word10 } else { 0.0 };
    let step = f32::from_bits(0x3fd5_5555);
    if target + step < height {
        height - step
    } else if height < target - step {
        height + step
    } else {
        target
    }
}
