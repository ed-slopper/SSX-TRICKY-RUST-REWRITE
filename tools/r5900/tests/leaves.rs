//! Run original leaf functions of SLUS_203.26 on hand-built inputs. Skips when TRICKY_ELF is not set.
//!
//! Each test also works the result out from the formula the decompiler shows, in f64, to show that the runner
//! computes the function rather than just reaching its end. The two agree to within the EE's rounding toward
//! zero (a few units in the last place). Comparing against tricky-rs's own functions is the next row.

use r5900::Runner;

/// A boarder with the rider and stats it points to, laid out as the original reads them
/// (`tricky-rs/docs/checking.md`, worked example).
struct Rider {
    stats: [(u32, u8); 4], // stats byte offsets and values, each /255 in the formulas
    natural_stance: u32,   // stats+0x40
    stance: u32,           // rider+0x1b4: riding switch when it differs from stats+0x40
    board_class: u32,      // rider+0x420: 0 BX, 1 freestyle, 2 alpine
    motion_state: u32,     // rider+0x290
    state_t: f32,          // rider+0x1bc
    state_len: f32,        // rider+0x298
    crouch: f32,           // rider+0x208
    boost_level: f32,      // rider+0x130
    boost: f32,            // rider+0x1fc
}

fn build(r: &mut Runner, d: &Rider) -> u32 {
    let stats = r.mem.alloc(0x50);
    for (off, v) in d.stats {
        r.mem.write_u8(stats + off, v);
    }
    r.mem.write_u32(stats + 0x40, d.natural_stance);
    let rider = r.mem.alloc(0x470);
    r.mem.write_u32(rider + 0x464, stats);
    r.mem.write_u32(rider + 0x1b4, d.stance);
    r.mem.write_u32(rider + 0x420, d.board_class);
    r.mem.write_u32(rider + 0x290, d.motion_state);
    r.mem.write_f32(rider + 0x1bc, d.state_t);
    r.mem.write_f32(rider + 0x298, d.state_len);
    r.mem.write_f32(rider + 0x208, d.crouch);
    r.mem.write_f32(rider + 0x130, d.boost_level);
    r.mem.write_f32(rider + 0x1fc, d.boost);
    let boarder = r.mem.alloc(0x10);
    r.mem.write_u32(boarder + 0x0c, rider);
    boarder
}

fn surface(r: &mut Runner, floats: &[(u32, f32)]) -> u32 {
    let row = r.mem.alloc(0x64);
    for &(off, v) in floats {
        r.mem.write_f32(row + off, v);
    }
    row
}

fn close(got: f32, want: f64) {
    let err = (got as f64 - want).abs();
    eprintln!("runner {got:>14.6} formula {want:>14.6} ({:+} in the last place)", (got.to_bits() as i64) - ((want as f32).to_bits() as i64));
    assert!(err <= want.abs() * 1e-5 + 1e-6, "runner {got} vs formula {want} (difference {err})");
}

fn rider(board_class: u32, switch: bool, motion_state: u32) -> Rider {
    Rider {
        stats: [(0x0e, 128), (0x11, 200), (0x13, 90), (0x19, 60)],
        natural_stance: 0,
        stance: switch as u32,
        board_class,
        motion_state,
        state_t: 0.1,
        state_len: 0.8,
        crouch: 0.25,
        boost_level: 0.6013,
        boost: 0.5,
    }
}

/// `Boarder_ForwardDrag` 0x109cb8, as the decompiler shows it (2026-10-10).
fn forward_drag_formula(v: f64, load: f64, d: &Rider, row: [f64; 3]) -> f64 {
    let b = |off: u32| d.stats.iter().find(|s| s.0 == off).unwrap().1 as f64;
    let l = load.max(1.0);
    let mut cub = b(0x19) * -0.29035342 * 0.003921569 + 1.2848105;
    let mut bst = b(0x11) * 0.9181778 * 0.003921569 + 0.6150995;
    let mut lin = if d.board_class == 2 {
        b(0x0e) * -0.30197912 * 0.003921569 + 0.70764244
    } else {
        b(0x0e) * -0.30845523 * 0.003921569 + 1.0649822
    };
    if d.stance != d.natural_stance && d.board_class != 1 {
        let k = if d.board_class == 0 { 0.84998363 } else { 0.69987833 };
        bst *= k;
        cub *= k;
        lin *= k;
    }
    let m = if (3..5).contains(&d.motion_state) { 0.5 - d.state_t as f64 / d.state_len as f64 } else { 1.0 };
    let bo = d.boost as f64;
    -v * m
        * (l * row[0] * lin
            + (1.0 - d.crouch as f64) * 0.10572864
            + (d.boost_level as f64 * 1.2153687 + 1.0) * 1.7513076 * bo * bo * bst
            + v.abs() * 0.001 * (l * row[1] + v.abs() * 0.001 * l * row[2] * cub))
}

