//! Runs single functions of the original SSX Tricky (PS2 NTSC-U, `SLUS_203.26`) so tests can compare tricky-rs
//! with them. Board row F4a; the design is `tricky-rs/docs/checking.md`.
//!
//! The executable is the player's own copy, found through `TRICKY_ELF`; nothing of the game is in this crate.
//!
//! ```no_run
//! let Some(mut r) = r5900::Runner::from_env() else { return };   // no TRICKY_ELF: the test skips
//! let boarder = r.mem.alloc(0x10);
//! r.cpu.set_f(12, 1000.0);
//! r.cpu.set_gpr(4, boarder as u64);
//! r.call(0x109cb8).unwrap();
//! let drag = r.cpu.f(0);
//! ```

mod exec;
pub mod md5;
pub mod ps2float;

use std::collections::{HashMap, HashSet};
use std::fmt;

/// The build every address belongs to.
pub const ELF_MD5: &str = "162580fd65611e72a90deed640a70aef";
/// Where a called function returns to; reaching it ends the run.
pub const RETURN_SENTINEL: u32 = 0xFFFF_FFF0;
const RAM_SIZE: usize = 32 << 20;
const SCRATCHPAD: u32 = 0x7000_0000;
const SCRATCHPAD_SIZE: usize = 16 << 10;
/// Test allocations grow up from here; the stack grows down from `STACK_TOP`.
const HEAP_BASE: u32 = 0x0180_0000;
const STACK_TOP: u32 = 0x01FF_0000;
const STEP_LIMIT: u64 = 5_000_000;

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// The ELF is not `SLUS_203.26` or not an ELF at all.
    BadElf(String),
    /// An address outside RAM and the scratchpad.
    BadAddress { pc: u32, addr: u32 },
    /// An instruction the runner does not implement yet.
    Unimplemented { pc: u32, word: u32, what: String },
    /// A call to a function with no stub that the test did not allow to run.
    UnstubbedCall { pc: u32, target: u32, name: String },
    /// The step limit was reached (a loop that never ends, or a call that never returns).
    TooLong { pc: u32 },
    /// `break` or `teq` trapped (integer division by zero, for one).
    Trap { pc: u32 },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::BadElf(s) => write!(f, "not SLUS_203.26: {s}"),
            Error::BadAddress { pc, addr } => write!(f, "0x{pc:06x}: access to unmapped address 0x{addr:08x}"),
            Error::Unimplemented { pc, word, what } => write!(f, "0x{pc:06x}: {what} (0x{word:08x}) not implemented"),
            Error::UnstubbedCall { pc, target, name } => write!(f, "0x{pc:06x}: unstubbed call to 0x{target:06x} ({name})"),
            Error::TooLong { pc } => write!(f, "step limit reached at 0x{pc:06x}"),
            Error::Trap { pc } => write!(f, "0x{pc:06x}: trap"),
        }
    }
}

impl std::error::Error for Error {}

/// EE RAM (32 MB, also seen through the uncached mirrors) and the 16 KB scratchpad.
pub struct Mem {
    ram: Vec<u8>,
    spr: Vec<u8>,
    heap: u32,
}

impl Mem {
    fn new() -> Mem {
        Mem { ram: vec![0; RAM_SIZE], spr: vec![0; SCRATCHPAD_SIZE], heap: HEAP_BASE }
    }

    fn slice(&mut self, addr: u32, len: usize) -> Option<&mut [u8]> {
        if (SCRATCHPAD..SCRATCHPAD + SCRATCHPAD_SIZE as u32).contains(&addr) {
            let o = (addr - SCRATCHPAD) as usize;
            return self.spr.get_mut(o..o + len);
        }
        let phys = match addr >> 28 {
            0 | 2 | 3 => addr & 0x0FFF_FFFF,   // kuseg, uncached, uncached accelerated
            8 | 0xA => addr & 0x1FFF_FFFF, // kseg0, kseg1
            _ => return None,
        } as usize;
        self.ram.get_mut(phys..phys + len)
    }

    pub fn read(&mut self, addr: u32, out: &mut [u8]) -> Option<()> {
        let n = out.len();
        out.copy_from_slice(self.slice(addr, n)?);
        Some(())
    }

