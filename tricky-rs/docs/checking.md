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
matches the formula in the port notes (`port-notes/`): the rubber band through both clamps, K = 11.517(0.5449 + 0.6742T)
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
| COP2 transfers | `lqc2` `sqc2` `qmtc2` `qmfc2` `cfc2` `ctc2` (VI, the clip flags, CMSAR0) |
| VU0 macro mode (and the micro modes' upper words) | every add, sub, madd, msub, mul, max and mini form (plain, broadcast, Q, I; to fd or ACC), `vitof` / `vftoi` (0, 4, 12, 15), `vabs`, `vclipw`, `vopmula` / `vopmsub`, `vdiv` `vsqrt` `vrsqrt` `vwaitq`, `vmtir` `vmfir` `vmove` `vmr32`, `vnop` |
| Hardware registers | the EE's 0x1000_0000..0x1001_0000 and the GS's 0x1200_0000..0x1200_2000 as plain memory (a test sets the status bits a loop waits for); a DMA channel's start bit (CHCR bit 8) clears as it is written: the transfer is taken as done, nothing moved |
| VU0 micro mode (`vcallms`, `vcallmsr`; the program from `Runner::vu0_micro`, which `load_vu0_chain` fills from the game's upload chain) | the macro upper ops; lower `iadd` `isub` `iaddi` `iaddiu` `isubiu` `iand` `ior` `fcget` `move`, the branches with their delay slot, the E bit's one more pair. Each pair reads the registers as they were before it and its results are there for the next (no pipeline or flag delays, so a program that reads a result early would differ) |
| VU1 and VIF1 (`r5900::vu1::Vu1`; `run_chain(mem, tadr)` walks a DMA source chain, its tags' upper halves as VIF codes as with TTE; row G7i) | the DMA tags `cnt` `next` `ref` `refs` `call` `ret` `end` `refe`; the VIF codes NOP, STCYCL, OFFSET, BASE, ITOP, STMOD, MSKPATH3, MARK, the FLUSHes, MSCAL / MSCALF / MSCNT (TOP from TOPS, the double buffer flipping BASE / BASE + OFFSET), STMASK, STROW, STCOL, MPG, DIRECT / DIRECTHL (to `gs`), UNPACK (S, V2, V3, V4 of 32 / 16 / 8 bits signed or not, V4-5, TOPS-relative, the cycle's skip and fill, the mode and mask). The micro programs as VU0's (no pipeline), with the lower `lq` `sq` `lqi` `sqi` `lqd` `sqd` `ilw` `isw` `ilwr` `iswr` `div` `sqrt` `rsqrt` `waitq` `mtir` `mfir` `mr32` `fceq` `fcset` `fcand` `fcor` `fcget` `b` `bal` `jr` `jalr` the integer branches `xtop` `xitop` `xgkick` (the GIF packet, PACKED / REGLIST / IMAGE until EOP, to `gs`, its place in `kicks`), the random unit's `rinit` `rnext` `rget` `rxor` (R in `Cpu::r`, stepped as PCSX2 steps it: shifted up one with the XOR of bits 4 and 22 shifted in, 23 bits under 1.0's exponent; the hardware's own step is not checked; row G7e2), `fmeq` `fmand` `fmor` on the MAC flags (`Cpu::mac`; each upper op's flags, as PCSX2 makes them, seen from the fourth pair after it: the FMAC's latency, with no stalls modelled; row G7e3), and the I bit's float (loaded after its pair's upper op, which sees the I before: as the game's program 6 is scheduled). Not modelled: the status flags, the EFU, stalls; V2 and V3 put x, y again in z, w and leave w (the hardware's choice for those lanes is not checked). All eight of the game's VU1 chains load (tests/vu1_chains.rs). A DMA tag's upper half is VIF codes only when a code is due: a command's data runs on through later tags (as the game's UNPACKs from a `cnt` into a `ref` need). `record` keeps each program call's registers and data memory before and after (and its micro memory), `trace` each pair's place and integer registers: what a port of a program is checked against (tests/vu1patch.rs, tests/vu1particle.rs) |

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

It would have failed then: our `drag_linear` and `drag_cubic` used the constants rounded to four places (0.7076
for 0.70764244). The "motion state 3 and 4" factor turned out to be the surface (rider+0x290: 3 and 4 are powder),
and the port had it as the powder depth. E8e and F4d fixed and checked this (below).

## F4b: race traces from PCSX2, over PINE

The runner checks functions one at a time; the trace checks that our tick does the same things in the same order.
It uses PINE, PCSX2's own IPC (stock PCSX2, Settings > Advanced > PINE), through `tools/pine.py`.

- **Read the game while it runs.** `pine.py` reads blocks of EE memory in batches (a 0x6000-byte block in
  about a millisecond, checked against word-by-word reads and the ELF on 2026-10-10), dumps them, and `watch` polls a block and keeps every distinct copy with a timestamp.
- **Know which tick it is.** PINE is not tied to the game's frames and cannot stop the game, so each sample also
  reads the game's own tick counter, and only pairs of consecutive ticks are kept. Finding that counter, the
  boarder array and the pad state (`watch` on candidates, compare dumps a tick apart) is F4b's first job; write
  the addresses into the port notes.
- **Same moment, every time.** `pine.py save <slot>` / `load <slot>` puts the game back to the same instant, so a
  tick can be recorded again with different inputs or replayed after a change to our code.
- **What to record per tick, per rider:** the boarder struct (position, velocity, orientation, motion state and
  state timer, meter, trick state), the pad state the game read that tick, and the random seed state.
- **Replay.** Load the recorded state at tick t into our structs, feed the recorded pad, step our code one tick and
  compare with tick t+1, field by field, within the tolerance F4c sets. Report the first field and tick that
  differ. The function runner can replay the game's own tick the same way (its memory from a dump), which makes
  per-tick checks of the original code possible without breakpoints.
- **Files.** Traces are made from the game, so they stay on the player's PC (`traces/`, ignored by git); the repo
  holds the tools and the list of fields.

### What PINE found in a race (2026-10-10)

Read live with `tools/pine.py`, SSX Tricky SLUS-20326 in PCSX2, six riders in a race; the heap addresses were the
same from race to race, but `pine.py riders` finds them anew each time, so nothing depends on them.

