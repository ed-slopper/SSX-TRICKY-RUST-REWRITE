//! The patch draw run whole in the function runner (board rows G7a4a and G7a4b..; TRICKY_ELF and TRICKY_DISC for
//! gari's `.pbd`, skipped without them), as the oracle for the VU1 patch programs' ports. All of it runs as original
//! code:
//! - the Bezier manager made by its constructor, with the tables;
//! - some of gari's patches queued (vf16) at each tessellation and mixed, clipped or not;
//! - the queue drawn (vf15) through `PS2Render_Submit` into a draw list, under a camera made by the frame's
//!   matrices (0x1e85b0) and the renderer's projection (vf20);
//! - the flush (`PS2Draw_FlushBuckets`, with `PS2Draw_WriteTexState`).
//!
//! The DMA chain the flush builds is then walked into `r5900::vu1::Vu1`, which runs the game's patch programs and
//! logs what they send to the GS. The renderer's texture and blend virtuals, the allocator, the fill, the texture
//! upload and the screen's size are stand-ins. Several cameras (far above a patch, close over it, inside the
//! course) so that triangles cross the frustum and the programs' clipper runs.
//!
//! `patch_programs_match_the_game` then checks `tricky_game::patchvu` against every call of a ported program: from
//! the registers and data memory it was called with, the same data memory, registers and GS packets after.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::vu1::{Run, Vu1};
use r5900::Runner;
use tricky_game::patchvu;
use tricky_game::vu::Vu;

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
    Some(tricky_data::pbd::Pbd::parse(&pbd)?.patches)
}

fn w32(r: &mut Runner, at: u32, v: u32) {
    r.mem.write_u32(at, v);
}
fn wf(r: &mut Runner, at: u32, v: f32) {
    r.mem.write_u32(at, v.to_bits());
}

/// A camera: its place relative to the first patch's box centre (x, y, height) and whether it looks down
/// (else along +y)
struct Camera {
    at: [f32; 3],
    down: bool,
    /// Views (vf17) added, so that the view passes run
    views: bool,
    /// The far plane
    far: f32,
}

