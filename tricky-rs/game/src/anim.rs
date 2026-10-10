//! Keyframed object animation (`cMeshAnimFrame`), as the level files describe it.

use glam::{Mat4, Vec3};
use serde::Deserialize;

/// A model object's animation: base pose (tx, ty, tz in cm, rx, ry, rz in degrees), a mask of the
/// animated channels (bit i = channel i), and per animated channel its cubic segments.
#[derive(Deserialize, Clone, Debug)]
pub struct ObjAnim {
    #[serde(rename = "U1", default)] pub u1: f32, #[serde(rename = "U2", default)] pub u2: f32, #[serde(rename = "U3", default)] pub u3: f32,
    #[serde(rename = "U4", default)] pub u4: f32, #[serde(rename = "U5", default)] pub u5: f32, #[serde(rename = "U6", default)] pub u6: f32,
    #[serde(rename = "AnimationAction", default)] pub action: u32,
    #[serde(rename = "AnimationEntries", default)] pub entries: Vec<AnimEntry>,
}
#[derive(Deserialize, Clone, Debug)]
pub struct AnimEntry { #[serde(rename = "AnimationMaths", default)] pub segs: Vec<AnimSeg> }
#[derive(Deserialize, Clone, Copy, Debug)]
pub struct AnimSeg {
    #[serde(rename = "Value1")] pub a: f32, #[serde(rename = "Value2")] pub b: f32, #[serde(rename = "Value3")] pub c: f32,
    #[serde(rename = "Value4")] pub d: f32, #[serde(rename = "Value5")] pub t0: f32, #[serde(rename = "Value6")] pub t1: f32,
}
impl ObjAnim {
    /// The object's local matrix at `t` seconds (`cMeshAnimFrame::vf1`): animated channels replace
    /// the base pose; each channel is ((a t + b) t + c) t + d on the segment holding t (clamped);
    /// T * Rz * Ry * Rx, angles in degrees.
    pub fn local(&self, t: f32) -> Mat4 {
        let mut ch = [self.u1, self.u2, self.u3, self.u4, self.u5, self.u6];
        let mut k = 0;
        for bit in 0..16 {
            if self.action >> bit & 1 == 0 { continue; }
            if let (true, Some(e)) = (bit < 6, self.entries.get(k)) {
                if let Some(s) = e.segs.iter().find(|s| t < s.t1).or(e.segs.last()) {
                    let tc = t.clamp(s.t0, s.t1);
                    ch[bit] = ((s.a * tc + s.b) * tc + s.c) * tc + s.d;
                }
            }
            k += 1;
        }
        let r = |d: f32| d.to_radians();
        Mat4::from_translation(Vec3::new(ch[0], ch[1], ch[2])) * Mat4::from_rotation_z(r(ch[5])) * Mat4::from_rotation_y(r(ch[4])) * Mat4::from_rotation_x(r(ch[3]))
    }
}
