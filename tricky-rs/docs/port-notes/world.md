# World objects: how the original works

Part of the port notes (index: [README.md](README.md)); units and wording as described there.

## World objects (`Boarder_InstanceCollisions` 0x125088)

- Crash bags / cans (cRollerNode, MainType 0 sub 0): launched with ½·approach speed along the rider's heading plus
  an upward pop min(100 + 5000/m, 600) cm/s, spin 10 rad/s; gravity −980; contacts e = 0, friction removes half
  the sliding speed, damping c·(1+2t); asleep under ~1 m/s or after 10 s where it lies. The rider loses no speed.
- Path markers and broken glass (cMeshAnimNode, sub 20): parts thrown with ½·s·v̂ + random (±2, ±2, +4 m/s),
  gravity 58800·U1 cm/s², no ground, gone after U2 s (markers 2 s). Markers have mass 0: the rider stumbles.
- Glass breaks on any contact (no threshold, no points); fences flex only. Pickups are used up for the race
  (DeadNode mode 2) by whoever touches them first, AI riders included; gems/points pickups only in show-off.
- HUD words (american.loc): "KNOCKDOWN!", "BIG AIR BONUS", chain "1x COMBO"…"4+ COMBO", race checkpoint
  "CHECKPOINT" + split, show-off "TIME BONUS" + "%d seconds", "FINAL LAP" / "%d LAPS TO GO", "FINISH", "TIME UP".

## World controllers (`TriggerScript_ExecOp`, `TriggerScript_Op0_SpawnController` 0x13c5d0)

- Controllers attach to instances (inst+0xe4); flags inst+0xe8: 0x4 animated, 0x80 bounces the rider, 0x800
  disabled. Script slots: 1 on collide, 3 on rest, 4 on timer/counter/break. Lifetimes are seconds × 60.
- Types: 0 Roller, 2 Debounce, 5 Dead, 6 Counter, 7 Boost, 8 Timer, 9 Rail (enable/disable a rail), 0xa UVScroll,
  0xb TexFlip, 0xc Fence, 0xd Flag, 0xe Cracked (health −= |v·n·k|·0.036 per hit, 30-tick cooldown), 0xf LapBoost,
  0x10 RandomBoost, 0x11 CrowdBox, 0x12 ZBoost, 0x13 UVScrollTexFlip, 0x14 MeshAnim, 0x15 TrickTrigger,
  0x16 Particle, 0x17 Movie (jumbotron), 0x18 TubeEndBoost, 0x100 AnimObject (keyframed model, loop once / wrap /
  ping-pong, speed/30, collides via a cube AABB), 0x101 AnimDelta (advances only while given delta), 0x102
  AnimCombo (loop + one-shot), 0x103 AnimTexFlip. Op 2: Emitter, SplinePath (vehicles/conveyors: N copies spaced
  along a spline, dist += v·100/60 a tick; mode 3 keeps 6000 units ahead of the race leader), CollideEmitter.
- Other ops: 1 camera anim, 3/9 SetParam, 4 wait, 5 conditions (rider speed in km/h, random, human, has
  controller), 6 points, 7 run another instance's script, 8 sound, 10 stop, 11 kill/restore, 0xd reset rider,
  0xe–0x12 pickups and boosts, 0x17 camera param, 0x18 teleport rider to an instance, 0x19 rail state.
- No hard-coded per-track hazards: everything moving comes from level data. Track ids: 0 Garibaldi, 1 Snowdream,
  2 Elysium, 3 Mesablanca, 4 Merqury, 5 Aloha, 6 Pipedream, 7 Untracked, 8 Tokyo Megaplex, 9 Big Air Dome,
  10 Trick Tutorial, 11 Alaska.

## Node virtual slots (all world nodes: `cBxObjNode` → `cBxTypeObjNode` → `cBxSortObjNode` → `cDeadNode` → …)

- vf0 the destructor (`cTimerNode::~cTimerNode`; sets the class vtable back, then the base's), vf1 `Update` (once a
  tick: scripts run their ops, timers and counters count down), vf2 `Draw` (hands the model to the renderer),
  vf7 `SaveSnapshot` (writes the node's state into the replay snapshot: a 0xdeadbXXX tag per class, then fields,
  through `Replay_Write` 0x192c10, `Replay_WriteNodeRef` 0x192f30 for node pointers as pool indices).