| What | Where | How it was found |
|---|---|---|
| Rider stats | fixed entries in `.data` at 0x32DB70 + slot·0x84 (slot 0–5); `cAI::vf11` indexes the same table | the F4f captures' stats pointers |
| Rider → its stats | rider+0x464 | `Boarder_ForwardDrag` reads it |
| Race object | holds the riders at +0xC4 (an array), their count at +0x88 | an array of all six rider pointers in RAM; `Race_UpdatePlacingsAndRubberBand` 0x115100 reads the same fields |
| Race tick | race+0x18, +1 per tick, 60 a second | sampled a second apart |
| Rider position | rider+0x140 (x, y, z, cm, world) | moves by velocity × time scale / 60 each tick |
| Rider velocity | rider+0x150 (cm/s) | as the jump and air code use it |
| Rider time scale | rider+0x12C (1.0, or the rubber band's 0.70005 for computer riders ahead) | the riders at 0.7001 move 0.7 × velocity/60 a tick |
| Rider place | rider+0x110 (0 = first) | as the placings code writes it |
| Board class, stance | rider+0x420 (0 BX, 1 freestyle, 2 alpine), rider+0x1B4 | as in the port notes |
| Other 60 Hz counters | 0x3BBA44, 0x3BBA8C, 0x3BBA90, 0x403A08–0x403A38 (frames since boot); 0x335720, 0x403D80 | `pine.py counters` |

`pine.py record <seconds> <file>` keeps, for every tick, the tick and each rider's first 0x600 bytes, only when
the tick did not change while reading: 181 consecutive ticks in 3 seconds, none missed.

**The pad** (F4b2): Sony's pad library writes the first controller into a double buffer, two 20-byte records
0x80 apart (0x845940 and 0x8459C0 in this boot; `pine.py` finds them by their shape and `record` saves one per
tick). Checked live with every key of a keyboard pad:

| Bytes | Field |
|---|---|
| 0, 1 | status; mode (0x73 analog, 0x79 pressure-sensitive) |
| 2–3 | buttons, little-endian, 0 = pressed: bit 0 Select, 3 Start, 4–7 Up Right Down Left, 8 L2, 9 R2, 10 L1, 11 R1, 12 Triangle, 13 Circle, 14 Cross, 15 Square |
| 4, 5 / 6, 7 | right stick x, y / left stick x, y (0x7F centred, 0x00 left or up, 0xFF right or down) |
| 8–19 | pressures, 0–255: right, left, up, down, triangle, circle, cross, square, L1, R1, L2, R2 |

A keyboard gives 255 or 0; a real controller gives the range. The replay test needs our riding code callable without Bevy (F4b3, F11).

## F4f: the runner against the running game

**Done 2026-10-10**, with PCSX2-MCP's DebugServer, which the project no longer relies on (AGENTS.md §14):
the captures below stay valid test data, new checks go per tick over PINE (F4b). `tools/pcsx2_debug.py capture` stopped the game at a function's entry
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

So in PCSX2 the game's add, subtract and multiply round to nearest; the runner does that by default
(`ps2float::Rules::default()`; `Rules::MANUAL` is the manual's rule, `TRICKY_FLOAT_RULES=add=ee,mul=chop` and the
like try others, including an EE adder that drops the bits it shifts out).

**F4g, 2026-10-10: the remaining differences.** `tools/pcsx2_debug.py probe 109cb8 240 1` stopped 135 more calls
of `Boarder_ForwardDrag` each at one point inside (every instruction outside delay slots, one per call). Over all
159 calls, final results:

| add/sub | multiply | Bit-exact | Largest difference |
|---|---|---|---|
| nearest | nearest (default) | 111 of 159 | 2 ulp |
| EE adder | nearest | 110 | 2 |
| toward zero | nearest | 97 | 2 |
| nearest | toward zero | 80 | 4 |
| EE adder | toward zero | 35 | 5 |
| toward zero | toward zero (manual) | 28 | 6 |

Divide and int-to-float rounding change nothing on these calls. The first instruction where runner and game part
is `add.s $f9, $f1, $f4` at 0x109d6c, the drag's `cub = S2·(−0.29035342)·(1/255) + 1.2848105` (stat byte
rider stats+0x19). Brute force over all 256 stat values: the game's results there (0x3f99610a for byte 76,
0x3f98a67c for byte 81) come out only with both multiplies toward zero and the add to nearest, or with the EE
adder (any multiply rounding), never with IEEE rounding throughout. Neither rule holds for the whole function,
so PCSX2 most likely emulates the EE's multiplier and adder bit by bit (its accurate FPU), which is neither IEEE
rule; reproducing that is what is left of F4g. Whether to follow PCSX2 or the console is F4c.

**F4g, continued 2026-10-10: normal play rounds to nearest.** The per-tick replays of F4h come from PCSX2
running normally (read over PINE, no breakpoints). `air_steps_tell_the_float_rules_apart` in
`tricky-rs/game/tests/replay.rs` runs the game's own `Air_IntegrateRK4` in the runner on the 341 airborne
rider-ticks under each rule (`TRICKY_FLOAT_RULES`):

| Rules | Game's air step reproduced |
|---|---|
| add, mul, div to nearest | **341 of 341** |
| divide toward zero | 252 |
| multiply toward zero | 214 |
| add toward zero (with any multiply) | 1 |
| EE adder (dropped shifted-out bits) | 0 |
| everything toward zero (the manual) | 1 |

So in normal play the air step's vector-unit adds and multiplies, its `vdiv`s and its FPU multiplies and divide
all round to nearest, as IEEE f32 does. (The rubber band replay does not tell the rules apart: its result sits on
its clamps and rate-limit steps.)

The 1–2 ulp differences above were all FPU (COP1) operations, captured while PCSX2-MCP's debugger stopped the
game at breakpoints. 20 single operations from the probe stops that could be read off cleanly (15 `mul.s`,
4 `add.s`) each match round-to-nearest, and yet the drag's chain `S·(−0.29035342)·(1/255)` for stat bytes 76 and
81 comes out truncated where its exact products were 0.76–0.84 ulp above a float. No fixed rule fits both, nor
does a radix-4 Booth multiplier with low partial-product columns dropped (best 14 of 19 samples, any column,
either operand recoded, negative partial products floored or truncated toward zero). What is open: whether
COP1 differs from the vector unit, or the debugger's mode differs from normal play. Settle it with a COP1-heavy
function per tick in normal play (the ground forces, F4h2/F4b3); until then the port stays IEEE-nearest, which
is what normal play shows.

The game's `$gp` at the breakpoint was 0x3C38F0, the value the runner works out from the entry code, and the code
in memory was byte-identical to `SLUS_203.26`.

**F4g, 2026-11-08 (irregulur/lynx): PCSX2's COP1 adder, from the per-tick replays.** F4h2 read the rule off
`AirPredict_Tick` 0x123ec8 (three COP1 `add.s`/`sub.s`, `checking.md` F4h2). It is now a runner rule:
`ps2float::Round::Guard` (`g1`). The smaller operand is cut toward zero to one bit below the larger's last
mantissa bit (one guard bit, no sticky bit), then the sum is rounded to nearest, ties to even. COP1's add and
subtract now have their own setting, `fadd`, apart from VU0's `add`. `add=` sets both and `fadd=` after it sets
the FPU's alone (`TRICKY_FLOAT_RULES=add=n,fadd=g1,mul=n,div=n,cvt=n`). `air_steps_tell_the_float_rules_apart`
in `tricky-rs/game/tests/replay.rs` now also runs the game's `AirPredict_Tick` in the runner on every recorded
air tick of the prediction record (rider+0x57b0). Recordings: the attract demo (rec 5) and a Single Event raced
by hand (rec 6, 90 s, 6 riders):

| Rules (mul, div, cvt nearest) | Air step (VU0, `Air_IntegrateRK4`) | `AirPredict_Tick` (COP1), single ticks |
|---|---|---|
| add=n (IEEE) | 1,940 / 1,940; 5,800 / 5,800 | 1,560 / 2,036; 4,207 / 5,877 |
| **add=n, fadd=g1** | **1,940 / 1,940; 5,800 / 5,800** | **2,036 / 2,036; 5,877 / 5,877** |
| add=n, fadd=ee (no guard bit, chop) | all | 1,730; 4,512 |
| add=n, fadd=c (chop) | all | 1,684; 5,443 |
| add=g1 (VU0 too) | 117; 357 | all |

So in normal play PCSX2's COP1 `add.s`/`sub.s` are `g1` and VU0's adds are nearest: the two units differ.
That answers the question F4f/F4g left open. `g1` also gives the two values at 0x109d6c that no IEEE rule
gave (`Boarder_ForwardDrag`'s cub term, stat bytes 76 and 81, with nearest multiplies). It was not yet run
on the 159 saved `Boarder_ForwardDrag`/`Boarder_SideFriction` captures, which stay on ed-slopper's PC
(`tools/r5900/tests/captured.rs`, `TRICKY_CAPTURES`, `TRICKY_FLOAT_RULES=add=n,fadd=g1`): row F4g2.

The runner's default stays `fadd=n`, the rule every `checked` row was checked by. With `fadd=g1` as the
default, 38 of the game crate's 622 runner tests part from our IEEE ports (mostly draw code: animation
curves, Bézier tables, fonts, quads, lights, fog, the limb IK). Whether the port follows PCSX2's adder
(and the console's, still unknown) is F4c, a decision for the humans.

## F4d: our functions against the original (done 2026-10-10)

`tricky-rs/game/src/ground.rs` holds `forward_drag` (`Boarder_ForwardDrag` 0x109cb8) and `side_friction`
(`Boarder_SideFriction` 0x109ef8) with the original's constants, units (cm, cm/s) and order of operations;
`Rider::step` calls them. `tricky-rs/game/tests/checked.rs` runs the game's own functions in the runner and ours
on 3,000 random inputs each (speeds, loads, every board class, switch, powder, crouch, boost, brake, full-lock
steer) and asks for the same bits: **all 6,000 identical.** Both are `checked` in the function index.
F4d2 added `spring_force` (`Boarder_GroundSpringForce` 0x109878) and `ground_thrust` (`Boarder_GroundThrust`
0x109950, including its vector-unit speed, sqrt(((x² + y²) + z²) + w²)): 3,000 cases each, all identical,
first try. Then `tricky-rs/game/src/air.rs`: `integrate_rk4` (`Air_IntegrateRK4` 0x12b340, the air step: one
classic RK4 step on the vector unit, gravity −850.24 cm/s² rising and −1900.84 falling chosen per stage, horizontal
drag −0.20002478·v, 1/2, 1/6 and 1/3 from `vdiv`, then the speed capped at 3347.22 cm/s, which it writes to
rider+0x1c4 on every call). 3,000 cases (riding frames and 0.2 s-stepped flight predictions, at and over the
cap), all identical, first try; `Rider::step` now flies with it. Five functions `checked`.

- The decompiler's expression is not always the machine code's order: the first try differed by one ulp until
  the cubic term was grouped as the instructions do it, `(|vF|·0.001)·((L·row₂)·cubic)` (0x109e70, 0x109ea4,
  0x109ed0). Read the float instructions (`tools/insn_census.py` style) when a check is one ulp off.
- The runner rounds every operation to nearest for this check (`Rules::parse("add=n,mul=n,div=n,cvt=n")`), as
  IEEE f32 does; nearest add and multiply is what PCSX2 showed (F4f, F4g), nearest divide is assumed (F4c).
- Our stats don't separate every stat byte the game reads (the cubic drag reads stats+0x19, side friction
  stats+0x13): `Rider` passes the speed and edging stats in their place until the stats tables are ported (G3).
