//! The interpreter: decode and run one R5900 instruction at a time, with branch delay slots.
//! Implements what `tools/insn_census.py` lists for the functions checked so far; anything else stops the run
//! with `Error::Unimplemented` naming the instruction, which is the cue to add it here with a test.

use crate::ps2float as pf;
use crate::{Error, Runner, RETURN_SENTINEL};

fn sext16(imm: u32) -> u64 {
    imm as u16 as i16 as i64 as u64
}

fn sext32(v: u32) -> u64 {
    v as i32 as i64 as u64
}

enum Next {
    /// Go on to pc + 4.
    Seq,
    /// Branch or jump taken: run the delay slot, then go to the target.
    Branch(u32),
    /// Branch-likely not taken: skip the delay slot.
    SkipSlot,
}

pub(crate) fn run(r: &mut Runner, on_step: &mut dyn FnMut(u32, &crate::Cpu)) -> Result<(), Error> {
    let mut pending: Option<u32> = None;
    while r.cpu.pc != RETURN_SENTINEL {
        r.steps += 1;
        if r.steps > r.step_limit {
            return Err(Error::TooLong { pc: r.cpu.pc });
        }
        let pc = r.cpu.pc;
        let next = step(r, pc)?;
        on_step(pc, &r.cpu);
        match (pending.take(), next) {
            (Some(target), _) => r.cpu.pc = enter(r, pc, target)?, // that was the delay slot
            (None, Next::Seq) => r.cpu.pc = pc.wrapping_add(4),
            (None, Next::Branch(t)) => {
                pending = Some(t);
                r.cpu.pc = pc.wrapping_add(4);
            }
            (None, Next::SkipSlot) => r.cpu.pc = pc.wrapping_add(8),
        }
    }
    Ok(())
}

/// Where control goes after a taken branch's delay slot: a call to a stubbed function runs the stub and returns.
fn enter(r: &mut Runner, slot_pc: u32, target: u32) -> Result<u32, Error> {
    let caller = slot_pc.wrapping_sub(4);
    let w = r.mem.read_u32(caller);
    let is_call = w >> 26 == 3 || (w >> 26 == 0 && w & 63 == 9); // jal, jalr
    // a tail call (`j` into a stubbed function) runs the stub and returns to the caller's caller
    let is_tail = w >> 26 == 2 && r.stubs.contains_key(&target);
    if !(is_call || is_tail) || target == RETURN_SENTINEL {
        return Ok(target);
    }
    if let Some(mut stub) = r.stubs.remove(&target) {
        stub(&mut r.cpu, &mut r.mem);
        r.stubs.insert(target, stub);
        return Ok(r.cpu.gpr(31) as u32);
    }
    if r.allowed.contains(&target) {
        return Ok(target);
    }
    Err(Error::UnstubbedCall { pc: caller, target, name: r.name(target) })
}

fn load<const N: usize>(r: &mut Runner, pc: u32, addr: u32) -> Result<[u8; N], Error> {
    let mut b = [0u8; N];
    r.mem.read(addr, &mut b).ok_or(Error::BadAddress { pc, addr })?;
    Ok(b)
}

fn store(r: &mut Runner, pc: u32, addr: u32, data: &[u8]) -> Result<(), Error> {
    r.mem.write(addr, data).ok_or(Error::BadAddress { pc, addr })
}

