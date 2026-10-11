//! The ground probe exactly as the original runs it (board row E8g): `Boarder_ProbeGround` 0x128ae8 casts a line
//! along the ground normal through the board and takes the ground's normal, point, surface and height from what it
//! hits; `Boarder_GetFallLine` 0x128a18 gives the downhill direction in the ground's plane. In cm and the game's z up,
//! with the vector unit's order. `tests/probe.rs` checks both bit for bit against the original in the function runner,
//! with the world's ray cast (`World_RayCast`, E7) answered by the test.

type V4 = [f32; 4];

fn dot(a: V4, b: V4) -> f32 {
    let p = [a[0] * b[0], a[1] * b[1], a[2] * b[2], a[3] * b[3]];
    ((p[0] + p[1]) + 1.0 * p[2]) + 1.0 * p[3]
}

fn scale(a: V4, k: f32) -> V4 {
    [a[0] * k, a[1] * k, a[2] * k, a[3] * k]
}

fn add(a: V4, b: V4) -> V4 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]]
}

fn sub(a: V4, b: V4) -> V4 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]]
}

/// vopmula / vopmsub: a × b in x, y, z; w set to 0.
fn cross(a: V4, b: V4) -> V4 {
    let acc = [a[1] * b[2], a[2] * b[0], a[0] * b[1]];
    [acc[0] - b[1] * a[2], acc[1] - b[2] * a[0], acc[2] - b[0] * a[1], 0.0]
}

/// The board's forward with its part along `n` taken out, made unit length (vrsqrt).
fn in_plane(fwd: V4, n: V4) -> V4 {
    let v = sub(fwd, scale(n, dot(fwd, n)));
    scale(v, 1.0 / dot(v, v).sqrt())
}

/// `Boarder_GetFallLine` 0x128a18(n, fallback): n × unit(n × up), or `fallback` when n is (nearly) straight up.
pub fn fall_line(n: V4, fallback: V4) -> V4 {
    let c = cross(n, [0.0, 0.0, 1.0, 0.0]);
    let len = dot(c, c).sqrt();
    if 0.0001 < len {
        cross(n, scale(c, 1.0 / len))
    } else {
        fallback
    }
}

/// The rider fields the probe reads and writes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rider {
    /// position (rider+0x140), the board's forward (+0x180), the ground normal (+0x2a0), the ground point (+0x2d0),
    /// forward in the ground's plane (+0x320), the board's side (+0x330), the fall line (+0x2b0)
    pub pos: V4,
    pub fwd: V4,
    pub normal: V4,
    pub ground: V4,
    pub plane_fwd: V4,
    pub side: V4,
    pub fall: V4,
    /// rider+0x244 and the stat at stats+0x60: how far along the side the probe starts
    pub x244: f32,
    pub stat60: f32,
    /// height over the ground along the normal (+0x1bc)
    pub height: f32,
    /// the surface (+0x290), the ticks on it (+0x29c, 0x80000001 on a new surface), the hit's kind (+0x2e0, 3 = none)
    pub surface: u32,
    pub surface_ticks: u32,
    pub kind: u32,
}

/// What the world's ray cast answered.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hit {
    pub kind: u32,
    pub point: V4,
    pub normal: V4,
    pub surface: u32,
}

/// The line the probe casts (`cWorldLine::cWorldLine`(0.5, from, to, 2)): from 100 cm under the probe point to 200 cm
/// over it, along the ground normal; the probe point is the position moved along the side by stat60·x244.
pub fn line(r: &Rider) -> (V4, V4) {
    let p = add(r.pos, scale(r.side, r.stat60 * r.x244));
    (add(p, scale(r.normal, -100.0)), add(p, scale(r.normal, 200.0)))
}

/// `Boarder_ProbeGround` 0x128ae8 with the world's answer (`None` when the ray cast returns a negative distance):
/// true when on the ground.
pub fn probe(r: &mut Rider, hit: Option<Hit>) -> bool {
    let Some(h) = hit else {
        r.normal = [0.0, 0.0, 1.0, 0.0];
        r.plane_fwd = in_plane(r.fwd, r.normal);
        r.side = cross(r.normal, r.plane_fwd);
        r.kind = 3;
        r.fall = r.fwd;
        return false;
    };
    let p = add(r.pos, scale(r.side, r.stat60 * r.x244));
    if r.surface == h.surface {
        if (r.surface_ticks as i32) >= 0 {
            r.surface_ticks = 0;
        }
    } else {
        r.surface = h.surface;
        r.surface_ticks = 0x8000_0001;
    }
    r.normal = h.normal;
    r.ground = h.point;
    r.plane_fwd = in_plane(r.fwd, r.normal);
    r.side = cross(r.normal, r.plane_fwd);
    r.height = dot(sub(p, h.point), r.normal);
    r.fall = fall_line(r.normal, r.fwd);
    r.kind = h.kind;
    true
}
