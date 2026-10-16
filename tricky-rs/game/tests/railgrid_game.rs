//! The rail grid against the one the game's own loader resolves (board row G20c2c1b2; TRICKY_DISC for the courses'
//! `.ltg` and `.pbd`, TRICKY_ELF for the function runner; skipped without them): the files loaded into the runner's
//! memory and `cCourseResolve_Common::Resolve` 0x252350 run on the course object (0x346488, its vtable 0x3a7d50) as
//! `Course_LoadRenderData` runs it, every spline switched on (+0x18 bit 0) as on our side, then the real
//! `Rail_ClosestPoint` 0x259330 on the resolved world against `RailWorld::closest_point` on
//! `course_ground::rail_world`'s grid, bit for bit, at points on and around every course's rails.

use r5900::ps2float::Rules;
use r5900::Runner;

type V4 = [f32; 4];
const GWORLD: u32 = 0x346488;

fn files(name: &str) -> Option<(Vec<u8>, Vec<u8>, Vec<u8>)> {
    let disc = std::env::var("TRICKY_DISC").ok()?;
    let mut files = tricky_data::big::Files::new(tricky_data::disc::Disc::open(&disc).ok()?);
    files.mount(&format!("data/models/{name}.big")).ok()?;
    let mut get = |ext: &str| files.load(&format!("|data/models/{name}.{ext}")).ok().flatten();
    Some((get("pbd")?, get("ltg")?, get("ssf")?))
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn unit(&mut self) -> f32 {
        (self.next() % 2_000_001) as f32 / 1_000_000.0 - 1.0
    }
}

#[test]
fn the_grid_is_the_one_the_game_resolves() {
    for (name, k) in [("gari", 0usize), ("elysium", 1)] {
        // (a runner of its own each: the files fill its heap)
        let Some(mut r) = Runner::from_env() else { return };
        r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
        // the resolve and everything it calls (the headers, materials, models, meshes, instances, splines, cameras, the
        // hash table and the TGroups with their INodes; the float conversions of the debug print), and the closest
        // point's own call
        r.step_limit = 400_000_000;
        for a in [
            0x252350u32, 0x255630, 0x255310, 0x253fb0, 0x254af0, 0x254e58, 0x254e98, 0x254600, 0x254678, 0x253540, 0x2535b0, 0x253630,
            0x2f7aa8, 0x2f7018, 0x2f6728, 0x2f5ab8, 0x2537d8, 0x2539b0, 0x255c20, 0x253b60, 0x253bd0, 0x253c50, 0x253d30, 0x255cc0,
            0x253400, 0x2522e0, 0x254a28, 0x2534c8, 0x252318, 0x254160, 0x254b20, 0x254b50, 0x2541e8, 0x2542a8, 0x254318, 0x2546a8,
            0x254a80, 0x252470, 0x252748, 0x2527e0, 0x252b88, 0x2549f8, 0x254a50, 0x102f50, 0x254120, 0x254078,
        ] {
            r.allow(a);
        }
        let Some((pbd_b, ltg_b, ssf_b)) = files(name) else { return };
        let cg = tricky_game::course_ground::CourseGround::new(&pbd_b, &ltg_b, &ssf_b).expect("the course");
        let rails = cg.rails.lock().unwrap().clone();
        // the files in memory, the course object's vtable, the resolve
        let ltg = r.mem.alloc(ltg_b.len() as u32 + 0x100);
        let pbd = r.mem.alloc(pbd_b.len() as u32 + 0x100);
        let (ltg, pbd) = ((ltg + 0x7f) & !0x7f, (pbd + 0x7f) & !0x7f);
        r.mem.write(ltg, &ltg_b).unwrap();
        r.mem.write(pbd, &pbd_b).unwrap();
        for o in (0..0x40).step_by(4) {
            r.mem.write_u32(GWORLD + o, 0);
        }
        r.mem.write_u32(GWORLD + 0x10, 0x3a7d50);
        r.cpu.set_gpr(4, GWORLD as u64);
        r.cpu.set_gpr(5, ltg as u64);
        r.cpu.set_gpr(6, pbd as u64);
        r.cpu.set_gpr(4, GWORLD as u64);
        r.cpu.set_gpr(5, ltg as u64);
        r.cpu.set_gpr(6, pbd as u64);
        r.call(0x252350).unwrap_or_else(|e| panic!("{name}: the resolve: {e:?}"));
        assert_eq!(r.cpu.gpr(2) as u32, 0, "{name}: resolved");
        // every spline on (the resolved header's +0x5c: the splines, 0x28 bytes each)
        let parsed = tricky_data::pbd::Pbd::parse(&pbd_b).unwrap();
        let sp = r.mem.read_u32(pbd + 0x5c);
        let sp = if pbd <= sp { sp } else { pbd + sp };
        for i in 0..parsed.splines.len() as u32 {
            let at = sp + 0x28 * i + 0x18;
            let v = r.mem.read_u32(at);
            r.mem.write_u32(at, v | 1);
        }
        let mut ours = rails;
        for i in 0..parsed.splines.len() as u32 {
            ours.set_live(i, true);
        }
        // queries on and around the rails
        let (pos_at, out, unused) = (r.mem.alloc(0x10), r.mem.alloc(0x40), r.mem.alloc(0x10));
        let mut g = Rng(0x5eed_9a11 + k as u64);
        let (mut n, mut found) = (0, 0);
        for seg in parsed.segments.iter() {
            for _ in 0..3 {
                let t = (g.next() % 1000) as f32 / 1000.0;
                let f = |i: usize| f32::from_bits(seg[i]);
                let p: V4 = std::array::from_fn(|c| f(c) * t * t * t + f(4 + c) * t * t + f(8 + c) * t + f(12 + c));
                let spread = if g.next() % 3 == 0 { 300.0 } else { 40.0 };
                let q = [p[0] + g.unit() * spread, p[1] + g.unit() * spread, p[2] + g.unit() * spread, 1.0];
                for (c, x) in q.iter().enumerate() {
                    r.mem.write_f32(pos_at + 4 * c as u32, *x);
                }
                for c in 0..0x10 {
                    r.mem.write_u32(out + 4 * c, 0xdead_beef);
                }
                r.cpu.set_gpr(4, GWORLD as u64);
                r.cpu.set_gpr(5, pos_at as u64);
                r.cpu.set_gpr(6, unused as u64);
                r.cpu.set_gpr(7, out as u64);
                r.call(0x259330).unwrap_or_else(|e| panic!("{name}: query {q:?}: {e:?}"));
                let hit = r.cpu.gpr(2) as u32 != 0;
                let mut v = |at: u32| [0, 1, 2, 3].map(|c| r.mem.read_u32(at + 4 * c));
                let (gp, gt) = (v(out + 0x10), v(out + 0x20));
                let game = hit.then(|| (gp, gt, r.mem.read_u32(out + 0x34)));
                let mine = ours.closest_point(q).map(|h| (h.point.map(f32::to_bits), h.tangent.map(f32::to_bits), h.word));
                assert_eq!(game, mine, "{name}: query {q:?}");
                n += 1;
                found += mine.is_some() as u32;
            }
        }
        println!("{name}: {n} queries, {found} found a rail, all equal to the game's");
        assert!(n > 300 && found > n / 2);
    }
}