fn step(r: &mut Runner, pc: u32) -> Result<Next, Error> {
    let w = load::<4>(r, pc, pc).map(u32::from_le_bytes)?;
    let (op, rs, rt, rd, sa, fun) =
        (w >> 26, ((w >> 21) & 31) as usize, ((w >> 16) & 31) as usize, ((w >> 11) & 31) as usize, (w >> 6) & 31, w & 63);
    let imm = w & 0xffff;
    let c = &mut r.cpu;
    let addr = (c.gpr(rs) as u32).wrapping_add(sext16(imm) as u32);
    let btarget = pc.wrapping_add(4).wrapping_add((sext16(imm) << 2) as u32);
    let unimpl = |what: &str| Err(Error::Unimplemented { pc, word: w, what: what.to_string() });
    let branch = |taken: bool, likely: bool| {
        Ok(if taken { Next::Branch(btarget) } else if likely { Next::SkipSlot } else { Next::Seq })
    };

    match op {
        0 => {
            let (a, b) = (c.gpr(rs), c.gpr(rt));
            let v = match fun {
                0 => sext32((b as u32) << sa),                   // sll (and nop)
                2 => sext32((b as u32) >> sa),                   // srl
                3 => sext32(((b as u32 as i32) >> sa) as u32),   // sra
                4 => sext32((b as u32) << (a & 31)),             // sllv
                6 => sext32((b as u32) >> (a & 31)),             // srlv
                7 => sext32(((b as u32 as i32) >> (a & 31)) as u32), // srav
                20 => b << (a & 63),                             // dsllv
                22 => b >> (a & 63),                             // dsrlv
                23 => ((b as i64) >> (a & 63)) as u64,           // dsrav
                8 => return Ok(Next::Branch(a as u32)),          // jr
                9 => {
                    // jalr
                    c.set_gpr(rd, pc.wrapping_add(8) as u64);
                    return Ok(Next::Branch(a as u32));
                }
                10 => {
                    if b == 0 { c.set_gpr(rd, a) }
                    return Ok(Next::Seq);
                } // movz
                11 => {
                    if b != 0 { c.set_gpr(rd, a) }
                    return Ok(Next::Seq);
                } // movn
                13 => return Err(Error::Trap { pc }), // break
                52 => {
                    // teq: traps only when equal
                    return if a == b { Err(Error::Trap { pc }) } else { Ok(Next::Seq) };
                }
                16 => c.hi as u64,                            // mfhi
                18 => c.lo as u64,                            // mflo
                17 | 19 => {
                    // mthi / mtlo: the low 64 bits (the pipeline-1 half kept)
                    let keep = |x: u128| (x >> 64) << 64;
                    if fun == 17 {
                        c.hi = keep(c.hi) | a as u128;
                    } else {
                        c.lo = keep(c.lo) | a as u128;
                    }
                    return Ok(Next::Seq);
                }
                24 => {
                    // mult: on the EE it also writes rd; LO1 / HI1 (the upper halves) are kept
                    let p = (a as u32 as i32 as i64) * (b as u32 as i32 as i64);
                    c.lo = (c.lo >> 64) << 64 | sext32(p as u32) as u128;
                    c.hi = (c.hi >> 64) << 64 | sext32((p >> 32) as u32) as u128;
                    sext32(p as u32)
                }
                25 => {
                    // multu: as mult, unsigned
                    let p = (a as u32 as u64) * (b as u32 as u64);
                    c.lo = (c.lo >> 64) << 64 | sext32(p as u32) as u128;
                    c.hi = (c.hi >> 64) << 64 | sext32((p >> 32) as u32) as u128;
                    sext32(p as u32)
                }
                26 => {
                    // div
                    let (n, d) = (a as u32 as i32, b as u32 as i32);
                    if d != 0 {
                        c.lo = (c.lo >> 64) << 64 | sext32(n.wrapping_div(d) as u32) as u128;
                        c.hi = (c.hi >> 64) << 64 | sext32(n.wrapping_rem(d) as u32) as u128;
                    }
                    return Ok(Next::Seq);
                }
                27 => {
                    // divu
                    let (n, d) = (a as u32, b as u32);
                    if d != 0 {
                        c.lo = (c.lo >> 64) << 64 | sext32(n / d) as u128;
                        c.hi = (c.hi >> 64) << 64 | sext32(n % d) as u128;
                    }
                    return Ok(Next::Seq);
                }
                33 => sext32((a as u32).wrapping_add(b as u32)), // addu
                35 => sext32((a as u32).wrapping_sub(b as u32)), // subu
                36 => a & b,
                37 => a | b,
                38 => a ^ b,
                39 => !(a | b),
                42 => ((a as i64) < (b as i64)) as u64, // slt
                43 => (a < b) as u64,                   // sltu
                45 => a.wrapping_add(b),                // daddu
                47 => a.wrapping_sub(b),                // dsubu
                56 => b << sa,                          // dsll
                58 => b >> sa,                          // dsrl
                59 => ((b as i64) >> sa) as u64,        // dsra
                60 => b << (sa + 32),                   // dsll32
                62 => b >> (sa + 32),                   // dsrl32
                63 => ((b as i64) >> (sa + 32)) as u64, // dsra32
                _ => return unimpl(&format!("special {fun}")),
            };
            c.set_gpr(rd, v);
            Ok(Next::Seq)
        }
        1 => {
            let a = c.gpr(rs) as i64;
            match rt {
                0 => branch(a < 0, false),  // bltz
                1 => branch(a >= 0, false), // bgez
                2 => branch(a < 0, true),   // bltzl
                3 => branch(a >= 0, true),  // bgezl
                _ => unimpl(&format!("regimm {rt}")),
            }
        }
        2 | 3 => {
            let target = (pc.wrapping_add(4) & 0xF000_0000) | ((w & 0x03FF_FFFF) << 2);
            if op == 3 {
                c.set_gpr(31, sext32(pc.wrapping_add(8)));
            }
            Ok(Next::Branch(target))
        }
        4 | 20 => branch(c.gpr(rs) == c.gpr(rt), op == 20), // beq, beql
        5 | 21 => branch(c.gpr(rs) != c.gpr(rt), op == 21), // bne, bnel
        6 | 22 => branch(c.gpr(rs) as i64 <= 0, op == 22),  // blez, blezl
        7 | 23 => branch(c.gpr(rs) as i64 > 0, op == 23),   // bgtz, bgtzl
        9 => {
            c.set_gpr(rt, sext32((c.gpr(rs) as u32).wrapping_add(sext16(imm) as u32))); // addiu
            Ok(Next::Seq)
        }
        10 => {
            c.set_gpr(rt, ((c.gpr(rs) as i64) < sext16(imm) as i64) as u64); // slti
            Ok(Next::Seq)
        }
        11 => {
            c.set_gpr(rt, (c.gpr(rs) < sext16(imm)) as u64); // sltiu: the immediate is sign-extended, compared unsigned
            Ok(Next::Seq)
        }
        12 => {
            c.set_gpr(rt, c.gpr(rs) & imm as u64);
            Ok(Next::Seq)
        }
        13 => {
            c.set_gpr(rt, c.gpr(rs) | imm as u64);
            Ok(Next::Seq)
        }
        14 => {
            c.set_gpr(rt, c.gpr(rs) ^ imm as u64);
            Ok(Next::Seq)
        }
        15 => {
            c.set_gpr(rt, sext32(imm << 16)); // lui
            Ok(Next::Seq)
        }
        17 => cop1(r, pc, w, rs, rt, rd, sa as usize, fun, btarget),
        18 => cop2(r, pc, w, rs, rt, rd, sa, fun),
        25 => {
            c.set_gpr(rt, c.gpr(rs).wrapping_add(sext16(imm))); // daddiu
            Ok(Next::Seq)
        }
        28 => mmi(r, pc, w, rs, rt, rd, fun),
        26 | 27 => {
            // ldl / ldr: the unaligned doubleword's high (ldl) or low (ldr) part, little-endian
            let byte = addr & 7;
            let mem = u64::from_le_bytes(load::<8>(r, pc, addr & !7)?);
            let old = r.cpu.gpr(rt);
            let v = if op == 26 {
                let sh = 8 * (7 - byte);
                let keep = if sh == 0 { 0 } else { (1u64 << sh) - 1 };
                (old & keep) | (mem << sh)
            } else {
                let sh = 8 * byte;
                let keep = if sh == 0 { 0 } else { !(u64::MAX >> sh) };
                (old & keep) | (mem >> sh)
            };
            r.cpu.set_gpr(rt, v);
            Ok(Next::Seq)
        }
        34 | 38 => {
            // lwl / lwr: the unaligned word's high (lwl) or low (lwr) part, little-endian; the result is
            // sign-extended when it holds the word's top byte (always for lwl, lwr only at byte 0)
            let byte = addr & 3;
            let mem = u32::from_le_bytes(load::<4>(r, pc, addr & !3)?);
            let old = r.cpu.gpr(rt);
            let old32 = old as u32;
            let v = if op == 34 {
                let sh = 8 * (3 - byte);
                let keep = if sh == 0 { 0 } else { (1u32 << sh) - 1 };
                sext32((old32 & keep) | (mem << sh))
            } else {
                let sh = 8 * byte;
                let keep = if sh == 0 { 0 } else { !(u32::MAX >> sh) };
                let w = (old32 & keep) | (mem >> sh);
                if byte == 0 { sext32(w) } else { (old & !0xffff_ffff) | w as u64 }
            };
            r.cpu.set_gpr(rt, v);
            Ok(Next::Seq)
        }
        42 | 46 => {
            // swl / swr: store the high (swl) or low (swr) part of the word at an unaligned address
            let byte = addr & 3;
            let at = addr & !3;
            let mem = u32::from_le_bytes(load::<4>(r, pc, at)?);
            let v = r.cpu.gpr(rt) as u32;
            let out = if op == 42 {
                let sh = 8 * (3 - byte);
                let mask = u32::MAX >> sh;
                (mem & !mask) | ((v >> sh) & mask)
            } else {
                let sh = 8 * byte;
                let mask = u32::MAX << sh;
                (mem & !mask) | (v << sh)
            };
            store(r, pc, at, &out.to_le_bytes())?;
            Ok(Next::Seq)
        }
        44 | 45 => {
            // sdl / sdr: store the high (sdl) or low (sdr) part of the doubleword at an unaligned address
            let byte = addr & 7;
            let at = addr & !7;
            let mem = u64::from_le_bytes(load::<8>(r, pc, at)?);
            let v = r.cpu.gpr(rt);
            let out = if op == 44 {
                let sh = 8 * (7 - byte);
                let mask = u64::MAX >> sh;
                (mem & !mask) | ((v >> sh) & mask)
            } else {
                let sh = 8 * byte;
                let mask = u64::MAX << sh;
                (mem & !mask) | (v << sh)
            };
            store(r, pc, at, &out.to_le_bytes())?;
            Ok(Next::Seq)
        }
        30 => {
            let v = u128::from_le_bytes(load::<16>(r, pc, addr & !15)?); // lq
            if rt != 0 {
                r.cpu.gpr[rt] = v;
            }
            Ok(Next::Seq)
        }
        31 => {
            let v = r.cpu.gpr[rt].to_le_bytes(); // sq
            store(r, pc, addr & !15, &v)?;
            Ok(Next::Seq)
        }
        32 | 33 | 35 | 36 | 37 | 39 | 55 => {
            let v = match op {
                32 => load::<1>(r, pc, addr)?[0] as i8 as i64 as u64, // lb
                36 => load::<1>(r, pc, addr)?[0] as u64,               // lbu
                33 => i16::from_le_bytes(load::<2>(r, pc, addr)?) as i64 as u64, // lh
                37 => u16::from_le_bytes(load::<2>(r, pc, addr)?) as u64,        // lhu
                35 => sext32(u32::from_le_bytes(load::<4>(r, pc, addr)?)),       // lw
                39 => u32::from_le_bytes(load::<4>(r, pc, addr)?) as u64,        // lwu
                _ => u64::from_le_bytes(load::<8>(r, pc, addr)?),                // ld
            };
            r.cpu.set_gpr(rt, v);
            Ok(Next::Seq)
        }
        40 | 41 | 43 | 63 => {
            let v = r.cpu.gpr(rt);
            match op {
                40 => store(r, pc, addr, &[v as u8])?,                      // sb
                41 => store(r, pc, addr, &(v as u16).to_le_bytes())?,       // sh
                43 => store(r, pc, addr, &(v as u32).to_le_bytes())?,       // sw
                _ => store(r, pc, addr, &v.to_le_bytes())?,                 // sd
            }
            Ok(Next::Seq)
        }
        49 => {
            let v = u32::from_le_bytes(load::<4>(r, pc, addr)?); // lwc1
            r.cpu.fpr[rt] = v;
            Ok(Next::Seq)
        }
        57 => {
            let v = r.cpu.fpr[rt].to_le_bytes(); // swc1
            store(r, pc, addr, &v)?;
            Ok(Next::Seq)
        }
        54 => {
            let b = load::<16>(r, pc, addr & !15)?; // lqc2
            set_vf(&mut r.cpu, rt, 0b1111, quad_to_vf(&b));
            Ok(Next::Seq)
        }
        62 => {
            let b = vf_to_quad(r.cpu.vf[rt]); // sqc2
            store(r, pc, addr & !15, &b)?;
            Ok(Next::Seq)
        }
        _ => unimpl(&format!("opcode {op}")),
    }
}

