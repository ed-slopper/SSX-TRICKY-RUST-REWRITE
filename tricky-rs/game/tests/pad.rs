//! The pad mapper against the original's (board row G1a5b2), in the function runner on the player's SLUS_203.26
//! (TRICKY_ELF; skipped without it): `cPS2Device` vf10 0x17b278 (the pad read), `Pad_ReadAxis` 0x179940, the four
//! `cInput` queries (0x179860, 0x179890, 0x1798c8, 0x179900) on random pads and maps; and, with
//! TRICKY_BTNMAP_DIR set to a folder holding the disc's `BTNMAP0.DAT` and `BTNMAP1.DAT`, the game's own parser
//! (`Pad_ParseButtonMap` 0x179b18) on them against `ButtonMap::btnmap`.

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::controls::{read_axis, ButtonMap, Mapper, PadDevice, PadMapper};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.next() % 1_000_000) as f32 / 1_000_000.0
    }
}

fn runner() -> Option<Runner> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    Some(r)
}

fn put_device(m: &mut r5900::Mem, at: u32, d: &PadDevice) {
    m.write_u32(at + 0xc, d.prev);
    m.write_u32(at + 0x10, d.buttons);
    for (o, v) in [(0x14, d.dpad[0]), (0x18, d.dpad[1]), (0x1c, d.left[0]), (0x20, d.left[1]), (0x24, d.right[0]), (0x28, d.right[1])] {
        m.write_f32(at + o, v);
    }
}

fn get_device(m: &mut r5900::Mem, at: u32) -> PadDevice {
    let f = |m: &mut r5900::Mem, o: u32| m.read_f32(at + o);
    PadDevice {
        prev: m.read_u32(at + 0xc),
        buttons: m.read_u32(at + 0x10),
        dpad: [f(m, 0x14), f(m, 0x18)],
        left: [f(m, 0x1c), f(m, 0x20)],
        right: [f(m, 0x24), f(m, 0x28)],
    }
}

fn random_device(g: &mut Rng) -> PadDevice {
    let mut v = |g: &mut Rng| match g.next() % 5 {
        0 => 0.0,
        1 => [1.0, -1.0, 0.38046828, -0.38046828, 0.3804682][g.next() as usize % 5],
        _ => g.f(-1.0, 1.0),
    };
    PadDevice { prev: g.next() | g.next() << 16, buttons: g.next() | g.next() << 16, dpad: [v(g), v(g)], left: [v(g), v(g)], right: [v(g), v(g)] }
}

#[test]
fn pad_read_matches_the_original() {
    let Some(mut r) = runner() else { return };
    r.allow(0x2ce114); // EAMem_Copy
    r.stub(0x17a958, |_, _| {}); // PadDevice_UpdateRepeat
    let dev = r.mem.alloc(0x100);
    let rec = r.mem.alloc(0x40);
    let mut g = Rng(0x7ad1_3c55_09e4_1b77);
    for case in 0..20_000 {
        let mut ours = random_device(&mut g);
        put_device(&mut r.mem, dev, &ours);
        let ps2 = g.next() as u16;
        let analog = if g.next() % 4 == 0 { None } else { Some([g.next() as u8, g.next() as u8, g.next() as u8, g.next() as u8]) };
        r.mem.write_u32(rec + 0x10, 1);
        r.mem.write_u32(rec + 0x14, analog.is_some() as u32);
        r.mem.write_u16(rec + 0x18, ps2);
        let a = analog.unwrap_or([0; 4]);
        for (k, b) in a.iter().enumerate() {
            r.mem.write_u32(rec + 0x1c + 4 * k as u32, *b as u32);
        }
        r.cpu.set_gpr(4, dev as u64);
        r.cpu.set_gpr(5, rec as u64);
        r.call(0x17b278).unwrap_or_else(|e| panic!("case {case}: {e}"));
        ours.read(ps2, analog);
        let theirs = get_device(&mut r.mem, dev);
        assert_eq!(format!("{theirs:?}"), format!("{ours:?}"), "case {case}: ps2 {ps2:#06x} analog {analog:?}");
    }
}

