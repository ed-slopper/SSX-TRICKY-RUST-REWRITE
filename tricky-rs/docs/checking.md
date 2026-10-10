# Checking our code against the original

How a function in `tricky-rs` gets from **ported** to **checked** in the function index (board row F4). Everything
here runs on the player's own copy of `SLUS_203.26` (SSX Tricky, PS2 NTSC-U, md5
`162580fd65611e72a90deed640a70aef`); nothing of the game is ever committed, and the checks skip, not fail, when the
file is not there (so CI without the game stays green).

Two tools, built by rows F4a and F4b:

| | F4a: function runner | F4b: race traces |
|---|---|---|
| Checks | one original function against our Rust function, on many inputs | our whole per-tick update against the real game, tick by tick |
| Runs | the original's machine code in a small R5900 interpreter in a Rust test | the real game in PCSX2, read over its PINE interface |
| Good for | leaf maths: drag, friction, jump, spin, scoring, rubber band | the order things happen in, state changes, anything the runner can't reach |
| Says a function is | `checked` when it matches on the input set | that a system's tick matches over a recorded race |

## F4a: the function runner

**Built** (2026-10-10): `tools/r5900`, no dependencies, builds with the GNU and MSVC toolchains.

```
cd tools/r5900
cargo test                                         # the instruction tests; the ELF tests skip
TRICKY_ELF=<path to your SLUS_203.26> cargo test   # also runs Boarder_ForwardDrag and Boarder_SideFriction
```

On hand-built inputs both match the formula the decompiler shows to within 2 to 7 units in the last place,
always toward zero: the EE's rounding at work, and the size of the difference F4c is about. With zeroed inputs,
`Boarder_GroundSpringForce`, `Boarder_GroundThrust` and `Air_IntegrateRK4` also run to the end; `Jump_ApplyImpulse`,
`Takeoff_SetSpinRates` and `AI_RubberBandSpeedScale` stop at their first call (0x250e98, 0x12bee0), which needs a
stub. Since F4e (2026-10-10) `tests/targets.rs` also runs `AI_RubberBandSpeedScale` 0x13a0f0, `Takeoff_SetSpinRates`
0x126e30, `Jump_ApplyImpulse` 0x1284e0 (snow and rail) and `Air_IntegrateRK4` 0x12b340 to the end, and each
matches the formula in `original-rules.md`: the rubber band through both clamps, K = 11.517(0.5449 + 0.6742T)
for the spin and flip rates, Δv = 630.88 along normalize(N + 0.2F) on snow and along the board's up on a rail,
gravity −850.24 and drag −0.20002v for one 1/60 s step. Their pure callees run as the original (`Math_Sin`
0x250d60, `Math_Cos` 0x250e98, `Vec4_Scale` 0x102f50, the table getter 0x15fdd0); callees with side effects are
stubs that record the call (scoring 0x156618, `Boarder_AddMeter` 0x11b018, the motion and clip setters).
Not implemented yet: MMI, the FPU accumulator forms (`adda.s`, `madd.s` …), the VU0 macro ops no target has
needed, and `cfc2` of the flag registers; each stops the run with its name and address.

A Rust crate, `tools/r5900` (a library with no Bevy), used from `cargo test` in `tricky-rs` as a dev-dependency.
It loads the ELF, sets up registers and memory, calls one original function and returns what it left in the
registers and memory, so a test can call our Rust function on the same inputs and compare.

### Loading

- Path from the environment: `TRICKY_ELF=<path to SLUS_203.26>`. Check the md5; skip the test with a message if
  the variable is unset or the md5 differs.
- 32 MB of EE RAM as a flat byte array. Copy each `PT_LOAD` segment to its virtual address (there is one:
  file offset 0x1000 → 0x00100000, so file offset = address − 0xFF000) and zero the rest (`.bss` ends at 0x40C174).
- 16 KB scratchpad at 0x70000000. Kernel space, hardware registers and the VU memories are not mapped: an access
  there stops the run with an error naming the instruction's address.
- `$gp`: the start-up code at `entry` 0x100008 loads it with a `lui`/`addiu` pair; read the value from those two
  instructions rather than writing it down. Code reads globals and constants relative to it.
- `$sp`: top of RAM minus a small margin. `$ra`: a sentinel address (0xFFFFFFF0); the run ends when `pc` reaches it.

### Calling a function

The EE's EABI as the game was compiled: integer and pointer arguments in `$a0`–`$a7`, float arguments in `$f12`–
`$f19`, counted separately (so `Boarder_ForwardDrag(float vF, float load, Boarder *b, SurfaceRow *s)` has `vF` in
`$f12`, `load` in `$f13`, `b` in `$a0`, `s` in `$a1`); results in `$v0`/`$v1` or `$f0`. General registers are
128 bits wide (`lq`, `sq` and the MMI instructions use all of them).

Calls out of the function (`jal`, `jalr`) go through a stub table keyed by target address: a stub is a Rust closure
that sees the registers and memory, records the call and sets the result. A call with no stub either runs the
original callee, when the test allows that address, or stops the run with "unstubbed call to 0x… (name)". Names
come from `tricky-rs/docs/function-index.csv`.