#[allow(clippy::too_many_arguments)]
fn cop1(r: &mut Runner, _pc: u32, w: u32, rs: usize, rt: usize, rd: usize, fd: usize, fun: u32, btarget: u32) -> Result<Next, Error> {
    let c = &mut r.cpu;
    let unimpl = |what: String| Err(Error::Unimplemented { pc: _pc, word: w, what });
    let (fs, ft) = (rd, rt);
    match rs {
        0 => c.set_gpr(rt, sext32(c.fpr[fs])),  // mfc1
        4 => c.fpr[fs] = c.gpr(rt) as u32,      // mtc1
        8 => {
            let (want, likely) = (rt & 1 == 1, rt & 2 == 2); // bc1f, bc1t, bc1fl, bc1tl
            return Ok(if c.fcc == want { Next::Branch(btarget) } else if likely { Next::SkipSlot } else { Next::Seq });
        }
        16 => {
            let (a, b) = (c.f(fs), c.f(ft));
            let v = match fun {
                0 => pf::add(a, b),
                1 => pf::sub(a, b),
                2 => pf::mul(a, b),
                3 => pf::div(a, b),
                4 => pf::sqrt(b), // sqrt.s fd, ft
                5 => a.abs(),
                6 => a,
                7 => -a,
                36 => {
                    c.fpr[fd] = pf::to_int(a) as u32; // cvt.w.s
                    return Ok(Next::Seq);
                }
                48 | 50 | 52 | 54 => {
                    let (a, b) = (pf::load(a), pf::load(b));
                    c.fcc = match fun {
                        48 => false,
                        50 => a == b,
                        52 => a < b,
                        _ => a <= b,
                    };
                    return Ok(Next::Seq);
                }
                _ => return unimpl(format!("cop1.s funct {fun}")),
            };
            c.set_f(fd, v);
        }
        20 if fun == 32 => {
            let v = pf::from_int(c.fpr[fs] as i32); // cvt.s.w
            c.set_f(fd, v);
        }
        _ => return unimpl(format!("cop1 rs {rs}")),
    }
    Ok(Next::Seq)
}

fn quad_to_vf(b: &[u8; 16]) -> [f32; 4] {
    let mut v = [0.0; 4];
    for (i, x) in v.iter_mut().enumerate() {
        *x = f32::from_le_bytes([b[i * 4], b[i * 4 + 1], b[i * 4 + 2], b[i * 4 + 3]]);
    }
    v
}

fn vf_to_quad(v: [f32; 4]) -> [u8; 16] {
    let mut b = [0u8; 16];
    for (i, x) in v.iter().enumerate() {
        b[i * 4..i * 4 + 4].copy_from_slice(&x.to_le_bytes());
    }
    b
}

/// Write the fields of `vf[reg]` selected by `dest` (bit 3 = x … bit 0 = w); VF0 is read-only.
/// The VU's ordering of floats: as sign-and-magnitude integers (−0 below +0)
fn vu_key(x: f32) -> i64 {
    let b = x.to_bits();
    if b & 0x8000_0000 != 0 {
        -((b & 0x7fff_ffff) as i64) - 1
    } else {
        b as i64
    }
}
fn vu_max(a: f32, b: f32) -> f32 {
    if vu_key(a) >= vu_key(b) { a } else { b }
}
fn vu_min(a: f32, b: f32) -> f32 {
    if vu_key(a) <= vu_key(b) { a } else { b }
}