- vf21/vf22/vf23 are the controller messages (vf23(value, node, kind): set a parameter, e.g. a timer's seconds ×60;
  vf21/vf22 finish the node, then it deletes itself through vf0(3)); vf17 runs when a rider touches the object
  (30-tick cooldown on the animated ones). Not named yet.
- Restoring (`Replay_RestoreNode` 0x18f8a8): reads the saved type id (`Replay_PeekNodeType`), allocates the node
  under its tag (the class name without c/Node: "Timer", "UVScrollTexFlip", "Script" …) and runs that class's
  stream constructor (`cTimerNode::cTimerNode` 0x13fe50 …), which reads the fields back with `Replay_Read` 0x192ca0.
  Ids: 0 Roller, 2 Debounce, 6 Counter, 7 Boost, 8 Timer, 9 Rail, 0xa UVScroll, 0xb TexFlip, 0xc Fence, 0xd Flag,
  0xe Cracked, 0xf LapBoost, 0x10 RandomBoost, 0x11 CrowdBox, 0x12 ZBoost, 0x13 UVScrollTexFlip, 0x14 MeshAnim,
  0x15 TrickTrigger, 0x16 Particle, 0x17 Movie, 0x18 TubeEndBoost, 0x100 AnimObject, 0x101 AnimDelta,
  0x102 AnimCombo, 0x103 AnimTexFlip, 0x3e9 Script, 0x3ea SplinePath, 0x3eb Emitter, 0x3ec CollideEmitter,
  0x3ed RailMan, 0x3ee Restore.
- The rider does the same without a factory: `Boarder_SaveSnapshot` 0x117d20 / `Boarder_LoadSnapshot` 0x117ec0
  walk the anim controller, air prediction, score, then every motion and control object (`<Class>_SaveSnapshot`,
  each followed in memory by its `<Class>_LoadSnapshot`).

## Object collision (`World_RayCast` 0x25aff8, `World_RayCastInstance` 0x25bf48, `World_QueryInstances` 0x25b878, `Boarder_InstanceBounce` 0x125a00)

- Instance record (inst+0xec): +0 mass (U0), +4 PlayerBounceAmmount, +0xc SurfaceType, +0x10 collision mode
  (short), +0x12 collision model index; inst+0xe8 flags 0x20 PlayerCollision, 0x80 PlayerBounce.
- SurfaceType ≥ 0: the object is part of the ground ray (`Boarder_GroundProbe` and every world ray) exactly like a
  patch: the hit's surface row is the instance's SurfaceType (ramps 1, glass floors 5, metal frames/platforms 13,
  wood 12, cliff walls 10 ...). Such objects are never in the bounce test.
- SurfaceType −1: never ground. `World_QueryInstances` (flag 0x20 and SurfaceType −1) gives the contact
  (Instance_OnPlayerContact = its scripts) and, only with PlayerBounce, `Boarder_InstanceBounce`: mass 0 →
  `Boarder_Stumble` only (no push; megaplex spinners); else pushed out by 1.1·depth and v += n·max((1+b)·vin,
  vin + 55.56 cm/s) (b = PlayerBounceAmmount, usually 0.5), then `Boarder_WallImpactCheck`. Without PlayerBounce
  the object is not solid at all (glass panes and undersides, boost volumes, penguins, fan blades, icicles):
  only its touch scripts run. Here: −1 + bounce objects are walls on every face (tops bounce, never wipe out
  as "surface 6"); the bounce amount is the rider's fixed 0.5; mass-0 stumble not done.
- Mode 1 collision models have one sub-mesh per model part that has a mesh, in part order, each in that
  part's frame (posed by the part's matrix); mode 3 uses the drawn meshes; mode 2 is a box (not solid here yet:
  tree trunks, rail supports, crowd stands, pylons).