Inputs live in a test heap the runner hands out (`mem.alloc(size)`), filled with `write_u8/u32/f32(ptr + offset)`
using the offsets the port notes give for each struct. Pointer chains (boarder → rider → stats) are just allocations
whose addresses are written into each other.

### Floats are not IEEE

The EE's FPU and VU0 do not follow IEEE 754. There are no infinities, NaNs or denormals: results that overflow clamp
to ±0x7F7FFFFF, denormal results and inputs become zero, and most operations round toward zero. The runner must do
the same, or it is checking the wrong thing. Write it from the behaviour as documented (the EE core and VU user's
manuals, and how PCSX2 describes its "chop/zero" rounding and clamping modes); do not copy PCSX2's source, which is
GPL, unless the humans decide the licence allows it.

That also means our Rust code, in IEEE `f32`, will differ in the last bit on some inputs. Whether `tricky-rs`
computes in a PS2-float type (bit-exact, slower to write) or in `f32` with a stated tolerance is a decision for the
humans: board row F4c. Until it is made, the runner reports both: exact bit match, and the largest difference in
ULPs over the input set.

### What the runner must implement

Counted with `tools/insn_census.py` (it reads your own ELF and prints only mnemonics and counts) over the first
targets, each function on its own (`--depth 0`): `Boarder_GroundSpringForce` 0x109878, `Boarder_GroundThrust`
0x109950, `Boarder_ForwardDrag` 0x109cb8, `Boarder_SideFriction` 0x109ef8, `Takeoff_SetSpinRates` 0x126e30,
`Jump_ApplyImpulse` 0x1284e0, `Air_IntegrateRK4` 0x12b340, `AI_RubberBandSpeedScale` 0x13a0f0, `Score_Crash`
0x156de0, `Score_BuildTrickId` 0x157490. 1,857 instructions of 75 kinds:

| Group | Instructions |
|---|---|
| Integer | `addiu` `addu` `subu` `daddu` `and` `or` `xor` `andi` `ori` `lui` `sll` `slti` `sltiu` `sltu` `mult` `div` `mflo` `break` (divide-by-zero trap after `div`) |
| Branches and jumps (with delay slots; the `l` forms skip the slot when not taken) | `beq` `bne` `beql` `bnel` `blez` `bgtz` `bgtzl` `bltz` `bgez` `jal` `jr`, and `nop` (counted apart from `sll`) |
| Loads and stores | `lw` `sw` `lbu` `sb` `ld` `sd` `lq` `sq` (128-bit, address rounded down to 16) |
| FPU | `lwc1` `swc1` `mtc1` `mfc1` `add.s` `sub.s` `mul.s` `div.s` `abs.s` `neg.s` `mov.s` `cvt.s.w` `cvt.w.s` `c.lt.s` `c.le.s` `bc1f` `bc1t` `bc1fl` `bc1tl` |
| COP2 transfers | `lqc2` `sqc2` `qmtc2` `qmfc2` `cfc2` |
| VU0 macro mode | `vadd` `vaddw` `vsub` `vmul` `vmulx` `vmulq` `vadday` `vmaddaz` `vmaddw` `vdiv` `vsqrt` `vrsqrt` `vwaitq` |

None of these ten makes an indirect call. Followed through every call, they reach 265 functions and 130 kinds of
instruction (the scoring code calls into sound, and that into `sprintf`), which is why the runner stubs calls rather
than running the game: start with the four leaves that call nothing (`Boarder_GroundSpringForce`,
`Boarder_ForwardDrag`, `Boarder_SideFriction`, `Air_IntegrateRK4`) and add instructions as targets need them, with
a unit test per instruction.

### Worked example: `Boarder_ForwardDrag` 0x109cb8

What the original reads (from the decompiler, 2026-10-10):

| Input | Where | Meaning (port notes) |
|---|---|---|
| `vF` | `$f12` | forward speed, cm/s |
| `load` | `$f13` | fN/g; the function uses max(1, load) |
| boarder | `$a0` | `boarder+0xc` → rider |
| surface row | `$a1` | the surface table row: floats +4 (linear), +8 (quadratic), +0xc (cubic drag) |
| rider+0x464 | pointer | the rider's stats; bytes +0x0e (speed S), +0x11 (E), +0x19 (S2), each /255 |
| rider+0x420 | int | board class: 0 BX, 1 freestyle, 2 alpine |
| rider+0x1b4 vs stats+0x40 | ints | riding switch when they differ: drag ×0.85 (BX) or ×0.70 (alpine), not for freestyle |
| rider+0x290 | int | motion state; in states 3 and 4 everything is scaled by 0.5 − rider+0x1bc / rider+0x298 |
| rider+0x208 | float | crouch |
| rider+0x130 | float | boost level |
| rider+0x1fc | float | boost amount B (squared) |
| result | `$f0` | drag acceleration on vF |

The test, once F4a exists (sketch):

