//! VU1 as the game's patch programs use it (rows G7a4b..e): its float and integer registers, its data memory, and
//! the vector operations with the VU's rounding (each multiply and add rounded as the EE rounds, the largest float
//! for an overflow, denormals as zero; `div` toward zero; `ftoi` toward zero, saturating). What a program sends to
//! the GS (`xgkick`) goes to [`Vu::gs`]. The programs' ports (`patchvu`) are written on these, in the order each
//! program does its arithmetic, so their results are the same to the bit; they are checked against the game's
//! programs run in the function runner's VU1.

use crate::animseq;
use crate::ps2f::F;

/// The lanes an operation writes (`dest`): x 8, y 4, z 2, w 1
pub const X: u32 = 8;
pub const Y: u32 = 4;
pub const Z: u32 = 2;
pub const W: u32 = 1;
pub const XY: u32 = X | Y;
pub const XYZ: u32 = X | Y | Z;
pub const ZW: u32 = Z | W;
pub const YZ: u32 = Y | Z;
pub const YZW: u32 = Y | Z | W;
pub const XYZW: u32 = 15;

/// A lane's index (0 x .. 3 w)
pub const LX: usize = 0;
pub const LY: usize = 1;
pub const LZ: usize = 2;
pub const LW: usize = 3;

fn add(a: f32, b: f32) -> f32 {
    (F(a) + F(b)).0
}
fn sub(a: f32, b: f32) -> f32 {
    (F(a) - F(b)).0
}
fn mul(a: f32, b: f32) -> f32 {
    (F(a) * F(b)).0
}
/// The order `max` and `mini` compare in: the bits as a sign and a magnitude
fn key(x: f32) -> i64 {
    let b = x.to_bits();
    if b >> 31 != 0 { -((b & 0x7fff_ffff) as i64) - 1 } else { b as i64 }
}

/// VU1's state: the registers, 16 KiB of data memory (as quadwords), TOP, and what was sent to the GS
#[derive(Clone)]
pub struct Vu {
    pub vf: [[f32; 4]; 32],
    pub vi: [u16; 16],
    pub acc: [f32; 4],
    pub q: f32,
    /// The clip flags: four tests of six bits, the latest lowest
    pub clip: u32,
    /// The random number register R: 1.0 to 2.0, a 23-bit shift register in the mantissa
    pub r: u32,
    /// The I register (an I bit's immediate)
    pub i: f32,
    /// The MAC flags as the program's flag reads see them (see [`Vu::mac_flags`]): set by a port where its
    /// program reads them and at its end
    pub mac: u32,
    pub mem: Vec<[u32; 4]>,
    pub top: u32,
    pub gs: Vec<[u32; 4]>,
    /// Each send's start in data memory and where its packet begins in `gs`
    pub kicks: Vec<(u32, usize)>,
}

impl Vu {
    pub fn new() -> Vu {
        let mut vf = [[0.0; 4]; 32];
        vf[0] = [0.0, 0.0, 0.0, 1.0];
        Vu { vf, vi: [0; 16], acc: [0.0; 4], q: 0.0, clip: 0, r: 0x3f80_0000, i: 0.0, mac: 0, mem: vec![[0; 4]; 0x400], top: 0, gs: Vec::new(), kicks: Vec::new() }
    }

    /// A register written in the lanes of `dest` (vf0 stays (0, 0, 0, 1))
    pub fn set(&mut self, r: usize, dest: u32, v: [f32; 4]) {
        if r == 0 {
            return;
        }
        for (i, x) in v.into_iter().enumerate() {
            if dest & (8 >> i) != 0 {
                self.vf[r][i] = x;
            }
        }
    }
    fn set_acc(&mut self, dest: u32, v: [f32; 4]) {
        for (i, x) in v.into_iter().enumerate() {
            if dest & (8 >> i) != 0 {
                self.acc[i] = x;
            }
        }
    }
    fn lanes(f: impl Fn(usize) -> f32) -> [f32; 4] {
        [f(0), f(1), f(2), f(3)]
    }

    // ---- to a register ----

