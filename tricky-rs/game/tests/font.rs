//! `tricky_game::font` (board row G8k) against the original in the function runner (TRICKY_ELF; skipped without
//! it): random SFN files (both record sizes, runs of consecutive codes and gaps) parsed by `Font_ParseGlyphs` into
//! a font object, then random ASCII and wide strings measured by `Font_MeasureAscii` / `_MeasureWide` at random
//! scales; the font object, its glyph table and the measured sizes compared, and every `Font_FindGlyph` answer.

use r5900::Runner;
use tricky_game::font::*;
use tricky_game::pf::Mem;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
}

/// A random SFN: the header, then the records (11 or 12 bytes) at +0x20, codes ascending
fn sfn(g: &mut Rng) -> Vec<u8> {
    let version: u16 = [100, 199, 200, 201][g.below(4) as usize];
    let count = g.below(60) as usize;
    let size = if version < 200 { 11 } else { 12 };
    let mut d = vec![0u8; 0x20 + size * count];
    d[0..4].copy_from_slice(b"FNTS");
    d[8..10].copy_from_slice(&version.to_le_bytes());
    d[0xa..0xc].copy_from_slice(&(count as u16).to_le_bytes());
    d[0x10] = g.next() as u8;
    d[0x11] = g.next() as u8;
    d[0x14..0x18].copy_from_slice(&0x20u32.to_le_bytes());
    let mut code = [0u16, 30, 32, 65, 200][g.below(5) as usize];
    for i in 0..count {
        // mostly consecutive codes, then gaps
        code += if i == 0 { 0 } else if g.below(4) == 0 { 1 + g.below(40) as u16 } else { 1 };
        let r = 0x20 + size * i;
        d[r..r + 2].copy_from_slice(&code.to_le_bytes());
        for k in 2..size {
            d[r + k] = if k == 8 || k == 9 || k == 10 { [0u8, 5, 12, 0xfc, 0x80, 30][g.below(6) as usize] } else { g.next() as u8 };
        }
    }
    d
}

const F: [f32; 8] = [1.0, 0.5, 1.4, 1.3, 0.75, 2.25, 0.9, 1.8];

#[test]
fn fonts_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let base = r.mem.alloc(0x4000);
    let (font, table, file, text, out) = (base, base + 0x100, base + 0x1000, base + 0x3000, base + 0x3f00);
    for f in [0x2ccf70u32, 0x23d3b0] {
        r.stub(f, move |c, _| c.set_gpr(2, table as u64));
    }
    for f in [0x2cc808u32, 0x19cca8] {
        r.stub(f, |_, _| {});
    }
    r.allow(0x19cc10);
    let mut g = Rng(0x19c9_10ee_8e8e_0001);
    for case in 0..20_000u32 {
        for k in 0..0x4000 / 4 {
            r.mem.write_u32(base + 4 * k, g.next());
        }
        let d = sfn(&mut g);
        for (k, b) in d.iter().enumerate() {
            r.mem.write_u8(file + k as u32, *b);
        }
        let (adv, yadd) = (g.next() as u8, [0i32, 3, -2, 40][g.below(4) as usize]);
        r.mem.write_u32(font, [0, 0x8000_0000, 0x4000_0000][g.below(3) as usize]);
        let snap = |r: &mut Runner| {
            let mut m = Mem::default();
            let mut b = vec![0u8; 0x4000];
            r.mem.read(base, &mut b).unwrap();
            m.regions.push((base, b));
            m
        };
        let mut m = snap(&mut r);
        r.cpu.set_gpr(4, font as u64);
        r.cpu.set_gpr(5, file as u64);
        r.cpu.set_gpr(6, adv as i8 as i64 as u64);
        r.cpu.set_gpr(7, yadd as i64 as u64);
        r.call(0x19c910).unwrap_or_else(|e| panic!("case {case}: parse: {e}"));
        parse_glyphs(&mut m, font, &d, adv, yadd, table);
        // the parts of memory the parse writes: the object and the table (the table's twelfth bytes untouched)
        let theirs = snap(&mut r);
        if let Some(i) = (0..0x1000).find(|&i| theirs.regions[0].1[i] != m.regions[0].1[i]) {
            panic!("case {case}: parse: memory at {:#x}", base + i as u32);
        }
        // the scales, then a few strings
        for k in [0x18u32, 0x1c] {
            let v = F[g.below(F.len() as u32) as usize].to_bits();
            r.mem.write_u32(font + k, v);
            m.w32(font + k, v);
        }
        let n = m.r32(font + 4) as i32;
        let codes: Vec<u16> = (0..n.max(0) as u32).map(|i| m.r16(table + 12 * i)).collect();
        for _ in 0..4 {
            let wide = g.below(2) == 0;
            let len = g.below(12);
            let mut at = text;
            for _ in 0..len {
                let c = if !codes.is_empty() && g.below(4) != 0 { codes[g.below(codes.len() as u32) as usize] } else { 1 + g.below(300) as u16 };
                let c = if wide { c.max(1) } else { (c & 0xff).max(1) };
                if wide {
                    r.mem.write_u16(at, c);
                    m.w16(at, c);
                    at += 2;
                } else {
                    r.mem.write_u8(at, c as u8);
                    m.w8(at, c as u8);
                    at += 1;
                }
            }
            for k in 0..2 {
                r.mem.write_u8(at + k, 0);
                m.w8(at + k, 0);
            }
            let scale = F[g.below(F.len() as u32) as usize];
            let dst = if g.below(3) == 0 { 0 } else { out };
            r.cpu.set_f(12, scale);
            r.cpu.set_gpr(4, font as u64);
            r.cpu.set_gpr(5, text as u64);
            r.cpu.set_gpr(6, dst as u64);
            r.call(if wide { 0x19dc78 } else { 0x19da30 }).unwrap_or_else(|e| panic!("case {case}: measure: {e}"));
            let w = measure(&mut m, wide, scale, font, text, dst);
            assert_eq!(r.cpu.gpr(2) as i32, w, "case {case}: width (wide {wide}, scale {scale})");
            let mut b = vec![0u8; 16];
            r.mem.read(out, &mut b).unwrap();
            let mine: Vec<u8> = (0..16).map(|k| m.r8(out + k)).collect();
            assert_eq!(b, mine, "case {case}: size (wide {wide})");
            // every code's lookup
            for c in [0u32, 1, 31, 32, 64, 65, 66, 100, 199, 200, 230, 255, 400] {
                r.cpu.set_gpr(4, font as u64);
                r.cpu.set_gpr(5, c as u64);
                r.call(0x19cc10).unwrap();
                assert_eq!(r.cpu.gpr(2) as u32, find_glyph(&m, font, c), "case {case}: glyph {c}");
            }
        }
    }
}
