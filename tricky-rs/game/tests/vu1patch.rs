//! The patch draw run whole in the function runner (board row G7a4a; TRICKY_ELF and TRICKY_DISC for gari's `.pbd`,
//! skipped without them), as the oracle for the VU1 patch programs' ports. All of it runs as original code:
//! - the Bezier manager made by its constructor, with the tables;
//! - some of gari's patches queued (vf16) at each tessellation and mixed, clipped or not;
//! - the queue drawn (vf15) through `PS2Render_Submit` into a draw list, under a camera made by the frame's
//!   matrices (0x1e85b0) and looking at the patches;
//! - the flush (`PS2Draw_FlushBuckets`, with `PS2Draw_WriteTexState`).
//!
//! The DMA chain the flush builds is then walked into `r5900::vu1::Vu1`, which runs the game's patch programs and
//! logs what they send to the GS. The renderer's texture and blend virtuals, the allocator, the fill, the texture
//! upload and the frame's view planes are stand-ins.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::vu1::Vu1;
use r5900::Runner;

const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x0010_0000;
const APP: u32 = ARENA;
const R: u32 = ARENA + 0x1000;
const RVT: u32 = ARENA + 0x2000;
const CAM: u32 = ARENA + 0x2400;
const TS: u32 = ARENA + 0x2500;
const BZ: u32 = ARENA + 0x3000;
const STEPS: u32 = ARENA + 0x3800;
const SHORTS: u32 = ARENA + 0x3900;
const SLOT: u32 = ARENA + 0x3a00;
const HEAP: u32 = ARENA + 0x4000;
const PATCHES: u32 = ARENA + 0x2_0000;
const D: u32 = ARENA + 0x4_0000;
const PKT: u32 = ARENA + 0x6_0000;
const OUT: u32 = ARENA + 0x8_0000;
const PAGES: u32 = ARENA + 0xa_0000;
const BATCHES: u32 = ARENA + 0xa_1000;
fn vaddr(off: u32) -> u32 {
    0x01ff_0000 + off
}

fn gari_patches() -> Option<Vec<Vec<u8>>> {
    let disc = std::env::var("TRICKY_DISC").ok()?;
    let d = tricky_data::disc::Disc::open(&disc).map_err(|e| eprintln!("TRICKY_DISC: {e:?}")).ok()?;
    let mut files = tricky_data::big::Files::new(d);
    files.mount("data/models/gari.big").map_err(|e| eprintln!("gari.big: {e:?}")).ok()?;
    let pbd = files.load("|data/models/gari.pbd").map_err(|e| eprintln!("gari.pbd: {e:?}")).ok().flatten()?;
    let parsed = tricky_data::pbd::Pbd::parse(&pbd);
    if parsed.is_none() {
        eprintln!("gari.pbd didn't parse");
    }
    Some(parsed?.patches)
}

fn w32(r: &mut Runner, at: u32, v: u32) {
    r.mem.write_u32(at, v);
}
fn wf(r: &mut Runner, at: u32, v: f32) {
    r.mem.write_u32(at, v.to_bits());
}