    /// fd = fs + ft
    pub fn add(&mut self, fd: usize, fs: usize, ft: usize, dest: u32) {
        let (s, t) = (self.vf[fs], self.vf[ft]);
        self.set(fd, dest, Self::lanes(|i| add(s[i], t[i])));
    }
    /// fd = fs + ft's lane `bc`
    pub fn add_bc(&mut self, fd: usize, fs: usize, ft: usize, bc: usize, dest: u32) {
        let (s, b) = (self.vf[fs], self.vf[ft][bc]);
        self.set(fd, dest, Self::lanes(|i| add(s[i], b)));
    }
    /// fd = fs − ft
    pub fn sub(&mut self, fd: usize, fs: usize, ft: usize, dest: u32) {
        let (s, t) = (self.vf[fs], self.vf[ft]);
        self.set(fd, dest, Self::lanes(|i| sub(s[i], t[i])));
    }
    /// fd = fs − ft's lane `bc`
    pub fn sub_bc(&mut self, fd: usize, fs: usize, ft: usize, bc: usize, dest: u32) {
        let (s, b) = (self.vf[fs], self.vf[ft][bc]);
        self.set(fd, dest, Self::lanes(|i| sub(s[i], b)));
    }
    /// fd = fs · ft
    pub fn mul(&mut self, fd: usize, fs: usize, ft: usize, dest: u32) {
        let (s, t) = (self.vf[fs], self.vf[ft]);
        self.set(fd, dest, Self::lanes(|i| mul(s[i], t[i])));
    }
    /// fd = fs · ft's lane `bc`
    pub fn mul_bc(&mut self, fd: usize, fs: usize, ft: usize, bc: usize, dest: u32) {
        let (s, b) = (self.vf[fs], self.vf[ft][bc]);
        self.set(fd, dest, Self::lanes(|i| mul(s[i], b)));
    }
    /// fd = fs + I
    pub fn add_i(&mut self, fd: usize, fs: usize, dest: u32) {
        let (s, i) = (self.vf[fs], self.i);
        self.set(fd, dest, Self::lanes(|k| add(s[k], i)));
    }
    /// fd = the larger of fs and ft's lane `bc` (as the VU compares: the floats' bits as sign and magnitude)
    pub fn max_bc(&mut self, fd: usize, fs: usize, ft: usize, bc: usize, dest: u32) {
        let (s, b) = (self.vf[fs], self.vf[ft][bc]);
        self.set(fd, dest, Self::lanes(|k| if key(s[k]) >= key(b) { s[k] } else { b }));
    }
    /// fd = the smaller of fs and I
    pub fn mini_i(&mut self, fd: usize, fs: usize, dest: u32) {
        let (s, i) = (self.vf[fs], self.i);
        self.set(fd, dest, Self::lanes(|k| if key(s[k]) <= key(i) { s[k] } else { i }));
    }
    /// The MAC flags an op with result `v` sets in the lanes of `dest` (zero 1, sign 0x10, underflow 0x100,
    /// overflow 0x1000, each shifted by 3 for x .. 0 for w; as PCSX2: ±0 zero, a denormal underflow and zero,
    /// exponent 255 overflow)
    pub fn mac_flags(v: [f32; 4], dest: u32) -> u32 {
        let mut f = 0;
        for (k, x) in v.iter().enumerate() {
            if dest & (8 >> k) == 0 {
                continue;
            }
            let (sh, b) = (3 - k, x.to_bits());
            if b >> 31 != 0 {
                f |= 0x10 << sh;
            }
            match (b & 0x7fff_ffff, b >> 23 & 0xff) {
                (0, _) => f |= 1 << sh,
                (_, 0) => f |= 0x101 << sh,
                (_, 255) => f |= 0x1000 << sh,
                _ => {}
            }
        }
        f
    }
    /// fd = fs · Q
    pub fn mul_q(&mut self, fd: usize, fs: usize, dest: u32) {
        let (s, q) = (self.vf[fs], self.q);
        self.set(fd, dest, Self::lanes(|i| mul(s[i], q)));
    }
    /// fd = ACC + fs · ft
    pub fn madd(&mut self, fd: usize, fs: usize, ft: usize, dest: u32) {
        let (s, t, a) = (self.vf[fs], self.vf[ft], self.acc);
        self.set(fd, dest, Self::lanes(|i| add(a[i], mul(s[i], t[i]))));
    }
    /// fd = ACC + fs · ft's lane `bc`
    pub fn madd_bc(&mut self, fd: usize, fs: usize, ft: usize, bc: usize, dest: u32) {
        let (s, b, a) = (self.vf[fs], self.vf[ft][bc], self.acc);
        self.set(fd, dest, Self::lanes(|i| add(a[i], mul(s[i], b))));
    }
    /// fd = ACC + fs · Q
    pub fn madd_q(&mut self, fd: usize, fs: usize, dest: u32) {
        let (s, q, a) = (self.vf[fs], self.q, self.acc);
        self.set(fd, dest, Self::lanes(|i| add(a[i], mul(s[i], q))));
    }
    /// fd = ACC − fs · Q
    pub fn msub_q(&mut self, fd: usize, fs: usize, dest: u32) {
        let (s, q, a) = (self.vf[fs], self.q, self.acc);
        self.set(fd, dest, Self::lanes(|i| sub(a[i], mul(s[i], q))));
    }