fn set_vf(c: &mut crate::Cpu, reg: usize, dest: u32, v: [f32; 4]) {
    if reg == 0 {
        return;
    }
    for (i, x) in v.into_iter().enumerate() {
        if dest & (8 >> i) != 0 {
            c.vf[reg][i] = x;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn cop2(r: &mut Runner, pc: u32, w: u32, rs: usize, rt: usize, rd: usize, _sa: u32, fun: u32) -> Result<Next, Error> {
    let c = &mut r.cpu;
    let unimpl = |what: String| Err(Error::Unimplemented { pc, word: w, what });
    if rs & 16 == 0 {
        match rs {
            1 => c.gpr[rt] = u128::from_le_bytes(vf_to_quad(c.vf[rd])), // qmfc2
            2 => {
                // cfc2: VI0-15, the clip flags (18), Q (22), CMSAR0 (27); the other flags are not modelled yet
                let v = match rd {
                    0..=15 => c.vi[rd] as u32,
                    18 => c.clip,
                    22 => c.q.to_bits(),
                    27 => c.cmsar0 as u32,
                    _ => return unimpl(format!("cfc2 from control register {rd}")),
                };
                c.set_gpr(rt, sext32(v));
            }
            5 => {
                let q = c.gpr[rt].to_le_bytes(); // qmtc2
                set_vf(c, rd, 0b1111, quad_to_vf(&q));
            }
            6 => {
                // ctc2: VI1-15, the clip flags (18), CMSAR0 (27)
                let v = c.gpr(rt) as u32;
                match rd {
                    1..=15 => c.vi[rd] = v as u16,
                    0 => {}
                    18 => c.clip = v & 0xff_ffff,
                    27 => c.cmsar0 = v as u16,
                    _ => return unimpl(format!("ctc2 to control register {rd}")),
                }
            }
            _ => return unimpl(format!("cop2 rs {rs}")),
        }
        return Ok(Next::Seq);
    }
    // vcallms imm15 (its start in 8 bytes, bits 6..20) / vcallmsr (CMSAR0): the micro program, run to its end
    if fun == 56 || fun == 57 {
        let start = if fun == 56 { (w >> 6) & 0x7fff } else { c.cmsar0 as u32 };
        vu0_micro(r, pc, start * 8)?;
        return Ok(Next::Seq);
    }
    vu_upper(c, w).map_err(|what| Error::Unimplemented { pc, word: w, what })?;
    Ok(Next::Seq)
}

/// A VU upper op (macro mode's special1 / special2; micro mode's upper word has the same fields): dest bits
/// 24..21 (x..w), ft 20..16, fs 15..11, fd 10..6, broadcast in the low two bits
fn vu_upper(c: &mut crate::Cpu, w: u32) -> Result<(), String> {
    let (rt, rd, sa, fun) = (((w >> 16) & 31) as usize, ((w >> 11) & 31) as usize, (w >> 6) & 31, w & 63);
    let dest = (w >> 21) & 15;
    let (ft, fs, fd) = (rt, rd, sa as usize);
    let (s, t) = (c.vf[fs], c.vf[ft]);
    let bc = t[(fun & 3) as usize];
    let lanes = |f: &dyn Fn(usize) -> f32| [f(0), f(1), f(2), f(3)];
    if fun < 60 {
        let v = match fun {
            0..=3 => lanes(&|i| pf::add(s[i], bc)),                                     // vaddbc
            8..=11 => lanes(&|i| pf::add(c.acc[i], pf::mul(s[i], bc))),                 // vmaddbc
            24..=27 => lanes(&|i| pf::mul(s[i], bc)),                                   // vmulbc
            28 => lanes(&|i| pf::mul(s[i], c.q)),                                       // vmulq
            40 => lanes(&|i| pf::add(s[i], t[i])),                                      // vadd
            42 => lanes(&|i| pf::mul(s[i], t[i])),                                      // vmul
            44 => lanes(&|i| pf::sub(s[i], t[i])),                                      // vsub
            16..=19 => lanes(&|i| vu_max(s[i], bc)),                                    // vmaxbc
            20..=23 => lanes(&|i| vu_min(s[i], bc)),                                    // vminibc
            // vopmsub: fd = ACC − (fs.y·ft.z, fs.z·ft.x, fs.x·ft.y) (with vopmula, the cross product)
            46 => {
                let p = [pf::mul(s[1], t[2]), pf::mul(s[2], t[0]), pf::mul(s[0], t[1])];
                [pf::sub(c.acc[0], p[0]), pf::sub(c.acc[1], p[1]), pf::sub(c.acc[2], p[2]), c.acc[3]]
            }
            _ => return Err(format!("vu0 funct {fun}")),
        };
        set_vf(c, fd, dest, v);
        return Ok(());
    }
    let fsf = ((w >> 21) & 3) as usize;
    let ftf = ((w >> 23) & 3) as usize;
    let acc = c.acc;
    let to_acc = match (sa << 2) | (fun & 3) {
        0..=3 => Some(lanes(&|i| pf::add(s[i], bc))),                   // vaddabc
        8..=11 => Some(lanes(&|i| pf::add(acc[i], pf::mul(s[i], bc)))), // vmaddabc
        24..=27 => Some(lanes(&|i| pf::mul(s[i], bc))),                 // vmulabc
        42 => Some(lanes(&|i| pf::mul(s[i], t[i]))),                    // vmula
        46 => Some([pf::mul(s[1], t[2]), pf::mul(s[2], t[0]), pf::mul(s[0], t[1]), acc[3]]), // vopmula
        48 => {
            set_vf(c, ft, dest, s); // vmove ft, fs
            None
        }
        56 => {
            c.q = pf::div(s[fsf], t[ftf]); // vdiv Q, fs.fsf, ft.ftf
            None
        }
        57 => {
            c.q = pf::sqrt(t[ftf]); // vsqrt Q, ft.ftf
            None
        }
        58 => {
            c.q = pf::div(s[fsf], pf::sqrt(t[ftf])); // vrsqrt Q, fs.fsf, ft.ftf
            None
        }
        59 => None, // vwaitq: the result is already there
        // vftoi0 / vftoi4 / vftoi12 / vftoi15 ft, fs: each lane to an integer (×1, 16, 4096, 32768), toward zero,
        // saturating; the integer's bits kept in the lane
        20..=23 => {
            let scale = [1.0f64, 16.0, 4096.0, 32768.0][(fun & 3) as usize];
            let v = lanes(&|i| f32::from_bits(((s[i] as f64) * scale).clamp(i32::MIN as f64, i32::MAX as f64) as i32 as u32));
            set_vf(c, ft, dest, v);
            None
        }
        // vmtir it, fs.fsf: the lane's low 16 bits to the integer register (it is ft's field)
        60 => {
            if ft != 0 {
                c.vi[ft & 15] = s[fsf].to_bits() as u16;
            }
            None
        }
        // vclipw.xyz fs, ft: the clip flags shifted up six and this test's below: x > |ft.w| 1, x < −|ft.w| 2,
        // then y 4 / 8, z 0x10 / 0x20
        31 => {
            let lim = t[3].abs();
            let mut f = 0;
            for k in 0..3 {
                if s[k] > lim {
                    f |= 1 << (2 * k);
                }
                if s[k] < -lim {
                    f |= 2 << (2 * k);
                }
            }
            c.clip = ((c.clip << 6) | f) & 0xff_ffff;
            None
        }
        47 => None, // vnop
        x => return Err(format!("vu0 special2 {x}")),
    };
    if let Some(v) = to_acc {
        for (i, x) in v.into_iter().enumerate() {
            if dest & (8 >> i) != 0 {
                c.acc[i] = x;
            }
        }
    }
    Ok(())
}

/// VU0 micro mode (`vcallms` / `vcallmsr`): the program in `Runner::vu0_micro` from byte `start` until the
/// instruction after the one with the E bit. Each pair's upper and lower read the registers as they were before it;
/// results are there for the next pair (no pipeline: a program that reads a result before its latency is up, or
/// relies on a flag's delay, would differ). The upper ops are the macro ones; the lower ones are the integer ops,
/// `fcget`, `move`, and the branches.
fn vu0_micro(r: &mut Runner, call_pc: u32, start: u32) -> Result<(), Error> {
    let mut pc = start & 0xff8;
    let (mut ending, mut branch): (bool, Option<u32>) = (false, None);
    for _ in 0..100_000 {
        let at = pc as usize;
        if at + 8 > r.vu0_micro.len() {
            return Err(Error::Unimplemented { pc: call_pc, word: pc, what: "vu0 micro program outside micro memory".into() });
        }
        let lo = u32::from_le_bytes(r.vu0_micro[at..at + 4].try_into().unwrap());
        let hi = u32::from_le_bytes(r.vu0_micro[at + 4..at + 8].try_into().unwrap());
        let unimpl = |what: String| Error::Unimplemented { pc: call_pc, word: hi, what: format!("vu0 micro {pc:#x}: {what}") };
        // (the I bit, an immediate in the lower word, is not modelled; M, D and T change nothing here)
        if hi >> 31 != 0 {
            return Err(unimpl("the I bit".into()));
        }
        let before = r.cpu.clone();
        vu_upper(&mut r.cpu, hi).map_err(unimpl)?;
        let mut next = pc + 8;
        if let Some(t) = branch.take() {
            next = t;
        }
        if let Some(t) = vu0_lower(&before, &mut r.cpu, lo, pc).map_err(unimpl)? {
            branch = Some(t);
        }
        if ending {
            return Ok(());
        }
        if hi >> 30 & 1 != 0 {
            ending = true;
        }
        pc = next & 0xff8;
    }
    Err(Error::Unimplemented { pc: call_pc, word: start, what: "vu0 micro program never ended".into() })
}

/// A micro lower op, reading `s` (the registers before the pair) and writing `c`; a branch's target
fn vu0_lower(s: &crate::Cpu, c: &mut crate::Cpu, w: u32, pc: u32) -> Result<Option<u32>, String> {
    let op = w >> 25;
    let (it, is, id) = (((w >> 16) & 15) as usize, ((w >> 11) & 15) as usize, ((w >> 6) & 15) as usize);
    let imm11 = w & 0x7ff;
    let simm11 = if imm11 & 0x400 != 0 { imm11 as i32 - 0x800 } else { imm11 as i32 };
    let target = pc.wrapping_add(8).wrapping_add((simm11 * 8) as u32);
    let imm15 = ((w & 0x7ff) | ((w >> 21) & 0xf) << 11) as u16;
    let set = |c: &mut crate::Cpu, r: usize, v: u16| {
        if r != 0 {
            c.vi[r] = v;
        }
    };
    let (a, b) = (s.vi[is], s.vi[it]);
    match op {
        0x40 => {
            let f = w & 0x3f;
            if f >= 0x3c {
                let k = ((w >> 6) & 0x1f) << 2 | (w & 3);
                match k {
                    // move ft, fs (the NOP is `move` with no fields)
                    0x30 => set_vf(c, ((w >> 16) & 31) as usize, (w >> 21) & 15, s.vf[((w >> 11) & 31) as usize]),
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
        0x08 => set(c, it, a.wrapping_add(imm15)), // iaddiu
        0x09 => set(c, it, a.wrapping_sub(imm15)), // isubiu
        0x1c => set(c, it, (s.clip & 0xfff) as u16), // fcget
        0x20 => return Ok(Some(target)),           // b
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

#[cfg(test)]
mod tests {
    //! One test per instruction: assemble a few words at 0x100000, call it, look at the registers.
    use crate::Runner;

    const CODE: u32 = 0x0010_0000;

    fn r(op: u32, rs: u32, rt: u32, rd: u32, sa: u32, fun: u32) -> u32 {
        (op << 26) | (rs << 21) | (rt << 16) | (rd << 11) | (sa << 6) | fun
    }
    fn i(op: u32, rs: u32, rt: u32, imm: i32) -> u32 {
        (op << 26) | (rs << 21) | (rt << 16) | (imm as u32 & 0xffff)
    }
    const JR_RA: u32 = 0x03e0_0008;
    const NOP: u32 = 0;
    fn fpu(fun: u32, ft: u32, fs: u32, fd: u32) -> u32 {
        r(17, 16, ft, fs, fd, fun)
    }
    fn vu(dest: u32, ft: u32, fs: u32, fd: u32, fun: u32) -> u32 {
        (18 << 26) | (1 << 25) | (dest << 21) | (ft << 16) | (fs << 11) | (fd << 6) | fun
    }

    fn run(code: &[u32], setup: impl FnOnce(&mut Runner)) -> Runner {
        let mut rn = Runner::empty();
        for (k, w) in code.iter().chain([JR_RA, NOP].iter()).enumerate() {
            rn.mem.write_u32(CODE + 4 * k as u32, *w);
        }
        setup(&mut rn);
        rn.call(CODE).expect("runs");
        rn
    }

    #[test]
    fn lui_ori_addiu_daddu() {
        let rn = run(&[i(15, 0, 8, 0x4234), i(13, 8, 8, 0x5678), i(9, 8, 9, -8), r(0, 8, 9, 10, 0, 45)], |_| {});
        assert_eq!(rn.cpu.gpr(8), 0x4234_5678);
        assert_eq!(rn.cpu.gpr(9), 0x4234_5670);
        assert_eq!(rn.cpu.gpr(10), 0x8468_ACE8);
        let rn = run(&[i(15, 0, 8, 0x8000u16 as i16 as i32)], |_| {});
        assert_eq!(rn.cpu.gpr(8), 0xFFFF_FFFF_8000_0000, "lui sign-extends");
    }

    #[test]
    fn divu_divides_unsigned() {
        // 0xFFFF_FFFE / 3 = 0x5555_5554 remainder 2 (signed div would give -2 / 3 = 0, remainder -2)
        let rn = run(&[r(0, 4, 5, 0, 0, 27), r(0, 0, 0, 8, 0, 18), r(0, 0, 0, 9, 0, 16)], |rn| {
            rn.cpu.set_gpr(4, 0xFFFF_FFFE);
            rn.cpu.set_gpr(5, 3);
        });
        assert_eq!(rn.cpu.gpr(8), 0x5555_5554, "lo");
        assert_eq!(rn.cpu.gpr(9), 2, "hi");
    }

    #[test]
    fn sltiu_compares_unsigned_with_sign_extended_immediate() {
        let rn = run(&[i(11, 4, 8, 2), i(11, 5, 9, -1)], |rn| {
            rn.cpu.set_gpr(4, 1);
            rn.cpu.set_gpr(5, 7);
        });
        assert_eq!(rn.cpu.gpr(8), 1);
        assert_eq!(rn.cpu.gpr(9), 1, "7 < 0xFFFF_FFFF_FFFF_FFFF");
    }

    #[test]
    fn loads_and_stores() {
        let rn = run(
            &[i(35, 4, 8, 0), i(36, 4, 9, 4), i(43, 4, 8, 32), i(30, 4, 10, 16), i(31, 4, 10, 48)],
            |rn| {
                let p = rn.mem.alloc(64);
                rn.mem.write_u32(p, 0x8000_0001);
                rn.mem.write_u8(p + 4, 0xfe);
                for k in 0..4 {
                    rn.mem.write_u32(p + 16 + 4 * k, 0x1111_1111 * (k + 1));
                }
                rn.cpu.set_gpr(4, p as u64);
            },
        );
        let p = rn.cpu.gpr(4) as u32;
        let mut m = rn;
        assert_eq!(m.cpu.gpr(8), 0xFFFF_FFFF_8000_0001, "lw sign-extends");
        assert_eq!(m.cpu.gpr(9), 0xfe, "lbu zero-extends");
        assert_eq!(m.mem.read_u32(p + 32), 0x8000_0001, "sw");
        assert_eq!(m.cpu.gpr[10] >> 96, 0x4444_4444, "lq fills all 128 bits");
        assert_eq!(m.mem.read_u32(p + 60), 0x4444_4444, "sq");
    }

    #[test]
    fn branches_run_the_delay_slot() {
        // beq taken: the slot runs, the instruction after it does not
        let rn = run(&[i(4, 0, 0, 2), i(9, 0, 8, 1), i(9, 0, 9, 1), i(9, 0, 10, 1)], |_| {});
        assert_eq!((rn.cpu.gpr(8), rn.cpu.gpr(9), rn.cpu.gpr(10)), (1, 0, 1));
        // bnel not taken: the slot is skipped
        let rn = run(&[i(21, 0, 0, 2), i(9, 0, 8, 1), i(9, 0, 9, 1)], |_| {});
        assert_eq!((rn.cpu.gpr(8), rn.cpu.gpr(9)), (0, 1));
        // beql taken: the slot runs
        let rn = run(&[i(20, 0, 0, 2), i(9, 0, 8, 1), i(9, 0, 9, 1), NOP], |_| {});
        assert_eq!((rn.cpu.gpr(8), rn.cpu.gpr(9)), (1, 0));
        // bne taken past one instruction
        let rn = run(&[i(5, 4, 0, 2), NOP, i(9, 0, 9, 1), NOP], |rn| rn.cpu.set_gpr(4, 3));
        assert_eq!(rn.cpu.gpr(9), 0);
    }

    #[test]
    fn fpu_arithmetic_and_moves() {
        let rn = run(
            &[
                r(17, 4, 4, 1, 0, 0), // mtc1 a0, f1
                fpu(0, 2, 1, 3),      // add.s f3 = f1 + f2
                fpu(1, 2, 1, 4),      // sub.s f4 = f1 - f2
                fpu(2, 2, 1, 5),      // mul.s
                fpu(3, 2, 1, 6),      // div.s
                fpu(5, 0, 4, 7),      // abs.s f7 = |f4|
                fpu(6, 0, 1, 8),      // mov.s
                fpu(7, 0, 1, 9),      // neg.s
                r(17, 0, 8, 6, 0, 0), // mfc1 t0, f6
            ],
            |rn| {
                rn.cpu.set_gpr(4, 3.0f32.to_bits() as u64);
                rn.cpu.set_f(2, 4.0);
            },
        );
        let c = &rn.cpu;
        assert_eq!((c.f(3), c.f(4), c.f(5), c.f(7), c.f(8), c.f(9)), (7.0, -1.0, 12.0, 1.0, 3.0, -3.0));
        assert_eq!(c.f(6).to_bits(), 0x3F40_0000, "3/4 = 0.75 exactly");
        assert_eq!(c.gpr(8), 0x3F40_0000, "mfc1");
    }

    #[test]
    fn fpu_compare_branch_and_convert() {
        // c.lt.s f1, f2 ; bc1f +1 ; slot ; addiu t1 (skipped when taken)
        let code = [fpu(52, 2, 1, 0), r(17, 8, 0, 0, 0, 0) | 1, NOP, i(9, 0, 9, 1)];
        let rn = run(&code, |rn| {
            rn.cpu.set_f(1, 1.0);
            rn.cpu.set_f(2, 2.0);
        });
        assert!(rn.cpu.fcc);
        assert_eq!(rn.cpu.gpr(9), 1, "bc1f not taken when the flag is set");
        let rn = run(&[r(17, 20, 0, 1, 2, 32), fpu(36, 0, 3, 4)], |rn| {
            rn.cpu.fpr[1] = (-7i32) as u32;
            rn.cpu.set_f(3, 2.75);
        });
        assert_eq!(rn.cpu.f(2), -7.0, "cvt.s.w");
        assert_eq!(rn.cpu.fpr[4], 2, "cvt.w.s truncates");
    }

    #[test]
    fn vu0_macro() {
        let rn = run(
            &[
                vu(15, 2, 1, 3, 40),        // vadd vf3 = vf1 + vf2
                vu(15, 2, 1, 4, 24),        // vmulx vf4 = vf1 * vf2.x
                vu(3 << 2, 2, 1, 0, 0x3c) | (14 << 6), // vdiv Q = vf1.x / vf2.w (ftf w in bits 24..23, fsf x)
                vu(15, 0, 1, 5, 28),        // vmulq vf5 = vf1 * Q
                vu(15, 2, 1, 0, 0x3c | 1),  // vadday ACC = vf1 + vf2.y
                vu(15, 2, 1, 6, 11),        // vmaddw vf6 = ACC + vf1 * vf2.w
                vu(15, 0, 0, 0, 0x3f) | (14 << 6), // vwaitq
                vu(3 << 2, 2, 1, 0, 0x3e) | (14 << 6), // vrsqrt Q = vf1.x / sqrt(vf2.w), checked below as vf7
            ],
            |rn| {
                rn.cpu.vf[1] = [1.0, 2.0, 3.0, 4.0];
                rn.cpu.vf[2] = [2.0, 10.0, 0.0, 4.0];
            },
        );
        let c = &rn.cpu;
        assert_eq!(c.vf[3], [3.0, 12.0, 3.0, 8.0]);
        assert_eq!(c.vf[4], [2.0, 4.0, 6.0, 8.0]);
        assert_eq!(c.vf[5], [0.25, 0.5, 0.75, 1.0], "vmulq by vdiv's Q = 1/4");
        assert_eq!(c.vf[6], [15.0, 20.0, 25.0, 30.0]);
        assert_eq!(c.q, 0.5, "vrsqrt: 1 / sqrt(4)");
    }

    #[test]
    fn vu0_max_min_ftoi_mtir() {
        // the fog value's sequence (cPS2FogVolume vf3 0x1ca63c..): vmaxx.w, vminix.w, vftoi0, vmtir vi1, vf.w
        let rn = run(
            &[
                vu(1, 0, 3, 3, 16),                  // vmaxx.w vf3 = max(vf3.w, vf0.x)
                vu(1, 5, 3, 3, 20),                  // vminix.w vf3 = min(vf3.w, vf5.x)
                vu(15, 4, 1, 5 << 0, 0x3c) | (5 << 6), // vftoi0 vf4, vf1
                vu(3, 1, 3, 15, 0x3c) | (15 << 6),   // vmtir vi1, vf3.w
            ],
            |rn| {
                rn.cpu.vf[1] = [2.75, -2.75, 1.0e12, -0.5];
                rn.cpu.vf[3] = [0.0, 0.0, 0.0, 300.5];
                rn.cpu.vf[5] = [200.0, 0.0, 0.0, 0.0];
            },
        );
        let c = &rn.cpu;
        assert_eq!(c.vf[3][3], 200.0, "clamped into 0..vf5.x");
        assert_eq!(c.vf[4].map(|x| x.to_bits() as i32), [2, -2, i32::MAX, 0], "toward zero, saturating");
        assert_eq!(c.vi[1], 200f32.to_bits() as u16, "vmtir takes the lane's low 16 bits");
    }

    #[test]
    fn vu0_micro_mode() {
        // a program at micro byte 0x40: vclipw.xyz with iaddiu, a nop, fcget, ior on the E pair, an add in the
        // pair after it (run), and one more (not run); called by vcallms and again by ctc2 CMSAR0 + vcallmsr
        let nop_lo = 0x8000_033c;
        let vnop = 0x2ff;
        let prog: [(u32, u32); 6] = [
            ((0b1110 << 21) | (1 << 16) | (1 << 11) | 0x1ff, (0x08 << 25) | (2 << 16) | 5), // vclipw.xyz vf1, vf1 | iaddiu vi2, vi0, 5
            (vnop, nop_lo),
            (vnop, (0x1c << 25) | (1 << 16)),                                            // fcget vi1
            ((1 << 30) | vnop, (0x40 << 25) | (2 << 16) | (1 << 11) | (3 << 6) | 0x35),  // [E] | ior vi3, vi1, vi2
            ((8 << 21) | (2 << 6) | 3, nop_lo),                                          // vaddw.x vf2, vf0, vf0w
            ((4 << 21) | (2 << 6) | 3, nop_lo),                                          // vaddw.y (after the end)
        ];
        let rn = run(
            &[
                (18 << 26) | (1 << 25) | (8 << 6) | 56,  // vcallms 0x40
                (18 << 26) | (2 << 21) | (2 << 16) | (3 << 11), // cfc2 $v0, vi3
                (18 << 26) | (6 << 21) | (4 << 16) | (27 << 11), // ctc2 $a0, CMSAR0
                (18 << 26) | (1 << 25) | (27 << 11) | 57, // vcallmsr
                (18 << 26) | (2 << 21) | (3 << 16) | (3 << 11), // cfc2 $v1, vi3
            ],
            |rn| {
                for (k, (hi, lo)) in prog.iter().enumerate() {
                    rn.vu0_micro[0x40 + 8 * k..0x44 + 8 * k].copy_from_slice(&lo.to_le_bytes());
                    rn.vu0_micro[0x44 + 8 * k..0x48 + 8 * k].copy_from_slice(&hi.to_le_bytes());
                }
                rn.cpu.vf[1] = [2.0, -3.0, 0.5, -1.0];
                rn.cpu.set_gpr(4, 8);
            },
        );
        let c = &rn.cpu;
        assert_eq!(c.gpr(2), 9 | 5, "x beyond +|w| (1), y beyond −|w| (8), with vi2");
        assert_eq!(c.gpr(3), (9 << 6) | 9 | 5, "fcget takes the last two tests (12 bits)");
        assert_eq!(c.clip, (9 << 6) | 9);
        assert_eq!((c.vf[2][0], c.vf[2][1]), (1.0, 0.0), "the pair after the E bit runs, the next does not");
    }

    #[test]
    fn tail_calls_go_through_stubs() {
        // j 0x200000 (a tail call) with a stub there: the stub runs, then control returns to $ra
        let mut rn = Runner::empty();
        rn.mem.write_u32(CODE, (2 << 26) | (0x0020_0000 >> 2)); // j 0x200000
        rn.mem.write_u32(CODE + 4, 0);
        rn.stub(0x0020_0000, |c, _| c.set_gpr(2, 7));
        rn.call(CODE).unwrap();
        assert_eq!(rn.cpu.gpr(2), 7);
    }

    #[test]
    fn unaligned_words() {
        // lwl/lwr pair at an odd address reads the 4 bytes there (sign-extended); swl/swr writes them back
        let mut rn = Runner::empty();
        let data: Vec<u8> = (0u8..32).map(|b| b | 0x80).collect();
        rn.mem.write(0x0030_0000, &data).unwrap();
        let (base, at) = (0x0030_0000u32, 5u32);
        rn.cpu.set_gpr(4, base as u64);
        let i = |op: u32, rt: u32, off: u32| (op << 26) | (4 << 21) | (rt << 16) | off;
        let code = [i(34, 2, at + 3), i(38, 2, at), i(42, 2, 0x10 + 2 + 3), i(46, 2, 0x10 + 2), 0x03e0_0008, 0];
        for (k, w) in code.iter().enumerate() {
            rn.mem.write_u32(CODE + 4 * k as u32, *w);
        }
        rn.call(CODE).unwrap();
        assert_eq!(rn.cpu.gpr(2), u32::from_le_bytes([0x85, 0x86, 0x87, 0x88]) as i32 as i64 as u64, "lwl/lwr");
        let mut out = [0u8; 6];
        rn.mem.read(base + 0x11, &mut out).unwrap();
        assert_eq!(out, [0x91, 0x85, 0x86, 0x87, 0x88, 0x96], "swl/swr and the bytes around");
    }

    #[test]
    fn unaligned_doublewords() {
        // ldl/ldr pair at an odd address reads the 8 bytes there; sdl/sdr writes them back elsewhere, unaligned
        let mut rn = Runner::empty();
        let data: Vec<u8> = (0u8..32).collect();
        rn.mem.write(0x0030_0000, &data).unwrap();
        let (base, at) = (0x0030_0000u32, 3u32);
        rn.cpu.set_gpr(4, base as u64);
        let i = |op: u32, rt: u32, off: u32| (op << 26) | (4 << 21) | (rt << 16) | off;
        let code = [i(26, 2, at + 7), i(27, 2, at), i(44, 2, 0x10 + 5 + 7), i(45, 2, 0x10 + 5), 0x03e0_0008, 0];
        for (k, w) in code.iter().enumerate() {
            rn.mem.write_u32(CODE + 4 * k as u32, *w);
        }
        rn.call(CODE).unwrap();
        assert_eq!(rn.cpu.gpr(2), u64::from_le_bytes([3, 4, 5, 6, 7, 8, 9, 10]), "ldl/ldr");
        let mut out = [0u8; 8];
        rn.mem.read(base + 0x15, &mut out).unwrap();
        assert_eq!(out, [3, 4, 5, 6, 7, 8, 9, 10], "sdl/sdr");
        let mut around = [0u8; 2];
        rn.mem.read(base + 0x14, &mut around[..1]).unwrap();
        rn.mem.read(base + 0x1d, &mut around[1..]).unwrap();
        assert_eq!(around, [0x14, 0x1d], "the bytes around are kept");
    }

    #[test]
    fn vu0_cross_product() {
        // vopmula.xyz ACC, vf1, vf2 then vopmsub.xyz vf3, vf2, vf1: vf3 = vf1 × vf2
        let mut rn = Runner::empty();
        rn.cpu.vf[1] = [1.0, 2.0, 3.0, 0.0];
        rn.cpu.vf[2] = [4.0, 5.0, 6.0, 0.0];
        let code = [vu(14, 2, 1, 0xb, 0x3e), vu(14, 1, 2, 3, 0x2e), 0x03e0_0008, 0];
        for (i, w) in code.iter().enumerate() {
            rn.mem.write_u32(CODE + 4 * i as u32, *w);
        }
        rn.call(CODE).unwrap();
        assert_eq!(&rn.cpu.vf[3][..3], &[2.0 * 6.0 - 5.0 * 3.0, 3.0 * 4.0 - 6.0 * 1.0, 1.0 * 5.0 - 4.0 * 2.0]);
    }

    #[test]
    fn vu0_transfers() {
        let rn = run(
            &[
                (54 << 26) | (4 << 21) | (7 << 16), // lqc2 vf7, 0(a0)
                (18 << 26) | (1 << 21) | (8 << 16) | (7 << 11), // qmfc2 t0, vf7
                (18 << 26) | (5 << 21) | (8 << 16) | (9 << 11), // qmtc2 t0, vf9
                (62 << 26) | (4 << 21) | (9 << 16) | 16,       // sqc2 vf9, 16(a0)
                (18 << 26) | (2 << 21) | (10 << 16) | (22 << 11), // cfc2 t2, Q
            ],
            |rn| {
                let p = rn.mem.alloc(32);
                for k in 0..4 {
                    rn.mem.write_f32(p + 4 * k, k as f32);
                }
                rn.cpu.set_gpr(4, p as u64);
                rn.cpu.q = 1.5;
            },
        );
        let p = rn.cpu.gpr(4) as u32;
        let mut m = rn;
        assert_eq!(m.cpu.vf[9], [0.0, 1.0, 2.0, 3.0]);
        assert_eq!(m.mem.read_f32(p + 28), 3.0);
        assert_eq!(m.cpu.gpr(10), 1.5f32.to_bits() as u64);
    }

    #[test]
    fn calls_go_through_stubs() {
        // move s0, ra ; jal 0x200000 ; nop ; move t1, v0 ; jr s0 ; nop  (jal overwrites ra, as in real code)
        let code = [r(0, 31, 0, 16, 0, 45), (3 << 26) | (0x0020_0000 >> 2), NOP, r(0, 2, 0, 9, 0, 45), r(0, 16, 0, 0, 0, 8), NOP];
        let mut rn = Runner::empty();
        for (k, w) in code.iter().enumerate() {
            rn.mem.write_u32(CODE + 4 * k as u32, *w);
        }
        assert!(matches!(rn.call(CODE), Err(crate::Error::UnstubbedCall { target: 0x0020_0000, .. })));
        rn.stub(0x0020_0000, |c, _| c.set_gpr(2, 42));
        rn.call(CODE).unwrap();
        assert_eq!(rn.cpu.gpr(9), 42);
    }

    #[test]
    fn unknown_instructions_say_so() {
        let mut rn = Runner::empty();
        rn.mem.write_u32(CODE, (28 << 26) | 0x04); // an MMI instruction not implemented (plzcw)
        let e = rn.call(CODE).unwrap_err();
        assert!(e.to_string().contains("MMI"), "{e}");
    }

    #[test]
    fn the_128_bit_shuffles() {
        // pextlw / pextuw / pcpyld / pcpyud r8, r4, r5 (rs = r4, rt = r5)
        let mmi = |fun: u32, sa: u32| (28 << 26) | (4 << 21) | (5 << 16) | (8 << 11) | (sa << 6) | fun;
        let rs = 0x0000_0033_0000_0022_0000_0011_0000_0000u128 | 0xaaaa;
        let rt = 0x0000_00cc_0000_00bb_0000_00aa_0000_0000u128 | 0x5555;
        for (w, want) in [
            // rt0, rs0, rt1, rs1
            (mmi(0x08, 0x12), 0x0000_0011_0000_00aa_0000_aaaa_0000_5555u128),
            // rt2, rs2, rt3, rs3
            (mmi(0x28, 0x12), 0x0000_0033_0000_00cc_0000_0022_0000_00bbu128),
            // rt low, rs low
            (mmi(0x09, 0x0e), (rt as u64 as u128) | (rs as u64 as u128) << 64),
            // rs high, rt high
            (mmi(0x29, 0x0e), (rs >> 64) | (rt >> 64) << 64),
        ] {
            let mut rn = Runner::empty();
            rn.cpu.gpr[4] = rs;
            rn.cpu.gpr[5] = rt;
            rn.mem.write_u32(CODE, w);
            rn.mem.write_u32(CODE + 4, r(0, 31, 0, 0, 0, 8));
            rn.mem.write_u32(CODE + 8, NOP);
            rn.call(CODE).unwrap();
            assert_eq!(rn.cpu.gpr[8], want, "{w:#x}");
        }
    }

    #[test]
    fn the_128_bit_lanes_and_logic() {
        // paddw / psubw / paddb / psubb / pand / pxor / por / pnor r8, r4, r5
        let mmi = |fun: u32, sa: u32| (28 << 26) | (4 << 21) | (5 << 16) | (8 << 11) | (sa << 6) | fun;
        let rs = 0x0000_0001_ffff_ffff_8000_0000_0102_03ffu128;
        let rt = 0x0000_0002_0000_0001_8000_0000_0101_0101u128;
        for (w, want) in [
            (mmi(0x08, 0x00), 0x0000_0003_0000_0000_0000_0000_0203_0500u128),
            (mmi(0x08, 0x01), 0xffff_ffff_ffff_fffe_0000_0000_0001_02feu128),
            (mmi(0x08, 0x08), 0x0000_0003_ffff_ff00_0000_0000_0203_0400u128),
            (mmi(0x08, 0x09), 0x0000_00ff_ffff_fffe_0000_0000_0001_02feu128),
            (mmi(0x09, 0x12), rs & rt),
            (mmi(0x09, 0x13), rs ^ rt),
            (mmi(0x29, 0x12), rs | rt),
            (mmi(0x29, 0x13), !(rs | rt)),
        ] {
            let mut rn = Runner::empty();
            rn.cpu.gpr[4] = rs;
            rn.cpu.gpr[5] = rt;
            rn.mem.write_u32(CODE, w);
            rn.mem.write_u32(CODE + 4, r(0, 31, 0, 0, 0, 8));
            rn.mem.write_u32(CODE + 8, NOP);
            rn.call(CODE).unwrap();
            assert_eq!(rn.cpu.gpr[8], want, "{w:#x}");
        }
    }

    #[test]
    fn mult1_uses_the_second_pipeline() {
        let mut rn = Runner::empty();
        // mult1 r3, r4, r5; mflo1 r6; mfhi1 r7; mflo r8; jr ra; nop
        let mmi = |rs: u32, rt: u32, rd: u32, f: u32| 28 << 26 | rs << 21 | rt << 16 | rd << 11 | f;
        for (i, w) in [mmi(4, 5, 3, 0x18), mmi(0, 0, 6, 0x12), mmi(0, 0, 7, 0x10), 18 | 8 << 11, 31 << 21 | 8, 0].iter().enumerate() {
            rn.mem.write_u32(CODE + 4 * i as u32, *w);
        }
        rn.cpu.set_gpr(4, (-70000i64) as u64);
        rn.cpu.set_gpr(5, 90000);
        rn.cpu.lo = 0x1234;
        rn.call(CODE).unwrap();
        let p = -70000i64 * 90000;
        assert_eq!(rn.cpu.gpr(3), p as u32 as i32 as i64 as u64);
        assert_eq!(rn.cpu.gpr(6), p as u32 as i32 as i64 as u64);
        assert_eq!(rn.cpu.gpr(7), (p >> 32) as u32 as i32 as i64 as u64);
        assert_eq!(rn.cpu.gpr(8), 0x1234, "LO0 untouched");
    }

    #[test]
    fn mult_keeps_lo1_and_madd1_adds_to_it() {
        let mut rn = Runner::empty();
        // mtlo1 r4; mult r3, r5, r6; madd1 r7, r5, r6; mflo1 r8; mflo r9; jr ra; nop
        let mmi = |rs: u32, rt: u32, rd: u32, f: u32| 28 << 26 | rs << 21 | rt << 16 | rd << 11 | f;
        let mult = 5 << 21 | 6 << 16 | 3 << 11 | 24;
        for (i, w) in [mmi(4, 0, 0, 0x13), mult, mmi(5, 6, 7, 0x20), mmi(0, 0, 8, 0x12), 18 | 9 << 11, 31 << 21 | 8, 0].iter().enumerate() {
            rn.mem.write_u32(CODE + 4 * i as u32, *w);
        }
        rn.cpu.set_gpr(4, 32000);
        rn.cpu.set_gpr(5, 7);
        rn.cpu.set_gpr(6, 6);
        rn.call(CODE).unwrap();
        assert_eq!(rn.cpu.gpr(3), 42);
        assert_eq!(rn.cpu.gpr(9), 42, "LO0 from mult");
        assert_eq!(rn.cpu.gpr(7), 32042, "madd1 on LO1 kept through mult");
        assert_eq!(rn.cpu.gpr(8), 32042);
    }
}

/// The MMI instructions implemented so far: the pipeline-1 multiply and divide (`mult1`, `multu1`, `div1`,
/// `divu1`, `madd1`, `maddu1`) and their HI1/LO1 moves (HI1 and LO1 are the upper 64 bits of HI and LO); the
/// lanes' add and subtract (`paddw`, `psubw`, `paddb`, `psubb`) and the 128-bit logic (`pand`, `pxor`, `por`,
/// `pnor`, as libc's `strcmp` uses them); `pextlw`, `pextuw`, `pcpyld`, `pcpyud`.
fn mmi(r: &mut Runner, pc: u32, w: u32, rs: usize, rt: usize, rd: usize, fun: u32) -> Result<Next, Error> {
    let c = &mut r.cpu;
    let (a, b) = (c.gpr(rs), c.gpr(rt));
    let set_hi1 = |c: &mut crate::Cpu, v: u64| c.hi = (c.hi & u64::MAX as u128) | ((v as u128) << 64);
    let set_lo1 = |c: &mut crate::Cpu, v: u64| c.lo = (c.lo & u64::MAX as u128) | ((v as u128) << 64);
    match fun {
        0x00 | 0x01 => {
            // madd / maddu: {HI, LO} (32 bits each) += rs * rt; LO, HI sign-extended, and rd = LO
            let acc = ((c.hi as u64 as u32 as u64) << 32 | c.lo as u64 as u32 as u64) as i64;
            let p = if fun == 0 { (a as u32 as i32 as i64).wrapping_mul(b as u32 as i32 as i64) } else { ((a as u32 as u64) * (b as u32 as u64)) as i64 };
            let t = acc.wrapping_add(p) as u64;
            let keep = |x: u128| (x >> 64) << 64;
            c.lo = keep(c.lo) | sext32(t as u32) as u128;
            c.hi = keep(c.hi) | sext32((t >> 32) as u32) as u128;
            c.set_gpr(rd, sext32(t as u32));
        }
        0x20 | 0x21 => {
            // madd1 / maddu1: as madd / maddu on HI1, LO1
            let acc = (((c.hi >> 64) as u64 as u32 as u64) << 32 | (c.lo >> 64) as u64 as u32 as u64) as i64;
            let p = if fun == 0x20 { (a as u32 as i32 as i64).wrapping_mul(b as u32 as i32 as i64) } else { ((a as u32 as u64) * (b as u32 as u64)) as i64 };
            let t = acc.wrapping_add(p) as u64;
            set_lo1(c, sext32(t as u32));
            set_hi1(c, sext32((t >> 32) as u32));
            c.set_gpr(rd, sext32(t as u32));
        }
        0x10 => c.set_gpr(rd, (c.hi >> 64) as u64), // mfhi1
        0x11 => set_hi1(c, a),                       // mthi1
        0x12 => c.set_gpr(rd, (c.lo >> 64) as u64), // mflo1
        0x13 => set_lo1(c, a),                       // mtlo1
        0x18 | 0x19 => {
            // mult1 / multu1: as mult / multu into LO1, HI1 (and rd)
            let p = if fun == 0x18 { (a as u32 as i32 as i64).wrapping_mul(b as u32 as i32 as i64) as u64 } else { (a as u32 as u64) * (b as u32 as u64) };
            set_lo1(c, sext32(p as u32));
            set_hi1(c, sext32((p >> 32) as u32));
            c.set_gpr(rd, sext32(p as u32));
        }
        0x1a => {
            let (n, d) = (a as u32 as i32, b as u32 as i32);
            if d != 0 {
                set_lo1(c, sext32(n.wrapping_div(d) as u32));
                set_hi1(c, sext32(n.wrapping_rem(d) as u32));
            }
        }
        0x1b => {
            let (n, d) = (a as u32, b as u32);
            if d != 0 {
                set_lo1(c, sext32(n / d));
                set_hi1(c, sext32(n % d));
            }
        }
        // the 128-bit lane arithmetic and logic: paddw / psubw / paddb / psubb (MMI0), pand / pxor (MMI2),
        // por / pnor (MMI3)
        0x08 | 0x09 | 0x29 if matches!((fun, (w >> 6) & 0x1f), (0x08, 0x00 | 0x01 | 0x08 | 0x09) | (0x09, 0x12 | 0x13) | (0x29, 0x12 | 0x13)) => {
            let (s, t) = (r.cpu.gpr[rs], r.cpu.gpr[rt]);
            let lanes = |bits: u32, f: &dyn Fn(u128, u128) -> u128| -> u128 {
                let mask = (1u128 << bits) - 1;
                (0..128 / bits).fold(0u128, |acc, i| acc | (f((s >> (bits * i)) & mask, (t >> (bits * i)) & mask) & mask) << (bits * i))
            };
            let v = match (fun, (w >> 6) & 0x1f) {
                (0x08, 0x00) => lanes(32, &|x, y| x.wrapping_add(y)),
                (0x08, 0x01) => lanes(32, &|x, y| x.wrapping_sub(y)),
                (0x08, 0x08) => lanes(8, &|x, y| x.wrapping_add(y)),
                (0x08, 0x09) => lanes(8, &|x, y| x.wrapping_sub(y)),
                (0x09, 0x12) => s & t,
                (0x09, 0x13) => s ^ t,
                (0x29, 0x12) => s | t,
                _ => !(s | t),
            };
            if rd != 0 {
                r.cpu.gpr[rd] = v;
            }
        }
        // the 128-bit shuffles (MMI0 / MMI1 / MMI2 / MMI3 groups, the operation in the sa field)
        0x08 | 0x28 | 0x09 | 0x29 if matches!((fun, (w >> 6) & 0x1f), (0x08 | 0x28, 0x12) | (0x09 | 0x29, 0x0e)) => {
            let (s, t) = (r.cpu.gpr[rs], r.cpu.gpr[rt]);
            let word = |v: u128, i: u32| (v >> (32 * i)) as u32 as u128;
            let v = match fun {
                // pextlw: rt0, rs0, rt1, rs1
                0x08 => word(t, 0) | word(s, 0) << 32 | word(t, 1) << 64 | word(s, 1) << 96,
                // pextuw: rt2, rs2, rt3, rs3
                0x28 => word(t, 2) | word(s, 2) << 32 | word(t, 3) << 64 | word(s, 3) << 96,
                // pcpyld: low = rt low, high = rs low
                0x09 => (t as u64 as u128) | (s as u64 as u128) << 64,
                // pcpyud: low = rs high, high = rt high
                _ => (s >> 64) | (t >> 64) << 64,
            };
            if rd != 0 {
                r.cpu.gpr[rd] = v;
            }
        }
        _ => return Err(Error::Unimplemented { pc, word: w, what: format!("MMI {fun:#x}") }),
    }
    Ok(Next::Seq)
}