#[test]
fn axes_and_queries_match_the_original() {
    let Some(mut r) = runner() else { return };
    let dev = r.mem.alloc(0x100);
    let inp = r.mem.alloc(0x200);
    r.mem.write_u32(inp, dev);
    let mut g = Rng(0x2f6c_a903_8811_d4e5);
    let mut nonzero = 0;
    for case in 0..30_000 {
        let d = random_device(&mut g);
        put_device(&mut r.mem, dev, &d);
        // an axis: any mix of the mask bits, often a map's own
        let mask = if g.next() % 2 == 0 { g.next() & 0x3ffff } else { [0x20020, 0x10010, 0x1000, 0x8000, 0x1001, 0x8008, 0x40, 0x200, 1, 8, 0][g.next() as usize % 11] };
        let negate = g.next() % 2 == 0;
        r.mem.write_u32(inp + 0x14c, mask);
        r.mem.write_u32(inp + 0x150, negate as u32);
        r.cpu.set_gpr(4, inp as u64);
        r.cpu.set_gpr(5, 0);
        r.call(0x179940).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let ours = read_axis(&d, mask, negate);
        assert_eq!(r.cpu.f(0).to_bits(), ours.to_bits(), "case {case}: axis mask {mask:#x} negate {negate} on {d:?}");
        nonzero += (ours != 0.0) as u32;
        // the four queries on action 0
        let pressed = [0xffff_ffff, 0, g.next() & 0x3fff, 1 << (g.next() % 16)][g.next() as usize % 4];
        let looked = pressed | [0, 0x3600, g.next() & 0xffff][g.next() as usize % 3];
        r.mem.write_u32(inp + 4, pressed);
        r.mem.write_u32(inp + 0xa8, looked);
        let mut map = ButtonMap { pressed: [0; 0x29], looked_at: [0; 0x29], axes: [(0, false); 16] };
        map.pressed[0] = pressed;
        map.looked_at[0] = looked;
        let mut m = Mapper { map: &map, pad: &d };
        for (addr, ours) in [(0x179860u32, m.q0c(0)), (0x179890, m.q14(0)), (0x1798c8, m.q1c(0)), (0x179900, m.q24(0))] {
            r.cpu.set_gpr(4, inp as u64);
            r.cpu.set_gpr(5, 0);
            r.call(addr).unwrap_or_else(|e| panic!("case {case}: {e}"));
            assert_eq!(r.cpu.gpr(2) as u32, ours, "case {case}: query {addr:#x} pressed {pressed:#x} looked {looked:#x} on {d:?}");
        }
    }
    assert!(nonzero > 1000, "axes read: {nonzero}");
}

#[test]
fn button_maps_parse_as_the_original() {
    let Some(dir) = std::env::var_os("TRICKY_BTNMAP_DIR") else { return };
    let Some(mut r) = runner() else { return };
    // the parser and the newlib it calls (sscanf); strlen's and strncmp's MMI loops are not in the runner: answered the same
    for f in [0x179b18u32, 0x179d98, 0x17a148, 0x17a208, 0x179fb0, 0x17a038, 0x17a0a0, 0x179e60, 0x2fcaa0, 0x17a1a8, 0x301090, 0x301b98] {
        r.allow(f);
    }
    // strncmp 0x2fd478 (MMI too): the C library's answer
    r.stub(0x2fd478, |c, m| {
        let (a, b, n) = (c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as u32);
        let byte = |m: &mut r5900::Mem, p: u32| (m.read_u32(p & !3) >> (8 * (p & 3))) as u8;
        let mut d = 0i32;
        for k in 0..n {
            let (x, y) = (byte(m, a + k), byte(m, b + k));
            if x != y || x == 0 {
                d = x as i32 - y as i32;
                break;
            }
        }
        c.set_gpr(2, d as i64 as u64);
    });
    r.stub(0x2fd260, |c, m| {
        let s = c.gpr(4) as u32;
        let mut n = 0;
        while (m.read_u32((s + n) & !3) >> (8 * ((s + n) & 3))) as u8 != 0 {
            n += 1;
        }
        c.set_gpr(2, n as u64);
    });
    for n in 0..2u32 {
        let text = std::fs::read(std::path::Path::new(&dir).join(format!("BTNMAP{n}.DAT"))).expect("BTNMAP file");
        let buf = r.mem.alloc(text.len() as u32 + 16);
        for (k, b) in text.iter().enumerate() {
            r.mem.write_u8(buf + k as u32, *b);
        }
        r.mem.write_u8(buf + text.len() as u32, 0);
        let ctx = r.mem.alloc(0x200);
        r.mem.write_u32(ctx, buf);
        r.mem.write_u32(ctx + 4, text.len() as u32);
        r.cpu.set_gpr(4, ctx as u64);
        loop {
            match r.call(0x179b18) {
                Ok(()) => break,
                Err(r5900::Error::UnstubbedCall { target, name, .. }) => panic!("allow {target:#x} ({name})"),
                Err(e) => panic!("map {n}: {e}"),
            }
        }
        // the parse context: +8/+0xc/+0x10 counts, +0x14 IS_BOOL pairs, +0x84 grab pairs, +0xfc axes
        assert_eq!((r.mem.read_u32(ctx + 8), r.mem.read_u32(ctx + 0xc), r.mem.read_u32(ctx + 0x10)), (14, 15, 16), "map {n}: counts");
        let slots: [u16; 15] = std::array::from_fn(|k| k as u16);
        let ours = ButtonMap::btnmap(n, slots);
        for k in 0..14u32 {
            let (p, o) = (r.mem.read_u32(ctx + 0x14 + 8 * k), r.mem.read_u32(ctx + 0x18 + 8 * k));
            assert_eq!((p, o | p), (ours.pressed[0x1b + k as usize], ours.looked_at[0x1b + k as usize]), "map {n}: button {k}");
        }
        for k in 0..15u32 {
            let (p, o) = (r.mem.read_u32(ctx + 0x84 + 8 * k), r.mem.read_u32(ctx + 0x88 + 8 * k));
            assert_eq!((p, o | p), (ours.pressed[k as usize], ours.looked_at[k as usize]), "map {n}: grab {k}");
        }
        for k in 0..16u32 {
            let (m, neg) = (r.mem.read_u32(ctx + 0xfc + 8 * k), r.mem.read_u32(ctx + 0x100 + 8 * k));
            assert_eq!((m, neg != 0), ours.axes[k as usize], "map {n}: axis {k}");
        }
    }
}
