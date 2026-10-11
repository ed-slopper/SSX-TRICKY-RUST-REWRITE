# Level scripts: how the original works

Part of the port notes (index: [README.md](README.md)); units and wording as described there.

## Level scripts (SSFLogic.json; `TriggerScript_Run`, `World_CellActivate_RunPersistant` 0x25fed0)

- A script is a list of effects run as its own thread (instance, rider, contact) until its first wait (op 4,
  WaitTime seconds, at least a frame); a condition (op 5) returning false ends the thread. EffectSlots:
  PersistantEffectSlot when the object's area wakes (near a rider or the camera), Slot3 when it sleeps,
  CollisionEffectSlot on rider contact (only when the object has no controller; flag PlayerCollision and no
  surface type; one object per rider per frame, every frame), Slot4 when a controller stops, EffectTriggerSlot
  when a controller reaches its goal (counter at 0, timer, broken, trick points).
- Op 5 (type5: U0 kind, U1 int, U2 float): 0 speed (M0 ≤ T cm/s, M1 ≥ T km/h), 1 random (M0 passes with
  probability T), 2 human only, 3 stop if the object already has a controller (the usual "once" guard, then
  op 0 Debounce). Op 7 runs EffectHeaders[EffectIndex] on Instances[InstanceIndex]; op 21 runs
  Functions[FunctionRunIndex]; op 13 resets the rider; op 24 teleports; op 25 switches a rail.
- The engine fires named functions: RaceMode (modes 0, 2, 4, 7, 9), ShowoffMode (3, 5), FreerideMode (others) at
  the start; StartCountDown / NoCountDown; EndCountDown at GO. In the data they call HideShowOff (show-off rails
  off), HideRace (race-only objects dead), HideStartGate, CountDownStart (the lights).
- DeadNodeMode: 0 restore, 1 stop (Slot4), 2 hidden with no collision for good, 3 also stops spline riders,
  4 hidden until the area sleeps (broken logos come back).
- Boost (sub-type 7): Mode, U1 re-arm seconds, U2 rate per second, BoostAmount ×100 cm/s target, BoostDir world
  direction; while touching: d = target − v·dir, if d > 0: v += dir·d·rate/60 a tick.
- Spline riders (op 2 sub-type 1): SplineIndex, U1 mode (0 stop, 1 loop, 2 back and forth, 3 pace the leader 60 m
  ahead), U2 orientation, InstanceCount copies spaced evenly, AnimationSpeed m/s, U5 yaw offset, U6 draw the
  cable in (R, G, B) — the Merqury subway, Snowdream's ski lifts.
