//! The world nodes' snapshot saves (board row G12j) against the game in the function runner (TRICKY_ELF; skipped
//! without it): each class's save on random node memory (its pointers aimed at real places), into a stream that is
//! sometimes nearly full or already overflowed; the stream, the node (its tags) and the control compared byte for
//! byte.

use r5900::Runner;
use tricky_game::pf::Mem;
use tricky_game::snapshot as sn;

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

const NODE_LEN: u32 = 0x800;
const SBUF: u32 = 0x3000;

#[test]
fn world_node_saves_are_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    let saves: Vec<u32> = [
        0x13aef8, 0x13b0b0, 0x148dc0, 0x13fd90, 0x141a88, 0x144710, 0x1453f0, 0x146660, 0x147f30, 0x1481d0, 0x1489f0, 0x149158, 0x13d3d8,
        0x1998f8, 0x19a410, 0x199ee0, 0x19a940, 0x1fb1a8,
    ]
    .into_iter()
    .chain(sn::SIMPLE.iter().map(|c| c.1))
    .collect();
    for a in saves.iter().copied().chain([
        0x154480, 0x1d8d48, 0x1cb198, 0x2519b0, 0x17d1d8, 0x17d208, 0x192c10, 0x192f30, 0x192f78, 0x192fe8, 0x193020, 0x2ce114,
    ]) {
        r.allow(a);
    }
    let node = r.mem.alloc(NODE_LEN);
    let inst = r.mem.alloc(0x100 * 8);
    let riders = r.mem.alloc(0x500 * 2);
    let extra = r.mem.alloc(0x800);
    let app = r.mem.alloc(0x800);
    let game = r.mem.alloc(0x100);
    let ctl = r.mem.alloc(0x80);
    let course = r.mem.alloc(0x80);
    let stream = r.mem.alloc(0x10);
    let sbuf = r.mem.alloc(SBUF);
    let regions = [
        (0x337c58u32, 4u32),
        (0x34648c, 4),
        (0x34649c, 8),
        (0x31d338, 0x18),
        (node, NODE_LEN),
        (inst, 0x800),
        (riders, 0xa00),
        (extra, 0x800),
        (app, 0x800),
        (game, 0x100),
        (ctl, 0x80),
        (course, 0x80),
        (stream, 0x10),
        (sbuf, SBUF),
    ];
    let snapshot = |r: &mut Runner| {
        let mut m = Mem::default();
        for (a, n) in regions {
            let mut b = vec![0u8; n as usize];
            r.mem.read(a, &mut b).unwrap();
            m.regions.push((a, b));
        }
        m
    };
    let mut g = Rng(0x7361_7665_736e_0a01);
    let mut seen = vec![0u32; saves.len()];
    let mut overflowed = 0;
    for trial in 0..20000 {
        let mut m = snapshot(&mut r);
        for (a, v) in m.regions.iter_mut() {
            if *a != 0x31d338 {
                v.iter_mut().for_each(|b| *b = g.next() as u8);
            }
        }
        m.w32(0x337c58, app);
        m.w32(app + 0x730, game);
        m.w32(game + 0x40, ctl);
        m.w32(ctl + 0x58, (g.below(10) == 0) as u32);
        m.w32(0x34648c, course);
        m.w32(course + 0x48, inst);
        let k = g.below(saves.len() as u32) as usize;
        let f = saves[k];
        // the class's pointers aimed at real places: its instance (the dead node's +0x28, wherever its dead node
        // sits), the script's rider, an anim object's overlap / key states / their count, a combo's and a spline
        // path's arrays
        let dead_at = match f {
            0x146660 => Some(0x24u32),
            0x1998f8 | 0x199ee0 => Some(0x30),
            0x19a410 => Some(0x1c + 0x30),
            0x19a940 => Some(0x18 + 0x30),
            0x147f30 | 0x1489f0 | 0x149158 | 0x13d3d8 | 0x1fb1a8 => None,
            _ => Some(0),
        };
        if let Some(d) = dead_at {
            let i = if g.below(5) == 0 { 0 } else { inst + 0x100 * g.below(8) };
            m.w32(node + d + 0x28, i);
            if i != 0 {
                m.w32(i + 0xe4, if g.below(2) == 0 { node + d } else { g.next() });
            }
        }
        m.w32(node + 0xe8, if g.below(3) == 0 { 0 } else { riders + 0x500 * g.below(2) });
        let anim_at = match f {
            0x1998f8 | 0x199ee0 => Some(0u32),
            0x19a410 => Some(0x1c),
            0x19a940 => Some(0x18),
            _ => None,
        };
        if let Some(a) = anim_at {
            m.w32(node + a + 0x64, if g.below(2) == 0 { 0 } else { extra + 0x10 * g.below(4) });
            m.w32(node + a + 0x60, extra + 0x100);
            m.w32(node + a + 0x5c, g.below(4));
        }
        if f == 0x19a410 {
            m.w32(node + 0x84, extra + 0x500);
        }
        if f == 0x1fb1a8 {
            m.w32(node + 0x7c, extra + 0x600);
            m.w32(node + 0x30, g.below(5));
        }
        // the stream: from its base, written up to somewhere; the control's size sometimes nearly reached
        m.w32(stream, sbuf);
        let used = g.below(0x200);
        m.w32(stream + 4, sbuf + used);
        m.w32(stream + 0xc, used);
        m.w32(ctl + 0xc, if g.below(4) == 0 { used + g.below(0x100) } else { SBUF - 0x400 });
        for (a, v) in &m.regions {
            r.mem.write(*a, v).unwrap();
        }
        let what = format!("trial {trial} save {f:#x}");
        assert!(sn::save_by_address(&mut m, f, node, stream), "{what}: no port");
        r.cpu.set_gpr(4, node as u64);
        r.cpu.set_gpr(5, stream as u64);
        r.call(f).unwrap_or_else(|e| panic!("{what}: {e:?}"));
        seen[k] += 1;
        overflowed += m.r32(ctl + 0x58);
        let theirs = snapshot(&mut r);
        for ((a, x), (_, y)) in theirs.regions.iter().zip(&m.regions) {
            if let Some(i) = (0..x.len()).find(|&i| x[i] != y[i]) {
                let i = i & !3;
                panic!("{what}: memory at {:#x} (+{i:#x} in {a:#x}): {:x?} vs {:x?}", a + i as u32, &x[i..(i + 4).min(x.len())], &y[i..(i + 4).min(y.len())]);
            }
        }
    }
    eprintln!("saves {seen:?}, {overflowed} ended overflowed");
}
