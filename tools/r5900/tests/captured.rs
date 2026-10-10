//! Replay calls captured from the running game (`tools/pcsx2_debug.py capture`) through the runner and compare
//! the result with what the game returned, bit for bit. Board row F4f. Skips unless TRICKY_ELF and TRICKY_CAPTURES are set;
//! the capture file is made from the game, so it stays on your own PC.

use r5900::Runner;

struct Call {
    addr: u32,
    gpr: Vec<(usize, u64)>,
    fpr: Vec<(usize, u32)>,
    mem: Vec<(u32, Vec<u8>)>,
    f0: u32,
    /// From `trace`: (pc, the 32 float registers just before that instruction ran).
    steps: Vec<(u32, [u32; 32])>,
}

fn hex(s: &str) -> u64 {
    u64::from_str_radix(s, 16).expect("hex")
}

fn bytes(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex")).collect()
}

fn parse(text: &str) -> Vec<Call> {
    let mut calls = Vec::new();
    let mut cur: Option<Call> = None;
    for line in text.lines() {
        let p: Vec<&str> = line.split_whitespace().collect();
        match p.as_slice() {
            ["call", a] => cur = Some(Call { addr: hex(a) as u32, gpr: vec![], fpr: vec![], mem: vec![], f0: 0, steps: vec![] }),
            ["gpr", n, v] => cur.as_mut().unwrap().gpr.push((n.parse().unwrap(), hex(v))),
            ["fpr", n, v] => cur.as_mut().unwrap().fpr.push((n.parse().unwrap(), hex(v) as u32)),
            ["mem", a, d] => cur.as_mut().unwrap().mem.push((hex(a) as u32, bytes(d))),
            ["ret", "f0", v] => cur.as_mut().unwrap().f0 = hex(v) as u32,
            ["step", pc, regs @ ..] if regs.len() == 32 => {
                let mut f = [0u32; 32];
                for (i, v) in regs.iter().enumerate() {
                    f[i] = hex(v) as u32;
                }
                cur.as_mut().unwrap().steps.push((hex(pc) as u32, f));
            }
            ["end"] => calls.push(cur.take().unwrap()),
            _ => {}
        }
    }
    calls
}

#[test]
fn captured_calls_match_the_game() {
    let (Ok(elf), Ok(path)) = (std::env::var("TRICKY_ELF"), std::env::var("TRICKY_CAPTURES")) else {
        eprintln!("TRICKY_ELF and TRICKY_CAPTURES not both set: skipping");
        return;
    };
    let elf = std::fs::read(elf).expect("TRICKY_ELF");
    let calls = parse(&std::fs::read_to_string(path).expect("TRICKY_CAPTURES"));
    assert!(!calls.is_empty(), "no calls in the capture file");
    let mut worst = 0i64;
    let mut exact = 0;
    for (k, c) in calls.iter().enumerate() {
        let mut r = Runner::from_elf(&elf).expect("SLUS_203.26");
        if let Ok(rules) = std::env::var("TRICKY_FLOAT_RULES") {
            r.float_rules = r5900::ps2float::Rules::parse(&rules).expect("TRICKY_FLOAT_RULES");
        }
        for (addr, data) in &c.mem {
            r.mem.write(*addr, data).expect("captured block inside RAM");
        }
        for &(n, v) in &c.gpr {
            r.cpu.set_gpr(n, v);
        }
        for &(n, v) in &c.fpr {
            r.cpu.fpr[n] = v;
        }
        if let Some((_, regs)) = c.steps.first() {
            r.cpu.fpr = *regs; // all 32, as the game had them at the call
        }
        let mut mine: Vec<(u32, [u32; 32])> = Vec::new();
        let mut before = r.cpu.fpr;
        r.call_traced(c.addr, |pc, cpu| {
            mine.push((pc, before));
            before = cpu.fpr;
        })
        .unwrap_or_else(|e| panic!("call {k}: {e}"));
        if !c.steps.is_empty() {
            first_difference(k, &c.steps, &mine);
        }
        let got = r.cpu.fpr[0];
        let ulps = (got as i32 as i64 - c.f0 as i32 as i64).abs();
        worst = worst.max(ulps);
        exact += (ulps == 0) as usize;
        eprintln!(
            "{k:2} {} game {:08x} ({:>12.6}) runner {:08x} ({:>12.6}) {}",
            r.name(c.addr),
            c.f0,
            f32::from_bits(c.f0),
            got,
            f32::from_bits(got),
            if ulps == 0 { "exact".to_string() } else { format!("{ulps} ulp") }
        );
    }
    eprintln!("{exact} of {} exact, largest difference {worst} ulp", calls.len());
    // Measured 2026-10-10 in PCSX2 over 159 calls of Boarder_ForwardDrag and Boarder_SideFriction: with the
    // default rules 111 exact, the rest 1 or 2 ulp (tricky-rs/docs/checking.md, F4f and F4g). TRICKY_FLOAT_RULES
    // tries other rules. Tighten to exact once PCSX2's rounding is reproduced (board row F4g).
    assert!(worst <= 2, "the runner is {worst} ulp from the game");
}

/// Walk the game's trace and the runner's together; print the first instruction after which a float register
/// differs. The game only stops where a breakpoint can be (not in delay slots), so match on pc in order.
fn first_difference(k: usize, game: &[(u32, [u32; 32])], mine: &[(u32, [u32; 32])]) {
    let mut j = 0;
    let mut last_pc = None;
    for (pc, regs) in game {
        while j < mine.len() && mine[j].0 != *pc {
            j += 1;
        }
        if j == mine.len() {
            eprintln!("   call {k}: the runner never reached 0x{pc:06x}");
            return;
        }
        if let Some(n) = (0..32).find(|&n| mine[j].1[n] != regs[n]) {
            eprintln!(
                "   call {k}: before 0x{pc:06x} (after 0x{:06x}) $f{n}: game {:08x} ({}) runner {:08x} ({})",
                last_pc.unwrap_or(0),
                regs[n],
                f32::from_bits(regs[n]),
                mine[j].1[n],
                f32::from_bits(mine[j].1[n]),
            );
            return;
        }
        last_pc = Some(*pc);
        j += 1;
    }
}
