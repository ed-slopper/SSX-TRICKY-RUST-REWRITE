//! `SpriteSet_InitHudGameRects` 0x1f4440 and `SpriteSet_BindHudGameTex` 0x1f9080 (board row E1b2g7d3b4g) against the
//! original in the function runner (TRICKY_ELF; skipped without it): a set filled at random, the original and
//! `hudrects` run on the same bytes, every word of the set compared. The binding with each shape found or missing
//! (`Shape_FindByName` 0x2c2528 answered by the test), the renderer's texture call (gApp+0x724, virtual +0x154)
//! answered by a stub.

use r5900::Runner;
use std::cell::Cell;
use std::rc::Rc;
use tricky_game::hudrects;
use tricky_game::pf::Mem;

const SIZE: u32 = 0x1600;

fn next(s: &mut u32) -> u32 {
    *s ^= *s << 13;
    *s ^= *s >> 17;
    *s ^= *s << 5;
    *s
}

fn compare(r: &mut Runner, set: u32, m: &Mem, case: &str) {
    for k in 0..SIZE / 4 {
        let (theirs, ours) = (r.mem.read_u32(set + 4 * k), m.r32(set + 4 * k));
        assert_eq!(theirs, ours, "{case}: set+{:#x}", 4 * k);
    }
}

fn fill(r: &mut Runner, set: u32, s: &mut u32) -> Mem {
    let mut m = Mem { regions: vec![(set, vec![0; SIZE as usize])] };
    for k in 0..SIZE / 4 {
        let v = next(s);
        r.mem.write_u32(set + 4 * k, v);
        m.w32(set + 4 * k, v);
    }
    m
}

#[test]
fn the_hud_rects_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    let set = r.mem.alloc(SIZE);
    let mut s = 0x1f44_4001u32;
    for case in 0..4 {
        let mut m = fill(&mut r, set, &mut s);
        r.cpu.set_gpr(4, set as u64);
        r.call(0x1f4440).unwrap();
        hudrects::init_hud_game_rects(&mut m, set);
        compare(&mut r, set, &m, &format!("init case {case}"));
    }
}

#[test]
fn the_hud_textures_bind_as_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    let set = r.mem.alloc(SIZE);
    let (rend, vt) = (r.mem.alloc(0x40), r.mem.alloc(0x200));
    let mut app = r.mem.read_u32(0x337c58);
    if app == 0 {
        app = r.mem.alloc(0x800);
        r.mem.write_u32(0x337c58, app);
    }
    r.mem.write_u32(app + 0x724, rend);
    r.mem.write_u32(rend + 0x10, vt);
    r.mem.write_u32(vt + 0x150, 0);
    // the renderer's texture call, at an address the runner stubs: the texture is 0x7700_0000 + the name's address
    r.mem.write_u32(vt + 0x154, 0x1000a8);
    r.stub(0x1000a8, |c, _| {
        let name = c.gpr(6) as u32;
        c.set_gpr(2, (0x7700_0000 | (name & 0xffff)) as u64);
    });
    let names = [0x39a500u32, 0x39a508, 0x39a510];
    let found = Rc::new(Cell::new(0u32));
    let f = found.clone();
    r.stub(0x2c2528, move |c, _| {
        let name = c.gpr(5) as u32;
        let k = names.iter().position(|&n| n == name).expect("one of the three names");
        c.set_gpr(2, if f.get() >> k & 1 != 0 { 0x100 + k as u64 } else { 0 });
    });
    let mut s = 0x1f90_8001u32;
    for mask in 0..8 {
        found.set(mask);
        let mut m = fill(&mut r, set, &mut s);
        r.cpu.set_gpr(4, set as u64);
        r.cpu.set_gpr(5, 0x1234);
        r.call(0x1f9080).unwrap();
        hudrects::bind_hud_game_tex(&mut m, set, &mut |k| (mask >> k & 1 != 0).then(|| 0x7700_0000 | (names[k] & 0xffff)));
        compare(&mut r, set, &m, &format!("bind, shapes found {mask:03b}"));
    }
}
