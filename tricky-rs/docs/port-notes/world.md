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
- vf21 `Finish`(runScript): unless the instance has a "finished" event script to run (`Controller_RunFinishScript`
  0x13a9f8, event 3), marks the instance done (flag 2) and deletes the node (vf0 with 3). vf22 `Kill`: the same
  without the script. vf23 `OnMessage`(value, kind): the base handles 7 (clear instance flag 0x800) and 8 (set it);
  classes add their own (a timer's 1 sets its seconds ×60, a counter's 1/2 tick bits off). vf17 `OnContact` on the
  animated ones runs the instance's contact script (event 1, `Controller_RunContactScript` 0x13aaf8) with a 30-tick
  cooldown. Event scripts come from `Instance_GetEventScript` 0x25fe78 (instance+0xec → +0x14 → 7 per instance:
  1 contact, 3 finished, 4 expired); each runs as a new `cScriptNode` (0x13b8c0).
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

## World queries (row E7a; `tricky_game::worldline`, checked: `tests/worldline.rs`)

- A cast takes a query object (vtable +0x08 into an instance's frame, +0x10 back, +0x18 its target t, +0x20 reject
  a hit, +0x28 triangle, +0x30 triangle from either side, +0x38 sphere tree, +0x40 box, +0x48 patch through the
  patch cache, +0x50 a patch's surface point). The shapes: `cWorldIntersectType` (base: no reject, patches),
  `cWorldLine` 0x3a7aa8 (the ground probe's and most others), `cWorldSphere` 0x3a8550, `cWorldSphTree` 0x3a85f8,
  `cWorldBoarder` 0x3a77d8.
- `cWorldLine` (0xa0): kind, in-frame flag, the ends' box and its middle, start / direction (in use and in the
  world), target. Triangle: t = ((a − start) · n) / (dir · n) unless |dir · n| < 1e-10 (t is written even when
  outside 0..1); for 0 ≤ t ≤ 1 the point and n, and the answer is `Tri_ContainsPointVu0`. Either side: n turned
  when dir · n < 0. Box: slabs from t = −1..2; the way in unless it is before 0 or (the way out is within 1 and
  nearer the target), then the way out if before 1; with the face's normal (0x3d2d10, filled on first use).
- `World_RayCastMeshTris` 0x25d3d8: an `.ssf` collision mesh (one-sided, or either side with its +0x1c bit 0),
  the hit nearest the target, first hit when the target is below 0 — but each hit is compared with the previous
  hit's t (the game updates its best on every hit), not the best so far.
- `Tri_ContainsPointVu0` 0x23b6b0 runs the VU0 micro program at 0x570 (the 215-instruction upload in the chain
  at 0x31cac0, `App_SendVu0InitChain`): with e1 = b − a, e2 = c − a, w = p − a, n = e1 × e2, d = e1 · (n × e2)
  beyond ±1e-10, u = w · (n × e2) / d, v = n · (w × e1) / d: inside when u ≥ 0, v ≥ 0, u < 1, u + v < 1 (sign
  flags, each read four cycles after its result). Its tolerance argument is unused. Read from the code, not run
  (the runner has no micro mode); the test answers it with the port and checks its arguments.

## The rider's shape (row E7a3; `tricky_game::worldboarder`, checked: `tests/worldboarder.rs`)

- `cWorldBoarder` (0x300 bytes, vtable 0x3a77d8), made from the rider's collision body (rider+0x470): kind 1 when
  made with flag 0, else 0 (`Boarder_WallCollide` and `Boarder_InstanceCollisions` pass 1: kind 0, so the coarse
  patch quads and a target of −1, the first hit); +0x10 / +0x20 the body's active bounds (`CollBody_GetActiveBounds`:
  the spheres the mask +0x24 names, or the bounding sphere when it is 0), +0x30 their middle; +0x50 the body in use
  (a copy put through the object part's frame at +0x60, `CollBody_Copy` / `CollBody_Transform`). Target: twice
  the body's radius for kind 1. Reject (+0x2d0 set, the walls do with the ground normal at +0x2e0): a hit whose
  normal · that > 0.8 (floors, for the walls). Front faces only when +0x2f0 is set (the object query sets it).
- A hit's t is its depth: the test gives a penetration vector, t = |v| and the normal v·(1/t); the point is where
  the sphere meets the plane.
- `Collide_SphereTriangle` 0x236bb8 (and its front-only 0x237480: 0 ≤ d; either-side 0x237d50: the normal turned
  to the sphere): the bounding sphere's distance d to the plane within its radius, its foot inside the triangle
  (`Tri_ContainsPointVu0`) or, when the centre is at least r from a, an edge (ab, ac, bc as segments) within r.
  Without a mask the penetration is n·(r − d); with one, each named sphere (d below its radius and the best) the
  same way, the shallowest kept.
- `Collide_SphereBox` 0x23b6e8: per axis whose other two lanes lie inside, the depths (c − lo) + r and (hi − c) + r
  (not negative), the least kept with its face's normal and the centre moved onto that face. `Collide_BoarderBox`
  0x238640: the bounding sphere's; with a mask the game tests the bounding sphere again for each named sphere
  (a slip in the game: same answer, or none if the mask names no sphere).

## Sphere trees in the world (row E7a4; `tricky_game::sphtree`, checked: `tests/sphtree.rs`)

- A tree in the world (0x150 bytes, `SphereTree::SphereTree` 0x239ba8): +0x10 the eight children's directions, from
  the table 0x3439d0 (±1 per axis, x the index's high bit; zero in the file, filled by start-up code 0x23bab0),
  +0x90 its centre (w 1), +0xa0 its header (+0xd0: `SphTree_Load`). `SphTree_SetTransform` 0x239d60 puts
  directions 0..3 through a matrix (4..7 their opposites) and the file's centre.
- A packed tree (header +8 set) has its occupancy bytes run-length coded; `SphereTreeCache_Get` 0x238f50 decodes
  them into a round-robin cache with `SphTree_RleDecode` 0x239b20 (a count byte c: c < 0 copies −c bytes, c > 0
  repeats the next byte c + 1 times, 0 ends).
- `SphTree_SphereRecurse` 0x23a728(r, c, node, level, index): R = r + the level's radius; if |c − node|² < R²: below
  the depth with an occupied node, each child k (node + direction·the next level's half-size, index (k + 1)·the
  level's stride + index) until one answers; else a leaf: penetration unit(c − node)·(R − |c − node|), contact
  c − n·(r − that).
- `Collide_BoarderSphTree` 0x2394a8: the body's bounding sphere, then each sphere (all of them, whatever the mask):
  the first that touches (when the bounding one touches but no sphere does, no hit).

## The rollers' shape (row E7a5; `sphtree::WorldSphTree`, checked: `tests/sphtree_shape.rs`)

- `cWorldSphTree` (vtable 0x3a85f8; `RollerNode_Collide` and 0x13f330 make them): kind 0 (coarse patch quads, target
  -1), box the tree's centre +- its top radius, the tree in use at +0x50 (a copy put through a frame at +0x60,
  `SphTree_Transform` 0x239f88: the directions and centre as they are now).
- Triangle (`SphTree_IntersectTriangle` 0x23a390 / `SphTree_TriangleRecurse` 0x23ada0): the children tried in an
  order from `SphTree_SortOctants` 0x23b398 (keys (int)(direction . -1000n), opposites negated, sorted largest first
  by `Sort_ShellPairs` 0x2cfd90, a shell sort with gaps 1, 4, 13, ...); best depth from 0 (only penetrations); the
  root skipped when behind the plane; a node goes on when its foot is inside the triangle or (not a leaf, radius 20
  or more) corner a is inside it or an edge passes through it; a leaf (at the depth, past level 2 or childless)
  keeps d - r when deeper. Either side (`SphTree_TouchesTriangle` 0x2395c0): within the top radius of the plane,
  the normal turned to the centre.
- Box (`SphTree_IntersectBox` / `SphTree_BoxRecurse` 0x23a9a0): `Collide_SphereBox` per node, the first leaf.
- Tree against tree (`SphTree_IntersectTree` / `SphTree_TreeRecurse` 0x23a570): no test at the root; each child
  as a sphere of this level radius against the other tree (`SphTree_SphereRecurse`), recursing where it touches;
  a node with no children, at the depth or past level 3 answers yes (keeping whatever was last written).
- `cWorldSphere` (0x3a8550) is never made by the game (no constructor, nothing points at its vtable).

## World casts (row E7a2; `tricky_game::world`, checked: `tests/world.rs`)

- The course object at 0x346488: +0 the `.ltg` grid, +4 the `.pbd`, +8 / +0xc material blocks and flips, +0x10 its
  vtable (0x3a7d50), +0x14 the `.ssf`, +0x18 sphere-tree groups (8 bytes: first tree, count; trees 0x150 apart),
  +0x20 collision groups (first mesh record, count), +0x24 the mesh records (0x14: tris, verts, three pointers).
- `World_RayCast` 0x25aff8(course, query, result, last): cells from (middle − corner + size) / size − 2 to + 0 on x
  (outer) and y (inner), clamped; each cell whose box meets the query's box, each of its +0x48 INodes with patches
  (+0x24) or objects (+0x26; +0x28 in show-off modes 3 / 5, `DAT_0032de8c`) whose box meets it. Patches: skipped
  when show-off only (+0x1ac < 0) outside those modes or surface 17, else box test, `World_LookupCollCache` and
  the shape's +0x48; a hit notes (cell, INode, patch) in `last` (+0x10 / +0x12 / +0x14 halves) when given. Objects:
  flag 0x20 and surface not −1, box test (+0xcc / +0xd8), `World_RayCastInstance`. A hit nearer the target than
  the best and not rejected (+0x20, given the normal) is written: +0 kind (0 ground), +0x10 point, +0x20 normal,
  +0x30 sphere tree, +0x34 surface, +0x38 object; a target below 0 takes the first. −1 for none.
- `World_RayCastInstance` 0x25bf48: collision mode 1 (meshes), 2 (each part's mesh box, mesh +4 / +0x10) or 3
  (sphere trees). Parts posed as `cMeshAnimNode` does (the animation's frames for animated parts when the
  controller's +0x14 has 0x100), each times the object's matrix; the query goes into the inverse (3 × 3
  transposed in place, position row = the transposed matrix applied to (−t, 0), w 1), popping before each part
  after the first; mesh n of the group per part with a mesh. Hits go back through the part's matrix (point and
  normal, all four lanes). Kind 1 with surface −1 for touch objects, else 2 for animated parts, 0.
- `World_QueryInstances` 0x25b878: the same walk over INodes not blank (words +0x24, +0x28, half +0x2c, word +0x48
  all 0), touch objects (flag 0x20, surface −1); an animated one's box becomes a cube about its position from
  the least and greatest offsets of its box; the first object `World_RayCastInstance` hits.
- An object's flags (+0xe8) are the physics record's word reset (defaults copied down); its box (+0xcc..+0xe0) is
  zero in the file and filled at run time by code not yet found (row E7a3; the test gives each a box).

## Terrain collision (row E7c1; `tricky_game::terrain`, checked: `tests/terrain.rs`)

- The patch cache (`CollCache_Init` 0x251ef8, `CacheBuffer_Lookup` 0x251f50 via `World_LookupCollCache` 0x25c750):
  entries of 0x1cd0 bytes, the least-used one evicted; a new entry is cleared (`CacheBuffer_ClearEntry`: 1e10 in
  every quad record's normals and box) and filled by the vector unit: `FUN_002576a8` loads the patch's 16
  coefficient vectors (+0x50) into vf1..vf16 and runs VU0 micro address 0; `Vu0_CopyOutData` copies the 100 points
  from VU0 data qword 32 to entry +0x10.
- The patch program (in the 215-instruction upload, chain 0x31cac0; its basis table (t³, t², t, 1) for t = k/9 at
  data qwords 16..25 comes from the chain at 0x31d1d0): for each row i, C_k = vf(4k+1)·Bᵢ.x + … + vf(4k+4)·Bᵢ.w,
  then point (i, j) = C₀·Bⱼ.x + C₁·Bⱼ.y + C₂·Bⱼ.z + C₃·Bⱼ.w (x, y, z; w 1): the patch's polynomial at s = j/9,
  t = i/9.
- The patch test (`cWorldIntersectType` +0x48, 0x25c7c8): kind 0 queries use the 3 × 3 coarse quads (every third
  point, records at +0x1a90), others the 9 × 9 fine ones (+0x650); each 0x40 record: triangle A's normal
  unit((p11 − p10) × (p10 − p01)), B's unit((p01 − p10) × (p10 − p00)), the box (made when first needed). The
  caller's last-hit record (+0 patch, +4 / +6 quad, +8 triangle A, +0xc kind) names a quad to try first (its
  triangle only, no box test); a miss clears the record (+0x10..+0x14 halves −1). Then every quad in order, box
  test, triangle A (p11, p10, p01) then B (p01, p10, p00) by the shape's +0x28: the **first** hit is taken and
  noted; a kind-2 query (the ground probe) then refines it with its +0x50 at the quad's middle (row E7c2).
- `cWorldLine` +0x50 (0x25df50, row E7c2; `WorldLine::refine`): from the quad's middle (u, v), P(u, v) = Σₖ Gₖ·Vₖ
  with Gₖ = coefficient rows 4k..4k+3 combined by (u³, u², u, 1) and V = (v³, v², v, 1); ∂P/∂u by (3u², 2u, 1, 0),
  ∂P/∂v by (3v², 2v, 1, 0) (each 3u² as (u·3)·u). Four Newton steps on P − λ·dir = start: with a = ∂P/∂u, b = ∂P/∂v,
  c = −dir, the nine cofactors over det = (aₓ(b_y c_z − b_z c_y) + bₓ(c_y a_z − c_z a_y)) + cₓ(a_y b_z − a_z b_y),
  du = (m₀rₓ + m₃r_y) + m₆r_z and dv = (m₁rₓ + m₄r_y) + m₇r_z for r = start − P. A step taking u or v out of 0..1
  stops with the hit as it was; after the fourth the hit's point becomes P (w 1) and its normal
  unit(∂P/∂v × ∂P/∂u); its t is not changed. A degenerate patch's determinant can be 0: the PS2's x/0 = MAX and
  clamped sums keep it finite (`ps2f::F`).

## Walls and objects (row E8b; `tricky_game::walls`, checked: `tests/walls.rs`)

- `Boarder_WallCollide` 0x126250: `World_RayCast` with `cWorldBoarder`(rider+0x470, 1) given the ground normal
  (shape +0x2e0). For t > 0: a normal facing into the ground (n · ground < 0) is made n − (n · g)·g, unit; then
  `Boarder_AccumulateImpact`, a push of (n·1.1)·t (`Boarder_Translate`, then rider+0x140 += it). With
  v_in = −n · v ≥ 0: s = max(v_in/2, 55.555553), v += (v_in + s)·n, `Boarder_TurnFromWall` in mode 2 (+0x424), the
  impact check at s with the record (hit point, unit v, n, v_in); the object's collision sound when the hit was
  one, the wall sound; true. Else false.
- `Boarder_WallImpactCheck` 0x126540(speed, rider, record, n): d = up (+0x4ac0) · n. Mode 1 under 833.3333 cm/s:
  +0x300 += (2.5 − d)·max(n.z, 0.2). Not mode 5: speed > 833.3333·d + 1944.4443 → `Boarder_Wipeout`(rider,
  record), `Score_Crash` → `Boarder_AddMeter`; else (not d > 0 in mode 1) speed > 833.3333·d → `Boarder_Stumble`
  along unit(n − (n · up)·up).
- `Boarder_InstanceCollisions` 0x125088: `World_QueryInstances` with the same shape; on t ≥ 0 the normal is
  turned off the ground in mode 2 (always), the contact record (point, unit v, n, v_in, unit v) goes to
  `Instance_OnPlayerContact`, a bouncy object (flag 0x80) to `Boarder_InstanceBounce`, then its sound. Then the
  rollers (rider+0x700 list: sphere trees and rigid-body impulses; Knockable objects, row E8c2).
- `Boarder_InstanceBounce` 0x125a00(t, rider, object, contact, record), not in control state 0x16: mass 0 →
  stumble along n only. Else a fixed object (no controller, or its +0x54 says so) pushes the rider by n·(1.1·t)
  and bounces at max(b·v_in + v_in, v_in + 55.555553) (b the record's bounce; wall turn in mode 2); a moving one
  shares it: k = 1/(speed stat/mass + 1), a = v_in·k, s = √max(a² − (1 − b)·k, 0) − a. Then the rider's virtual
  +0x6c (0.0012·v_in), v += s·n, the impact check at s with the contact record.

## Live cells (row E7b; `tricky_game::cells`, checked: `tests/cells.rs`)

- The AI world (`cAIWorld`) keeps the grid cells (TGroups) around the riders live. It has 108 slots at +0xc
  (8 bytes each: the TGroup, +4 a short counting ticks since it was last marked, +6 a short "new") and their grid
  indices at +0x36c.
- `cAIWorld::vf1` 0x115e28, each tick: the patch cache ages (`FUN_00251d70`, only speed). Each rider's grid cell
  (+0x16c) comes from its position (+0x140) through `Boarder_UpdateGridCell` 0x11d438: (p − corner + size)/size − 1
  per axis, truncated and clamped. If the first rider is still in the cell it had (+0x42c), the race object's +0x1c
  decides: below 2 the camera point (`FUN_00173c18`: camera +0x42c, or the point at 0x3368b0 when 0x337774 is set)
  is marked too and the pass goes on; otherwise the other riders are checked in turn, and if none changed cell the
  tick stops there.
- The pass: every held slot ages (`FUN_001162c0`, +4 less 1), `AIWorld_MarkCellsAround` 0x1160f8 marks the cells
  around each rider (and the camera point): x from cell − 2 to cell (x outer), y the same, with cell computed
  without the − 1, so the 3 × 3 block is centred on the rider.s cell; each clamped.
  `AIWorld_MarkCellActive` 0x116420 ignores index 0xffff and empty cells; a held cell gets +4 = 1, a new one the
  first free slot with +4 = 1 and +6 = 1 (the 108 slots never fill on the disc's courses; a full table drops it).
- `AIWorld_UpdateActiveCells` 0x1162f8: a new slot's cell is turned on and +6 cleared; otherwise a slot with +4
  below 1 is turned off and freed (index −1). `AIWorld_ReleaseActiveCells` 0x1163a0 turns off and frees every one.
- The objects a cell turns on or off: each INode (+0x38, 0x4c bytes) whose show-off count (+0x28) is not 0, its first
  +0x26 objects, or +0x28 in show-off modes (0x32de8c = 3 or 5). An INode with only the show-off count 0 is skipped
  even outside show-off.
- `World_CellActivate_RunPersistant` 0x25fed0: flag 0x100 on (+0xe8). Without a controller (+0xe4) its event-0 script
  starts (`Instance_GetEventScript` 0x25fe78: physics record +0x14 a slot, the .ssf's +0x10 table of 0x1c-byte
  records, −1 for none; `TriggerScript_Spawn` 0x13d538). With one, the controller's virtual +0x64 and then +0x7c
  are asked, and if both say yes its +0x74 runs.
- `World_CellDeactivate` 0x260108: flag 0x100 off. A controller of type 5 (+0x14) is left alone; otherwise the
  event-2 script starts, or, with no such script, the controller's +0xbc (kill) runs.
- Checked in the runner on 400 made-up grids (empty cells, 1 to 16 INodes per cell, objects with and without
  controllers of each kind and scripts), 3,200 steps of marking, ageing, updating, releasing and whole ticks with
  1, 2 or 4 riders: every script started and controller call in order, the flags, the slots and the riders' cells.

## Rigid bodies (row E8c1; `tricky_game::rigidbody`, checked: `tests/rigidbody.rs`)

- `cRigidBody` (a roller node's +0x30): +8 the inverse mass, +0xc the world inverse inertia (3 × 3 column by column:
  ωx = (i0·Lx + i3·Ly) + i6·Lz), +0x30 the position, +0x40 the orientation (w first), +0x50 the momentum, +0x60 the
  angular momentum, +0x70 the velocity, +0x80 the spin. The momenta are the state; v and ω are derived.
- `RigidBody_SetPosition` 0x153808 sets the spin, not the position: ω stored, L = (I⁻¹)⁻¹·ω by cofactors.
- `RigidBody_UpdateVelocity` 0x153ad8: v = p·(1/m), ω = I⁻¹·L; |ω| above 23.561945 rad/s scales ω and L down.
- `RigidBody_Integrate` 0x153c50(dt, force, torque): explicit Euler from the old rates: x += v·dt; q += ½(0, ω)q·dt,
  normalised (a zero quaternion becomes w = 1); p += F·dt, L += τ·dt. v and ω wait for the next UpdateVelocity.
- `RigidBody_AddImpulse` 0x154338(r, j): p += j, L += r × j, then v and ω (no spin limit).
- `RigidBody_ContactImpulse` 0x153ee8(e, μ, point, n): no check that the point moves into the surface (callers do).
  With μ ≠ 0 first a sliding impulse: j = −μ·|v_t| / (1/m + t·((I⁻¹(r × t)) × r)) along t = v_t/|v_t| (not limited
  by the normal impulse: friction proportional to the sliding speed), and if the point then leaves the surface
  (v·n > 0) it stops there. Then the bounce −(1 + e)(v·n) / (1/m + n·((I⁻¹(r × n)) × r)) along n. The dot products
  are the VU0's four-lane ones, so a w in n or the point counts.
- `CollBody_Transform` 0x236498: every sphere centre (and the bounding centre) through a 4 × 4 matrix.
  `CollBody_PointMassInertia` 0x236530: the spheres as point masses from the table at 0x3a3f68 (4, 2 × 7, 4, 4,
  2 × 4, 3 × 4: the wipe-out body), giving the mass, the centre of mass and the inertia about it.
- Checked in the runner on 20,000 cases (random bodies, zero masses, zero quaternions, contacts with no sliding):
  every field bit for bit.

## Knockable objects (row E8c2; `tricky_game::roller`, checked: `tests/roller.rs`, `tests/walls.rs`)

- `cRollerNode` (type 0) past its controller layer (scripts.md, Movers): +0x30 the rigid body, +0xc8 the energy
  average, +0xcc the mass, +0xd8 the bounce (0), +0xdc the friction (0.5), +0xe0 277.7778 cm/s (10 km/h), +0xe4
  seconds awake, +0xe8 the knock, +0x100 the body inverse inertia, +0x130 the centre of mass in the object's frame,
  +0x140 asleep, +0x150 its sphere tree (a copy of the course's, `FUN_002391e8`), +0x2a0 / +0x2b0 the tree's box,
  +0x2c0 its broadphase entry (+0x84 the node).
- The constructor (0x13d6c0, from script op 0): the mass and +0xd0 from the op's record (+0xc, +0x10); the tree
  header's centre of mass (+0x2c) and inverse inertia (+0x5c) × 0.075; the body at the object's place plus the
  centre of mass turned by its orientation (`Mat4_ToPosQuat`); the object's flags `& ~0x22 | 0x45`. Then a push:
  with no velocity in the record (+0x18..+0x20 all 0) from the rider's contact (`FUN_0013dd40`: v = direction ×
  speed / 2 + (0, 0, min(5000/m + 100, 600)), p = v·m, the spin 10 × the contact's +0x20), else that velocity
  (`FUN_0013dbe8`: the spin 10 × the unit vector against it). Either sets the energy and clears the knock.
- `RollerNode_UpdateTransform` 0x13e140: the world inverse inertia R·B·Rᵀ. `FUN_0013e6a0`: the object's place is
  the body's less the turned centre of mass; `Mat4_FromPosQuat` 0x250428 then places the tree; the box is the
  tree's centre ± its radius.
- Each awake frame after the controller's forces (`cRollerNode::Update` 0x13e8a8): `RigidBody_Integrate`(1/60),
  the inertia, `RigidBody_UpdateVelocity`, the tree and box, `Broadphase_MoveBox`, then `RollerNode_Collide`
  0x13eda8: the tree as a `cWorldSphTree` against the world, then against the objects. A hit (depth > 0) pushes
  the body out by the depth (at least 2 cm, `RollerNode_Translate`), damps L and v by 0.96666664 (p = v·m again),
  and if the contact point moves into the surface: `RigidBody_ContactImpulse`(bounce, 0.5, ...) with no bounce off
  the ground under 10 km/h, the knock + 1/60, and a ground hit after 5 s awake puts the time to 10 s (asleep next
  frame).
- Two rollers (`Roller_CheckCollision` 0x13f5c8, reached through the broadphase; its caller is not found yet): the
  trees' touch (`SphTree_IntersectTree`); if the points close along the push n, j = −(1 + (e_a + e_b)/2)·closing /
  (1/m_a + 1/m_b + n·((I_a⁻¹(r_a × n)) × r_a) + n·((I_b⁻¹(r_b × n)) × r_b)) along n to a and against b, and b gets
  `Roller_OnHit`. A touch that writes nothing leaves the game reading its stack; the port skips it.
- `Roller_OnHit` 0x13f448 only acts on an asleep roller: awake and knock 0, the energy from its motion, its event-1
  script started (a `cScriptNode` of kind 3) or, without one, set awake; a rider's hit plays
  `Sfx_InstanceCollision`.
- The rider (`Boarder_InstanceCollisions`' rollers' loop over the rider's broadphase pairs, rider+0x700): for a
  type-0 node, `Collide_BoarderSphTree` with the rider's body; n the push made unit; the rider's speed into n,
  if positive, goes to `Boarder_WallImpactCheck` (the record: the point, the rider's direction, n, the closing
  speed n·(v_rider − v_point)); if the closing speed is negative the roller takes 1.3·closing / (n·((I⁻¹(r × n)) ×
  r) + 1/m + 1/`Boarder_GetSpeedStat`) along n and `Roller_OnHit` from the rider. Nodes of types 0x100..0x103 take
  another path (`WorldNode_RayCast`, row E8c3).
- Checked in the runner on every course's trees (4,400 cases: the transform, the frame and matrix, collisions with
  the world's answers scripted and the shapes handed over compared, whole updates, hits, both pushes, the
  constructor, pairs of rollers) and in tests/walls.rs (3,700 rider passes with a roller): every field and call.

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

## Random numbers (`Rand_Next` 0x2401e0)

- Two streams of the same generator: `Rand_Fx` 0x23d4a8 (state 0x345e58: effects, frontend, scene picks, career
  shuffles; seeded by `Rand_SeedFx`) and `Rand_Game` 0x23d4d8 (state 0x345e70: AI decisions, stumbles, path picks;
  seeded by `Rand_SeedGame` at race init and saved in the replay snapshot by `Rand_SaveGameSnapshot`).
- State is six u32 words s0..s5. Next: s4 += s5, s3 += s4 + c, s2 += s3 + c, s1 += s2 + c, s0 += s1 + c (each c
  the carry out of the add before it, tested as new < old for that word; the first, s4 + s5, as sum < either), then s5 += 1; if s5 wrapped to 0, s4 += 1 and so on up the chain while each
  wraps (s0 += 1 last). Returns s0.
- Seed x: s0 = x − 0x0dd2f1aa, s1 = x − 0x77ced917, s2 = x − 0x39db22d1, s3 = x + 0x0702c49c,
  s4 = x − 0x61cac083, s5 = x + 0x6fdf3b64.
- Floats: (r & 0x7fffff | 0x3f800000) as f32 − 1.0 gives [0, 1); `Rand_GameRange`(lo, hi) = lo + (hi − lo)·that.
  Ported in `tricky-rs/game/src/rand.rs` (`Rand`, `Streams`) and checked bit for bit in the function runner
  (`tests/rand.rs`, E1b1). The streams are globals in the game (0x345e58, 0x345e70) and so in the port
  (`rand::streams()`, `rand_game()`, `seed_game()`): every consumer draws from the same one, so the order of draws across
  riders and systems is part of the result. The port's other xorshift stand-ins (props.rs, intro.rs, the music and sound
  pickers) still have to switch to it (E1b2c; the props, world animations and animated-object clocks are on it since E1b2b, below).
- Race init (row E1b2a, proven): `cAI::vf15_1114d0` 0x1114d0 stores a `Rand_Fx` draw in `DAT_0032de90` (0x111520, also
  written at 0x1115d0) and `cAI::vf14_111108` 0x111108 calls `Rand_SeedGame(DAT_0032de90)` (0x1112dc; `Init` 0x17e148 does
  too, 0x17e248). STANDIN: the port seeds with `rand::RACE_SEED` at `Race::new` and `Race::restart`, having no front-end draws.
- Every `Rand_Game` call site (`get_xrefs_to` 0x23d4d8): `AnimClip_ResolveAnmIndex` 0x15ae54, `Ride_AITauntCheck` 0x1036d4,
  `CruiseControl_RandomFlourish` 0x103c00, `Boarder_Stumble` 0x124f84 and 0x124fd0, `Boarder_ChoosePathAhead` 0x1189ac,
  `FUN_0011d368` 0x11d39c (a random grab of the rider's fifteen: `table[rider+0x44][rider+0x5c][Rand_Game() % 15]`,
  from `AIComputer_PlanJumpTrick` and `AIComputer_AirTrickInput` 0x138f7c), `FUN_0011d3d8` 0x11d40c,
  `Boarder_RiderCollisions` 0x124384, `AIComputer_PlanJumpTrick` 0x139d9c, 0x139dc0, 0x139e88, 0x139f04, 0x139f24,
  `FUN_00138bb8` 0x138c98 and `Rand_GameRange` 0x23d50c. All the `% 100`, `% 15` are unsigned (`divu`), whatever the
  decompiler shows.
- Read for E1b2d, not ported (they hang off clip and state systems the port does not have): `Boarder_Stumble` 0x124dd0 draws once
  (0x124f84 or 0x124fd0, `% 5` into the word table at 0x368540, stored at `rider+0x5ae0`+0x618) only for the *fast* knock
  (the hit's speed along the rider's travel over 1388.8888 cm/s with the part along its normal under 1111.1111), in the two
  branches that play clips 0x249 and 0x248; the slow ones (clips 0x244..0x247) draw nothing; it returns without effect unless
  the control state (+0x428) is 3 or 0xe, and `Boarder_SetState`(2) follows. `Ride_AITauntCheck` 0x1036a0 draws once per tick
  when the layer-1 clip type is 8 (cruise) and its timer (+0xc, +1/60 a tick) is at least 1.0253273: taunts when `% 100 < 91`
  (0x5b), then looks for a rival within 1000.13 cm and more than 1.0471976 rad off the heading to turn at (clips 0x211,
  0x212, 0x213).
- `AIComputer_PlanJumpTrick` 0x139d58 draws, in order: the trick roll (`Rand_Game() % 100 - 16 < tricks stat`) only when a
  spin or a flip is allowed; the late draw (always; late when `20 < (draw % 100) * k`, `k = 0` for skill 1.0, else
  `1 - skill / 14.454015`); the grab roll (always) and, when it passes, the pick (`% 15`); then, only when the trick roll
  passed, one draw each for the spin and flip direction (bit 0 set: negated). Ported in `race::AiDriver::plan_trick`
  (tests `race::draw_tests`); the port drew the two direction bits whether or not the trick roll passed.

- The world's draws (row E1b2b, read from the disassembly): every world-animation node draws from `Rand_Fx` (all of them
  `Rand_Fx` 0x23d4a8; `get_xrefs_to`), except the keyframed objects' clocks, which are `Rand_Game`.
  - `cTexFlipNode` ctor 0x142e90: a start frame < 0 draws once, `ceil(8u)`. `cTexFlipNode::Update` 0x143100 (the code at
    0x14319c, 0x143214, 0x1432e0) draws only in a *random step* (start frame -2, flipbook of two or more frames:
    `ceil((count - 1) u)`) and in the blink's off state (state 2: rate = base (0.75 u + 0.25)). State 1 and the plain
    steps draw nothing. `cUVScrollTexFlipNode::Update` 0x1462d0 draws the same way (not ported, no node for it yet).
  - `TexFlip_SetFrame` 0x142c80 with -1 (the cracked glass, ctor 0x145458 through `TexFlip_Init`): from three frames up
    one draw, `ceil((n - 1) u + 1)`; one or two frames show frame 1 with no draw; never past the last frame.
  - `cFlagNode::Init` 0x144938: the amplitude (if it is -1: `(45 u + 5) / 60`), then the rate (if -1: `(1.5 u + 0.5) / 60`),
    then the phase (always, `u`). The port had drawn the rate first and all three every time.
  - `cCrowdBoxNode::Init` 0x145c00: per cell (16) a phase draw `& 15` then a sequence draw `& 3`; `StepCells` 0x145e08 draws
    one word when a cell's sequence ends (sequence = `& 3`, pause when `(r & 0xff) < table`, then `-(r & 0x3f)`).
    `Crowd_LoadTexturesRandomize` 0x1458e0 (from `cGame::Init`, 0x17e8c0 area) draws one `& 15` per cell of every crowd box.
  - `cMeshAnimNode` start `FUN_00146f40` 0x146f40 (broken pieces and path markers): nine draws a piece, in two loops over all
    the pieces: first the tumble rate (`8 u + 2`) and its axis (x, y, z each `2 u - 1`, normalised), then the spread: across
    (`u`, then a sign draw `& 1`), along (the same) and up (`u`). Ported in `props::debris_draws` / `throw_all` (test
    `props::draw_tests`); the port had a per-piece xorshift and drew the spread twice for a scripted piece.
  - `cAnimObjectNode` ctor 0x198810 (`Rand_Game`, `Rand_GameRange` 0x23d4f8 / `Rand_GameJitter` 0x23d560): the random start
    first (`start + (end - start) u`, only when the script asks), then the rate (`speed ± jitter u'`, `u'` in -1..1, only
    when the jitter word is set; the jitter is a spread around the speed, not a second speed). The port had it the other
    way round and as a second speed. `cGame::Init` seeds `Rand_Game` from `DAT_0032de94` (0x17e1c4) before the objects are
    built and the race init reseeds it, so the riders never see these draws.
  - Not read or not ported yet (E1b2e): the order the nodes are made in (the port: instance order for the clocks, script-op
    order for the controllers; the game's node list order is the order the cells wake and the scripts run), the other
    `Rand_Fx` users in the draws list (`FUN_001d5180` 0x1d5180 (nine draws), `FUN_001d9208`, `FUN_001c8ec8`, `cGame::Init`'s
    snow and rain draws 0x17e8c0..0x17e8e8, the emitters and the splash and spray code, which are G15 / G19).

## Course file loading (`cCourseResolve_Common` / `cCourseResolve_PS2`)

- The course data comes in with every pointer stored as an offset; `Resolve` (vf4, returns 0 ok, 1 failed,
  3/4 missing file, 6 more than 4000 entries) fixes them up section by section. Each virtual names its section in
  a debug print that is compiled out (`Course_DebugPrint` 0x255310 is empty): TGroup header (version, the grid of
  tgroups: width/height/num/used/empty), tgroups, tgroup (counts of patches, instances, particle instances, splines,
  lights inside), inode, data header (PS2 data header on the PS2 class), instances, model pointers, models, model,
  model hierarchies, mesh, PS2 mesh context, material blocks, animation, channel, particle instances / model
  pointers / models / model / hierarchies / mesh, splines, spline segments, camera pointers, cameras, camera,
  materials, hash table.
- Lookups by index: `Course_GetPatch` 0x2549f8 (0x1c0 bytes each), `Course_GetInstance` 0x254a28,
  `Course_GetParticleInstance` 0x254a50 (0x70), `Course_GetSpline` 0x254b50 (0x80), `Course_GetLight` 0x254a80
  (0x5c), `Course_GetMaterial` 0x254af0 (0x48).
