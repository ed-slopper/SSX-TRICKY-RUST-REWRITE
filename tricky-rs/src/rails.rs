//! Grindable rails: the level's splines, flattened to polylines in Bevy space.

use bevy::math::Vec3;

pub struct Rail {
    pub pts: Vec<Vec3>,
    /// distance along the rail at each point
    cum: Vec<f32>,
    min: Vec3,
    max: Vec3,
}

impl Rail {
    pub fn new(pts: Vec<Vec3>) -> Option<Self> {
        if pts.len() < 2 { return None; }
        let mut cum = vec![0.0];
        for w in pts.windows(2) { cum.push(cum.last().unwrap() + (w[1] - w[0]).length()); }
        let min = pts.iter().fold(Vec3::MAX, |a, p| a.min(*p));
        let max = pts.iter().fold(Vec3::MIN, |a, p| a.max(*p));
        (*cum.last().unwrap() > 1.0).then_some(Self { pts, cum, min, max })
    }
    pub fn len(&self) -> f32 { *self.cum.last().unwrap() }
    /// Position and direction at a distance along the rail.
    pub fn at(&self, d: f32) -> (Vec3, Vec3) {
        let d = d.clamp(0.0, self.len());
        let i = self.cum.partition_point(|c| *c <= d).clamp(1, self.pts.len() - 1);
        let (a, b) = (self.pts[i - 1], self.pts[i]);
        let seg = (self.cum[i] - self.cum[i - 1]).max(1e-5);
        (a.lerp(b, (d - self.cum[i - 1]) / seg), (b - a) / seg)
    }
}

#[derive(Default)]
pub struct Rails(pub Vec<Rail>);

impl Rails {
    /// Closest rail point within `reach` of `p`: (rail index, distance along it, how far away).
    pub fn nearest(&self, p: Vec3, reach: f32) -> Option<(usize, f32, f32)> {
        let mut best: Option<(usize, f32, f32)> = None;
        for (ri, rail) in self.0.iter().enumerate() {
            if p.cmplt(rail.min - reach).any() || p.cmpgt(rail.max + reach).any() { continue; }
            for i in 1..rail.pts.len() {
                let (a, b) = (rail.pts[i - 1], rail.pts[i]);
                let ab = b - a;
                let t = ((p - a).dot(ab) / ab.length_squared().max(1e-8)).clamp(0.0, 1.0);
                let dist = (a + ab * t - p).length();
                if dist < reach && best.is_none_or(|b| dist < b.2) {
                    best = Some((ri, rail.cum[i - 1] + ab.length() * t, dist));
                }
            }
        }
        best
    }
}