#[test]
fn forward_drag_runs() {
    let Some(mut r) = Runner::from_env() else { return };
    assert_eq!(r.gp(), 0x3c38f0, "$gp from the entry code");
    let row = [1.3f32, 0.4, 0.2];
    for (class, switch, state, v, load) in [
        (1, false, 0, 1500.0f32, 1.0f32),
        (0, true, 0, -800.0, 2.5),
        (2, true, 3, 2600.0, 0.5),
        (1, true, 4, 30.0, 1.7),
    ] {
        let d = rider(class, switch, state);
        let boarder = build(&mut r, &d);
        let s = surface(&mut r, &[(4, row[0]), (8, row[1]), (0xc, row[2])]);
        r.cpu.set_f(12, v);
        r.cpu.set_f(13, load);
        r.cpu.set_gpr(4, boarder as u64);
        r.cpu.set_gpr(5, s as u64);
        r.call(0x109cb8).unwrap_or_else(|e| panic!("{e}"));
        let want = forward_drag_formula(v as f64, load as f64, &d, row.map(|x| x as f64));
        close(r.cpu.f(0), want);
    }
}

/// `Boarder_SideFriction` 0x109ef8, as the decompiler shows it (2026-10-10).
fn side_friction_formula(v_side: f64, v_fwd: f64, load: f64, d: &Rider, grip: f64) -> f64 {
    let s = v_fwd.abs();
    let curve = if s < 555.55554 {
        s * 0.0008899726 + 0.20103703
    } else if s < 1388.8888 {
        (s - 555.55554) * 0.00036129795 + 0.6954663
    } else {
        (s - 1388.8888) * -0.00010441317 + 0.9965479
    };
    let mut p = v_side * curve;
    let l = load.abs() * 1.0001484;
    if l >= 1.0 {
        p *= l;
    }
    let e = d.stats.iter().find(|x| x.0 == 0x13).unwrap().1 as f64;
    let mut k = e * 1.1412175 * 0.003921569 + 0.001;
    if d.stance != d.natural_stance && d.board_class != 1 {
        k *= if d.board_class == 0 { 0.84998363 } else { 0.69987833 };
    }
    let mut f = p * -grip * k;
    if d.boost_level > 0.0 {
        f *= 1.0 / (d.boost_level as f64 * 3.4999945 + 1.0);
    }
    f
}

#[test]
fn side_friction_runs() {
    let Some(mut r) = Runner::from_env() else { return };
    for (class, switch, v_side, v_fwd, load) in [
        (1, false, 120.0f32, 400.0f32, 1.0f32),
        (0, true, -300.0, 900.0, 1.4),
        (2, true, 50.0, -2000.0, 0.3),
    ] {
        let d = rider(class, switch, 0);
        let boarder = build(&mut r, &d);
        let grip = 1.7f32;
        let s = surface(&mut r, &[(0x10, grip)]);
        r.cpu.set_f(12, v_side);
        r.cpu.set_f(13, v_fwd);
        r.cpu.set_f(14, load);
        r.cpu.set_gpr(4, boarder as u64);
        r.cpu.set_gpr(5, s as u64);
        r.call(0x109ef8).unwrap_or_else(|e| panic!("{e}"));
        let want = side_friction_formula(v_side as f64, v_fwd as f64, load as f64, &d, grip as f64);
        close(r.cpu.f(0), want);
    }
}

#[test]
fn refuses_other_files() {
    assert!(Runner::from_elf(b"\x7fELF not the game").is_err());
}
