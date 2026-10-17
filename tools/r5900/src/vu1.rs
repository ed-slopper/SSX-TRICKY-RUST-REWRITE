//! VU1 and VIF1 as the game's VU1 chains drive them (board row G7i): a DMA source chain walked in EE memory (its
//! tags' upper halves carried as VIF codes, as with TTE set), the VIF1 codes, and VU1's micro programs run from
//! `MSCAL` / `MSCALF` / `MSCNT` until the E bit. What VU1 sends to the GS (`XGKICK`) and VIF1's `DIRECT` data
//! land in [`Vu1::gs`] as quadwords, with each packet's start in [`Vu1::kicks`].
//!
//! The model is the VU0 micro one: no pipeline (each pair's upper and lower read the registers as they were
//! before it, and results are there for the next pair), so `div` is ready at once and `waitq` waits for nothing.
//! The MAC and status flags, the EFU (`esin` and the others, `mfp`), the random unit and the I bit's integer uses
//! are not modelled: a program that reaches one fails with the instruction's place.

use crate::exec::{set_vf, vu_upper};
use crate::ps2float as pf;
use crate::{Cpu, Mem};

const MEM: usize = 0x4000;

/// VU1's registers and memories and VIF1's
pub struct Vu1 {
    /// The VU registers (vf, vi, acc, q, i and the clip flags of a `Cpu`; the rest unused)
    pub regs: Cpu,
    pub micro: Vec<u8>,
    pub data: Vec<u8>,
    /// The next program's start for `MSCNT` (in bytes)
    pub tpc: u32,
    pub base: u32,
    pub offset: u32,
    pub tops: u32,
    pub top: u32,
    pub dbf: bool,
    pub itops: u32,
    pub itop: u32,
    pub cl: u32,
    pub wl: u32,
    pub mode: u32,
    pub mask: u32,
    pub row: [u32; 4],
    pub col: [u32; 4],
    /// Every quadword sent to the GS: `XGKICK` packets (tags and data) and `DIRECT` data
    pub gs: Vec<[u32; 4]>,
    /// Each `XGKICK`'s start in VU1 memory (in quadwords) and where its packet begins in `gs`
    pub kicks: Vec<(u32, usize)>,
    /// Each `MSCAL` / `MSCALF` / `MSCNT`'s start (in bytes) and TOP when it ran
    pub calls: Vec<(u32, u32)>,
    /// With `record` set: each program call's state before and after (for checking a port of the programs)
    pub record: bool,
    pub runs: Vec<Run>,
    /// With `trace` set: each pair run (its place) and the integer registers after it, for finding where a port
    /// and the program part
    pub trace: bool,
    pub traced: Vec<(u32, [u16; 16])>,
}

/// One program call as `record` keeps it: VU1's registers, data memory, TOP and ITOP on the way in; the registers
/// and data memory on the way out; what it sent to the GS (`gs` from `gs_from`)
#[derive(Clone)]
pub struct Run {
    pub start: u32,
    pub top: u32,
    pub itop: u32,
    pub regs_in: Cpu,
    pub data_in: Vec<u8>,
    pub regs_out: Cpu,
    pub data_out: Vec<u8>,
    pub gs_from: usize,
    pub gs_to: usize,
    /// The micro memory it ran (which program)
    pub micro: Vec<u8>,
    /// Its pairs in `traced` (with `trace` set)
    pub traced_from: usize,
    pub traced_to: usize,
}

impl Default for Vu1 {
    fn default() -> Vu1 {
        Vu1 {
            regs: Cpu::new(),
            micro: vec![0; MEM],
            data: vec![0; MEM],
            tpc: 0,
            base: 0,
            offset: 0,
            tops: 0,
            top: 0,
            dbf: false,
            itops: 0,
            itop: 0,
            cl: 1,
            wl: 1,
            mode: 0,
            mask: 0,
            row: [0; 4],
            col: [0; 4],
            gs: Vec::new(),
            kicks: Vec::new(),
            calls: Vec::new(),
            record: false,
            runs: Vec::new(),
            trace: false,
            traced: Vec::new(),
        }
    }
}