    pub fn write(&mut self, addr: u32, data: &[u8]) -> Option<()> {
        self.slice(addr, data.len())?.copy_from_slice(data);
        Some(())
    }

    /// A zeroed block for test inputs, 16-byte aligned. Panics past the stack.
    pub fn alloc(&mut self, size: u32) -> u32 {
        let at = self.heap;
        self.heap = (self.heap + size + 15) & !15;
        assert!(self.heap < STACK_TOP - 0x10_0000, "test heap full");
        self.write(at, &vec![0; size as usize]).unwrap();
        at
    }

    pub fn write_u8(&mut self, addr: u32, v: u8) {
        self.write(addr, &[v]).expect("unmapped");
    }
    pub fn write_u16(&mut self, addr: u32, v: u16) {
        self.write(addr, &v.to_le_bytes()).expect("unmapped");
    }
    pub fn write_u32(&mut self, addr: u32, v: u32) {
        self.write(addr, &v.to_le_bytes()).expect("unmapped");
    }
    pub fn write_f32(&mut self, addr: u32, v: f32) {
        self.write_u32(addr, v.to_bits());
    }
    pub fn read_u32(&mut self, addr: u32) -> u32 {
        let mut b = [0; 4];
        self.read(addr, &mut b).expect("unmapped");
        u32::from_le_bytes(b)
    }
    pub fn read_f32(&mut self, addr: u32) -> f32 {
        f32::from_bits(self.read_u32(addr))
    }
}

/// The registers: 128-bit GPRs, the FPU, and VU0 as the core sees it in macro mode.
#[derive(Clone)]
pub struct Cpu {
    pub gpr: [u128; 32],
    pub hi: u128,
    pub lo: u128,
    pub fpr: [u32; 32],
    /// FPU condition flag (FCR31 C).
    pub fcc: bool,
    /// VU0 float registers, [x, y, z, w]; VF0 always reads (0, 0, 0, 1).
    pub vf: [[f32; 4]; 32],
    pub vi: [u16; 16],
    pub acc: [f32; 4],
    pub q: f32,
    pub pc: u32,
}

impl Cpu {
    fn new() -> Cpu {
        let mut vf = [[0.0; 4]; 32];
        vf[0] = [0.0, 0.0, 0.0, 1.0];
        Cpu { gpr: [0; 32], hi: 0, lo: 0, fpr: [0; 32], fcc: false, vf, vi: [0; 16], acc: [0.0; 4], q: 0.0, pc: 0 }
    }

    pub fn gpr(&self, r: usize) -> u64 {
        self.gpr[r] as u64
    }
    /// Sets the low 64 bits, as the ordinary instructions do (the upper 64 are kept).
    pub fn set_gpr(&mut self, r: usize, v: u64) {
        if r != 0 {
            self.gpr[r] = (self.gpr[r] & !(u64::MAX as u128)) | v as u128;
        }
    }
    pub fn f(&self, r: usize) -> f32 {
        f32::from_bits(self.fpr[r])
    }
    pub fn set_f(&mut self, r: usize, v: f32) {
        self.fpr[r] = v.to_bits();
    }
}

pub type Stub = Box<dyn FnMut(&mut Cpu, &mut Mem)>;

/// One loaded executable, ready to call functions of.
pub struct Runner {
    pub cpu: Cpu,
    pub mem: Mem,
    gp: u32,
    stubs: HashMap<u32, Stub>,
    allowed: HashSet<u32>,
    names: HashMap<u32, String>,
    /// Instructions run by the last `call`.
    pub steps: u64,
}

