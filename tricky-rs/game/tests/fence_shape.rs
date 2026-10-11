//! A fence's mesh bending (board row E9g3) against the game in the function runner (TRICKY_ELF; skipped without
//! it): `FenceNode_InitShape` 0x143ff8 on random quads from a stubbed renderer, then `FenceNode_UpdateShape`
//! 0x143cd0 for many angles, comparing the node's fields and what reaches the renderer's mesh (+0x34) bit for bit.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::breakables::FenceShape;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn pick<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.next() as usize % v.len()]
    }
}

type Drawn = (Vec<u32>, Vec<u32>, Vec<u32>);

#[test]
fn fence_shape_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    r.allow(0x143cd0);
    let app = r.mem.alloc(0x800);
    let rend = r.mem.alloc(0x40);
    let rvt = r.mem.alloc(0x400);
    let mesh = r.mem.alloc(0x40);
    let mvt = r.mem.alloc(0x40);
    let make = r.mem.alloc(0x10);
    let quad = r.mem.alloc(0x10);
    let draw = r.mem.alloc(0x10);
    r.mem.write_u32(0x337c58, app);
    r.mem.write_u32(app + 0x724, rend);
    r.mem.write_u32(rend + 0x10, rvt);
    r.mem.write_u32(rvt + 0x228, 0);
    r.mem.write_u32(rvt + 0x22c, make);
    r.mem.write_u32(rvt + 0x2b0, 0);
    r.mem.write_u32(rvt + 0x2b4, quad);
    r.mem.write_u32(mesh, mvt);
    r.mem.write_u32(mvt + 0x30, 0);
    r.mem.write_u32(mvt + 0x34, draw);
    // the renderer: a mesh (or none), the model's quad (four corners, normal, four UVs), the drawn arrays
    let handle = Rc::new(RefCell::new(0u32));
    let h = handle.clone();
    r.stub(make, move |c, _| {
        assert_eq!((c.gpr(5), c.gpr(6), c.gpr(7) as u32), (8, 2, 0x6000_0000));
        c.set_gpr(2, *h.borrow() as u64);
    });
    let quad_in: Rc<RefCell<([[f32; 3]; 4], [f32; 3], [[f32; 2]; 4])>> = Rc::new(RefCell::new(Default::default()));
    let q = quad_in.clone();
    r.stub(quad, move |c, m| {
        let (corners, normal, uvs) = *q.borrow();
        let (pa, na, ua) = (c.gpr(6) as u32, c.gpr(7) as u32, c.gpr(8) as u32);
        for (i, p) in corners.iter().enumerate() {
            for k in 0..3 {
                m.write_f32(pa + 12 * i as u32 + 4 * k as u32, p[k]);
            }
        }
        for k in 0..3 {
            m.write_f32(na + 4 * k as u32, normal[k]);
        }
        for (i, p) in uvs.iter().enumerate() {
            for k in 0..2 {
                m.write_f32(ua + 8 * i as u32 + 4 * k as u32, p[k]);
            }
        }
    });
    let drawn: Rc<RefCell<Vec<Drawn>>> = Rc::new(RefCell::new(vec![]));
    let d = drawn.clone();
    r.stub(draw, move |c, m| {
        assert_eq!(c.gpr(4) as u32, mesh);
        let read = |m: &mut r5900::Mem, a: u32, n: u32| (0..n).map(|k| m.read_u32(a + 4 * k)).collect::<Vec<u32>>();
        let v = (read(m, c.gpr(5) as u32, 48), read(m, c.gpr(6) as u32, 48), read(m, c.gpr(7) as u32, 32));
        d.borrow_mut().push(v);
    });

    let node = r.mem.alloc(0x200);
    let object = r.mem.alloc(0x100);
    let model = r.mem.alloc(0x40);
    let modelx = r.mem.alloc(0x10);
    let args = r.mem.alloc(0x20);
    let mut g = Rng(0xfe9c_e000_0000_0007);
    let mut bends = 0;
    for case in 0..3000 {
        let f = |g: &mut Rng| g.pick(&[0.0f32, 1.0, -1.0, 37.5, -120.25, 300.0, 0.004, 1e-3, 2500.0]);
        let corners = [[f(&mut g), f(&mut g), f(&mut g)], [f(&mut g), f(&mut g), f(&mut g)], [f(&mut g), f(&mut g), f(&mut g)], [f(&mut g), f(&mut g), f(&mut g)]];
        let normal = [f(&mut g), f(&mut g), f(&mut g)];
        let uv = |g: &mut Rng| g.pick(&[0.0f32, 1.0, 0.25, 0.875, -0.5]);
        let uvs = [[uv(&mut g), uv(&mut g)], [uv(&mut g), uv(&mut g)], [uv(&mut g), uv(&mut g)], [uv(&mut g), uv(&mut g)]];
        *quad_in.borrow_mut() = (corners, normal, uvs);
        let has_mesh = g.next() % 8 != 0;
        *handle.borrow_mut() = if has_mesh { mesh } else { 0 };
        let scale = g.pick(&[0.0f32, 0.5, 1.0, 2.0, 3.75]);
        let mode = g.pick(&[0u32, 1, 2]);
        let bend = g.pick(&[0.0f32, 1.0, 45.0, -30.0, 500.0]);
        for o in (0..0x200).step_by(4) {
            r.mem.write_u32(node + o, 0);
        }
        r.mem.write_u32(node + 0x28, object);
        r.mem.write_u32(node + 0x50, args);
        r.mem.write_u32(args + 0xc, mode);
        r.mem.write_f32(args + 0x10, bend);
        r.mem.write_u32(object + 0xc0, model);
        r.mem.write_u32(object + 0xe8, g.next());
        r.mem.write_u32(model + 8, modelx);
        r.mem.write_f32(model + 0x18, scale);
        drawn.borrow_mut().clear();
        r.cpu.set_gpr(4, node as u64);
        r.call(0x143ff8).unwrap_or_else(|e| panic!("case {case}: {e:?}"));
        let sh = FenceShape::new(corners, normal, uvs, scale, mode, bend);
        let v3 = |m: &mut r5900::Mem, a: u32| [0, 1, 2].map(|k| m.read_u32(a + 4 * k));
        let b3 = |p: [f32; 3]| p.map(f32::to_bits);
        let m = &mut r.mem;
        assert_eq!(
            (m.read_u32(node + 0x48), m.read_u32(node + 0x4c), m.read_u32(node + 0x1a4)),
            (sh.mode, sh.bend.to_bits(), sh.scale.to_bits()),
            "case {case}"
        );
        for i in 0..8u32 {
            let k = i as usize;
            assert_eq!((v3(m, node + 0x58 + 12 * i), v3(m, node + 0xb8 + 12 * i)), (b3(sh.a[k]), b3(sh.b[k])), "case {case}: point {i}");
            let uv = |m: &mut r5900::Mem, a: u32| [m.read_u32(a), m.read_u32(a + 4)];
            assert_eq!((uv(m, node + 0x118 + 8 * i), uv(m, node + 0x158 + 8 * i)), (sh.uv_a[k].map(f32::to_bits), sh.uv_b[k].map(f32::to_bits)), "case {case}: uv {i}");
        }
        assert_eq!(v3(m, node + 0x198), b3(sh.normal), "case {case}: normal");
        // the angles the swing goes through
        let mut angles = vec![0.0f32];
        for _ in 0..6 {
            angles.push(g.pick(&[0.5f32, -0.5, 1.0, -0.73, 0.01, 0.2]));
        }
        drawn.borrow_mut().clear();
        for &a in &angles[1..] {
            r.mem.write_f32(node + 0x34, a);
            r.cpu.set_gpr(4, node as u64);
            r.call(0x143cd0).unwrap();
        }
        let got = drawn.borrow().clone();
        if !has_mesh {
            assert!(got.is_empty(), "case {case}: drawn without a mesh");
            continue;
        }
        assert_eq!(got.len(), angles.len() - 1);
        let uv_bits: Vec<u32> = sh.uv_a.iter().chain(&sh.uv_b).flat_map(|p| p.map(f32::to_bits)).collect();
        for (&a, (verts, normals, uvs)) in angles[1..].iter().zip(&got) {
            let mesh = sh.bent(a);
            let vb: Vec<u32> = mesh.verts.iter().flat_map(|p| p.map(f32::to_bits)).collect();
            let nb: Vec<u32> = mesh.normals.iter().flat_map(|p| p.map(f32::to_bits)).collect();
            assert_eq!((verts, normals, uvs), (&vb, &nb, &uv_bits), "case {case}: angle {a}, mode {mode}");
            bends += 1;
        }
    }
    eprintln!("{bends} bent meshes compared");
}
