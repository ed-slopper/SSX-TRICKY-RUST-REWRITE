//! Ground-riding forces exactly as the original computes them: its constants, its units (cm, cm/s, cm/s²)
//! and its order of operations, so each function gives the same float, bit for bit, as the game's own code
//! run in the function runner (`tools/r5900`, board rows E8e and F4d). `Rider::step` converts to and from
//! its metres.
//!
//! Stats are the rider's stat bytes (0–255, each scaled by 1/255 inside, as the game does).

/// What `Boarder_ForwardDrag` reads besides its two float arguments.
#[derive(Clone, Copy, Debug, Default)]
pub struct DragState {
    /// stat bytes: stats+0x0e (speed: the linear term), +0x11 (edging: the brake term), +0x19 (the cubic term)
    pub speed: f32,
    pub edging: f32,
    pub cubic: f32,
    /// board class (rider+0x420): 0 BX, 1 freestyle, 2 alpine
    pub class: u32,
    /// riding switch (rider+0x1b4 differs from the stats' natural stance, stats+0x40)
    pub switch: bool,
    /// on powder (surface 3 or 4, rider+0x290): the board's height (rider+0x1bc) and the spring band's
    /// depth (rider+0x298)
    pub powder: Option<(f32, f32)>,
    /// crouch (rider+0x208), boost level (rider+0x130), brake (rider+0x1fc)
    pub crouch: f32,
    pub boost_level: f32,
    pub brake: f32,
}

/// `Boarder_ForwardDrag` 0x109cb8: the drag along the board, in cm/s², for forward speed `vf` (cm/s),
/// `load` (the spring force over gravity) and the surface row's drag terms `row` (surface table +4, +8, +0xc).
pub fn forward_drag(vf: f32, load: f32, row: [f32; 3], s: &DragState) -> f32 {
    let l = if 1.0 <= load { load } else { 1.0 };
    let mut cubic = s.cubic * -0.29035342 * 0.003921569 + 1.2848105;
    let mut brake = s.edging * 0.9181778 * 0.003921569 + 0.6150995;
    let mut linear = if s.class == 2 {
        s.speed * -0.30197912 * 0.003921569 + 0.70764244
    } else {
        s.speed * -0.30845523 * 0.003921569 + 1.0649822
    };
    if s.switch && s.class != 1 {
        let k = if s.class == 0 { 0.84998363 } else { 0.69987833 };
        brake *= k;
        cubic *= k;
        linear *= k;
    }
    let depth = match s.powder {
        Some((h, d2)) => 0.5 - h / d2,
        None => 1.0,
    };
    let a = vf.abs();
    -vf * depth
        * (l * row[0] * linear + (1.0 - s.crouch) * 0.10572864
            + (s.boost_level * 1.2153687 + 1.0) * 1.7513076 * s.brake * s.brake * brake
            // grouped as the machine code does it (0x109e70, 0x109ea4, 0x109ed0): (|vF|·0.001)·((L·row₂)·cubic)
            + a * 0.001 * (l * row[1] + a * 0.001 * (l * row[2] * cubic)))
}

/// What `Boarder_SideFriction` reads besides its float arguments.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrictionState {
    /// stat byte stats+0x13 (edging grip)
    pub grip: f32,
    pub class: u32,
    pub switch: bool,
    /// boost level (rider+0x130)
    pub boost_level: f32,
}

/// `Boarder_SideFriction` 0x109ef8: the force against sideways slip, in cm/s², for sideways speed `vr` and
/// forward speed `vf` (cm/s), the rider's rider+0x214 (most likely the shaped steer: |x|·1.0001 reaches 1
/// only at full lock), and the surface row's grip (surface table +0x10).
pub fn side_friction(vr: f32, vf: f32, x214: f32, grip: f32, s: &FrictionState) -> f32 {
    let v = vf.abs();
    let curve = if v < 555.55554 {
        (v - 0.0) * 0.0008899726 + 0.20103703
    } else if v < 1388.8888 {
        (v - 555.55554) * 0.00036129795 + 0.6954663
    } else {
        (v - 1388.8888) * -0.00010441317 + 0.9965479
    };
    let mut p = vr * curve;
    let x = x214.abs() * 1.0001484 + 0.0;
    if 1.0 <= x {
        p *= x;
    }
    let mut k = s.grip * 1.1412175 * 0.003921569 + 0.001;
    if s.switch && s.class != 1 {
        k *= if s.class == 0 { 0.84998363 } else { 0.69987833 };
    }
    let mut f = p * -grip * k;
    if 0.0 < s.boost_level {
        f *= 1.0 / (s.boost_level * 3.4999945 + 1.0);
    }
    f
}

/// `Boarder_GroundSpringForce` 0x109878: the force (cm/s²) holding the board on the snow, for its height `h`
/// above the snow (cm, negative when sunk in) and its speed along the normal `vn` (cm/s); `d1`, `d2` are the
/// spring band (rider+0x294, rider+0x298), `g` and `damping` the surface row's +0x0 and +0x24.
/// The 30 is the game's global at 0x31d304.
pub fn spring_force(h: f32, vn: f32, d1: f32, d2: f32, g: f32, damping: f32) -> f32 {
    if 0.0 < h {
        let mut f = -(g / 30.0) * h;
        if 0.0 < vn {
            f = f + -damping * vn;
        }
        f
    } else if -d1 < h {
        (-g * h) / d1 + -damping * vn
    } else {
        let x = if -d2 < h { h } else { -d2 };
        let t = x + d1;
        // (t + t): an add in the machine code (0x10992c), not a multiply by 2
        g * (1.0 - (t + t) / (d2 - d1)) + -damping * vn
    }
}