- MeshAnim (sub-type 20): breakable debris; U0 stays broken, U1 gravity scale (×980 cm/s²), U2 life, U3..U5 base
  velocity (0: the rider's), U6..U8 random spread, U9 multiplier. Cracked (14): U1 health, hits take
  |v·n| km/h (30-tick cooldown), U0 reset seconds. Fence (12): FlexAmmount bend, decaying zig-zag.

## Op and controller table (`TriggerScript_ExecOp` 0x13bfd0, `TriggerScript_Op0_SpawnController` 0x13c5d0; E9a)

Proven from the two switches. An op is a record whose word 0 is the op number; its arguments are words 2, 3, 4
(p2, p3, p4). "Controller" = the instance's node at instance+0xe4; "rider" = the thread's rider (thread+0xe8).
ExecOp returns 1 to go on, 0 to wait (op 4), −1 to end the thread (a failed op 5).

| op | what it does | handler |
|---|---|---|
| 0 | spawn a controller of type p2 on the instance (table below); the same type already running gets its vf 0x74 (restart); a running one of another type is stopped first (vf 0xc, 3); type 5 is never replaced | `TriggerScript_Op0_SpawnController` 0x13c5d0 |
| 1 | spawn a camera node | `TriggerOp_SpawnCameraNode` 0x13d0b8 |
| 2 | spawn an effect (sub-type 1: spline riders) | `TriggerOp_SpawnEffect` 0x13d130 |
| 3 | message the controller: its vf 0xc4(p2, p3 as a float in f12), if there is an instance and it has one | (inline) |
| 4 | wait p2 seconds (thread+0x40) | (inline) |
| 5 | condition p2, ending the thread when it holds: 0 speed (|rider+0x150| over four lanes, w too; p3 0: > p4 cm/s, 1: < p4 × 27.777777, km/h; no rider: passes), 1 random (r = `Rand_GameRange`(0, 1); p3 0: p4 < r, 1: r < p4), 2 not the human (rider+0x41c = 0), 3 the instance already has a controller; other kinds and modes pass | (inline) |
| 6 | points pickup p2 (show-off modes 3, 5 only): score+0x94 += p2, meter +0.04 | `Pickup_Points` 0x11e608 → `Score_AddPickupPoints` 0x155b10 |
| 7 | run effect p3 on instance p2 (a new script thread) | `cScriptNode::cScriptNode` |
| 8 | play script sound p2 at the instance | `Sfx_ScriptSoundPlay` 0x2165c8 |
| 9 | as 3, without the null check | (inline) |
| 10 | controller vf 0xb4(p2): p2 0 or 1 (on / off) | (inline) |
| 11 | controller vf 0xbc when p2 = 0 | (inline) |
| 12, 19, 20, 22 | nothing | |
| 13 | reset the rider (rider+0x304 = 1, `Boarder_RequestReset`) | |
| 14 | multiplier pickup p2 (2, 3, 5; show-off; only in the air or on a rail): the multiplier rises to p2 | `Pickup_Multiplier` 0x11e660 → `Score_SetPickupMultiplier` 0x155cd0 |
| 15 | meter pickup p2 (show-off) | `Pickup_Meter` 0x11e958 → `Boarder_AddMeter` |
| 16 | time pickup p2 seconds (1, 5, 10; show-off): race+0x14 += p2 | `Pickup_Time` 0x11e8a0 → `Score_AddPickupTime` 0x155bb8 |
| 17 | speed boost p2 seconds | `Boarder_AddSpeedBoostTime` 0x11e910 |
| 18 | spin boost p2 seconds | `Boarder_AddSpinBoostTime` 0x11e930 |
| 21 | run function p2 (a new thread with the rider) | `cScriptNode::cScriptNode` |
| 23 | shake the rider's camera (p3, p2 × the view's +0xcc) | `Boarder_GetCamera` 0x11ca88, `Cam_Shake` 0x177758 |
| 24 | teleport the rider to instance p2 | `Boarder_TeleportToInstance` 0x11a098 |
| 25 | switch rail p2 to state p3 | `RailMan_SetRail` 0x149030 |
| 26 | run function p2 with no rider | `cScriptNode::cScriptNode` |

Pickups ported (row E9c; `tricky_game::pickup`, checked: `tests/pickup.rs`). Score fields: +0x94 points from
pickups; counts +0x98/+0x9c/+0xa0 (500/2000/5000 points), +0xa4/+0xa8/+0xac (1/5/10 s), +0xb0/+0xb4/+0xb8 (x2/x3/x5);
`Score_Reset` clears them. Points: a 2.5 s message of kind 6, then `Boarder_AddMeter`(0.04). Time: a 2.5 s message
of kind 9 with the whole seconds, the seconds onto the race clock's limit (gApp+0x730 → +0x1c → +0x14), and
`Sfx_PickupTime` 0x219fc0 in every mode. Multiplier: counted for a rider whose +0x424 is 1 or 3; a higher one than
the score's +0x28 replaces it with a message of kind 12 (time −1); `Sfx_PickupMultiplier` 0x219d70 for the rider whose
+0x41c is 1, in every mode. Boosts (17, 18): the time left becomes the larger (`Boarder_AddSpeedBoostTime` 0x11e910
+0x134; `Boarder_AddSpinBoostTime` 0x11e930 +0x138, and +0x13c = 1); not limited to show-off.