    // ---- to ACC ----

    /// ACC = fs · ft
    pub fn mula(&mut self, fs: usize, ft: usize, dest: u32) {
        let (s, t) = (self.vf[fs], self.vf[ft]);
        self.set_acc(dest, Self::lanes(|i| mul(s[i], t[i])));
    }
    /// ACC = fs · ft's lane `bc`
    pub fn mula_bc(&mut self, fs: usize, ft: usize, bc: usize, dest: u32) {
        let (s, b) = (self.vf[fs], self.vf[ft][bc]);
        self.set_acc(dest, Self::lanes(|i| mul(s[i], b)));
    }
    /// ACC += fs · ft's lane `bc`
    pub fn madda_bc(&mut self, fs: usize, ft: usize, bc: usize, dest: u32) {
        let (s, b, a) = (self.vf[fs], self.vf[ft][bc], self.acc);
        self.set_acc(dest, Self::lanes(|i| add(a[i], mul(s[i], b))));
    }
    /// ACC = fs + ft's lane `bc`
    pub fn adda_bc(&mut self, fs: usize, ft: usize, bc: usize, dest: u32) {
        let (s, b) = (self.vf[fs], self.vf[ft][bc]);
        self.set_acc(dest, Self::lanes(|i| add(s[i], b)));
    }
    /// ACC = fs − ft's lane `bc`
    pub fn suba_bc(&mut self, fs: usize, ft: usize, bc: usize, dest: u32) {
        let (s, b) = (self.vf[fs], self.vf[ft][bc]);
        self.set_acc(dest, Self::lanes(|i| sub(s[i], b)));
    }
    /// The matrix (rows `m`..`m + 3`) times `v`: ACC = m0·v.x, + m1·v.y, + m2·v.z, then fd = ACC + m3·v.w
    pub fn transform(&mut self, fd: usize, m: usize, v: usize, dest: u32) {
        self.mula_bc(m, v, LX, dest);
        self.madda_bc(m + 1, v, LY, dest);
        self.madda_bc(m + 2, v, LZ, dest);
        self.madd_bc(fd, m + 3, v, LW, dest);
    }
    /// As [`transform`](Self::transform) with the four rows given
    pub fn transform_rows(&mut self, fd: usize, rows: [usize; 4], v: usize, dest: u32) {
        self.mula_bc(rows[0], v, LX, dest);
        self.madda_bc(rows[1], v, LY, dest);
        self.madda_bc(rows[2], v, LZ, dest);
        self.madd_bc(fd, rows[3], v, LW, dest);
    }

    // ---- conversions, the clip test, Q ----