- `side_friction`'s third input is rider+0x214, most likely the shaped steer (|x|·1.0001 ≥ 1 only at full lock).

## F4h: against the running game, tick by tick (done 2026-10-10)

`tricky-rs/game/tests/replay.rs` replays races recorded over PINE (`pine.py record`; `TRICKY_RECORDING`, files
separated by `;`, they stay on your PC; skipped without them). Two recordings from F4b1 (181 and 121 ticks, six
riders each):

- **Air step.** The game calls `Air_IntegrateRK4` once a tick on the rider itself: dt = time scale (rider+0x12c)
  × 0.016666668, position +0x140, velocity +0x150, no sub-steps (0x100000.c's caller). Our `integrate_rk4` from
  tick t gives tick t+1's position and velocity **bit for bit on 341 of the 342 rider-ticks in the air**; every
  other miss is far off (the rider was on the snow). The one exception (trace1 tick 7750, rider 1, mid-hop) has
  the velocity exact and x, y 2 ulp (0.016 cm) further on: no rounding rule of the runner gives the game's value
  (they all give ours or less), so something else nudged the position that tick, most likely a collision push.
  So PCSX2's own floats agree with our IEEE-nearest port here, not just the runner.
- **Rubber band.** `AI_RubberBandSpeedScale` 0x13a0f0 (with `Math_Cos` 0x250e98) run in the function runner on
  the rider as recorded at tick t, then `Boarder_SetTimeScaleRateLimited` 0x11cff0 (at most 0.008446341 a tick),
  against the time scale at t+1: **1,782 of 1,800 rider-ticks exact** (trace2 720 of 720). Each of the 18 misses
  is exactly one rate-limit step ahead or behind, caught up on the next ticks: the AI update ran twice or not at
  all between two recorded race ticks, a matter of when the recorder reads, not of the function.
- Takeoff, the jump impulse, the flight prediction (RK4 with sub-steps, $a3 ≠ 0) and the jump's steep-lip
  branch are row F4h2.

## F4h2: takeoffs and the flight prediction from a recorded race (2026-10-10, 2026-11-08)

A third recording (`pine.py record 40`, 2,334 ticks, six riders, with jumps), and each rider's stat block saved
over PINE in the same race (TRICKY_STATS: `rider stats-address hex` lines).

- **Air step:** our `integrate_rk4` gives **4,275 of 4,275** airborne rider-ticks bit for bit, none off by an ulp;
  in the runner only round-to-nearest gives them all (F4g). Rubber band 13,861 of 13,998, misses one rate-limit
  step either way as before.
- **Takeoffs:** `takeoffs_against_the_running_game` finds the ticks a rider in the jump state (0xa) leaves the
  snow, runs the game's `Jump_ApplyImpulse` 0x1284e0 in the runner on the rider recorded at that tick (Math_Cos,
  Math_Sin and Vec4_Scale run as the original; scoring stubbed) and takes it from the game's velocity change.
  With the smallest jump 630.88 cm/s (the argument the 0x1003xx caller passes; 0 from 0x128470 leaves 150–540
  cm/s unexplained), 13 of 18 takeoffs leave 6–62 cm/s: one tick of gravity (21.7 cm/s at 1300.85 cm/s²) and the
  ground forces of that tick. Three (Δv 260–440 cm/s, falling) leave the snow in the jump state without an
  impulse; two of rider 1's and one larger jump leave 143–163 cm/s: charged jumps, whose impulse depends on how
  far the crouch unwound (rider+0x1c8 − rider+0x208) within the tick.
- Bit for bit needs the state inside the tick (crouch, the ground forces before the impulse): replaying the whole
  tick, row F4b3. The flight prediction (RK4 with sub-steps) and the steep-lip branch are still to do.

**F4h2, 2026-11-08 (irregulur/lynx): the flight prediction.** Recorded with
`pine.py record 60 <file> 0x600 57b0:64`, which now also reads the prediction record (rider+0x57b0, 0x54 bytes:
+8 time left, +0x10 point, +0x20 velocity, +0x30 normal, +0x40 surface word, +0x44 landing time, +0x48 time to the
top, +0x4c time since take-off, +0x50 time to the top left) in the same PINE message as the rider. (The first try
read the windows after all six riders: 6 of 41 predictions then sat one tick ahead of their rider. Reading each
rider's block and windows in one message fixed that.) The race was the attract demo, run twice: same ticks, same
jumps. `flight_predictions_against_the_running_game` in `tricky-rs/game/tests/replay.rs`, on the second recording:

- 35 take-off predictions (a record whose +0x4c is 0 and which changed this tick), plus 2 read while the game was
  still inside `AirPredict_AtTakeoff` 0x123b10 (+8 counting up, +0x44 still 0; skipped).
- **No landing (7):** the record then holds the prediction's own position, velocity and normal, so it can be
  made again from them: `air::predict_at_takeoff` gives all 0x4c bytes bit for bit on 7 of 7. Those inputs are
  the rider's at the end of the tick on 6; on one (tick 19500, rider 0, state 3 → 0xd) the rider's velocity
  was about halved after the prediction, later in the same tick (inferred: a hit or the 0xd placing).
- **A landing (28):** made from the rider at the end of the tick (+0x140, +0x150, +0x2a0). The time to the top
  is bit for bit on 27. Without the course here, the test's ray answers with the game's own landing (point,
  normal, word) on the first of our segments that passes within 0.05 cm of the game's point. That happens on
  the same 27, at most 0.0084 cm off, which shows our flight's steps are the game's. The landing time is then
  within 5.8e-6 s and the velocity within 0.011 cm/s; both use our ray's fraction, not the game's.
- The other one (tick 21105, rider 4, state 0xa → 0x10): the game's time to the top is 0, so its velocity was
  not rising when it predicted. The rider ends the tick rising at 345.8 cm/s, and 21104 shows a 576 cm/s jump
  impulse. Inferred: this time the impulse came after the air motion's enter, within the tick. Settling it needs
  the in-tick state (F4b3).
- **`AirPredict_Tick` 0x123ec8**, each tick in the air: `Predict::tick` in IEEE f32 gives the record bit for
  bit on 1,604 of 2,239 ticks. With **PCSX2's FPU adder** as these ticks show it, it gives 2,101, and 131
  more are two ticks apart (the recorder fell a frame behind). The rule: the smaller operand is cut toward
  zero to one bit below the larger's last mantissa bit, then the sum is rounded to nearest, ties to even
  (`pcsx2_add` in the test). Its three operations are plain COP1 `add.s` and `sub.s`.
  - On the first recording, of the operations where the rules differ: IEEE nearest fits 38 of 448 adds and
    471 of 604 subtracts; toward zero fits 396 and 193; the EE adder (no guard bit) 396 and 314; one guard bit
    with nearest fits **434 of 448** and **604 of 604** (the 14 adds are the two-tick gaps).
  - It also gives F4g's `add.s` at 0x109d6c (0x3f99610a for stat byte 76, 0x3f98a67c for 81) with
    nearest multiplies, where IEEE gives 1 ulp less.
  - The air step (`Air_IntegrateRK4`, VU0 macro code) stays IEEE-nearest: 1,940 of 1,940 in the runner on this
    recording. So in normal play PCSX2's COP1 add/sub differs from its vector unit. This is the lead for F4g;
    the port stays IEEE until F4c decides.
- Takeoffs (`takeoffs_against_the_running_game`, stats from the same demo): 13 jumps, left-overs as before
  (0.7–133 cm/s). The steep-lip branch is still in no recording.
- A Single Event raced by hand (90 s, F4g's recording): 54 predictions (+3 half-made); 8 of 8 no-landing records
  bit for bit, all from the rider at the end of the tick. 46 with a landing all have the time to the top bit for
  bit; on 42 the game's landing is on our flight (0.0029 cm, 1.1e-6 s). On 4 (ticks 2462, 3027 and 4296 of rider 4,
  tick 3318 of rider 0) it is 30–157 cm off every segment: the same vertical but another sideways path. Either
  the velocity changed sideways after the prediction, or the flight reads something of the rider that ours does
  not (row F4h3).

## F4b3a: the air's spin state from a recorded race (2026-11-08)

`tests/replay.rs` `spin_ticks_against_the_running_game` replays `SpinState_Update` 0x100908 over a race: every pair of
samples one tick apart where a rider is in the air (motion +0x424 = 1) and in control state 0x10 (+0x428) at both.
It runs our `spin::spin_update` and the game's own code in the function runner (IEEE adds, then PCSX2's COP1 adder
`fadd=g1`) on the first sample, and compares with the second. Recording: `pine.py record 110 rec8.txt 0x600 57b0:64
-170:74` (the `-170` window is the spin object), with the stats blocks saved after it (`TRICKY_STATS`).