- Moving objects: `World_RayCastInstance` poses each part with `cAnimObjectNode_GetFrameMatrix` when the
  instance has an anim controller (flag 0x100 of controller+0x14), and QueryInstances recomputes the box, so the
  collision of every keyframed object follows its clip (flippers, BEntrance ramps, bumpers, pillars, the
  rotating door's reset zone, aloha's side-to-side barriers and boats, merqury's sewer fans, mesa's bridge).
  Clips a touch script starts (op 0 sub 256/258) play from the start; mode 0 ones end where they rest.
  Here: `CollisionWorld::movers` keeps their triangles per part and re-poses them when the clip moves (the
  game pushes the drawn clip's time; the sim runs its own clock and plays a clip when its trigger box is
  touched). The rider is pushed out / lifted, not carried sideways (the original adds no platform velocity).

## Keyframed objects (`cAnimObjectNode` 0x198810, `cMeshAnimFrame::vf1` 0x1cb498, `AnimCurve_FindSegment` 0x1cb770)

- Model objects with an Animation: base pose U1..U6 (tx, ty, tz cm; rx, ry, rz degrees), AnimationAction a mask
  of animated channels (bit i = channel i; entries in bit order), each a list of cubic segments
  ((V1 t + V2) t + V3) t + V4 over [V5, V6] with t absolute seconds, clamped at the ends. Animated channels
  replace the base; local = T · Rz · Ry · Rx; world = parent · local. The object's static matrix is unused.
- The node's clock: frames / 30 s; start U1, end U2 (−1: the model's AnimTime), rate U3 frames a second (30 =
  normal; random up to U4), U0 mode 0 once, 1 loop, 2 back and forth; U6 random start; U7 = 4 reversed.
  Fans, penguins, sailboats, side-to-side barriers, helicopters, the multiplier gems (which spin at 1.5× and
  are never removed: their touch script only sets the multiplier and bursts particles).

## World animations (`cTexFlipNode` 0x142e90, `cUVScrollNode` 0x141ff0, `cAnimDeltaNode` 0x199d10, `cFlagNode` 0x144938)

- All tick at 60 Hz; lifetime U×60 ticks (0 for ever), at 0 the controller dies (base texture back, scroll gone).
- TexFlip (11; update 0x1430b8): frames = the material's TextureFlipbook (by MaterialID); one frame for every
  flipbook mesh of the instance, count from the first. U0 start frame (< 0: ceil(8r); −2: a random frame
  each step), Direction (≠ 0 backwards), Speed frames/s (acc += Speed/60, step at ≥ 1; 0 holds), Length s, U4
  mode: 0 cycle with wrap; 1 blink: next frame for 6 ticks, then the next (wrapping to 0) for
  60/(Speed(0.25 + 0.75r)) ticks. SetParam (0x1433d8, ops 3 and 9): 1 rate a tick, 2 frame = ceil(v) for
  0 ≤ v < 9, 3 step (wrap to 1), 4 visible.
- Start lights: StartCountDown → CountDownStart: TexFlip U0 0 Speed 0 Length 7 on Mdl_StartLights, then frame
  1, 2, 3, 4 at 1.0, 1.5, 2.0, 2.5 s (GO) — off, red, yellow, yellow, green.
- UVScroll (10; update 0x1422e0): shared by every instance of the model. U1, U2 du, dv a tick; U3 run s, U4
  pause s (both ≤ 0: still; run 1 pause 0 = for ever), U5 life; U0 0 linear, 1 eased (du·min(τ, run−τ)/run),
  1 and 2 turn round after each run; offsets wrap into [−1, 1].
- AnimDelta (257; 0x199db8): a keyframed clip (the 256 fields) that only runs while its budget is positive;
  SetParam(2, N) adds N/30 s. Trees falling, sewer gates opening, Elysium's kicker.
- Flag (13; `cFlagNode_BuildStrip` 0x144ec8): the instance's one quad (v0..v3 in strip order) becomes 8 × 2:
  rows v0→v1 and v2→v3, column k at k/7. Phase random [0, 1), += U1/60 a tick (U1 −1: 0.5–2); column
  k = 1..7 moves U2·(k/8)·sin(2π(k/8 − phase)) along model X (U2 cm, divided by the instance scale; −1
  random); U0 = 1 mirrors (column 7 fixed). U3 life.
- Scripts run as threads: a wait (op 4) parks the thread; op 7 runs the other object's script at once.
  The collision slot runs only while the object has no controller: Debounce (2) keeps it busy D s (0 for good),
  a dead node for good.