    /// ft = fs to integers ×16, toward zero, saturating (the integers' bits kept in the lanes)
    pub fn ftoi4(&mut self, ft: usize, fs: usize, dest: u32) {
        self.ftoi(ft, fs, 16.0, dest);
    }
    /// ft = fs to integers, toward zero, saturating
    pub fn ftoi0(&mut self, ft: usize, fs: usize, dest: u32) {
        self.ftoi(ft, fs, 1.0, dest);
    }
    fn ftoi(&mut self, ft: usize, fs: usize, scale: f64, dest: u32) {
        let s = self.vf[fs];
        let v = Self::lanes(|i| f32::from_bits(((s[i] as f64) * scale).clamp(i32::MIN as f64, i32::MAX as f64) as i32 as u32));
        self.set(ft, dest, v);
    }
    /// ft = fs's integer bits as floats
    pub fn itof0(&mut self, ft: usize, fs: usize, dest: u32) {
        let s = self.vf[fs];
        self.set(ft, dest, Self::lanes(|i| animseq::cvt_s_w(s[i].to_bits() as i32).0));
    }
    /// The clip test of fs's x, y, z against ±|ft.w|: the flags shifted up six, this test's below (x > 1, x < −2,
    /// then y 4 / 8, z 0x10 / 0x20)
    pub fn clipw(&mut self, fs: usize, ft: usize) {
        let (s, lim) = (self.vf[fs], self.vf[ft][LW].abs());
        let mut f = 0;
        for k in 0..3 {
            if s[k] > lim {
                f |= 1 << (2 * k);
            }
            if s[k] < -lim {
                f |= 2 << (2 * k);
            }
        }
        self.clip = ((self.clip << 6) | f) & 0xff_ffff;
    }
    /// Q = fs's lane / ft's lane
    pub fn div(&mut self, fs: usize, fsf: usize, ft: usize, ftf: usize) {
        self.q = animseq::div(F(self.vf[fs][fsf]), F(self.vf[ft][ftf])).0;
    }
    /// ft's lanes = the integer register, sign-extended
    pub fn mfir(&mut self, ft: usize, is: usize, dest: u32) {
        let v = f32::from_bits(self.vi[is] as i16 as i32 as u32);
        self.set(ft, dest, [v; 4]);
    }
    /// `move`: ft = fs in the lanes of `dest`
    pub fn mov(&mut self, ft: usize, fs: usize, dest: u32) {
        let s = self.vf[fs];
        self.set(ft, dest, s);
    }
    /// ft = fs turned one lane (fs's y, z, w, x), in the lanes of `dest`
    pub fn mr32(&mut self, ft: usize, fs: usize, dest: u32) {
        let s = self.vf[fs];
        self.set(ft, dest, [s[1], s[2], s[3], s[0]]);
    }
    /// `rinit`: R = fs's lane's mantissa (as 1.0 to 2.0)
    pub fn rinit(&mut self, fs: usize, fsf: usize) {
        self.r = self.vf[fs][fsf].to_bits() & 0x7f_ffff | 0x3f80_0000;
    }
    /// `rnext`: R stepped (shifted up one, the XOR of its bits 4 and 22 shifted in), then ft = R in the lanes of
    /// `dest`; for vf0 nothing at all (as PCSX2, the runner's model)
    pub fn rnext(&mut self, ft: usize, dest: u32) {
        if ft == 0 {
            return;
        }
        let x = (self.r >> 4 ^ self.r >> 22) & 1;
        self.r = (self.r << 1 ^ x) & 0x7f_ffff | 0x3f80_0000;
        self.set(ft, dest, [f32::from_bits(self.r); 4]);
    }
    /// The integer register = fs's lane's low half
    pub fn mtir(&mut self, it: usize, fs: usize, fsf: usize) {
        if it != 0 {
            self.vi[it] = self.vf[fs][fsf].to_bits() as u16;
        }
    }
    /// `fcand`: any of the clip flags in `mask` (the answer in vi1, as the instruction leaves it)
    pub fn fcand(&mut self, mask: u32) -> bool {
        let b = self.clip & mask != 0;
        self.vi[1] = b as u16;
        b
    }
    /// `fcor`: the flags or `mask` all set (the answer in vi1)
    pub fn fcor(&mut self, mask: u32) -> bool {
        let b = ((self.clip | mask) & 0xff_ffff) == 0xff_ffff;
        self.vi[1] = b as u16;
        b
    }

    // ---- memory ----

    fn at(qa: u32) -> usize {
        (qa & 0x3ff) as usize
    }
    /// ft = memory at `qa`, in the lanes of `dest`
    pub fn lq(&mut self, ft: usize, qa: u32, dest: u32) {
        let v = self.mem[Self::at(qa)].map(f32::from_bits);
        self.set(ft, dest, v);
    }
    /// memory at `qa` = fs, in the lanes of `dest`
    pub fn sq(&mut self, fs: usize, qa: u32, dest: u32) {
        let v = self.vf[fs];
        let m = &mut self.mem[Self::at(qa)];
        for k in 0..4 {
            if dest & (8 >> k) != 0 {
                m[k] = v[k].to_bits();
            }
        }
    }
    /// The integer register `vi` stored in the lanes of `dest` at `qa`
    pub fn isw(&mut self, vi: u16, qa: u32, dest: u32) {
        let m = &mut self.mem[Self::at(qa)];
        for (k, w) in m.iter_mut().enumerate() {
            if dest & (8 >> k) != 0 {
                *w = vi as u32;
            }
        }
    }
    /// The low half of the lane `lane` at `qa`
    pub fn ilw(&self, qa: u32, lane: usize) -> u16 {
        self.mem[Self::at(qa)][lane] as u16
    }

    /// `xgkick`: the GIF packet at `qa` (its tags and their data, PACKED / REGLIST / IMAGE, to the one with EOP)
    pub fn kick(&mut self, qa: u32) {
        self.kicks.push((qa & 0x3ff, self.gs.len()));
        let mut q = qa;
        for _ in 0..0x400 {
            let tag = self.mem[Self::at(q)];
            self.gs.push(tag);
            q += 1;
            let nloop = (tag[0] & 0x7fff) as usize;
            let nreg = match (tag[1] >> 28) & 15 {
                0 => 16,
                n => n as usize,
            };
            let n = match (tag[1] >> 26) & 3 {
                0 => nloop * nreg,
                1 => (nloop * nreg).div_ceil(2),
                _ => nloop,
            };
            for _ in 0..n {
                let d = self.mem[Self::at(q)];
                self.gs.push(d);
                q += 1;
            }
            if tag[0] & 0x8000 != 0 {
                return;
            }
        }
    }
}

impl Default for Vu {
    fn default() -> Vu {
        Vu::new()
    }
}