Proven (read from the code):
- The spin object is the state manager *(rider+0x5ae0) + 0x740, and its +0x70 is the rider. The manager is rider −
  0x8b0 for five of the six riders and rider − 0x880 for the sixth, so `-170` is the spin object for the five only;
  the test checks +0x70 and leaves the sixth out.
- `cBoarder::vf7` 0x117198 runs the motion first (`Boarder_UpdateMotion` 0x11c770 at 0x117960; in the air it is
  `AirMotion_Update`, whose `AirPredict_Tick` 0x123ec8 moves the time to touch down +0x57b8) and then the control
  (`Boarder_UpdateControl` 0x11c9a0 at 0x117b78). So the spin state reads this tick's time to touch down.
- The pad word the control gets is built on the stack (virtual 9 of the rider, at 0x117b6c, into sp+0x30) and never
  stored, so it cannot be recorded. The animation's answers (clip, type, phase, trick clips) are not recorded either.
  Both are 0 in the test: only the fields that do not read them are compared (the turn toward the landing slope, the
  spin and flip angles; the rates, bands and largest errors only in mode 2).

Found (rec8: a 110 s race by hand, 6,585 samples):
- 7,566 pairs in state 0x10. Left out: 1,259 of the sixth rider, 1,183 not one update apart (the position or the
  angles moved by 0 or 2 ticks' worth: the recorder reads the race's tick counter, which does not move at the same
  moment as the riders). 5,124 compared.
- Ours and the game's code under IEEE adds give the same counts on every field: quaternion 1,591, axes 1,431, spin
  4,508, flip 4,358, angle left 4,201 of 5,124; rates 1,442, bands 1,550, largest errors 1,444 of 1,551 (mode 2).
- The game's code under `fadd=g1`: quaternion 5,077, axes 5,075, spin 5,046, flip 5,122, angle left 5,078 of 5,124;
  rates 1,549, bands 1,550, largest errors 1,551 of 1,551. So the running game's `SpinState_Update` is its own code
  with PCSX2's COP1 adder (F4g), and our port matches that code under IEEE. The gap between ours and the game is the
  adder alone: the port's floats follow the runner's default rules (F4c decides them).
- 28 of the `g1` matches needed the first sample's time to touch down instead of the second's. Inferred: there the
  samples fell inside a frame, between the rider's motion and its control, so the control's input was already in
  the first sample.
- Inferred, not checked: most of the 78 spin misses are mode 2 ticks where the spin moves while its rate is 0 (the
  half-pipe clips 0x23c / 0x23d add their phase × π; the phase is the animation's answer, recorded as 0).

Still to do: the pad and the animation's answers (the rows that port them, G5, would have to be recorded or
replayed too); the ground tick (F4b3b) and the take-off tick (F4b3c).

## F4b3b: the ground tick from a recorded race (2026-11-08)

`tests/ground_motion.rs` `ground_ticks_against_the_running_game` replays `GroundMotion_Update` 0x10a0d8 over a race:
every pair of samples one tick apart where a rider is on the snow (motion +0x424 = 2) at both. It runs our
`ground_motion::update` and the game's own code in the function runner (the world of `ground_motion_is_the_originals`,
under IEEE adds and under `fadd=g1`) on the first sample and compares with the second, field by field. The course is
not loaded: the world's ray cast under `Boarder_ProbeGround` is answered with what the probe left in the second sample
(ground point +0x2d0, normal +0x2a0, surface +0x290, kind +0x2e0). Recording: `pine.py record 100 rec10.txt 0x600
-400:10 46c0:10 5ae0:4` (the ground motion object and the clip), with `TRICKY_STATS`. The recording reader moved to
`tests/recording/mod.rs`, shared with `tests/replay.rs`.

Proven (read from the code):
- The ground motion object is the state manager *(rider+0x5ae0) + 0x4b0 (its +0xc the rider, +8 the phase): rider −
  0x400 for five riders, rider − 0x3d0 for the one whose manager is rider − 0x880 (left out, as in F4b3a).
- `cBoarder::vf7` 0x117198 moves the input records before the motion (from 0x117244 on: value +0, rate +4, target +8;
  the value steps toward the target by rate · 0.016666668): brake +0x1fc, crouch +0x208, steer +0x214 and on to
  +0x244, the probe's side offset. It also writes the spring band +0x294 / +0x298 (0x11787c, 0x1178cc), the speed
  timer +0x134 and the body height +0x308. So the motion reads those as the second sample holds them; the test takes
  brake, crouch, steer, +0x244 and the band from there.

Found (rec10: 100 s raced by hand, 5,991 samples, 6 riders):
- 19,606 ground pairs. Left out: 3,744 of the sixth rider and 2,612 not one update apart (the position moved by 0 or 2
  ticks' worth, as in F4b3a). 13,250 compared.
- Ours and the game's code under IEEE adds give the same count on every field; neither gets a whole tick (the board's
  turn, `Boarder_RotateAboutAxis`, is off on almost every tick: quaternion 157 of 13,250).
- The game's code under `fadd=g1`: every written field bit for bit on 12,122 of 13,250 ticks (91.5%); position 12,727,
  velocity 6,857 of 7,357 (below the speed limit), quaternion 12,642, yaw 12,651, drag 12,904, height 12,635, bob
  12,806, phase 12,708, lean 13,227, normal, ground, surface and kind 13,250. So, as for the air (F4b3a), the running
  game's ground tick is its own code with PCSX2's COP1 adder.
- Not compared: the rider's up (+0x2c0), which the motion only reads and something later in the tick turns. The
  velocity was first left out where the speed was 2,788 cm/s or more (after the motion `cBoarder::vf7` scales it to
  the speed limit); since F4b3c the test applies the limit as the game does, and the velocity is compared everywhere:
  rec10 under `fadd=g1` velocity 12,498 of 13,250, whole ticks 12,103; rec11 whole ticks 20,061 of 21,276 (94.3%).
- Inferred, not checked: most of the 1,128 left are the parts of the tick after the motion or outside it: riders
  near each other pushed apart (`Boarder_RiderCollisions` 0x123fc8; seen: two riders 100 cm apart whose velocities
  are 7 cm/s off), walls and instances (answered as none), and control-state changes (59 ticks where only the state
  differs).