#[test]
fn the_patch_draw_runs_on_vu1() {
    let Some(mut r) = Runner::from_env() else { return };
    let Some(patches) = gari_patches() else { return };
    r.mem.write(ARENA, &vec![0u8; SIZE as usize]).unwrap();
    for f in [0x1d9be0u32, 0x1d9c88, 0x1db150, 0x1d9d20, 0x1db1e8, 0x1db530, 0x1d9d50, 0x1d9f58, 0x1e8838, 0x1e88c8, 0x1c4d68, 0x1c4d30, 0x1c4f28, 0x1c4340, 0x1c4468, 0x1c6f98, 0x1c7068, 0x1c7088, 0x1c4fe0, 0x1c4220, 0x1c51f0, 0x1c5368, 0x1c42a0, 0x1c5c88, 0x1ea068, 0x1c0340, 0x1c03e8, 0x1c04f8, 0x1c0560, 0x1c05d8, 0x1c06a0, 0x1e85b0, 0x2cfc00] {
        r.allow(f);
    }
    let heap = Rc::new(RefCell::new(HEAP));
    let h = heap.clone();
    // the allocator: the next 16-aligned place; the fill: as asked
    r.stub(0x2ccf70, move |c, _| {
        let mut p = h.borrow_mut();
        let at = *p;
        *p += (c.gpr(5) as u32 + 15) & !15;
        c.set_gpr(2, at as u64);
    });
    r.stub(0x2cc808, |c, m| {
        let (at, v, n) = (c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as u32);
        for k in 0..n / 4 {
            m.write_u32(at + 4 * k, v);
        }
    });
    r.stub(0x1c3818, |_, _| {});
    r.allow(0x1c4650);
    r.allow(0x1c4c38);
    for off in [0x1c8u32, 0x1f0, 0x1f8, 0x200] {
        r.stub(vaddr(off), |_, _| {});
    }
    // the app and the renderer: its vtable (the Bezier manager's getter real), texture state, draw list, packets
    w32(&mut r, 0x337c58, APP);
    w32(&mut r, APP + 0x724, R);
    w32(&mut r, R + 0x10, RVT);
    for off in [0x1c8u32, 0x1f0, 0x1f8, 0x200] {
        w32(&mut r, RVT + off + 4, vaddr(off));
    }
    w32(&mut r, RVT + 0x24, 0x1ea068);
    w32(&mut r, R + 0x470, TS);
    w32(&mut r, R + 0x480, D);
    w32(&mut r, R + 0x484, BZ);
    w32(&mut r, R + 0x680, PKT);
    w32(&mut r, R + 0x3f4, CAM);
    // the draw list: no pages, batches or sort entries; the z buffer words
    w32(&mut r, D + 4, PAGES);
    w32(&mut r, D + 0xc, BATCHES);
    w32(&mut r, D + 0xfa64, 0x32);
    w32(&mut r, D + 0xfa68, 0x8c);
    // the Bezier manager: constructed (its records, defaults, tables), the views off
    r.cpu.set_gpr(4, BZ as u64);
    r.call(0x1d9be0).expect("the Bezier manager's constructor");
    w32(&mut r, BZ + 0x20, 0);
    // gari's first patches, queued at 4, 6, 8 steps and mixed
    let n = patches.len().min(24);
    for (k, p) in patches.iter().take(n).enumerate() {
        let at = PATCHES + 0x1c0 * k as u32;
        r.mem.write(at, p).unwrap();
        let steps: [u8; 4] = match k % 4 {
            0 => [4; 4],
            1 => [6; 4],
            2 => [8; 4],
            _ => [4, 6, 8, 6],
        };
        r.mem.write(STEPS, &steps).unwrap();
        w32(&mut r, SHORTS, 0);
        w32(&mut r, SHORTS + 4, 0);
        for (i, v) in [BZ, at, STEPS, SHORTS, (k / 4 % 2) as u32].into_iter().enumerate() {
            r.cpu.set_gpr(4 + i, v as u64);
        }
        r.call(0x1d9d50).expect("the queue");
    }
    // a camera over the first patch's box, looking down
    let b: Vec<f32> = (0..6).map(|k| f32::from_bits(u32::from_le_bytes(patches[0][0x150 + 4 * k..0x154 + 4 * k].try_into().unwrap()))).collect();
    let centre = [(b[0] + b[3]) / 2.0, (b[1] + b[4]) / 2.0, (b[2] + b[5]) / 2.0];
    for base in [0x1730u32, 0x4d0, 0x570, 0x5b0] {
        for k in 0..16 {
            wf(&mut r, R + base + 4 * k, if k % 5 == 0 { 1.0 } else { 0.0 });
        }
    }
    // the view (row vectors, translation last): 2000 above the patch, looking down (view z = −world z)
    let view = [[1.0, 0.0, 0.0, 0.0], [0.0, -1.0, 0.0, 0.0], [0.0, 0.0, -1.0, 0.0], [-centre[0], centre[1], centre[2] + 2000.0, 1.0]];
    for (i, row) in view.iter().enumerate() {
        for (j, v) in row.iter().enumerate() {
            wf(&mut r, CAM + 16 * i as u32 + 4 * j as u32, *v);
        }
    }
    w32(&mut r, R + 0x17d0, 0);
    // the projection: the renderer's vf20 (field of view, near, far), on a 640 × 448 screen (its +0x60, +0x58)
    r.allow(0x1dc3e0);
    r.allow(0x2f3e38);
    r.allow(0x2f3108);
    r.allow(0x2f2880);
    r.allow(0x250c10);
    for (off, v) in [(0x60u32, 640u64), (0x58, 448)] {
        w32(&mut r, RVT + off + 4, vaddr(off));
        r.stub(vaddr(off), move |c, _| c.set_gpr(2, v));
    }
    r.cpu.set_gpr(4, R as u64);
    r.cpu.set_f(12, 1.0);
    r.cpu.set_f(13, 10.0);
    r.cpu.set_f(14, 100000.0);
    r.call(0x1dc3e0).expect("the projection");
    // (its x and y scales come out 0 with these arguments: a perspective of 1, 1 put in their place, keeping its
    // depth terms and the screen's scale and offset at +0x550 / +0x560)
    for base in [0x4d0u32, 0x570] {
        wf(&mut r, R + base, 1.0);
        wf(&mut r, R + base + 0x14, 1.0);
    }
    if std::env::var("DUMP").is_ok() {
        for at in (0x550u32..0x5f0).step_by(16) {
            let f: Vec<String> = (0..4).map(|k| format!("{:12.4}", f32::from_bits(r.mem.read_u32(R + at + 4 * k)))).collect();
            println!("R+{at:#x} {}", f.join(" "));
        }
    }
    // the queue drawn into the draw list, then the list flushed into one DMA chain
    r.cpu.set_gpr(4, BZ as u64);
    r.cpu.set_gpr(5, R as u64);
    r.call(0x1d9f58).expect("the queue drawn");
    w32(&mut r, SLOT, OUT);
    r.cpu.set_gpr(4, D as u64);
    r.cpu.set_gpr(5, SLOT as u64);
    r.call(0x1c4fe0).expect("the flush");
    let end = r.mem.read_u32(SLOT);
    println!("the chain: {:#x} bytes from {OUT:#x}; {} batches", end - OUT, r.mem.read_u32(D + 8));
    if std::env::var("DUMP").is_ok() {
        let set = r.mem.read_u32(D + 0xfa5c);
        println!("matrix set {set}");
        for row in 0..24u32 {
            let b = D + 0x20 + 0x180 * set.min(8) + 16 * row;
            let f: Vec<String> = (0..4).map(|k| format!("{:10.4}", f32::from_bits(r.mem.read_u32(b + 4 * k)))).collect();
            println!("  +{:#05x} {}", 16 * row, f.join(" "));
        }
        let (mut at, mut stack) = (OUT, Vec::new());
        for _ in 0..60 {
            let w: Vec<u32> = (0..4).map(|k| r.mem.read_u32(at + 4 * k)).collect();
            let (qwc, id, addr) = (w[0] & 0xffff, (w[0] >> 28) & 7, w[1]);
            println!("{at:#x}: id {id} qwc {qwc:#x} addr {addr:#x} vif {:08x} {:08x}", w[2], w[3]);
            let data = if matches!(id, 0 | 3 | 4) { addr } else { at + 16 };
            for q in 0..qwc.min(6) {
                let d: Vec<String> = (0..4).map(|k| format!("{:08x}", r.mem.read_u32(data + 16 * q + 4 * k))).collect();
                println!("      {}", d.join(" "));
            }
            at = match id {
                1 => at + 16 + 16 * qwc,
                2 => addr,
                3 | 4 => at + 16,
                5 => {
                    stack.push(at + 16 + 16 * qwc);
                    addr
                }
                6 => match stack.pop() {
                    Some(a) => a,
                    None => break,
                },
                _ => break,
            };
        }
    }
    let mut v = Vu1::default();
    let tags = v.run_chain(&mut r.mem, OUT).expect("the chain on VU1");
    println!("{tags} tags, {} program calls, {} kicks, {} GS quadwords", v.calls.len(), v.kicks.len(), v.gs.len());
    if std::env::var("DUMP").is_ok() {
        for &(at, from) in v.kicks.iter().take(3) {
            let to = v.kicks.iter().find(|k| k.1 > from).map_or(v.gs.len(), |k| k.1);
            println!("kick at {at:#x}: {} quadwords", to - from);
            for q in &v.gs[from..(from + 12).min(to)] {
                println!("   {:08x} {:08x} {:08x} {:08x}", q[0], q[1], q[2], q[3]);
            }
        }
    }
    // one program call per patch; every kick a PACKED tag of ST, RGBAQ, XYZF2 vertices; real screen places
    assert_eq!(v.calls.len(), n);
    assert!(v.kicks.len() >= n);
    for &(_, from) in &v.kicks {
        let tag = v.gs[from];
        assert_eq!((tag[1] >> 26 & 3, tag[2]), (0, 0x412), "a kick's tag {tag:x?}");
    }
    let xs: Vec<u32> = v.kicks.iter().map(|&(_, from)| v.gs[from + 3][0]).collect();
    assert!(xs.iter().any(|&x| x != xs[0]), "the vertices spread over the screen");
}