/// One whole draw: `n` patches from `first`, the steps by `k % 4` (4, 6, 8, mixed), clipped by `k / 4 % 2`; the
/// VU1 that ran the chain (its calls recorded)
fn draw(patches: &[Vec<u8>], first: usize, n: usize, cam: &Camera) -> Option<Vu1> {
    let mut r = Runner::from_env()?;
    r.mem.write(ARENA, &vec![0u8; SIZE as usize]).unwrap();
    for f in [0x1d9be0u32, 0x1d9c88, 0x1db150, 0x1d9d20, 0x1db1e8, 0x1db530, 0x1d9d50, 0x1d9f58, 0x1e8838, 0x1e88c8, 0x1c4d68, 0x1c4d30, 0x1c4f28, 0x1c4340, 0x1c4468, 0x1c6f98, 0x1c7068, 0x1c7088, 0x1c4fe0, 0x1c4220, 0x1c51f0, 0x1c5368, 0x1c42a0, 0x1c5c88, 0x1ea068, 0x1c0340, 0x1c03e8, 0x1c04f8, 0x1c0560, 0x1c05d8, 0x1c06a0, 0x1e85b0, 0x2cfc00, 0x1c4650, 0x1c4c38, 0x1da5c8] {
        r.allow(f);
    }
    // the projection's calls: vf20, Math_SinCos, tanf and its parts
    for f in [0x1dc3e0u32, 0x250c10, 0x2f2880, 0x2f3108, 0x2f3e38] {
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
    for off in [0x1c8u32, 0x1f0, 0x1f8, 0x200] {
        r.stub(vaddr(off), |_, _| {});
    }
    for (off, v) in [(0x60u32, 640u64), (0x58, 448)] {
        r.stub(vaddr(off), move |c, _| c.set_gpr(2, v));
    }
    // the app and the renderer: its vtable (the Bezier manager's getter real), texture state, draw list, packets
    w32(&mut r, 0x337c58, APP);
    w32(&mut r, APP + 0x724, R);
    w32(&mut r, R + 0x10, RVT);
    for off in [0x1c8u32, 0x1f0, 0x1f8, 0x200, 0x60, 0x58] {
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
    // the lists' VU1 program tables, then the Bezier manager: constructed (its records, defaults, tables)
    r.call(0x1db640).expect("StaticInit_Render");
    r.cpu.set_gpr(4, BZ as u64);
    r.call(0x1d9be0).expect("the Bezier manager's constructor");
    w32(&mut r, BZ + 0x20, cam.views as u32);
    if cam.views {
        // two views: a quad, the box test's matrix, the matrix the view pass puts the points through (a scaled,
        // turned one), a texture word, a float; the box test (the renderer's +0x288) keeping every view
        r.stub(vaddr(0x288), |c, _| c.set_gpr(2, 0));
        w32(&mut r, RVT + 0x288 + 4, vaddr(0x288));
        let view = ARENA + 0x3c00;
        for (k, (scale, turn)) in [(0.001f32, 0.0f32), (0.0007, 0.6)].into_iter().enumerate() {
            let (c, s) = (turn.cos(), turn.sin());
            let m = [[scale * c, scale * s, 0.0, 0.0], [-scale * s, scale * c, 0.0, 0.0], [0.0, 0.0, scale, 0.0], [0.5, 0.5, 0.0, 1.0]];
            for k in 0..16 {
                wf(&mut r, view + 0x10 + 4 * k, if k % 5 == 0 { 1.0 } else { 0.0 });
            }
            for (i, row) in m.iter().enumerate() {
                for (j, x) in row.iter().enumerate() {
                    wf(&mut r, view + 0x50 + 16 * i as u32 + 4 * j as u32, *x);
                }
            }
            for (i, v) in [BZ, view, view + 0x10, view + 0x50, 7 + k as u32].into_iter().enumerate() {
                r.cpu.set_gpr(4 + i, v as u64);
            }
            r.cpu.set_f(12, 0.5);
            r.call(0x1d9eb8).expect("a view");
        }
    }
    // the patches, queued at 4, 6, 8 steps and mixed
    for (k, p) in patches.iter().skip(first).take(n).enumerate() {
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
    // the camera (row vectors, translation last), the frame's other matrices plain
    let b: Vec<f32> = (0..6).map(|k| f32::from_bits(u32::from_le_bytes(patches[first][0x150 + 4 * k..0x154 + 4 * k].try_into().unwrap()))).collect();
    let c = [(b[0] + b[3]) / 2.0 + cam.at[0], (b[1] + b[4]) / 2.0 + cam.at[1], (b[2] + b[5]) / 2.0 + cam.at[2]];
    for base in [0x1730u32, 0x4d0, 0x570, 0x5b0] {
        for k in 0..16 {
            wf(&mut r, R + base + 4 * k, if k % 5 == 0 { 1.0 } else { 0.0 });
        }
    }
    let view = if cam.down {
        // view z = −world z
        [[1.0, 0.0, 0.0, 0.0], [0.0, -1.0, 0.0, 0.0], [0.0, 0.0, -1.0, 0.0], [-c[0], c[1], c[2], 1.0]]
    } else {
        // view z = world y, view y = −world z
        [[1.0, 0.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, -1.0, 0.0, 0.0], [-c[0], c[2], -c[1], 1.0]]
    };
    for (i, row) in view.iter().enumerate() {
        for (j, v) in row.iter().enumerate() {
            wf(&mut r, CAM + 16 * i as u32 + 4 * j as u32, *v);
        }
    }
    w32(&mut r, R + 0x17d0, 0);
    // the projection: the renderer's vf20 (field of view, near, far), on a 640 × 448 screen
    r.cpu.set_gpr(4, R as u64);
    r.cpu.set_f(12, 1.0);
    r.cpu.set_f(13, 10.0);
    r.cpu.set_f(14, cam.far);
    r.call(0x1dc3e0).expect("the projection");
    // (its x and y scales come out 0 with these arguments: a perspective of 1, 1 put in their place, keeping its
    // depth terms and the screen's scale and offset at +0x550 / +0x560)
    for base in [0x4d0u32, 0x570] {
        wf(&mut r, R + base, 1.0);
        wf(&mut r, R + base + 0x14, 1.0);
    }
    // the queue drawn into the draw list, then the list flushed into one DMA chain, then that on VU1
    r.cpu.set_gpr(4, BZ as u64);
    r.cpu.set_gpr(5, R as u64);
    r.call(0x1d9f58).expect("the queue drawn");
    w32(&mut r, SLOT, OUT);
    r.cpu.set_gpr(4, D as u64);
    r.cpu.set_gpr(5, SLOT as u64);
    r.call(0x1c4fe0).expect("the flush");
    let mut v = Vu1 { record: true, trace: std::env::var("TRACE").is_ok(), ..Vu1::default() };
    v.run_chain(&mut r.mem, OUT).expect("the chain on VU1");
    Some(v)
}

fn cameras() -> Vec<Camera> {
    let mut c = Vec::new();
    for (k, z) in [2000.0, 300.0, 40.0].into_iter().enumerate() {
        c.push(Camera { at: [0.0, 0.0, z], down: true, views: k % 2 == 1, far: 100000.0 });
    }
    for (k, (x, y, z)) in [(0.0, -600.0, 80.0), (200.0, -200.0, 20.0), (-150.0, 100.0, 60.0)].into_iter().enumerate() {
        c.push(Camera { at: [x, y, z], down: false, views: k % 2 == 0, far: 100000.0 });
    }
    // inside a patch looking along it (its near plane through the patches), and two near far planes
    c.push(Camera { at: [0.0, 0.0, 10.0], down: false, views: false, far: 100000.0 });
    c.push(Camera { at: [0.0, 0.0, 2000.0], down: true, views: true, far: 2050.0 });
    c.push(Camera { at: [0.0, -300.0, 60.0], down: false, views: false, far: 600.0 });
    c
}

#[test]
fn the_patch_draw_runs_on_vu1() {
    let Some(patches) = gari_patches() else { return };
    let Some(v) = draw(&patches, 0, 24, &cameras()[0]) else { return };
    println!("{} program calls, {} kicks, {} GS quadwords", v.calls.len(), v.kicks.len(), v.gs.len());
    // one program call per patch; every kick a PACKED tag of ST, RGBAQ, XYZF2 vertices; real screen places
    assert_eq!(v.calls.len(), 24);
    assert!(v.kicks.len() >= 24);
    for &(_, from) in &v.kicks {
        let tag = v.gs[from];
        assert_eq!((tag[1] >> 26 & 3, tag[2]), (0, 0x412), "a kick's tag {tag:x?}");
    }
    let xs: Vec<u32> = v.kicks.iter().map(|&(_, from)| v.gs[from + 3][0]).collect();
    assert!(xs.iter().any(|&x| x != xs[0]), "the vertices spread over the screen");
}

/// The micro memory a chain loads, to its last instruction (a longer program's tail may follow it in the VU)
fn program(chain: u32) -> Option<Vec<u8>> {
    let mut r = Runner::from_env()?;
    let mut v = Vu1::default();
    v.run_chain(&mut r.mem, chain).ok()?;
    let end = v.micro.chunks(8).rposition(|p| p.iter().any(|b| *b != 0))? + 1;
    v.micro.truncate(8 * end);
    Some(v.micro)
}

/// A recorded call against the port: the same data memory, registers and GS packets
fn check(run: &Run, gs: &[[u32; 4]], port: fn(&mut Vu, u32), what: &str) -> usize {
    let mut p = Vu::new();
    p.vf = run.regs_in.vf;
    p.vi = run.regs_in.vi;
    p.acc = run.regs_in.acc;
    p.q = run.regs_in.q;
    p.clip = run.regs_in.clip;
    p.top = run.top;
    for (k, q) in p.mem.iter_mut().enumerate() {
        *q = [0, 1, 2, 3].map(|j| u32::from_le_bytes(run.data_in[16 * k + 4 * j..16 * k + 4 * j + 4].try_into().unwrap()));
    }
    port(&mut p, run.start);
    let theirs = &gs[run.gs_from..run.gs_to];
    if let Some(i) = theirs.iter().zip(&p.gs).position(|(a, b)| a != b) {
        panic!("{what} from {:#x}: GS quadword {i}: theirs {:x?} ours {:x?}", run.start, theirs[i], p.gs[i]);
    }
    assert_eq!(theirs.len(), p.gs.len(), "{what} from {:#x}: GS quadwords", run.start);
    for k in 0..0x400 {
        let t = [0, 1, 2, 3].map(|j| u32::from_le_bytes(run.data_out[16 * k + 4 * j..16 * k + 4 * j + 4].try_into().unwrap()));
        assert_eq!(t, p.mem[k], "{what} from {:#x}: data memory at quadword {k}", run.start);
    }
    for k in 0..32 {
        assert_eq!(run.regs_out.vf[k].map(f32::to_bits), p.vf[k].map(f32::to_bits), "{what} from {:#x}: vf{k}", run.start);
    }
    assert_eq!(run.regs_out.vi, p.vi, "{what} from {:#x}: vi (clip flags theirs {:#x} ours {:#x}, in {:#x})", run.start, run.regs_out.clip, p.clip, run.regs_in.clip);
    assert_eq!(run.regs_out.acc.map(f32::to_bits), p.acc.map(f32::to_bits), "{what} from {:#x}: ACC", run.start);
    assert_eq!(run.regs_out.q.to_bits(), p.q.to_bits(), "{what} from {:#x}: Q", run.start);
    assert_eq!(run.regs_out.clip, p.clip, "{what} from {:#x}: the clip flags", run.start);
    theirs.len()
}

#[test]
fn patch_programs_match_the_game() {
    let Some(patches) = gari_patches() else { return };
    let ports: [(u32, fn(&mut Vu, u32), &str); 3] = [(0x30f960, patchvu::run4, "the 4-step program"), (0x3110f0, patchvu::run6, "the 6-step program"), (0x312af0, patchvu::run8, "the 8-step program")];
    let mut programs = Vec::new();
    for (chain, port, what) in ports {
        let Some(image) = program(chain) else { return };
        programs.push((image, port, what));
    }
    let mut stats: Vec<(usize, usize, usize, std::collections::BTreeMap<u32, usize>)> = vec![Default::default(); 3];
    for cam in cameras() {
        for first in (0..patches.len().saturating_sub(48)).step_by(97).take(6) {
            let Some(v) = draw(&patches, first, 48, &cam) else { return };
            for run in &v.runs {
                for (k, (image, port, what)) in programs.iter().enumerate() {
                    if run.micro[..image.len()] == image[..] {
                        let st = &mut stats[k];
                        st.1 += check(run, &v.gs, *port, what);
                        st.0 += 1;
                        st.2 += (run.regs_out.vi[14] != run.regs_in.vi[14]) as usize;
                        *st.3.entry(run.start).or_insert(0) += 1;
                    }
                }
            }
        }
    }
    for (k, (calls, quadwords, clipped, starts)) in stats.iter().enumerate() {
        println!("{}: {calls} calls, {quadwords} GS quadwords, {clipped} with a triangle clipped; by entry {starts:x?}", programs[k].2);
        assert!(*calls > 100 && *clipped > 10, "{}", programs[k].2);
        for start in [0, 0x28, 0x80, 0xe0] {
            assert!(starts.get(&start).copied().unwrap_or(0) > 10, "{}: the entry at {start:#x} ran", programs[k].2);
        }
    }
}