| controller | class (constructor) | | controller | class (constructor) |
|---|---|---|---|---|
| 0 | `cRollerNode` 0x13dee8 | | 14 | `cCrackedNode` 0x145458 |
| 2 | `cDebounceNode` 0x13fc58 | | 15 | `cLapBoostNode` 0x140a98 |
| 5 | no node: `TriggerScript_ApplyControllerOp` 0x13ac88 (dead-node modes) | | 16 | `cRandomBoostNode` 0x1412e8 |
| 6 | `cCounterNode` 0x13b128 | | 17 | `cCrowdBoxNode` 0x145b28 |
| 7 | `cBoostNode` 0x140590 | | 18 | `cZBoostNode` 0x141be0 |
| 8 | `cTimerNode` 0x13fe50 | | 19 | `cUVScrollTexFlipNode` 0x146258 |
| 9 | `cRailNode` 0x1401a0 | | 20 | `cMeshAnimNode` 0x146e10 (also replaces any running controller) |
| 10 | `cUVScrollNode` 0x141ff0 | | 21 | `cTrickTriggerNode` 0x1479d0 (needs a rider) |
| 11 | `cTexFlipNode` 0x142e90 | | 22 | `cParticleNode` 0x148028 |
| 12 | `cFenceNode` 0x143870 | | 23 | `cMovieNode` 0x148ad0 |
| 13 | `cFlagNode` 0x144778 | | 24 | `cTubeEndBoostNode` 0x141800 |
| 0x100 | `cAnimObjectNode` 0x198810 | | 0x101 | `cAnimDeltaNode` 0x199d10 |
| 0x102 | `AnimComboNode_Init` 0x199f20 | | 0x103 | unnamed node (constructor 0x19a5a8) |

Types 1, 3 and 4 spawn nothing.

## Threads (row E9b; `tricky_game::script`, checked: `tests/script.rs`)

- A thread (`cScriptNode`, 0xf0 bytes, list type 0x3e9): +0x38 op index, +0x3c byte offset, +0x40 wait, +0xe0 the
  script (an effect header, `FUN_0025feb8`: file+0x28 + 8·i; or a function, `Course_GetScriptEntry` 0x260e68:
  file+0x30 + 0x18·i, neither bounds-checked above), +0xe4 the instance, +0xe8 the rider, +0x50 / +0x90 two 0x40-byte
  contexts with flags +0xd0 / +0xd4. The constructors run it at once (`TriggerScript_Run`), so a script with no wait
  is over before its constructor returns.