Still to do (row F4b3e): the rest of the tick after the motion (the up's turn, the speed limit, the rider collisions)
ported and replayed, the walls and instances recorded, the sixth rider (its windows sit 0x30 further on).

## F4b3c: the take-off tick from a recorded race (2026-11-08)

`tests/ground_motion.rs` `takeoff_ticks_against_the_running_game`: every pair of samples one tick apart in which a
rider goes from the snow to the air (motion +0x424 2 → 1). The ground motion runs as in F4b3b (same inputs from the
second sample, same answered ray cast), then the way up, three ways: ours (`ground_motion::update`, `orient::take_off`,
`jump::jump_impulse`; the jump control is not ported, so the test makes its calls), and the game's code in the runner
(`Boarder_TakeOff` 0x128470 or `JumpControl_Update` 0x1046b0 run whole, with `Jump_ApplyImpulse` 0x1284e0) under IEEE
adds and under `fadd=g1`. `Boarder_SetMotion` 0x11f610 is recorded with the rider's position, velocity and normal at
the call: that is what `AirMotion_Enter` 0x108310 hands `AirPredict_AtTakeoff`. Recording: `pine.py record 100
rec11.txt 0x600 57b0:64 -400:10 -170:74 46c0:10 5ae0:4` and `rec11.stats` (100 s raced by hand, 5,888 samples).

Proven (read from the code):
- `Boarder_UpdateMotion` 0x11c770 runs only the current motion, so on the take-off tick the air motion does not run:
  the second sample holds the ground motion's step plus the jump.
- Two ways up. (1) `GroundMotion_Update` ends with `Boarder_TakeOff` (0x10ab78, then straight to its epilogue):
  `Boarder_SetMotion`(1) first, so the prediction sees the velocity before the jump; then out of the jump control
  rider+0x1c8 = +0x208, `Jump_ApplyImpulse`(0) (no strength left: the velocity only changes off a steep lip) and state
  0xd; in the jump control (0xa) `Jump_ApplyImpulse`(0) with the crouch it has, then `Takeoff_SetSpinRates` 0x126e30.
  (2) `JumpControl_Update` (state 0xa, from `Boarder_UpdateControl` after the motion) once the crouch +0x208 is 0:
  `Jump_ApplyImpulse` with 630.8786 cm/s (0x441db83b) at least, then `Boarder_SetMotion`(1), so this prediction sees
  the velocity after the jump, then `Takeoff_SetSpinRates`.
- The speed limit (`cBoarder::vf7` 0x1179a4..0x117ab0) is applied after the motion only when the motion is not the
  air (0x11799c), so it applies before the jump control's jump but not after a take-off in the motion. Target 3347.2222
  cm/s while the speed timer +0x134 runs; else while boosting (+0x130 > 0) by the meter +0x1c: over 0.6659923 3347.2222,
  over 0.33362994 3072.1418, over 0 2932.43; else 2788.8394. The limit +0x1c4 is the larger of the target and last
  tick's limit less 3.472222 (a COP1 `sub.s`), and over it the velocity is scaled by limit / speed. (The sample after
  a take-off holds 3347.2222 at +0x1c4: something on the way up sets it, so the test works the limit out from the
  first sample.) Now applied in F4b3b's test too, so the velocity is compared on every ground tick.

Found (rec11):
- 99 take-offs: 31 of the rider whose motion object is elsewhere (left out, as in F4b3b), 6 not one update apart; 62
  compared: 43 left the snow in the motion, 17 by the jump control, 2 neither (rider bumps: another rider 165 and 206 cm
  away; `Boarder_RiderHit` 0x124920 jumps and sets the air motion itself).
- Ours equals the game's code under IEEE on every field, as before; the take-off itself (calls, the jump) matches,
  but the ground step before it is off as in F4b3b (quaternion 2 of 62).
- Under `fadd=g1`: the whole tick bit for bit on 54 of 62; position 57, velocity 57, quaternion 55, the way up (one
  `Boarder_SetMotion`(1)) 60. The prediction's inputs, where it found no landing and so keeps them: 11 of 12 equal the
  rider at the `Boarder_SetMotion` call; the time to the top (+0x48) from those inputs 54 of 54 (all three ways).
- 7 prediction records were read while the game was making them (6 with `AirPredict_Reset` 0x123ab8 done, velocity
  0, and the state still 3 rather than 0xd: the PINE read landed between `Boarder_SetMotion` and `Boarder_SetState`);
  left out of the prediction counts.
- Inferred, not checked: of the 8 off, 2 are the rider bumps, 3 riders near another (69 and 114 cm: pushed apart after
  the motion), 2 take-offs in the motion from the jump control state whose orientation alone is off (the spin state
  0x10 set by `Takeoff_SetSpinRates` may already run its update this tick), and 1 velocity 56–87 cm/s off with no
  rider near (a wall or instance, answered as none).

Still to do: rows F4b3e (after the motion: collisions, the rider's up) and F4b3f (the jump control ported, the
take-offs by rider bumps and the same-tick spin update replayed).

## F4b3e: the end of the rider's tick from a recorded race (2026-11-08)

