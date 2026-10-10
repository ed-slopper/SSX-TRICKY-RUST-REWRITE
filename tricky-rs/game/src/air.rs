//! The air step exactly as the original computes it (`Air_IntegrateRK4` 0x12b340): its constants, its units
//! (cm, cm/s, cm/s², the game's z up) and its order of operations on the vector unit, so it gives the same
//! floats, bit for bit, as the game's own code run in the function runner (board row F4d2). `Rider::step`
//! converts to and from its metres (y up).

/// Gravity rising and falling, cm/s² (0xc4548f73, 0xc4ed9ac0), and the horizontal drag (0xbe4cd34c).
const GRAVITY_RISING: f32 = -850.2414;
const GRAVITY_FALLING: f32 = -1900.8359;
const DRAG: f32 = -0.20002478;

/// The speed cap in the air, cm/s: the game writes it to rider+0x1c4 on every call (0x4551338e).
pub fn cap() -> f32 {
    f32::from_bits(0x4551_338e)
}

/// The acceleration at velocity `v`, built as the game builds it: (0, 0, g, 0) + (vx·drag, vy·drag, 0, 0).
fn accel(v: [f32; 4]) -> [f32; 4] {
    let g = if 0.0 < v[2] { GRAVITY_RISING } else { GRAVITY_FALLING };
    [0.0 + v[0] * DRAG, 0.0 + v[1] * DRAG, g + 0.0, 0.0 + 0.0]
}

fn add(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]]
}

fn scale(a: [f32; 4], k: f32) -> [f32; 4] {
    [a[0] * k, a[1] * k, a[2] * k, a[3] * k]
}

/// `Air_IntegrateRK4` 0x12b340: steps position `pos` and velocity `vel` (four floats each, as the vector unit
/// holds them) through `dt` seconds with one classic Runge-Kutta step, then caps the speed at [`cap`].
///
/// With `substeps` (the game's fourth argument, set for its flight predictions) the time goes in steps of at
/// most 0.2 s; without it the whole `dt` is one step, but it is still repeated once for every 0.2 s of `dt`,
/// as the game's loop does (riding always passes a frame, so once).
pub fn integrate_rk4(dt: f32, pos: &mut [f32; 4], vel: &mut [f32; 4], substeps: bool) {
    let cap = cap();
    // 1/2, 1/6 and 1/3 come from vdiv (1 over 2.0, 6.0, 3.0)
    let (half, sixth, third) = (1.0f32 / 2.0, 1.0f32 / 6.0, 1.0f32 / 3.0);
    let mut left = dt;
    let mut h = dt;
    while 0.0 < left {
        if substeps {
            h = if 0.2 < left { 0.2 } else { left };
        }
        let (p, v) = (*pos, *vel);
        let k1p = scale(v, h);
        let k1v = scale(accel(v), h);
        let v2 = add(v, scale(k1v, half));
        let k2p = scale(v2, h);
        let k2v = scale(accel(v2), h);
        let v3 = add(v, scale(k2v, half));
        let k3p = scale(v3, h);
        let k3v = scale(accel(v3), h);
        let v4 = add(v, k3v);
        let k4p = scale(v4, h);
        let k4v = scale(accel(v4), h);
        let p1 = add(p, scale(add(k1p, k4p), sixth));
        let v1 = add(v, scale(add(k1v, k4v), sixth));
        *pos = add(p1, scale(add(k2p, k3p), third));
        *vel = add(v1, scale(add(k2v, k3v), third));
        // speed on the vector unit (0x12b9f0-0x12ba00): sqrt(((x² + y²) + 1·z²) + 1·w²)
        let sq = [vel[0] * vel[0], vel[1] * vel[1], vel[2] * vel[2], vel[3] * vel[3]];
        let speed = (((sq[0] + sq[1]) + 1.0 * sq[2]) + 1.0 * sq[3]).sqrt();
        if cap < speed {
            *vel = scale(*vel, cap / speed);
        }
        left -= 0.2;
    }
}
