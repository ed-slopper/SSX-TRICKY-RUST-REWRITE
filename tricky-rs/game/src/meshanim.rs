//! A keyframed object's part at a time on its clip (`cMeshAnimFrame`, row F4h5l): what `MeshAnim_ApplyFrames`
//! 0x199258 leaves in each animated part's frame (+0x90, the matrix `MeshAnimNode_GetFrameMatrix` 0x1998d0 hands
//! `World_RayCastInstance` 0x25bf48). Checked against the game's code in the function runner (`tests/meshanim.rs`).

use tricky_data::anim::ObjAnim;

type V4 = [f32; 4];

/// 0.017453292, the degrees-to-radians factor the frame code uses.
const DEG: f32 = 0.017_453_292;

/// The channels a frame starts from: `AnimFrame_Clear` 0x1cb150 (all 0), then `AnimFrame_SetKey` 0x1cb0d8 (the
/// animation's base pose: +0x4c..+0x54 the position as it is, +0x58..+0x60 the three angles × 0.017453292).
pub fn key_channels(a: &ObjAnim) -> [f32; 16] {
    let mut ch = [0.0f32; 16];
    ch[0] = a.u1;
    ch[1] = a.u2;
    ch[2] = a.u3;
    ch[3] = a.u4 * DEG;
    ch[4] = a.u5 * DEG;
    ch[5] = a.u6 * DEG;
    ch
}

/// `cAnimFrame::vf1` 0x1cb300 at `t` (clip seconds): each channel the animation's mask (+0x18) has (bit i, channel
/// +0x4c + 4i), in mask order its curve: the segment holding `t` (`AnimCurve_FindSegment` 0x1cb770: t0 ≤ t < t1,
/// else the first with t < t1, else the last), its cubic at `t` held to the segment's [t0, t1],
/// ((a·x + b)·x + c)·x + d. Channels the mask leaves out keep what they had.
pub fn eval_channels(a: &ObjAnim, t: f32, ch: &mut [f32; 16]) {
    let mut k = 0;
    for bit in 0..16 {
        if a.action >> bit & 1 == 0 {
            continue;
        }
        if let Some(e) = a.entries.get(k) {
            if let Some(s) = e.segs.iter().find(|s| s.t0 <= t && t < s.t1).or_else(|| {
                let n = e.segs.len();
                e.segs[..n.saturating_sub(1)].iter().find(|s| t < s.t1).or(e.segs.last())
            }) {
                let x = if t < s.t0 {
                    s.t0
                } else if s.t1 < t {
                    s.t1
                } else {
                    t
                };
                ch[bit] = ((s.a * x + s.b) * x + s.c) * x + s.d;
            }
        }
        k += 1;
    }
}

/// `cMeshAnimFrame::vf1` 0x1cb498 at `t`: the channels (`eval_channels`), then the frame (+0x90, rows) from the
/// position (+0x4c) and the angles (+0x58, +0x5c, +0x60, degrees): with (sx, cx), (sy, cy), (sz, cz) the
/// `Math_SinCos` of −(angle × 0.017453292),
/// (cy·cz, −cy·sz, sy), (sx·sy·cz + cx·sz, cx·cz − sx·sy·sz, −sx·cy), (sx·sz − cx·sy·cz, cx·sz·sy + sx·cz, cx·cy).
/// The channels carry over from one call to the next (a channel the mask leaves out keeps its key: its angle
/// there is already in radians and is turned again, as the game does).
pub fn mesh_frame(a: &ObjAnim, t: f32, ch: &mut [f32; 16]) -> [V4; 4] {
    eval_channels(a, t, ch);
    let (sx, cx) = crate::math::sin_cos(-(ch[3] * DEG));
    let (sy, cy) = crate::math::sin_cos(-(ch[4] * DEG));
    let (sz, cz) = crate::math::sin_cos(-(ch[5] * DEG));
    [
        [cy * cz, -cy * sz, sy, 0.0],
        [sx * sy * cz + cx * sz, cx * cz - sx * sy * sz, -sx * cy, 0.0],
        [sx * sz - cx * sy * cz, cx * sz * sy + sx * cz, cx * cy, 0.0],
        [ch[0], ch[1], ch[2], 1.0],
    ]
}

/// A keyframed object's animated parts at `t`: each part's frame as a fresh node leaves it (the key, then the
/// frame at `t`, `cAnimObjectNode::cAnimObjectNode` 0x198810 and `MeshAnim_ApplyFrames` 0x199258), in part order.
pub fn object_frames(anims: &[ObjAnim], t: f32) -> Vec<[V4; 4]> {
    anims
        .iter()
        .map(|a| {
            let mut ch = key_channels(a);
            mesh_frame(a, t, &mut ch)
        })
        .collect()
}