```rust
#[test]
fn forward_drag_matches_original() {
    let Some(mut cpu) = r5900::Runner::from_env() else { return };   // skips without TRICKY_ELF
    for case in drag_cases() {                                       // random + edge cases, ranges from F4b traces
        let stats = cpu.mem.alloc(0x50);
        cpu.mem.write_u8(stats + 0x0e, case.speed_stat);
        // ... the other stats bytes, rider fields and surface row as in the table above
        let rider = cpu.mem.alloc(0x470);
        cpu.mem.write_u32(rider + 0x464, stats);
        let boarder = cpu.mem.alloc(0x10);
        cpu.mem.write_u32(boarder + 0x0c, rider);
        let row = cpu.mem.alloc(0x64);
        cpu.set_f(12, case.v_forward);
        cpu.set_f(13, case.load);
        cpu.set_gpr(4, boarder);
        cpu.set_gpr(5, row);
        cpu.call(0x109cb8).expect("runs to the end");
        let original = cpu.f(0);
        let ours = rider::forward_drag(&case.to_rider(), &case.to_surface(), case.v_forward, case.load);
        assert_close(original, ours, case);                         // exact or within ULPs, per F4c
    }
}
```

It would fail today, which is the point of building it. Our `drag_linear` and `drag_cubic` (`rider.rs`) use the
constants rounded to four places (0.7076 for 0.70764244), and the state 3/4 factor above is not in
`original-rules.md`. Both are gaps for the port, not for this row.

## F4b: race traces from PCSX2

The runner checks functions one at a time; the trace checks that our tick does the same things in the same order.

- **Stop the game on every tick.** PCSX2-MCP (row F3b) adds a debug server to PCSX2 (TCP port 21512) with
  breakpoints, register and memory reads. A trace script sets a breakpoint on the boarder's per-tick update, and
  at each hit reads what it needs and continues, so every tick is recorded and nothing moves while it reads. A
  script talks to the debug server directly; calling an MCP tool per tick would take far too long for a race.
  Without F3b, PCSX2's own PINE interface (Settings > Advanced; slot 28011) can read memory but is not tied to
  the game's frames: then each sample also reads the game's tick counter and only pairs of consecutive ticks are
  kept.
- **Find the addresses first.** The per-tick update to break on, the boarder array, the pad state and the tick
  counter, with PCSX2-MCP's memory search and diff (`pcsx2_find_pattern`, `pcsx2_memory_diff`) during a race.
  Write them into the port notes.
- **What to record per tick, per rider:** the boarder struct (position, velocity, orientation, motion state and
  state timer, meter, trick state), the pad state the game read that tick, and the random seed state.
- **Replay.** Load the recorded state at tick t into our structs, feed the recorded pad, step our code one tick and
  compare with tick t+1, field by field, within the tolerance F4c sets. Report the first field and tick that
  differ.
- **Files.** Traces are made from the game, so they stay on the player's PC (`traces/`, ignored by git); the repo
  holds the tools and the list of fields.

## F4f: the runner against the running game

**Done 2026-10-10.** `tools/pcsx2_debug.py capture` stops the game (PCSX2-MCP's DebugServer) at a function's entry
during a race, records the argument registers and the memory blocks the function reads at their real addresses,
runs to the return and records `$f0`. `tools/r5900/tests/captured.rs` writes that memory back into the runner,
calls the same function and compares the result bit for bit (`TRICKY_ELF`, `TRICKY_CAPTURES`; the capture file is
made from the game and stays on your PC). Struct layouts are not needed: the memory is replayed as it was.

24 calls in a race (12 of `Boarder_ForwardDrag` 0x109cb8, 12 of `Boarder_SideFriction` 0x109ef8, three riders):

| Runner float rules | Bit-exact | Largest difference |
|---|---|---|
| everything toward zero (the EE manual) | 3 of 24 | 6 ulp |
| add/sub to nearest, multiply toward zero | 6 | 4 |
| add/sub toward zero, multiply to nearest | 10 | 2 |
| add/sub and multiply to nearest (divide, int-to-float either way) | **18** | **1** |
| as above, with an EE adder that drops the shifted-out bits | 17 | 1 |

So in PCSX2 the game's add, subtract and multiply round to nearest; the runner now does that by default
(`ps2float::Rules::Measured`, `Rules::Manual` keeps the manual's rule). Six calls still differ by one unit in the
last place, always in the last few operations; the cause is not found yet (row F4g, with `pcsx2_debug.py probe`,
which stops each call at one more point to show where the runner and the game first part). What a real console
does, PCSX2 aside, is for F4c.

The game's `$gp` at the breakpoint was 0x3C38F0, the value the runner works out from the entry code, and the code
in memory was byte-identical to `SLUS_203.26`.

## How a row gets to `checked`

1. Port the function (address in its doc comment, AGENTS.md §13).
2. Add a runner test like the one above, or a trace check for code the runner can't reach.
3. When it passes, set `checked` for that address in `tricky-rs/docs/function-index.csv` and commit; F4a's runner
   will later write these marks itself from the test results.