fn rd32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
fn wr32(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

impl Vu1 {
    /// A quadword of data memory (its address in quadwords, wrapped to 16 KiB)
    fn qw(&self, qa: u32) -> [u32; 4] {
        let at = (qa as usize * 16) % MEM;
        [0, 1, 2, 3].map(|k| rd32(&self.data, at + 4 * k))
    }
    fn set_qw(&mut self, qa: u32, dest: u32, v: [u32; 4]) {
        let at = (qa as usize * 16) % MEM;
        for k in 0..4 {
            if dest & (8 >> k) != 0 {
                wr32(&mut self.data, at + 4 * k, v[k]);
            }
        }
    }

    /// The DMA source chain from `tadr` in EE memory sent to VIF1: each tag's upper half and its data as VIF
    /// codes (`cnt`, `next`, `ref`, `refs`, `call`, `ret`, `end`, `refe`). The number of tags walked
    pub fn run_chain(&mut self, mem: &mut Mem, tadr: u32) -> Result<usize, String> {
        let mut at = tadr;
        let mut stack: Vec<u32> = Vec::new();
        let mut stream: Vec<u32> = Vec::new();
        let mut is_tag: Vec<bool> = Vec::new();
        for n in 1..100_000 {
            let mut tag = [0u8; 16];
            mem.read(at, &mut tag).ok_or(format!("DMA tag outside memory at {at:#x}"))?;
            let lo = u32::from_le_bytes(tag[0..4].try_into().unwrap());
            let addr = u32::from_le_bytes(tag[4..8].try_into().unwrap()) & 0x7fff_fff0;
            let (qwc, id) = (lo & 0xffff, (lo >> 28) & 7);
            stream.push(u32::from_le_bytes(tag[8..12].try_into().unwrap()));
            stream.push(u32::from_le_bytes(tag[12..16].try_into().unwrap()));
            is_tag.extend([true, true]);
            let data_at = match id {
                0 | 3 | 4 => addr,
                _ => at + 16,
            };
            let mut data = vec![0u8; 16 * qwc as usize];
            mem.read(data_at, &mut data).ok_or(format!("DMA data outside memory at {data_at:#x}"))?;
            stream.extend(data.chunks(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())));
            is_tag.resize(stream.len(), false);
            let after = at + 16 + 16 * qwc;
            let next = match id {
                0 => None,
                1 => Some(after),
                2 => Some(addr),
                3 | 4 => Some(at + 16),
                5 => {
                    stack.push(after);
                    Some(addr)
                }
                6 => stack.pop(),
                _ => None,
            };
            match next {
                Some(t) => at = t,
                None => {
                    self.vif_tagged(&stream, &is_tag)?;
                    return Ok(n);
                }
            }
        }
        Err("DMA chain never ended".into())
    }

    /// VIF1 codes. `MPG` fills micro memory, `UNPACK` data memory (every format, with the cycle, the mode
    /// and the mask), `DIRECT` the GS log; the calls run the program.
    pub fn vif(&mut self, w: &[u32]) -> Result<(), String> {
        self.vif_tagged(w, &vec![false; w.len()])
    }

    /// As [`vif`](Self::vif), with the words that are DMA tags' upper halves marked: those are VIF codes when a
    /// code is due, but a code's data skips them (an `UNPACK` going on from a `cnt` into a `ref`'s data)
    fn vif_tagged(&mut self, all: &[u32], is_tag: &[bool]) -> Result<(), String> {
        let mut i = 0usize;
        // `take(i, n)`: the next n data words from i, skipping tag words; and where they end
        let take = |i: usize, n: usize| -> Option<(Vec<u32>, usize)> {
            let mut out = Vec::with_capacity(n);
            let mut j = i;
            while out.len() < n {
                if j >= all.len() {
                    return None;
                }
                if !is_tag[j] {
                    out.push(all[j]);
                }
                j += 1;
            }
            Some((out, j))
        };
        while i < all.len() {
            let code = all[i];
            i += 1;
            let (cmd, num, imm) = ((code >> 24) & 0x7f, (code >> 16) & 0xff, code & 0xffff);
            match cmd {
                0x00 | 0x06 | 0x07 | 0x10 | 0x11 | 0x13 => {} // NOP, MSKPATH3, MARK, FLUSHE, FLUSH, FLUSHA
                0x01 => {
                    self.cl = imm & 0xff;
                    self.wl = imm >> 8;
                }
                0x02 => {
                    self.offset = imm & 0x3ff;
                    self.dbf = false;
                    self.tops = self.base;
                }
                0x03 => self.base = imm & 0x3ff,
                0x04 => self.itops = imm & 0x3ff,
                0x05 => self.mode = imm & 3,
                0x14 | 0x15 | 0x17 => {
                    let start = if cmd == 0x17 { self.tpc } else { imm * 8 };
                    self.itop = self.itops;
                    self.top = self.tops & 0x3ff;
                    if self.dbf {
                        self.tops = self.base;
                    } else {
                        self.tops = self.base + self.offset;
                    }
                    self.dbf = !self.dbf;
                    self.calls.push((start, self.top));
                    let before = self.record.then(|| (self.regs.clone(), self.data.clone(), self.gs.len()));
                    let traced_from = self.traced.len();
                    self.exec(start)?;
                    if let Some((regs_in, data_in, gs_from)) = before {
                        let (regs_out, data_out, gs_to) = (self.regs.clone(), self.data.clone(), self.gs.len());
                        self.runs.push(Run { start, top: self.top, itop: self.itop, regs_in, data_in, regs_out, data_out, gs_from, gs_to, micro: self.micro.clone(), traced_from, traced_to: self.traced.len() });
                    }
                }
                0x20 => {
                    let (d, j) = take(i, 1).ok_or("STMASK past the data")?;
                    self.mask = d[0];
                    i = j;
                }
                0x30 | 0x31 => {
                    let (d, j) = take(i, 4).ok_or("STROW / STCOL past the data")?;
                    let v: [u32; 4] = d.try_into().unwrap();
                    if cmd == 0x30 {
                        self.row = v;
                    } else {
                        self.col = v;
                    }
                    i = j;
                }
                0x4a => {
                    // (the program follows the code at once: a stream puts the code where its program lands on 64
                    // bits)
                    let n = if num == 0 { 256 } else { num as usize };
                    let (code, j) = take(i, 2 * n).ok_or("MPG past the data")?;
                    for (k, x) in code.iter().enumerate() {
                        let at = (imm as usize * 8 + 4 * k) % MEM;
                        wr32(&mut self.micro, at, *x);
                    }
                    i = j;
                }
                0x50 | 0x51 => {
                    let n = if imm == 0 { 0x10000 } else { imm as usize };
                    let (d, j) = take(i, 4 * n).ok_or("DIRECT past the data")?;
                    self.gs.extend(d.chunks(4).map(|c| [c[0], c[1], c[2], c[3]]));
                    i = j;
                }
                0x60..=0x7f => {
                    let (d, j) = take(i, self.unpack_words(cmd, num)).ok_or("UNPACK past the data")?;
                    self.unpack(cmd, num, imm, &d)?;
                    i = j;
                }
                _ => return Err(format!("VIF code {code:#010x}")),
            }
        }
        Ok(())
    }

    /// The words an `UNPACK` reads: its vectors that carry data (all with CL ≥ WL; with WL > CL, CL of each WL)
    fn unpack_words(&self, cmd: u32, num: u32) -> usize {
        let (vn, vl) = ((cmd >> 2) & 3, cmd & 3);
        let n = if num == 0 { 256 } else { num as usize };
        let (cl, wl) = (self.cl.max(1) as usize, self.wl.max(1) as usize);
        let data_vectors = if wl <= cl { n } else { (n / wl) * cl + (n % wl).min(cl) };
        let bits = if vl == 3 { 16 } else { [32usize, 16, 8][vl as usize] * (vn as usize + 1) };
        (bits * data_vectors).div_ceil(32)
    }

    /// One `UNPACK`: `num` vectors to data memory from `imm`'s address (+ TOPS with bit 15), written by the cycle
    /// (CL ≥ WL skips, WL > CL fills from the row and column by the mask); the words read
    fn unpack(&mut self, cmd: u32, num: u32, imm: u32, w: &[u32]) -> Result<usize, String> {
        let (vn, vl, masked) = ((cmd >> 2) & 3, cmd & 3, cmd & 0x10 != 0);
        let usn = imm & 0x4000 != 0;
        let mut addr = imm & 0x3ff;
        if imm & 0x8000 != 0 {
            addr += self.tops;
        }
        if vl == 3 && vn != 3 {
            return Err(format!("UNPACK {cmd:#x}: no such format"));
        }
        let lanes = (vn + 1) as usize;
        let bits = [32usize, 16, 8, 5][vl as usize];
        let n = if num == 0 { 256 } else { num as usize };
        let (cl, wl) = (self.cl.max(1) as usize, self.wl.max(1) as usize);
        // the vectors that carry data: all with CL ≥ WL; with WL > CL, CL of each WL
        let data_vectors = if wl <= cl { n } else { (n / wl) * cl + (n % wl).min(cl) };
        let total_bits = if vl == 3 { 16 * data_vectors } else { bits * lanes * data_vectors };
        let words = total_bits.div_ceil(32);
        let src = w.get(..words).ok_or("UNPACK past the data")?;
        let bytes: Vec<u8> = src.iter().flat_map(|x| x.to_le_bytes()).collect();
        let elem = |k: usize| -> u32 {
            match bits {
                32 => u32::from_le_bytes(bytes[4 * k..4 * k + 4].try_into().unwrap()),
                16 => {
                    let v = u16::from_le_bytes(bytes[2 * k..2 * k + 2].try_into().unwrap());
                    if usn { v as u32 } else { v as i16 as i32 as u32 }
                }
                _ => {
                    let v = bytes[k];
                    if usn { v as u32 } else { v as i8 as i32 as u32 }
                }
            }
        };
        let mut next = 0usize;
        for v in 0..n {
            let (cycle, filling) = if wl <= cl { (v % wl, false) } else { (v % wl, v % wl >= cl) };
            let qa = if wl <= cl { addr + ((v / wl) * cl + v % wl) as u32 } else { addr + v as u32 };
            let mut val = [0u32; 4];
            if !filling {
                if vl == 3 {
                    let h = u16::from_le_bytes(bytes[2 * next..2 * next + 2].try_into().unwrap()) as u32;
                    val = [(h & 0x1f) << 3, ((h >> 5) & 0x1f) << 3, ((h >> 10) & 0x1f) << 3, ((h >> 15) & 1) << 7];
                } else {
                    let e: Vec<u32> = (0..lanes).map(|k| elem(next * lanes + k)).collect();
                    val = match lanes {
                        1 => [e[0]; 4],
                        2 => [e[0], e[1], e[0], e[1]],
                        3 => [e[0], e[1], e[2], 0],
                        _ => [e[0], e[1], e[2], e[3]],
                    };
                }
                next += 1;
            }
            let old = self.qw(qa);
            let mut out = old;
            for k in 0..4 {
                let m = if masked { (self.mask >> (2 * (cycle.min(3) * 4 + k))) & 3 } else { 0 };
                let m = if filling && !masked { 1 } else { m };
                out[k] = match m {
                    0 => match self.mode {
                        1 => val[k].wrapping_add(self.row[k]),
                        2 => {
                            let r = val[k].wrapping_add(self.row[k]);
                            self.row[k] = r;
                            r
                        }
                        _ => val[k],
                    },
                    1 => self.row[k],
                    2 => self.col[cycle.min(3)],
                    _ => old[k],
                };
                // V3 leaves w alone when not masked (a later vector's x on the hardware: not modelled)
                if lanes == 3 && k == 3 && m == 0 && !filling {
                    out[k] = old[k];
                }
            }
            self.set_qw(qa, 15, out);
        }
        Ok(words)
    }

    /// The micro program from byte `start` until the pair after the one with the E bit
    pub fn exec(&mut self, start: u32) -> Result<(), String> {
        let mut pc = start & 0x3ff8;
        let (mut ending, mut branch): (bool, Option<u32>) = (false, None);
        for _ in 0..1_000_000 {
            let at = pc as usize;
            let lo = rd32(&self.micro, at);
            let hi = rd32(&self.micro, at + 4);
            let err = |what: String| format!("vu1 micro {pc:#x}: {what}");
            let before = self.regs.clone();
            if hi >> 31 != 0 {
                self.regs.i = f32::from_bits(lo);
            }
            vu_upper(&mut self.regs, hi).map_err(err)?;
            let mut next = pc + 8;
            if let Some(t) = branch.take() {
                next = t;
            }
            if hi >> 31 == 0 {
                if let Some(t) = self.lower(&before, lo, pc).map_err(err)? {
                    branch = Some(t);
                }
            }
            if self.trace {
                self.traced.push((pc, self.regs.vi));
            }
            if ending {
                self.tpc = next & 0x3ff8;
                return Ok(());
            }
            if hi >> 30 & 1 != 0 {
                ending = true;
            }
            pc = next & 0x3ff8;
        }
        Err(format!("vu1 micro program from {start:#x} never ended"))
    }

    /// A lower op (reading the registers as they were before the pair, `s`); a branch's target
    fn lower(&mut self, s: &Cpu, w: u32, pc: u32) -> Result<Option<u32>, String> {
        let op = w >> 25;
        let dest = (w >> 21) & 15;
        let (it, is, id) = (((w >> 16) & 15) as usize, ((w >> 11) & 15) as usize, ((w >> 6) & 15) as usize);
        let (ft, fs) = (((w >> 16) & 31) as usize, ((w >> 11) & 31) as usize);
        let imm11 = w & 0x7ff;
        let simm11 = if imm11 & 0x400 != 0 { imm11 as i32 - 0x800 } else { imm11 as i32 };
        let target = pc.wrapping_add(8).wrapping_add((simm11 * 8) as u32);
        let imm15 = ((w & 0x7ff) | ((w >> 21) & 0xf) << 11) as u16;
        let imm24 = w & 0xff_ffff;
        let (a, b) = (s.vi[is], s.vi[it]);
        let c = &mut self.regs;
        let set = |c: &mut Cpu, r: usize, v: u16| {
            if r != 0 {
                c.vi[r] = v;
            }
        };
        let bits = |v: [f32; 4]| v.map(f32::to_bits);
        let floats = |v: [u32; 4]| v.map(f32::from_bits);
        let link = ((pc + 16) / 8) as u16;
        match op {
            0x40 => {
                let f = w & 0x3f;
                if f >= 0x3c {
                    let k = ((w >> 6) & 0x1f) << 2 | (w & 3);
                    let fsf = ((w >> 21) & 3) as usize;
                    let ftf = ((w >> 23) & 3) as usize;
                    match k {
                        0x30 => set_vf(c, ft, dest, s.vf[fs]),                         // move
                        0x31 => set_vf(c, ft, dest, [s.vf[fs][1], s.vf[fs][2], s.vf[fs][3], s.vf[fs][0]]), // mr32
                        0x34 | 0x36 => {
                            // lqi ft, (is++) / lqd ft, (--is)
                            let qa = if k == 0x34 { a } else { a.wrapping_sub(1) };
                            let v = floats(self.qw(qa as u32));
                            let c = &mut self.regs;
                            set_vf(c, ft, dest, v);
                            set(c, is, if k == 0x34 { a.wrapping_add(1) } else { qa });
                        }
                        0x35 | 0x37 => {
                            // sqi fs, (it++) / sqd fs, (--it)
                            let qa = if k == 0x35 { b } else { b.wrapping_sub(1) };
                            self.set_qw(qa as u32, dest, bits(s.vf[fs]));
                            set(&mut self.regs, it, if k == 0x35 { b.wrapping_add(1) } else { qa });
                        }
                        0x38 => c.q = pf::div(s.vf[fs][fsf], s.vf[ft][ftf]),         // div Q, fs.fsf, ft.ftf
                        0x39 => c.q = pf::sqrt(s.vf[ft][ftf]),                       // sqrt Q, ft.ftf
                        0x3a => c.q = pf::div(s.vf[fs][fsf], pf::sqrt(s.vf[ft][ftf])), // rsqrt
                        0x3b => {}                                                   // waitq
                        0x3c => set(c, it, s.vf[fs][fsf].to_bits() as u16),          // mtir it, fs.fsf
                        0x3d => {
                            // mfir.dest ft, is
                            let v = f32::from_bits(a as i16 as i32 as u32);
                            set_vf(c, ft, dest, [v; 4]);
                        }
                        0x3e => {
                            // ilwr.dest it, (is): the one lane's low half
                            let q = self.qw(a as u32);
                            let lane = (0..4).find(|k| dest & (8 >> k) != 0).unwrap_or(0);
                            set(&mut self.regs, it, q[lane] as u16);
                        }
                        0x3f => {
                            // iswr.dest it, (is)
                            self.set_qw(a as u32, dest, [b as u32; 4]);
                        }
                        0x68 => set(c, it, self.top as u16),  // xtop
                        0x69 => set(c, it, self.itop as u16), // xitop
                        0x6c => {
                            // xgkick is: the GIF packet at is sent
                            let start = a as u32;
                            self.kick(start)?;
                        }
                        _ => return Err(format!("lower special {k:#x}")),
                    }
                    return Ok(None);
                }
                match f {
                    0x30 => set(c, id, a.wrapping_add(b)), // iadd
                    0x31 => set(c, id, a.wrapping_sub(b)), // isub
                    0x32 => {
                        let i5 = ((w >> 6) & 31) as i32;
                        set(c, it, a.wrapping_add((if i5 & 16 != 0 { i5 - 32 } else { i5 }) as u16)) // iaddi
                    }
                    0x34 => set(c, id, a & b), // iand
                    0x35 => set(c, id, a | b), // ior
                    _ => return Err(format!("lower {f:#x}")),
                }
            }
            0x00 => {
                // lq.dest ft, imm(is)
                let v = floats(self.qw((a as i32 + simm11) as u32));
                set_vf(&mut self.regs, ft, dest, v);
            }
            0x01 => {
                // sq.dest fs, imm(it)
                self.set_qw((b as i32 + simm11) as u32, dest, bits(s.vf[fs]));
            }
            0x04 => {
                // ilw.dest it, imm(is)
                let q = self.qw((a as i32 + simm11) as u32);
                let lane = (0..4).find(|k| dest & (8 >> k) != 0).unwrap_or(0);
                set(&mut self.regs, it, q[lane] as u16);
            }
            0x05 => {
                // isw.dest it, imm(is)
                self.set_qw((a as i32 + simm11) as u32, dest, [b as u32; 4]);
            }
            0x08 => set(c, it, a.wrapping_add(imm15)), // iaddiu
            0x09 => set(c, it, a.wrapping_sub(imm15)), // isubiu
            0x10 => set(c, 1, ((s.clip & 0xff_ffff) == imm24) as u16), // fceq
            0x11 => c.clip = imm24,                                     // fcset
            0x12 => set(c, 1, (s.clip & imm24 != 0) as u16),           // fcand
            0x13 => set(c, 1, (((s.clip | imm24) & 0xff_ffff) == 0xff_ffff) as u16), // fcor
            0x1c => set(c, it, (s.clip & 0xfff) as u16),               // fcget
            0x20 => return Ok(Some(target)),                           // b
            0x21 => {
                set(c, it, link); // bal
                return Ok(Some(target));
            }
            0x24 => return Ok(Some(a as u32 * 8)), // jr
            0x25 => {
                set(c, it, link); // jalr
                return Ok(Some(a as u32 * 8));
            }
            0x28 => return Ok((b == a).then_some(target)), // ibeq
            0x29 => return Ok((b != a).then_some(target)), // ibne
            0x2c => return Ok(((a as i16) < 0).then_some(target)),
            0x2d => return Ok(((a as i16) > 0).then_some(target)),
            0x2e => return Ok(((a as i16) <= 0).then_some(target)),
            0x2f => return Ok(((a as i16) >= 0).then_some(target)),
            _ => return Err(format!("lower op {op:#x}")),
        }
        Ok(None)
    }

    /// `XGKICK`: the GIF packet at quadword `start` (tags and their data, PACKED / REGLIST / IMAGE, until EOP)
    /// appended to the GS log
    fn kick(&mut self, start: u32) -> Result<(), String> {
        self.kicks.push((start & 0x3ff, self.gs.len()));
        let mut qa = start;
        for _ in 0..0x400 {
            let tag = self.qw(qa);
            self.gs.push(tag);
            qa += 1;
            let nloop = (tag[0] & 0x7fff) as usize;
            let eop = tag[0] & 0x8000 != 0;
            let flg = (tag[1] >> 26) & 3;
            let nreg = match (tag[1] >> 28) & 15 {
                0 => 16,
                n => n as usize,
            };
            let qwords = match flg {
                0 => nloop * nreg,
                1 => (nloop * nreg).div_ceil(2),
                _ => nloop,
            };
            for _ in 0..qwords {
                let q = self.qw(qa);
                self.gs.push(q);
                qa += 1;
            }
            if eop {
                return Ok(());
            }
        }
        Err(format!("XGKICK at {start:#x}: no EOP"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn upper(dest: u32, ft: u32, fs: u32, fd: u32, fun: u32) -> u32 {
        (dest << 21) | (ft << 16) | (fs << 11) | (fd << 6) | fun
    }
    const NOP_U: u32 = 0x2ff; // vnop
    const NOP_L: u32 = 0x8000_033c; // move. vf0, vf0 (as the game's programs write the lower nop)
    fn lower_special(dest: u32, ft: u32, fs: u32, k: u32) -> u32 {
        (0x40 << 25) | (dest << 21) | (ft << 16) | (fs << 11) | ((k >> 2) << 6) | 0x3c | (k & 3)
    }
    fn lower_i(op: u32, dest: u32, it: u32, is: u32, imm11: i32) -> u32 {
        (op << 25) | (dest << 21) | (it << 16) | (is << 11) | (imm11 as u32 & 0x7ff)
    }
    fn program(v: &mut Vu1, at: usize, pairs: &[(u32, u32)]) {
        for (k, (hi, lo)) in pairs.iter().enumerate() {
            wr32(&mut v.micro, at + 8 * k, *lo);
            wr32(&mut v.micro, at + 8 * k + 4, *hi);
        }
    }

    #[test]
    fn unpack_v4_32_with_tops_and_skipping() {
        let mut v = Vu1::default();
        // BASE 0x10, OFFSET 0x20, STCYCL 2/1 (write one of each two), UNPACK V4-32 2 vectors to TOPS + 1
        let words = [0x0300_0010u32, 0x0200_0020, 0x0100_0102, 0x6c02_8001, 1, 2, 3, 4, 5, 6, 7, 8];
        v.vif(&words).unwrap();
        assert_eq!(v.qw(0x11), [1, 2, 3, 4]);
        assert_eq!(v.qw(0x13), [5, 6, 7, 8], "CL 2 skips one");
        // V2-16 signed, V1-8 unsigned
        v.vif(&[0x0100_0101, 0x6501_0040, 0x0002_ffff, 0x6201_4041, 0x0000_00ff]).unwrap();
        assert_eq!(v.qw(0x40), [u32::MAX, 2, u32::MAX, 2]);
        assert_eq!(v.qw(0x41), [0xff; 4]);
    }

    #[test]
    fn mscal_double_buffers_and_runs() {
        let mut v = Vu1::default();
        // the program at 0: xtop vi1; [E] then sq vf1 at 0(vi1); vf1 = (1,2,3,4) via I and lower immediates
        program(&mut v, 0, &[(NOP_U, lower_special(0, 1, 0, 0x68)), (NOP_U | (1 << 30), lower_i(0x01, 15, 1, 1, 0)), (NOP_U, NOP_L)]);
        v.regs.vf[1] = [1.0, 2.0, 3.0, 4.0];
        v.vif(&[0x0300_0010, 0x0200_0020, 0x1400_0000, 0x1400_0000, 0x1400_0000]).unwrap();
        assert_eq!(v.calls.iter().map(|c| c.1).collect::<Vec<_>>(), vec![0x10, 0x30, 0x10], "TOP alternates base, base + offset");
        assert_eq!(floats(v.qw(0x30)), [1.0, 2.0, 3.0, 4.0]);
        fn floats(q: [u32; 4]) -> [f32; 4] {
            q.map(f32::from_bits)
        }
    }

    #[test]
    fn lower_ops_and_kick() {
        let mut v = Vu1::default();
        // vi1 = 5 (iaddiu); lqi vf2, (vi1++); vf3 = vf2 * I (I = 2); sqi vf3, (vi2++); div Q = vf2.x / vf2.y;
        // vf4 = vf2 * Q; bal vi15 to +2 (skipping one pair after the delay slot); xgkick vi3
        let two = 2f32.to_bits();
        let pairs = [
            (NOP_U, lower_i(0x08, 0, 1, 0, 5)),
            (NOP_U, lower_i(0x08, 0, 2, 0, 9)),
            (NOP_U, lower_special(15, 2, 1, 0x34)),
            (upper(15, 0, 2, 3, 30) | (1 << 31), two),
            (NOP_U, lower_special(15, 2, 3, 0x35)),
            (NOP_U, lower_special(4, 2, 2, 0x38)),
            (upper(15, 0, 2, 4, 28), lower_i(0x21, 0, 15, 0, 2)),
            (NOP_U, NOP_L),
            (NOP_U, lower_i(0x08, 0, 6, 0, 99)),
            (NOP_U, lower_i(0x08, 0, 3, 0, 0x20)),
            (NOP_U, lower_special(0, 0, 3, 0x6c)),
            (NOP_U | (1 << 30), NOP_L),
            (NOP_U, NOP_L),
        ];
        program(&mut v, 0, &pairs);
        v.set_qw(5, 15, [8f32.to_bits(), 2f32.to_bits(), 0, 0]);
        // a GIF tag at 0x20: NLOOP 1, EOP, PACKED, NREG 1, then one quadword
        v.set_qw(0x20, 15, [0x8001, 1 << 28, 0xe, 0]);
        v.set_qw(0x21, 15, [7, 7, 7, 7]);
        v.exec(0).unwrap();
        assert_eq!(v.regs.vi[1], 6, "lqi's post-increment");
        assert_eq!(v.regs.vf[3], [16.0, 4.0, 0.0, 0.0], "muli");
        assert_eq!(v.qw(9), [16f32.to_bits(), 4f32.to_bits(), 0, 0], "sqi");
        assert_eq!(v.regs.vi[2], 10);
        assert_eq!(v.regs.q, 4.0, "div Q = 8 / 2");
        assert_eq!(v.regs.vf[4], [32.0, 8.0, 0.0, 0.0], "mulq");
        assert_eq!(v.regs.vi[15], 8, "bal links past the delay slot");
        assert_eq!(v.regs.vi[6], 0, "the pair bal jumps over");
        assert_eq!(v.gs, vec![[0x8001, 1 << 28, 0xe, 0], [7, 7, 7, 7]]);
        assert_eq!(v.kicks, vec![(0x20, 0)]);
    }

    #[test]
    fn calls_tops_and_the_delay_slot() {
        let mut v = Vu1::default();
        // at 0: xtop vi1; isw.x vi1, 0x40(vi0); [E] iaddiu vi2, vi2, 1; then (the delay slot) iaddiu vi3, vi3, 2
        // at 0x20 (MSCAL 4): iadd vi4, vi3, vi2 [E]; iaddiu vi5, vi4, 7
        program(&mut v, 0, &[
            (NOP_U, lower_special(0, 1, 0, 0x68)),
            (NOP_U, lower_i(0x05, 8, 1, 0, 0x40)),
            (NOP_U | (1 << 30), lower_i(0x08, 0, 2, 2, 1)),
            (NOP_U, lower_i(0x08, 0, 3, 3, 2)),
            (NOP_U | (1 << 30), (0x40 << 25) | (2 << 16) | (3 << 11) | (4 << 6) | 0x30),
            (NOP_U, lower_i(0x08, 0, 5, 4, 7)),
        ]);
        v.vif(&[0x0300_0010, 0x0200_0020]).unwrap();
        let mut tops = Vec::new();
        for _ in 0..4 {
            v.vif(&[0x1400_0000]).unwrap();
            tops.push(v.qw(0x40)[0]);
        }
        assert_eq!(tops, vec![0x10, 0x30, 0x10, 0x30], "xtop: TOP by the double buffer");
        assert_eq!((v.regs.vi[2], v.regs.vi[3]), (4, 8), "the pair after E runs");
        assert_eq!(v.tpc, 0x20);
        v.vif(&[0x1700_0000]).unwrap();
        assert_eq!((v.regs.vi[4], v.regs.vi[5]), (12, 19), "MSCNT goes on at the end; iadd");
        v.vif(&[0x1400_0004]).unwrap();
        assert_eq!(v.calls.last(), Some(&(0x20, 0x30)), "MSCAL's start in 8 bytes");
    }

    #[test]
    fn mpg_unpack_formats_and_offsets() {
        let mut v = Vu1::default();
        // an MPG after a NOP: its program follows at once
        v.vif(&[0, 0x4a01_0003, 0x1111_1111, 0x2222_2222]).unwrap();
        assert_eq!((rd32(&v.micro, 0x18), rd32(&v.micro, 0x1c)), (0x1111_1111, 0x2222_2222));
        // V4-8 signed and unsigned, S-8 signed
        v.vif(&[0x0100_0101, 0x6e01_0010, 0x80ff_0102, 0x6e01_4011, 0x80ff_0102, 0x6201_0012, 0x0000_0080]).unwrap();
        assert_eq!(v.qw(0x10), [2, 1, u32::MAX, (-128i32) as u32]);
        assert_eq!(v.qw(0x11), [2, 1, 0xff, 0x80]);
        assert_eq!(v.qw(0x12), [(-128i32) as u32; 4]);
        // lq / sq at offsets from a base; iaddiu on a base
        program(&mut v, 0x100, &[
            (NOP_U, lower_i(0x08, 0, 1, 0, 0x10)),
            (NOP_U, lower_i(0x08, 0, 2, 1, 0x30)),
            (NOP_U, lower_i(0x00, 15, 6, 1, 2)),
            (NOP_U | (1 << 30), lower_i(0x01, 0b1010, 2, 6, -1)),
            (NOP_U, NOP_L),
        ]);
        v.exec(0x100).unwrap();
        assert_eq!(v.regs.vi[2], 0x40);
        assert_eq!(v.qw(0x3f), [(-128i32) as u32, 0, (-128i32) as u32, 0], "sq.xz of lq 2(vi1) at −1(vi2)");
    }

    #[test]
    fn kicks_and_chains() {
        let mut r = crate::Runner::empty();
        let mut v = Vu1::default();
        // two tags: REGLIST NLOOP 3 NREG 1 (two quadwords), then IMAGE NLOOP 2 with EOP; a later kick's place
        v.set_qw(0x50, 15, [3, (1 << 28) | (1 << 26), 0, 0]);
        v.set_qw(0x53, 15, [0x8002, 2 << 26, 0, 0]);
        v.set_qw(0x60, 15, [0x8001, 2 << 28, 0, 0]);
        v.kick(0x50).unwrap();
        v.kick(0x60).unwrap();
        assert_eq!(v.gs.len(), 6 + 3);
        assert_eq!(v.kicks, vec![(0x50, 0), (0x60, 6)]);
        // NLOOP's whole 15 bits (the data read round the 16 KiB)
        v.set_qw(0x70, 15, [0xc001, 2 << 26, 0, 0]);
        v.kick(0x70).unwrap();
        assert_eq!(v.gs.len(), 9 + 1 + 0x4001);
        // a chain: cnt (an UNPACK), call to a ref'd block that returns, end
        let (a, b, d) = (0x1000u32, 0x2000u32, 0x3000u32);
        let put = |r: &mut crate::Runner, at: u32, w: &[u32]| {
            for (k, x) in w.iter().enumerate() {
                r.mem.write_u32(at + 4 * k as u32, *x);
            }
        };
        put(&mut r, a, &[0x1000_0001, 0, 0, 0x6c01_0000, 1, 2, 3, 4]);
        put(&mut r, a + 0x20, &[0x5000_0000, b, 0, 0]);
        put(&mut r, a + 0x30, &[0x7000_0000, 0, 0, 0]);
        put(&mut r, b, &[0x3000_0001, d, 0, 0x6c01_0001]);
        put(&mut r, b + 0x10, &[0x4000_0001, d + 0x10, 0, 0x6c01_0002]);
        put(&mut r, b + 0x20, &[0x6000_0000, 0, 0, 0]);
        put(&mut r, d + 0x10, &[9, 10, 11, 12]);
        put(&mut r, d, &[5, 6, 7, 8]);
        assert_eq!(v.run_chain(&mut r.mem, a).unwrap(), 6);
        // an UNPACK of two vectors begun in a cnt and finished in a ref's data: the ref tag's own VIF words (here
        // a code that would fail) are skipped while the data runs on
        let e = 0x4000u32;
        put(&mut r, e, &[0x1000_0001, 0, 0, 0x6c02_0010]);
        put(&mut r, e + 0x10, &[21, 22, 23, 24]);
        put(&mut r, e + 0x20, &[0x3000_0001, e + 0x40, 0x7f00_0000, 0x7f00_0000]);
        put(&mut r, e + 0x30, &[0x7000_0000, 0, 0, 0]);
        put(&mut r, e + 0x40, &[25, 26, 27, 28]);
        v.run_chain(&mut r.mem, e).unwrap();
        assert_eq!((v.qw(0x10), v.qw(0x11)), ([21, 22, 23, 24], [25, 26, 27, 28]));
        assert_eq!((v.qw(0), v.qw(1), v.qw(2)), ([1, 2, 3, 4], [5, 6, 7, 8], [9, 10, 11, 12]), "cnt, ref, refs");
    }
}