`tests/ground_motion.rs` `ground_ticks_against_the_running_game` now runs the rest of `cBoarder::vf7` 0x117198 that
needs no other rider: the speed limit (F4b3c) and `Boarder_UpdateLeanTargets` 0x129500 at the end of the tick (ours
`ground_tick::lean_up` and `lean_targets`; the runner calls the game's), and compares the rider's up +0x2c0 and the lean
record's target +0x24c too. New recording `rec12.txt` (150 s, raced by hand in the pack: `pine.py record 150 rec12.txt
0x600 57b0:64 -400:10 -3d0:10 -170:74 -140:74 46c0:10 5ae0:4`, and `rec12.stats`): the `-3d0` window is the sixth
rider's ground motion object (its manager is at rider − 0x880), so every rider is now compared; `-140:74` is its spin
object, for F4b3d.

Found:
- Samples read inside a tick (inferred): PINE reads while the EE runs, so a sample can hold a rider after its motion
  but before `Boarder_UpdateLeanTargets`: its up is still the last tick's while the rest has moved. 344 of the first
  831 up/target misses in rec10 were such samples, and the next pair then starts from the stale up. The test leaves
  out a pair when either sample's up equals the one before it while differing from the normal (`late`): 1,069 pairs
  in rec10, 1,146 in rec11, 2,463 in rec12.
- Under `fadd=g1`, whole ground ticks (every field, up and lean target included): rec10 11,482 of 12,181 (94.3%,
  was 91.5%), rec11 19,550 of 20,130 (97.1%, was 94.3%), rec12 33,152 of 34,538 (96.0%). rec12 per field: position
  33,775, velocity 34,067, quaternion 33,910, up 34,291, lean target 34,456, state 34,353. Take-offs (F4b3c) on rec12
  with the sixth rider: 96 of 116 whole.
- Ours equals the game's code under IEEE on every field except the up on 3 ticks of rec12 (34,288 against 34,291; the
  vector unit's `vrsqrt` against our `unit`, not looked into).
- Position misses on rec12 (763): 656 with another rider within 3 m (440 within 1.5 m, median 0.9 cm off): rider
  collisions (`Boarder_RiderCollisions` 0x123fc8), not replayed; 107 with no rider within 3 m (median 0.16 cm, up to
  33 cm): walls and instances, answered as none.
- 186 up misses in rec10 remain where both samples look complete; there the game's up moved about 1.5 times our
  step (unexplained).

Still to do: row F4b3g.

## F4b3g: riders pushed apart, from a recorded race (2026-11-08)

`tests/ground_motion.rs` `ground_ticks_against_the_running_game` now replays, every frame, the pushes of
`Boarder_RiderCollisions` 0x123fc8 (`frame_pushes`) when the recording holds the riders' collision bodies (window
`0x700`, the body at +0x470..+0x6e0).

Proven (code and PINE):
- `cBoarder::vf7` 0x117198 calls `Boarder_RiderCollisions` at 0x117cd4, after the motion, the speed limit, the
  control and the score, and only while the race's state (race +0x1c) is 3 or more and not 5.
- `Boarder_Translate` 0x1267a0 moves only the body, its bounds +0x6e0/+0x6f0 and the broadphase box; the driver
  moves the positions itself (0x1240fc, 0x124150, 0x12416c). The ground motion moves no body.
- The bodies are rebuilt from the bones once a frame, by `cPostAI::vf1` 0x114730 (`Boarder_UpdateBroadphase`
  0x126748 for each rider in slot order), which sits in object list 11. `Game_UpdateObjectLists` 0x1814c0 runs the lists in
  the order of the table at 0x337c70: 0, 1, 2, 3, 5, 6, 7, 8, 4, 10, 11. The riders' `cBoarderHandler` objects
  (vtable 0x36a9c0, `cPlayer` 0x36f218 for the one the pad drives) are list 8. Walking that list (`BxList_First`
  0x17d860, header list·0x34 + *(0x337c60+4)) gives slot 5 first, slot 0 last. So the collisions of frame F use
  the bodies of frame F−1's end, moved by the pushes of riders that ticked earlier in F.
- Each rider's speed-sample count, rider+0x5900 (score +0xe0, a float that `Boarder_AddSpeedSample` 0x155dc8 adds
  1 to every tick), tells whether the rider has ticked yet this frame.

The recordings were the problem. Up to rec13, `pine.py record` read the six riders in six PINE messages right
after the race tick moved. PINE reads while the EE runs, and the race tick moves at the start of the frame, so
most samples held riders from both sides of their tick, with bodies from mixed moments. A 4 s check had 61 of 241
samples at a frame's end. On rec13 (old reader, 150 s) the pushes went wrong: 7 of 232 pushed ticks right. The pushes
themselves were confirmed there: where two riders were pushed, their position misses are exact opposites. `record`
now waits until every rider's +0x5900 has moved (at most 30 ms), then reads every block and window in one batch
between two reads of the race tick. 235 of 235 samples were then at a frame's end. With the window `5900:4`, the
test also leaves out frames whose samples are not both at a frame's end (none in rec14).

rec14 (150 s, raced by hand in the pack: `pine.py record 150 rec14.txt 0x700 57b0:64 -400:10 -3d0:10 46c0:10
5ae0:4 5900:4`, and `rec14.stats`): 37,852 ground ticks compared, 3 left out as not one update apart, 11 as read
inside a tick (rec13: 2,584 and 1,509). Under `fadd=g1`:
- every field 37,135 (98.1%; 37,019 without the pushes); position 37,703 (37,497 without); the up 37,852 of
  37,852. The 186 ups "1.5 steps on" of rec10 were samples read mid-frame.
- 343 ticks with a push: 205 without a hit, position right on 152; 138 where the pair was also hit
  (`Boarder_RiderHit` 0x124920, every third tick per pair, not replayed): velocity wrong on all 138.
- Overlapping pairs against the game's bump records: of all frames where two bodies at the frame's start overlap
  and the pair may be hit again, the game hit them on all but 3.
- Misses with no push: 235 the control state only, about 200 the board's orientation only (fwd, quaternion, yaw):
  the control after the motion (`Boarder_UpdateControl` 0x11c9a0) is not replayed on the snow; 18 positions
  (walls and instances).

Still to do: rows F4b3h and F4b3i.

## F4b3h: the frame's riders replayed together, with their hits (2026-11-09)

`tests/ground_motion.rs` `ground_ticks_against_the_running_game` now replays each recorded frame whole, each way
(ours, the runner under IEEE, the runner under `fadd=g1`): the riders in tick order (slot 5 to 0); a ground rider's
motion starts from where the pushes and hits of the riders that ticked before it left it, then the speed limit, then
its collisions through the port's driver (`ridercol::rider_collisions`, `Frame` as its env) with the hits as
`hit::rider_hit`, then `Boarder_UpdateLeanTargets` on the velocity as it is then. Riders whose tick is not replayed
(air, rail, wiped out) take their velocity and motion from the second sample at their turn.

Proven (code):
- `cBoarder::vf7` 0x117198 calls `Boarder_RiderCollisions` at 0x117cd4, `Rivals_UpdateGrudgeTimeout` at 0x117cdc,
  then `Boarder_UpdateLeanTargets` at 0x117ce4: the lean target is aimed with the velocity after the rider's own
  hits, before the hits of the riders that tick after it.
- `Boarder_GetSpeedStat` 0x11d7b0 = stats+0x1e × (stats+0x12 × 2.145779 × 0.003921569 + 0.79599), × (1 + +0x130 ×
  7.999995).
- `Boarder_RiderHit` changes the hit rider's +0x150, which the driver reads again for the next pair's closing
  speeds: `ColEnv::rider_hit` now hands the driver's velocity to the env (`ridercol.rs`).

Inferred: the pair records' distance and bearing (+0x40, +0x44; `Race_UpdatePairDistances` 0x115a80 from the cAI
update 0x113820) are taken from the second sample; they are read only by a hop or a wipe-out, which leave the ground.

rec14 under `fadd=g1` (F4b3g's numbers in brackets): every field 37,207 (37,135) of 37,852; position 37,750
(37,703); velocity 37,809 (37,702). Pushed ticks 343, every field right 193 (121): 205 not hit, position right 152
(152); 138 hit, position right 107 (60), velocity right 111 (0). Bump records (+0x48 + 0x20·j) that the game or the
replay changed: 268 records, 238 equal. Under IEEE every field stays at 18 (the adds are PCSX2's, F4g).

What is left (row F4b3j, `TRICKY_DBG=1` lists them):
- 15 pairs whose bump the replay and the game disagree on (5 the game's only, 10 the replay's only): 9 with an air
  rider (its body moved by the step between the samples, not by `AirMotion_Update`), 2 with a wiped-out rider (its
  body not moved at all), 4 between two ground riders (ticks 1257, 1843, 4354, 4400). The neighbour list +0x700 that
  picks the pairs, and its order, is not recorded (the window ends at +0x700); every other rider stands in for it.
- 27 hit ticks with the velocity off: 22 by far, all but tick 8005 with the position off too (the push vector, so
  the bodies); 5 by 3 to 40,000 ulps (tick 4352 rider 3, 4386 riders 1 and 4, 6925, 7996), the last two with the
  position right: the hit's arithmetic is ours under IEEE, not the game's under PCSX2's adder.
- the 53 pushed ticks with no hit whose position is still off (as F4b3g).

## F4b3i: the control after the motion (2026-11-09)

`ground_ticks_against_the_running_game` now runs `Boarder_UpdateControl` 0x11c9a0 after a ground rider's motion and
speed limit, before its collisions, on the states it replays: bump (2), change fakie (4), fakie cruise (5, not on
its turn clip 0x202), hard landing (0xb) and normal landing (0xc). Ours is `control::update_control` on an animation
controller laid out in a `pf::Mem` (`animctl`'s queries, the clip table copied from the ELF) with the board turned by
`orient::rotate_about_axis`; the runner's is the original with its bump, fakie change and landing updates and the
animation queries (`AnimCtl_GetClipType` 0x15eec0, `AnimCtl_GetLayerPhase` 0x15fe70, `AnimLayer_FindClip` 0x15a6c8,
`AnimClip_GetPhase` 0x159e80) on the same layout; the state `Boarder_SetState` 0x11f608 is asked for becomes the
rider's.

Recording: `tools/pine.py record` takes windows through pointers (`<off>><off>...:<len>`, read in later batches):
rec15 adds `5ae0>600:230` (the handler's control objects: bump +0x610, its turn +8 and the turn so far +0xc; normal
landing +0x6f0, its crouch +8), `4718>0:80` and `4718>30>0:80` (layer 0's first two sequences) and their lengths
`4718>78>8>a:2`, `4718>30>78>8>a:2`.

Proven (code):
- `cBoarder::vf7` calls the controller's virtual (+0x54: `cComputer::GetControls` 0x1384b8, `cPlayer::GetControls`
  0x151250) for a pad word on its stack, then `Boarder_UpdateControl(handler, &pad)`, after the motion and the
  speed limit and before `Score_Tick`, `Boarder_RiderCollisions`, `Boarder_UpdateLeanTargets`.
- `BumpControl_Update` turns about the board's up (+0x1a0, `a1` at 0x1042dc) by the turn so far less turn × phase.
- `AnimClip_GetPhase` = t (+0x48) / (length − 1) (1 when 0), (length − 1 − t) / (length − 1) when the speed +0x10 is
  below 0; `AnimLayer_FindClip` walks the layer's list (+0x30, next +0x30, sentinel +0x38) for +0x40 = the clip.

Inferred: the clips are best taken from the second sample and the sequences from the first (rec15: every field
37,252 against 37,234 with both from the first, 37,086 with both from the second): a fakie change's clip is already
0x1ff when its update runs, and the bump's phase is the first sample's. The animation seems to move on before the
rider's tick, and the sequence windows (later PINE batches) to be read after it. `TRICKY_CLIP_A`, `TRICKY_ANIM_B`
switch them. STANDIN in the test: the computer's pad word is on the stack and not recorded; it is 0 with the brake
bit (4) when the rider ends the tick prewinding (0xe).

rec15 (150 s, 8,991 ticks; `TRICKY_NOCTL=1` turns the control off, its numbers in brackets) under `fadd=g1`: 37,501
ground ticks, every field 37,252 (36,954); quaternion 37,461 (37,241); state 37,388 (37,293). Controls replayed:
bump 402, change fakie 251, hard landing 102, normal landing 320; 1,030 of the 1,075 have every field right, the 45
others miss the position (walls, instances), not the control. Ours and the runner ask for the same states (37,388
both). Ours run on the runner's rider gives the runner's result in 1,000 of 1,075 (IEEE) and 699 (`fadd=g1`); its
quaternion is the game's in 876, the runner's in 1,058 under `fadd=g1`. rec14 (no control windows) is unchanged.

What is left:
- Bump turns: in 182 bump ticks ours turns 1 ulp off the game's, which the runner (`div=n`) has right: `animseq::div`
  rounded `AnimClip_GetPhase`'s `div.s` toward zero, the game here to nearest (fixed in F4b3l). Ours rotates under IEEE,
  the game under PCSX2's adder (as the motion, F4g).
- Cruise (3) and prewind (0xe) need the pad word: 48 ticks 3 → 0xe, 6 ticks 3 → 4, 2 ticks 3 → 2, 48 ticks 0xe → 0xa
  (the release) miss the state (row F4b3k). No fakie cruise (5) in rec15.

## F4h3: the take-off's jump and the flight prediction it sees (2026-11-09)

`flight_predictions_against_the_running_game` (`tests/replay.rs`) and `takeoff_ticks_against_the_running_game`
(`tests/ground_motion.rs`), on the old recordings and rec16 (240 s of Snow Dream raced by hand with save states, so
ticks repeat; `pine.py record 240 rec16.txt 0x700 57b0:64 -400:10 -3d0:10 -170:74 46c0:10 5ae0:4 5900:4` and
F4b3i's control windows, plus `rec16.stats`).

Proven (code): `Boarder_TakeOff` 0x128470 calls `Boarder_SetMotion`(1) (the prediction, from `AirMotion_Enter`) and
only then `Jump_ApplyImpulse` 0x1284e0 with 0: out of the jump state (0xa) with the crouch it has, else after setting
+0x1c8 to +0x208, so the jump has no strength and only a steep lip changes the velocity (v′ = ((1−b)v + b|v|L)/2).
`JumpControl_Update` 0x1046b0 jumps before it predicts.

Found:
- **Tick 19500, rider 0 (rec5; F4h2's "halved after"):** a take-off off a steep lip (normal z 0.469, state 3 → 0xd).
  The prediction holds the velocity before the jump; the game's `Jump_ApplyImpulse` in the runner on it gives the
  rider's velocity at the end of the tick bit for bit under PCSX2's adder (`fadd=g1`), 1 ulp off in x and z under
  IEEE (ours too: the float rule, F4c).
- **Tick 21105, rider 4 (rec5) and rec6's 4 landings 30–157 cm off our flight (ticks 2462, 3027, 4296 of rider 4,
  3318 of rider 0):** take-offs in the motion, predicted before the jump (21105, 3027, 3318, 4296 out of the jump
  state with strength; 2462 off a steep lip out of prewind, 0xe). The velocity before the jump, found from the one
  after it (`unjump`: Newton's steps, then the f32 values within 6 ulps that the game's jump in the runner under
  `fadd=g1` turns into the rider's end velocity bit for bit; found on every one), gives the time to the top bit for
  bit and the game's landing on our flight on all 5 (rec5 with rec4's stat blocks: the attract demo has the same
  riders).
- **The steep-lip branch in recordings:** rec5 tick 19500, rec6 tick 2462, rec15 one take-off (the whole tick bit for
  bit in the runner under `fadd=g1`), rec16 tick 11278 (rider 5, pushed by rider 0 146 cm away in the same tick).
- **rec16 predictions:** 267 (+7 read half-made). 38 with no landing, all 38 bit for bit from their own inputs, 35
  of them the rider's at the end of the tick. 229 with a landing: the time to the top bit for bit on 229 (6 from the
  velocity before the tick's jump), the game's landing on our flight on 219. The other 10 (5 take-offs met again
  after save states) and the 3 no-landing records whose inputs are not the rider's all have another rider 99–172 cm
  away: pushed apart after the prediction (inferred; the runner's own `AirPredict_AtTakeoff` from the end of the tick
  misses by the same cm). `AirPredict_Tick` 21,089 of 21,089 with PCSX2's adder (rec15 11,546 of 11,546): with
  samples at a frame's end the two-tick gaps are gone.
- **Whole take-off ticks** (`takeoff_ticks_against_the_running_game`, rec16): 235 (125 in the motion, 108 by the
  jump control, 2 by rider hits); under `fadd=g1` the runner has all fields on 210 (rec15: 102 of 110), ours (IEEE)
  on 1, as before. Of the 25 off (with repeats): 17 have another rider within 180 cm (pushes and hits after the
  motion, not replayed here), 6 (4 different) take off in the motion from the jump state with only the orientation off (F4b3f's
  same-tick spin update), 2 have the ground step off with no rider near (ticks 11535 rider 2, 12509 rider 1).
  `TRICKY_DBG=1` lists them.

What is left: the rider collisions after the motion in the take-off test, and the prediction made before a push
(row F4h4); the orientation and the jump control (F4b3f); the float rule (F4c).

## F4h4: the frame's rider collisions around the take-off (2026-11-09)

`takeoff_ticks_against_the_running_game` (`tests/ground_motion.rs`) now replays the frame's
`Boarder_RiderCollisions` 0x123fc8 around the rider that takes off (`FrameRun`, when the bodies are recorded): the
riders that tick before it in the frame (slot 5 down to 0, as F4b3g) push and hit it before its motion, so its
motion starts from there; after its take-off (motion 1, as `Boarder_SetMotion` left it) it and the riders after it
collide. The other riders are taken as `ground_ticks_against_the_running_game` takes those it does not replay (the
second sample's velocity and motion, air bodies moved by their step). The prediction check now also asks whether the
game's landing point lies on the flight from what `Boarder_SetMotion` saw (`predict_like`).

- **rec16 (Snow Dream):** whole take-off ticks under `fadd=g1` **221 of 235** (210 before); 13 pushed, 11 of them
  whole. The prediction from the take-off's own inputs: no-landing inputs 34 of 38, time to the top 235 of 235, the
  game's landing on the flight 194 of 197. So the 10 landings and 3 no-landing records of F4h3 that the flight test
  (made from the end of the tick) could not place are the pushes: from the inputs the motion saw they are the game's,
  except tick 7807 rider 2 (three times, after save states) below. Tick 6765 rider 0 of the flight test is no
  take-off: it is in the wipe-out motion (5, state 0x13) all along, its prediction re-made there; not looked at.
- **rec15:** 104 of 110 (102 before); 2 pushed, both whole; landing on the flight 91 of 91. rec11 has no bodies
  (54 of 62 as before).
- **Left on rec16** (10 different ticks, 14 with repeats):
  - 4 with only the orientation off (10397 r2, 10458 r1, 10578 r3, 7897 r5 ×3): take-offs in the motion from the
    jump state, F4b3f.
  - 5 with the ground step's velocity off by 0.002–1.2 cm/s before the take-off (the no-landing inputs show it):
    11535 r2, 6167 r1, 6335 r4 (pushed too), 7807 r2 ×3 in the bump state (2), 12509 r1 in 3 (rec15: 10316 r4,
    state 2). Proven: `GroundMotion_Update` calls `Boarder_WallCollide` 0x126250 and `Boarder_InstanceCollisions`
    0x125088, which this test answers with no wall and no object (not in the recording). Inferred: walls or
    objects; 11535 r2 has nobody within 2,359 cm and the ground test misses it the two ticks before too (3 → 2 at
    11533 with nobody near: a bump not from a rider), 7807 r2 likewise at 7803.
  - 7325 r4 (jump control, pushed by rider 1, 168 cm): ends 2 cm and 10 cm/s off. Rider 1 ticks after it and is
    taken from the second sample, after the hit; the ground test misses rider 1 at that tick too.

What is left: the walls and objects inside the ground motion in these tests (a recording of what the world answers,
or the course); the riders around a take-off replayed rather than taken from the second sample (row F4h5).

## F4b3f: the jump control and the spin state on the take-off tick (2026-11-09)

`JumpControl_Update` 0x1046b0 is ported (`control::jump`, read from its disassembly): with rider+0x208 (the
crouch) at 0 it calls `Jump_ApplyImpulse` 0x1284e0 with 630.8786 (0x441db83b), `Boarder_SetMotion`(1) and
`Takeoff_SetSpinRates` 0x126e30; else `Boarder_SetSteerTarget`(0); then the reset check (pad bit 0 or rider+0x290
set). `tests/control.rs` runs it against the game's on every case (`9 jump`, `9 steer`, `9 reset`), and the take-off
test now takes off by it through `ControlEnv` (two new calls: `jump_impulse`, `takeoff_spin_rates`).

The orientation-only take-offs of F4h4 are motion take-offs from the jump state (0xa). Proven:
`Boarder_TakeOff` 0x128470 in state 0xa calls `Takeoff_SetSpinRates`, which sets state 0x10 through
`Boarder_SetState` 0x11f608, whose `Boarder_EnterControlState` 0x11c8e8 runs `SpinControl_Enter` 0x100598; this
happens inside the motion, so the control that follows in `cBoarder::vf7` the same tick is already
`SpinState_Update` 0x100908, whose landing alignment turns the board (about 0.005 rad). A jump-control take-off
enters the spin state inside the control, so its first update is next tick. The test now runs, for motion
take-offs from state 0xa, `spin::spin_enter` and `spin::spin_update` (pad 0) on ours, and the game's 0x100598 and
0x100908 in the runner (`spin_on_takeoff_tick`; the clip type from rider−0x170's mode when recorded).

- **rec16:** whole take-off ticks under `fadd=g1` **227 of 235** (221 before); all 6 orientation-only ticks fixed.
- **rec15:** 108 of 110 (104). **rec11:** 56 of 62 (54; no −0x170 window, no bodies).
- Ours still equals the runner under IEEE on every field.
- **Left:** rec15 9803 r5 (motion take-off from 0xa): forward 1e-4 off in ours and in the runner, so an input
  of `SpinState_Update` the recording lacks (inferred: the rider's fields past +0x700, such as +0x46d4, +0x46e0,
  +0x4ab0, which the test leaves 0; `Takeoff_Setup` 0x12bee0 writes +0x46e0 for kinds 3 and 4 and is not
  replayed, STANDIN in `spin_in`). The rest are F4h5's (walls, objects, riders around) and rec11's missing windows.

## F4b3m: the time scale the control reads (2026-11-09)

The last take-off of F4b3f (rec15 tick 9803 rider 5, a motion take-off from the jump state) turned 1e-4 rad too
little toward the landing slope, in ours and in the runner. Its time scale (+0x12c) went from 0.9449985 in the
first sample to 0.95344484 in the second, and the turn of `SpinState_Update` 0x100908 is `align · dt / land` with
dt = time scale / 60, so it was 0.9% short.

Proven: the only per-tick writer of +0x12c is `Boarder_SetTimeScaleRateLimited` 0x11cff0 (at most 0.008446341 a
tick toward its target; the other store is the reset `cBoarder::vf8` 0x116ab8, 1.0). Its callers are
`cComputer::GetControls` 0x1384b8 and `Boarder_UpdateSlopeTimeScale` 0x153130 (from `cPlayer::GetControls`
0x151250), the controller virtual that `cBoarder::vf7` 0x117198 calls after the motion and before
`Boarder_UpdateControl` 0x11c9a0. So the motion reads the old time scale and the control the new one: the second
sample's.

- `takeoff_ticks_against_the_running_game` (`spin_in`) and `spin_ticks_against_the_running_game` now give the
  spin update the second sample's time scale (ours, and the runner's rider +0x12c).
- Take-off ticks whole under `fadd=g1`: rec15 **109 of 110** (108), rec16 227 of 235, rec11 56 of 62 (unchanged; the
  rest are F4h5's and rec11's missing windows). Ours still equals the runner under IEEE.
- Spin ticks (rec8) under `fadd=g1`: quaternion 5,090 (5,077), axes 5,088 (5,075), spin 5,059 (5,046), angle left
  5,091 (5,078) of 5,124. Under IEEE ours and the runner agree (quaternion 1,592, spin 4,518, angle left 4,211).

## F4b3l: the game's divide rounds to nearest (2026-11-09)

The EE manual says `div.s` rounds toward zero, and so did `animseq::div` (and `animseq::sqrt`) and the runner's
default rule. The running game does not:
- **Proven, rec15:** `AnimClip_GetPhase` 0x159e80 divides the sequence's time by the clip's length (`div.s`). In the
  182 bump ticks where that quotient is not exact, the bump control's turn (`BumpControl_Update` 0x1041b0, F4b3i)
  is the game's only with the divide to nearest: ours (toward zero) was 1 ulp off, the runner with `div=n` right.
- **Proven, F4g:** the air step's divides, 341 of 341 to nearest, 252 toward zero.
- **Inferred:** `sqrt.s` rounds as the divide (same unit; the runner's `div` rule has always covered both, and
  PCSX2 has one rounding setting for the two). No recording tells a square root apart yet.

So `animseq::div` is now `ps2f::F`'s divide (to nearest, the EE's clamps) and `animseq::sqrt` rounds to nearest.
Every port that uses them (the animation sequences, controller, data, modes, pose, curves; fog, lights, snowfall,
the lens effects, the shadow, the skin, the limb IK, the draws) now divides as the game does. Their 29 runner tests
ran the game's code under the runner's default `div=chop`; each now sets `div=n` (one line after its
`Runner::from_env()`), and all pass (before the change all passed under chop; with it and without `div=n` the 29
failed). The runner's default stays `div=chop` for the other tests (row F4b3n).

rec15 ground ticks: the controls run by ours on the runner's rider now give the runner's IEEE result in 1,075 of
1,075 (was 1,000); the quaternion is the game's in 877 under `fadd=g1` (876 before; the runner's 1,058: the rest is
PCSX2's adder, which ours doesn't use, F4c). Take-offs (rec16, rec15) and spin ticks (rec8) are unchanged.

## F4b3j: the pair list and the air riders' bodies (2026-11-09)

`tools/pine.py record` can now record the neighbour list a rider's collisions walk (`L700:78`: from the list head
rider+0x700, each node's address and its first 0x10 bytes, up to six nodes); `tests/recording/mod.rs` reads it back
(`pair_list`, `rider_addrs`). rec17 (Elysium Alps, 300 s, 17,976 ticks) has it. `ground_ticks_against_the_running_game`
now walks each replayed rider's pairs in that order (the second sample's list; `TRICKY_LIST=a` the first's,
`TRICKY_LIST=none` every other rider in slot order, as before and as recordings without the list do).

Proven (code):
- `Boarder_RiderCollisions` 0x123fc8 walks the list from rider+0x700: a node's +0 and +4 are the two riders' list
  heads, the next node is at node+8 + 4·side (side: node+4 is this rider's head, `Obj_HasId` 0x110838); the other
  rider is the object at the other head's +0x84 (id 0x3ef) less 0x82c.
- `AirMotion_Update` 0x108378 integrates the position (`Air_IntegrateRK4`) and never moves the body by the step: its
  only `Boarder_Translate` is the push out of the world after its ray cast. The bodies come from the bones in
  `Boarder_UpdateBroadphase` 0x126748, which `cPostAI::vf1` 0x114730 calls for every rider after all have ticked. The
  test moved an air rider's body by its step between the samples (F4b3g); it now leaves it as the first sample holds it
  (`TRICKY_AIRSTEP=1` restores the old way).

rec17 under `fadd=g1` (moving the air bodies in brackets): every field 71,209 (71,144) of 71,775; pushed ticks 874
(896), every field right 799 (768); bump records changed 472 (534), equal 450 (438). rec15: every field 37,289
(37,252), bumps 122 of 140 (120 of 184); rec16: 51,638 (51,622), bumps 374 of 420 (354 of 452). The list's order
barely counts: rec17 with the first sample's list 71,210, with every rider 71,203 (all with the bodies left).

What is left (row F4b3o, `TRICKY_DBG=1` lists them):
- 11 pairs of rec17 whose bump the game and the replay disagree on: ground pairs at ticks 6533 (1-4), 11132 (1-2),
  11818 (1-4), 2204 (2-4) the game's only, 10242 (1-4), 3881 (2-5), 664 (2-3) the replay's only; 1871 (0-3) with an
  air rider, the game's only; 10937, 11006, 13565 with a wiped-out rider (motion 5 or 6), the game's only. The recorded bodies do not explain them: at 6532 riders 1 and 4 are
  38 cm apart and the game bumps them in the next frame; from 10241 to 10244 they overlap by 3 to 5 cm and the game
  bumps only at 10243. The gap jumps between samples (11, 5, 38, -3, 2 cm at 6530 to 6534): the bones may not be
  posed every frame (`Boarder_UpdatePose` runs only when +0x46b0 is set, `cPostAI::vf1`), or the sample is read
  across a frame.
- 40 hit ticks with the velocity off: 27 with the position off too (the push vector), 6 by 1 to 4 ulps (ticks 9990,
  11002, 11126, 2329, 2332, 2335: the hit's arithmetic is ours under IEEE, not the game's under PCSX2's adder), 7 far
  with the position right (9055, 9808, 9987, 19130, 848, 1439, 2326).
- `FrameRun` (the take-off test) still walks every other rider, not the list.

## How a row gets to `checked`

1. Port the function (address in its doc comment, AGENTS.md §13).
2. Add a runner test like the one above, or a trace check for code the runner can't reach.
3. When it passes, set `checked` for that address in `tricky-rs/docs/function-index.csv` and commit; F4a's runner
   will later write these marks itself from the test results.
