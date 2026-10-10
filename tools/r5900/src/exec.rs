//! The interpreter: decode and run one R5900 instruction at a time, with branch delay slots.
//! Implements what `tools/insn_census.py` lists for the functions checked so far; anything else stops the run
//! with `Error::Unimplemented` naming the instruction, which is the cue to add it here with a test.

use crate::ps2float as pf;
use crate::{Error, Runner, RETURN_SENTINEL, STEP_LIMIT};

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
        if r.steps > STEP_LIMIT {
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
    if !is_call || target == RETURN_SENTINEL {
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
                24 => {
                    // mult: on the EE it also writes rd
                    let p = (a as u32 as i32 as i64) * (b as u32 as i32 as i64);
                    c.lo = sext32(p as u32) as u128;
                    c.hi = sext32((p >> 32) as u32) as u128;
                    sext32(p as u32)
                }
                26 => {
                    // div
                    let (n, d) = (a as u32 as i32, b as u32 as i32);
                    if d != 0 {
                        c.lo = sext32(n.wrapping_div(d) as u32) as u128;
                        c.hi = sext32(n.wrapping_rem(d) as u32) as u128;
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
        28 => unimpl("MMI"),
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
fn cop2(r: &mut Runner, pc: u32, w: u32, rs: usize, rt: usize, rd: usize, sa: u32, fun: u32) -> Result<Next, Error> {
    let c = &mut r.cpu;
    let unimpl = |what: String| Err(Error::Unimplemented { pc, word: w, what });
    if rs & 16 == 0 {
        match rs {
            1 => c.gpr[rt] = u128::from_le_bytes(vf_to_quad(c.vf[rd])), // qmfc2
            2 => {
                // cfc2: VI0-15, Q (22); flags and the rest are not modelled yet
                let v = match rd {
                    0..=15 => c.vi[rd] as u32,
                    22 => c.q.to_bits(),
                    _ => return unimpl(format!("cfc2 from control register {rd}")),
                };
                c.set_gpr(rt, sext32(v));
            }
            5 => {
                let q = c.gpr[rt].to_le_bytes(); // qmtc2
                set_vf(c, rd, 0b1111, quad_to_vf(&q));
            }
            _ => return unimpl(format!("cop2 rs {rs}")),
        }
        return Ok(Next::Seq);
    }
    // VU0 macro mode: dest bits 24..21 (x..w), ft 20..16, fs 15..11, fd 10..6, broadcast in the low two bits
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
            _ => return unimpl(format!("vu0 macro funct {fun}")),
        };
        set_vf(c, fd, dest, v);
        return Ok(Next::Seq);
    }
    let fsf = ((w >> 21) & 3) as usize;
    let ftf = ((w >> 23) & 3) as usize;
    let acc = c.acc;
    let to_acc = match (sa << 2) | (fun & 3) {
        0..=3 => Some(lanes(&|i| pf::add(s[i], bc))),                   // vaddabc
        8..=11 => Some(lanes(&|i| pf::add(acc[i], pf::mul(s[i], bc)))), // vmaddabc
        24..=27 => Some(lanes(&|i| pf::mul(s[i], bc))),                 // vmulabc
        42 => Some(lanes(&|i| pf::mul(s[i], t[i]))),                    // vmula
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
        59 => None, // vwaitq: the result is already there
        x => return unimpl(format!("vu0 macro special2 {x}")),
    };
    if let Some(v) = to_acc {
        for (i, x) in v.into_iter().enumerate() {
            if dest & (8 >> i) != 0 {
                c.acc[i] = x;
            }
        }
    }
    Ok(Next::Seq)
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
            ],
            |rn| {
                rn.cpu.vf[1] = [1.0, 2.0, 3.0, 4.0];
                rn.cpu.vf[2] = [2.0, 10.0, 0.0, 4.0];
            },
        );
        let c = &rn.cpu;
        assert_eq!(c.vf[3], [3.0, 12.0, 3.0, 8.0]);
        assert_eq!(c.vf[4], [2.0, 4.0, 6.0, 8.0]);
        assert_eq!(c.q, 0.25);
        assert_eq!(c.vf[5], [0.25, 0.5, 0.75, 1.0]);
        assert_eq!(c.vf[6], [15.0, 20.0, 25.0, 30.0]);
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
        rn.mem.write_u32(CODE, 28 << 26); // an MMI instruction
        let e = rn.call(CODE).unwrap_err();
        assert!(e.to_string().contains("MMI"), "{e}");
    }
}