impl Runner {
    /// The player's ELF from `TRICKY_ELF`, or `None` (with the reason on stderr) so a test can skip.
    pub fn from_env() -> Option<Runner> {
        let path = match std::env::var("TRICKY_ELF") {
            Ok(p) => p,
            Err(_) => {
                eprintln!("TRICKY_ELF not set: skipping the check against SLUS_203.26");
                return None;
            }
        };
        let data = match std::fs::read(&path) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("TRICKY_ELF={path}: {e}: skipping");
                return None;
            }
        };
        match Runner::from_elf(&data) {
            Ok(r) => Some(r),
            Err(e) => {
                eprintln!("TRICKY_ELF={path}: {e}: skipping");
                None
            }
        }
    }

    /// Load `SLUS_203.26`; refuses any other file.
    pub fn from_elf(data: &[u8]) -> Result<Runner, Error> {
        let sum = md5::hex(data);
        if sum != ELF_MD5 {
            return Err(Error::BadElf(format!("md5 {sum}, expected {ELF_MD5}")));
        }
        let mut r = Runner::empty();
        let u16_at = |o: usize| u16::from_le_bytes([data[o], data[o + 1]]);
        let u32_at = |o: usize| u32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]);
        let (phoff, phnum) = (u32_at(28) as usize, u16_at(44) as usize);
        for i in 0..phnum {
            let p = phoff + i * 32;
            let (kind, off, va, filesz) = (u32_at(p), u32_at(p + 4) as usize, u32_at(p + 8), u32_at(p + 16) as usize);
            if kind == 1 {
                r.mem.write(va, &data[off..off + filesz]).ok_or_else(|| Error::BadElf("segment outside RAM".into()))?;
            }
        }
        r.gp = r.find_gp(u32_at(24))?;
        r.load_names();
        Ok(r)
    }

    /// No executable: for the instruction tests, which write their own code.
    pub fn empty() -> Runner {
        Runner {
            cpu: Cpu::new(),
            mem: Mem::new(),
            gp: 0,
            stubs: HashMap::new(),
            allowed: HashSet::new(),
            names: HashMap::new(),
            steps: 0,
        }
    }

    /// `$gp` as the start-up code sets it: it builds the value in a register with `lui`/`addiu` and copies it
    /// into $28 (`daddu $gp, $a0, $zero` at 0x100050 builds 0x3c38f0).
    fn find_gp(&mut self, entry: u32) -> Result<u32, Error> {
        let mut regs = [0u32; 32];
        for pc in (entry..entry + 0x100).step_by(4) {
            let w = self.mem.read_u32(pc);
            let (op, rs, rt, rd, imm) = (w >> 26, (w >> 21) & 31, (w >> 16) & 31, (w >> 11) & 31, w & 0xffff);
            match op {
                15 => regs[rt as usize] = imm << 16,
                9 => regs[rt as usize] = regs[rs as usize].wrapping_add(imm as i16 as i32 as u32),
                0 if (w & 63) == 0x2d && rd == 28 => return Ok(regs[rs as usize]),
                _ => {}
            }
        }
        Err(Error::BadElf("no $gp set-up in the entry code".into()))
    }

    fn load_names(&mut self) {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tricky-rs/docs/function-index.csv");
        if let Ok(text) = std::fs::read_to_string(path) {
            for line in text.lines().skip(1) {
                let mut p = line.split(',');
                if let (Some(a), Some(_), Some(n)) = (p.next(), p.next(), p.next()) {
                    if let Ok(a) = u32::from_str_radix(a.trim_start_matches("0x"), 16) {
                        self.names.insert(a, n.to_string());
                    }
                }
            }
        }
    }

    pub fn name(&self, addr: u32) -> String {
        self.names.get(&addr).cloned().unwrap_or_else(|| format!("FUN_{addr:08x}"))
    }

    pub fn gp(&self) -> u32 {
        self.gp
    }

    /// Replace calls to `addr` with `stub`, which reads the arguments and sets the result.
    pub fn stub(&mut self, addr: u32, stub: impl FnMut(&mut Cpu, &mut Mem) + 'static) {
        self.stubs.insert(addr, Box::new(stub));
    }

    /// Let calls to `addr` run the original.
    pub fn allow(&mut self, addr: u32) {
        self.allowed.insert(addr);
    }

    /// Call the function at `addr` with the arguments already in the registers; returns when it returns.
    pub fn call(&mut self, addr: u32) -> Result<(), Error> {
        self.cpu.set_gpr(28, self.gp as i32 as i64 as u64);
        self.cpu.set_gpr(29, STACK_TOP as u64);
        self.cpu.set_gpr(31, RETURN_SENTINEL as i32 as i64 as u64);
        self.cpu.pc = addr;
        self.steps = 0;
        self.allowed.insert(addr);
        exec::run(self)
    }
}