- `TriggerScript_Run` 0x13be78 (= `cScriptNode::Update` 0x13bd38): a positive wait loses 1/60 s (0.016666668) and
  holds while it is still ≥ 0, then is cleared; the ops run in order. A wait (op 4: +0x40 = p2's bits) steps past
  itself and returns, ending the thread instead when it was the last op; a failed op 5 ends the thread at once; the
  end of the ops ends it. Ending = its vf 0xc(3). So a wait of t seconds resumes on the first frame where
  t − n·(1/60) < 0 in single floats (1/60 s: two frames).
- Ops 7 / 21 pass the thread's rider and contexts to the new thread; op 26 none; op 7's instance comes from
  `Course_GetInstance`(p2) and nothing happens without one; ops 21 / 26 do nothing for p2 < 0.
- Starters: area wake → event 0 (objects with no controller; `TriggerScript_Spawn`), contact → event 1 (rider and
  contexts), controller goal → event 3, `World_RunScript` → a function by name after ending the first running
  thread on it. Events 0 and 1 take any slot value but −1; event 3 only ≥ 0.

## Logic controllers (row E9e; `tricky_game::controllers`, checked: `tests/controllers.rs`)

- A controller node: +0x14 its type, +0x28 the object (whose +0xe4 points back). Virtuals used by the scripts:
  Update (+0x10), Finish(flag) (+0xb4), Kill (+0xbc), OnMessage(value in f12, kind) (+0xc4), the deleting
  destructor (+0xc, 3; it detaches from the object). Finish(1) first tries the object's event-3 script and stops
  there if it ran one; Finish(0) never does.
- Object flags (+0xe8): the `.ssf` record's flags word; resets copy the upper half (defaults) into the lower and set
  2 (`m = f & mask; m | m >> 16 (arithmetic) | 2`): mask 0xffff0100 in Finish and type-5 mode 0, 0xffff0000 in Kill.
  Message kinds 7 / 8 clear / set 0x800; dead and restore nodes write `f & 0xffffff0d | 4` (0x800 too for mode 3).
- Debounce (2): `(int)(p3·60)` frames, then Finish(1); Kill and Finish delete it only while its count is ≥ 0.
- Counter (6): p3's low half = hits needed. Message values 1..10 are bits 0..9; kind 1 counts a bit once, kind 2
  counts value 1 or a bit whose lower neighbour is already set (in order), kind 3 always counts. At 0 hits it runs
  event 4 and Finish(0); else `(int)(p4·60)` frames to Finish(1).
- Timer (8): every frame +0x38 − 1; below 1 it runs event 4 and Finish(0) (then still counts +0x34 on the deleted
  node); +0x34 = `(int)(p3·60)` frames to Finish(1); message kind 1 sets +0x38 = `(int)(value·60)`.
- Trick trigger (21, needs a rider): records the rider's points (+0x5910) and +0x460; each frame walks the
  objects overlapping its box (+0x50 in the broadphase): for its rider, +0x424 = 1 waits; else points gained ≥ p4
  runs event 4 and forgets the rider; anything else Finish(1). With no rider, `(int)(p5·60)` frames to Finish(0).
- Op 0 replacing: a type-5 node is never replaced; the same type restarts (vf 0x74) only when its vf 0x64 says it is
  running (0 for these four); another type deletes the old first (type 20 takes the old over and deletes it after).

## Boosts (row E9f; `tricky_game::boosts`, checked: `tests/boosts.rs`)

- Op 0 words (op+0xc on): plain / random / tube end (`cBoostNode` 0x140498): mode (half; 1 = always on), re-arm
  seconds (`(int)(·60)` frames, +0x58; mode 1 starts armed, +0x5c), rate (+0x54), target (×100, +0x50), direction
  (+0x40, w 0). Random adds a second corner (op+0x28 → +0x110); tube end three aim points (op+0x28, +0x34, +0x40 →
  +0x110..) and three targets (op+0x4c.. ×100 → +0x1a0). Z (`cZBoostNode` 0x141af0): rate, target, direction, the
  height (op+0x20 → +0x5c) and the snap distance (op+0x24 → +0x60).
- Update (0x140688): on while mode 0 or the re-arm count (decremented first) is still > 0; the overlapping riders
  are walked when mode 1 or the count is out, and each one the object's mesh hits (`WorldNode_RayCast`) is pushed:
  d = target − v·dir (four lanes, VU0 order); d > 0: v += dir·(d·rate·1/60). Pushing keeps it on; otherwise
  Finish(1). Message kind 1 re-arms.
- Random: a rider's direction is picked once — the box's middle plus `Rand_GameRange(−h, h)` per axis where the
  half-size h > 0.005, normalised — and forgotten when the boost is idle and the rider was not pushed that frame.
  Tube end: the direction is aim point n normalised and the target n, n from 0x31d338[rider] (set by the lap boost).
  A zero direction normalises to 0 (VU0's 1/√0 is the largest float).
- Z: riders the mesh hits are marked; each marked rider below the height is lifted: within the snap distance its z
  is set to the height, else (re-placed first when on a rail, +0x424 = 5) it is pushed and its x, y velocity
  zeroed; a marked rider at or above the height is unmarked. No marked rider below: Finish(1).
- Not modelled: VU0's clamping on overflow (only reached with rates and targets far beyond the courses').
- Lap boost (row E9f2; `boosts::LapBoost`, checked: `tests/lapboost.rs`), Megaplex's gates: rate p3, target p4 ×100,
  direction p5..p7; at creation each rider's lap (+0x114) is noted (+0x58, halves). Each frame, for every rider
  overlapping the box (no mesh test): the noted count drops by one while it is ≥ the rider's lap; below 0 the rider
  is left alone. On its first frame in, the rider's side is written to 0x31d338 for the tube-end boosts: 0 when it is
  within 10 m of the box's floor, else 2, or 1 when it is behind the gate's centre along the object's first matrix
  row (x, y only). Motion 6: velocity × 0.95, nothing more. Motion 5 (rail): x, y velocity × 0.95 and the rail object
  gets velocity / its +0x568 (+0x5b0) and the velocity (+0x5d0). Then, within 10 m of the box's centre (x, y): on the
  first frame in, moved 20 toward the centre and 10 up; from then on (also when farther out but already in) x, y
  velocity × 0.92 and the push. Riders not overlapping are marked out. Kill does nothing; Finish(1) only runs the
  object's event-3 script, the node stays.

## Movers (row E9h; `controllers::RailNode`, `controllers::Roller`, checked: `tests/movers.rs`)

- Rail node (9): p3 (a half) the spline segment, p4 ≠ 0 on, p5 seconds (`(int)(·60)` frames, then Finish(1)). The
  segment's flags word (+0x18; the `.ssf` word with its defaults in the upper half): bit 0 = the rail is live;
  messages 0 / 1 switch it; deleting the node copies the defaults back (and restores the object's flags, mask
  0xffff0100). The constructor sets the object's flags `& 0xffffffdd | 0x44`.
- Roller (0, knockable objects): asleep while +0x140 is set (vf 0x64 reads it, 0x6c sets it, 0x74 clears it —
  op 0 on an awake roller of the same type restarts it). Each awake frame: energy = v·p + ω·L (VU0 order), the
  average +0xc8 = 0.95·avg + 0.05·energy / mass (mass −1 when the inverse mass is 0); 1/60 s more awake. Average
  < 10000 or 10 s awake: momenta and velocities zeroed and Finish(1) (the object's event-3 script, else sleep).
  Else the knock +0xe8 decays × 0.8333333 and s = clamp(knock·10, 0, 2); k = 2·awake·s + s; force = (0, 0,
  −980 / inverse mass) − k·momentum, torque = −k·L, handed to `RigidBody_Integrate`(1/60) — the body, its
  transform and `RollerNode_Collide` are row E8c. Kill deletes it; Finish never does.

## Breakables (row E9g; `tricky_game::breakables`, checked: `tests/breakables.rs`)

- A contact (the thread's context): +0x10 a, +0x20 b (four lanes each), +0x30 a speed.
- Cracked (14): p3 seconds to live (`(int)(·60)`), p4 health; built with the thread's contact applied at once; the
  object's flags `& ~2 | 4`; its texture flip set up. A hit (vtable +0x90) counts when the 30-frame cooldown is
  over: health −= |a·b · speed| · 0.036 (0x3d1374bd), cooldown 30. Update: cooldown down; the time to live down (at
  0: Finish(1)); while health ≤ 0, the object's event-4 script is started every frame (no script: the node gives way
  to a dead node). Messages: kind 1 time to live, kind 2 health; no base messages. Kill / Finish restore the flags.
- Fence (12): swing size (≤ 1) and speed (a tenth of the size, keeping its direction). Built from a contact: swing
  b·a (no shape) or the object's normalised first matrix row ·a, × (|speed| · 0.001), passed to both neighbours at
  once. A hit while the size is < 1: b·a × (speed · 0.001), passing restarted (4 frames). Passing to a neighbour:
  0.4 × size against the speed's direction, when ≥ 0.25: a free neighbour gets a fence node (which passes it on to
  both sides in 4 frames), a fence neighbour gets `Fence_Swing`. Update: angle += speed; at either end it bounces at
  half the size; at rest (size and |angle| < 0.01) Finish(1). The mesh bending is drawing (`FenceNode_UpdateShape`).

## Texture animation (row E9i; `tricky_game::texanim`, checked: `tests/texanim.rs`)

- UV scroll (10): op words mode, du, dv, run (s), pause (s), life (s). Each frame the clock gains 1/60; moving for
  `run` seconds then (with a pause) still for `pause`; modes 1 and 2 reverse the step at each turn, mode 1 also eases
  (the step × the time from the nearer end / run). The offsets wrap at ±1. Messages 1–6 set step, run / pause
  (0..60) and the offsets (−4..4). Contacts run the object's contact script with a 30-frame cooldown. Each scrolled
  model takes a slot of the 128-entry registry 0x31d938 (model, users) — the first free slot, unless its model is
  there already.
- Flip-book (11): op words start frame (below 0: one of the first eight at random; −2 also random at each step),
  backwards, rate (frames a second), life, mode. Mode 0 steps forward or back (wrapping), modes 1 / 2 flicker: a step
  at 10 a second, then one at the base rate × (0.25 + 0.75·random). `TexFlip_SetFrame` clamps to the last frame; a
  negative frame there is random (1 for three frames or fewer). Messages: 1 rate, 2 frame (rounded up, 0..9), 3 next
  frame (wrapping to 1), 4 shown. The random number is `Rand_Fx`'s word made a float in [0, 1).
- Both (19): the scroll and a flip-book from op words 8–11 (start, rate, mode, backwards), the flip stepped first.

## Object animation (row E9i2; `tricky_game::objanim`, checked: `tests/objanim.rs`)

- Flag (13): op words mirror, rate (cycles a second; a word of −60 is random 0.5..2), size (−1: random 5..50, then
  over 250 × the model's length, at least 1), life; the phase starts random (`Rand_Fx`) and wraps at 1.
- Keyframed (0x100 and the three built on it): op words loop (0 once, 1 loop, 2 back and forth), start and end
  frames (end < 0: the model's), speed (frames a second; its jitter uses `Rand_GameRange`(−1, 1)), contact re-arm (s),
  random start (`Rand_GameRange`(start, end)), direction (4 backwards). The files count 30 frames a second; the
  cursor moves rate = speed / 30 / 60 a frame, wrapping or bouncing at the ends; a one-shot clip ends with
  Finish(1). Message 1 delays (s), 4 jumps to a frame inside the clip. A contact while not cooling down runs the
  object's contact script and re-arms the cooldown.
- Delta (0x101): not playing by itself; message 2 adds frames to play (at |rate|).
- Combo (0x102): message 3 starts a second clip (op words 9..11: start, end, speed) from its start; op word 12 ≠ 0
  stops the node for good at its end (Kill then leaves it).
- Flip (0x103): the clip plus a flip-book (op words 9, 10: start frame, rate), stepping forward or at random (−2).

## Crowds, particles, jumbotron (row E9j; `tricky_game::scenery`, checked: `tests/scenery.rs`)

- Crowd box (17): op words 2, 3 rows and columns. 16 cells, each a phase (random 0..15) and an animation set (random
  0..3; the four 16-frame sets at 0x31d378). Every fifth race frame the excitement is set: 7 when the race's +0x1c
  is 1, else from the human rider within 200 m (`CrowdSys_GetBoxState` + 4, & 7), else 0. While excited, each cell's
  phase advances; at each cycle's end it takes a new random set and, below 7, sits out up to 63 frames with the
  chance 0x31d478[excitement]/256 (256, 0, 50, 100, 200, 100, 50). A cell shows set[(phase + cell) & 15], or the
  set's first frame while sitting out.
- Particle node (22): the op's effect's lifetime; a running emitter (< 0) steps every frame until message 1 gives
  it a lifetime; a lifetime counts down 1/60 a frame and at 0 the node finishes (Finish(1)).
- Movie (23): opens `data/video/j<course>.sss` (jgari, jsnow, jelysium, jmesa, jmerquer, jaloha, jpipe, jmegaple,
  jalaska; `jumbo.sss` otherwise) at 128 × 64.
