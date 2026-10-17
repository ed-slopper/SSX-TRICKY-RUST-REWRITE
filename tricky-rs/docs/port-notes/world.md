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
  (F1c, proven) `cSplinePathNode::Update` 0x1fa6a0 → `SplinePathNode_PlaceAll` 0x1fa838: copy i (count +0x30) at
  distance +0x50 + i·spacing (+0x48), wrapped by the spline length (+0x74); `SplinePathNode_PlaceOne` 0x1fa8d8
  evaluates the spline (+0x64) into the copy's 0x20-byte slot (+0x7c) and sets its rotation: z = +0x44 + π/2 (+ π if
  mode +0x34 is 2 and speed +0x4c < 0), minus atan2 of the direction vector Update passes in unless facing +0x38 is 2
  or 3; x = ∓ its z (by speed sign) unless facing is 1 or 3. **Inferred:** that vector is the spline's tangent.
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
- The node lists (`BxList_*`, the list set at 0x337c60, 14 lists of 0x34 bytes): `BxList_UpdateAll` 0x17d950 calls
  every node's vf1, `BxList_DrawAll` 0x17d9d8 every node's vf2 (`Game_DrawLists` 0x1c7290 does the 13 lists named by
  the table at 0x33d7d8; the frontend's 3D view draws lists 3 and 4). `BxList_SetSorted` 0x17dae0 sets bit 0 of a
  list's flags (`cGame::Init` does it for list 3): `BxList_Add` then puts new nodes on the pending list (+0x1c), which
  `BxList_MergePending` 0x17d548 inserts in order with the nodes' vf3
  (does the new node go before this one?) and vf4 (if set, the new node takes the found one's place and the old one
  is deleted).
- Ported (board row E9k; `tricky_game::bxlist` on the game's bytes, `pf::Mem`, checked: `tests/bxlist.rs`):
  - **Layout.** A list is 0x34 bytes: the flags, the live list's head and tail sentinels (+4, +0x10), and the
    pending list's (+0x1c, +0x28). The sentinels are `cBxObjNode`s (vtable 0x386520).
  - **Adding.** `BxList_Add` links a node at the head, on the pending list when the list is sorted.
  - **Walking.** `First` / `NextOrNull` walk head to tail; `SortedFirst` / `SortedNextOrNull` tail to head;
    `PendingFirst` / `PendingNextOrNull` walk the pending list.
  - **Merging.** `MergePending` takes each pending node from first to last. It walks the live list back from the
    tail to the first node it goes before (vf3). It links in after that node, or takes its place when it replaces
    it (vf4; the old node's links are cleared and its vf0 called with 3). With no such node it goes at the head.
  - **Moving and removing.** `Unlink` (only a node linked both ways), `Remove`, `Move`, `MoveAll` (merged when the
    target is sorted).
  - **Every node.** `UpdateAll` (vf1, taking the next first), `DrawAll` (vf2, taking the next after), `DeleteAll`
    (vf0 with 3).
  - **Making and freeing.** `Init(n)` allocates n lists after a 0x10-byte header holding n (filled with 0xdeadc0ed)
    and links them empty. `Free` deletes every node, then destroys the sentinels from the last list back.
    `~cBxObjNode` always unlinks from the world's set at 0x337c60.
  - **Node constructors.**
    - `cBxObjNode(list)` sets its vtable and adds itself to the world's set.
    - `cBxTypeObjNode(list, type)` keeps the type at +0x14 (vtable 0x3864c0).
    - `cBxSortObjNode(list, type)` (vtable 0x386460) numbers itself from 0x337c68 (type 5) or 0x337c6c (the
      others) at +0x20. It keeps the race's +0x18 at +0x24, or 0 with no game.
    - The constructors taking a replay stream read +0xc..+0x18 and +0x18..+0x28 back.
  - **Snapshots.** The `SaveSnapshot`s tag their headers (length, 0xdeadbefc / 0xdeadbefb) and write them.
    `BxList_SaveCounters` / `_LoadCounters` keep the two counters.
  - **Checked in the runner:** 1,500 random sets (up to six lists, some sorted) with 24 fake nodes whose vtables
    point at stand-ins: vf3 compares a key word and vf4 reads a flag. 40 random calls per set over 26 entry
    points, then the free. Every result, every virtual call and all the memory were compared. 2,672 ordering
    questions were asked in merges, and 154 nodes were replaced.
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
- `Collide_SphereTriangle` 0x236bb8 (and its front-only 0x237480: 0 ≤ d; either-side `Collide_SphereTriangleTwoSided` 0x237d50: the normal turned
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

- `cWorldSphTree` (vtable 0x3a85f8; `RollerNode_Collide` and `RollerNode_HitNode` 0x13f330 make them): kind 0 (coarse patch quads, target
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
  every quad record's normals and box) and filled by the vector unit: `Vu0_RunPatchProgram` 0x2576a8 loads the patch's 16
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

## The app's ground on the course (row E7c3; `tricky_game::course_ground`, `World::from_course`)

- Proven: `World::from_course` builds the course object from the `.pbd`, `.ltg` and `.ssf` alone, the same as the
  game's loaders leave it (grid, cells, INode lists, patches, collision meshes, sphere trees, each object's rows,
  flags, surface, mode, group and model); `tests/world.rs` compares it with what the game builds in the function
  runner on every course.
- With `TRICKY_DISC` the app loads them (`|data/models/<course>.pbd/.ltg/.ssf`, from the course's `.big`) into
  `CollisionWorld::course`. The ground probe (`GroundEnv::ray_cast`) is then the game's: `cWorldLine`(0.5, from,
  to, 2) along the normal and `World_RayCast` with the rider's last-patch record (`Rider::last_hit`, rider+0x430).
  A folder level (no course files) keeps the drawn mesh.
- `CollisionWorld::ground` (the landing, wipe-outs, props, the sim's autopilot and camera) casts down a vertical
  `cWorldLine`(0, top, bottom, 2) on the course and takes the first hit that faces up at least `GROUND_MIN_Y`,
  casting again from under any other (STANDIN: a vertical query for the rays along the rider's normal, rows
  G20c3c and E8b). The keyframed objects still come from the drawn mesh.
- Proven: a patch faces the way its tessellation does (`unit((p11 − p10) × (p10 − p01))`), and some faces down
  where the drawn mesh (two-sided normals from the folder) has ground: aloha 24 of 127 patches hit by
  vertical test lines (19 of them surface 5), elysium 36 of 162 (14 surface 10). The probe takes them as the game
  does; the vertical ground query skips them (the game's landing, `AirMotion_Update` 0x108378, takes only a surface
  the rider moves into).
- STANDINs left: the patch cache keeps every patch it tessellates (no fixed size, no eviction: speed only); an
  object's box (+0xcc..+0xe0) is its parts' mesh boxes through its matrices (row E7a6); objects with no
  controller (E7b2); keyframed objects (`cAnimObjectNode`) and switched-off, broken or open ones are left out of
  the course's casts by clearing their bit 0x20 for the cast (row E7c3b: pose them as `World_RayCastInstance`
  does); the walls are still `CollisionWorld::push_out` on the drawn mesh (row E8b wiring).
- TRICKY_SIM=600 from the disc, before (drawn mesh) and after: falls out of the world 46 to 28 over the 11 courses (alaska 16 to 10, megaplex 20 to 8, aloha 2 to 6: row E7c3d), stuck 20 to 29 (walls on the mesh: row E7c3c).

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
- `Boarder_TurnFromWall` 0x1257a0(rider, n) (proven; `orient::turn_from_wall`, checked bit for bit in
  `tests/orient.rs`, 30,000 cases): a = unit(up × n) (up = +0x1a0, the board's), θ = atan2(fwd · a, fwd · (up × a))
  (fwd = +0x180, the inline atan2 round `Math_Atan`); f = 1.9198622 − θ held to [0, 0.34906584] when θ > 0, else
  −1.9198622 − θ held to [−0.34906584, 0]; f ≠ 0 turns the board by −f about its up (`Boarder_RotateAboutAxis`).
  So at most 20° a tick, until the forward is 110° from up × n.
- The collision body for walls (row E7c3c): `cBoarder` vf8 0x116ab8 sets it up with `CollBody_SetSkeleton` and
  `CollBody_InitRider`(stats+0x60, body): the bounding radius is the body scale × 100 cm (0.7 to 1.0 m). While
  `Boarder_WallCollide` runs the mask (+0x494) is 3: only spheres 0 and 1, on bones 2 and 4, which the riders'
  `_body.mpf` skeleton (the same for all riders) names `upper_back` and `head` (radii 25 and 20 cm); the legs and
  the board are not in it. In the bind pose (standing straight) they are 23.0 and 52.8 cm over the hips, the feet
  89.5 to 91.8 cm under them.
- Wiring (row E7c3c, proven only for the calls; the body's place is ours): with `TRICKY_DISC` the ground tick's
  `wall_collide` runs `walls::wall_collide` on `CourseGround` (`WallGlue` in `rider/ground_tick.rs`). STANDIN: the
  body is the bind pose standing straight up the board's up from the position, × stats+0x60 (upper back 114.7 cm,
  head 144.5 cm, bounding centre at the hips 91.7 cm), for `CollBody_UpdateFromBones` on the animated skeleton
  (G5). `Boarder_AccumulateImpact` 0x125c48 (+0x2e4 += 1 − max(n · +0x2f0, 0), +0x2f0 = n) is not kept (nothing
  ported reads them). The air's walls (row E8o) and the keyframed objects (E7c3b) stay on the drawn mesh.
- The landing of `AirMotion_Update` 0x108378 (proven, 0x108554..0x1089bc): P = the board frame (+0x4aa0) times
  minus the pivot bone's position (bone +0x44d8, its +0x8c8), U = the frame's up row (+0x4ac0); `cWorldLine`(0.5,
  P − 100·U, P + 200·U, 1) and `World_RayCast` with the rider's last-hit record (+0x430). On a hit the normal
  (+0x2a0) is the hit's; when v · n < 0 the position and quaternion are saved, the position moved by the pivot
  turned by half the spin (+0x1d0) and half the flip (+0x1d4), `Boarder_ProbeGround` run, the position put back;
  when the probe hits, vn = n′ · v and the rider has landed if the height (+0x1bc) is under half the surface's
  word 6 (+0x18 of its row); below 0 the position becomes the ground point (+0x2d0) minus that offset and the
  height 0. Landing on surface 0, 6 or 10 is a crash (`Landing_CheckAngles` early out), and the get-up of a rider
  on surface 0 is a reset (air.md): the game puts a rider who lands off the course back, as the port does.
- Wiring (row E7c3d): with `TRICKY_DISC` `Rider::course_landing` (`rider/ground_tick.rs`) is the port's landing
  test, through `CollisionWorld::course_line`. STANDINs: P is the position (no frame, no pivot bone: G5) and the
  probe runs from it (no predicted attitude); the down ray's normal is kept only when the rider lands (row
  G20c2b); the side is n × the heading in n's plane; the keyframed objects come from the drawn mesh
  (`CollisionWorld::mover_ground`, E7c3b). What follows the landing (speed, crash test) is still the port's own
  (row E8r).
- TRICKY_SIM=600 over the 11 disc courses, "fell out of the world" / "autopilot stuck": E7c3 28 / 29, E7c3c 40 /
  11, E7c3d 30 / 18 (megaplex 4 / 11, mesablanca 8 / 1, aloha 8 / 0). The count of falls is twice the falls: each
  one is the rider's reset plus the autopilot's rejoin (`race.rs`, both through `Rider::respawn`). Every fall
  traced (aloha, alaska, elysium, megaplex, mesablanca) is the game's own reset: a landing or get-up on a surface-0
  patch off the course (aloha 4 of 4: the slope's side at (35, 453, 8), (210, 386, 30), (−35, 141, 306),
  (−127, −216, −152) port m), a crash on surface 10, or the course's reset boxes (megaplex (−206, −123,
  −167)); none comes from the walls (their pushes are under 2 cm). Which ones the game's riders would also
  reach is the autopilot's line (ours), not the ground. Megaplex's new stucks wait under a vertical stretch of
  its line (x −217, z −247, 35 m up). Not a lift (row E7c3b found no keyframed object there): an upward boost
  pad ((−221..−212, −147..−138, −251..−241) port m, direction up, target 50) on surface 18 that leaves a
  grounded rider on the floor, with the course's landing and with the old one alike (row E7c3g).
- Megaplex's boost pads (row E7c3g): the pad does lift a grounded rider. `cBoostNode`'s push (vf27 0x140838,
  `boosts.rs`) only adds speed along its direction; off the powder the ground tick bleeds 40 % of the speed
  along the normal and keeps the speed, the rider rises, and once the probe's height passes the surface's word
  6 (2.6 cm on surface 18) `GroundMotion_Update` 0x10a0d8 takes off. The port never pushed there: the app
  dropped every boost pad on a course with the tube (megaplex's 37 are all plain `cBoostNode`s, sub-type 7;
  the tube's own are the lap, Z and tube-end controllers, other sub-types) and the self-test (TRICKY_SIM) never
  applied any pad. `Rider::apply_pads` (game crate, moved from `src/main.rs`) now runs in both, with the pads
  kept. TRICKY_SIM=600 over the 11 disc courses: stuck / falls 18 / 32 → 11 / 34 (megaplex 10 / 10 → 4 / 12:
  further down the course, 9,255 m against 8,392; three of its falls are the tube's put-backs). STANDIN: the
  pad is our box round the body point, not the controller's own touch test, and no push while down (row
  E7c3h).
- The boosts' touch test (row E7c3h): `cBoostNode` Update 0x140688 walks the objects in its broadphase box
  (+0x70), and for each rider (object type 0x3ef) builds `cWorldBoarder`(rider+0x470, 1) and calls
  `WorldNode_RayCast` 0x13aad0 = `World_RayCastInstance` 0x25bf48 on the course for the boost's own instance
  (node+0x28); > 0 pushes (vf +0xe4), with no check of what the rider is doing (down riders are pushed too).
  `Rider::apply_pads` does that on `CourseGround::ray_cast_instance` with `Rider::bind_body`: all 18 spheres
  (mask −1) from `CollBody_UpdateFromBones` 0x2361b0 on the riders' bind pose (`Body3000` bones, cm from the
  hips, x foot to foot: upper back 23.0 up, head 52.8, thighs −13.0, feet −91.8; the hips 91.7 cm over the
  board), along the board's forward, up and side. STANDINs: the bind pose for the animated skeleton (G5), the
  pad's box against the body's bounding sphere for the broadphase. (The walls' body, `wall_body`, puts the
  bounding centre at the hips, 91.7 cm; the bind pose puts it between the thighs, 78.7 cm.) Teleports and
  pickups still use the box round the body point. `tests/course_ground.rs`: megaplex's up pad (instance 1213)
  touches a rider on its floor, not one 20 m up. TRICKY_SIM=600 over the 11 disc courses: stuck / falls 11 / 34
  → 12 / 26 (megaplex 4 / 12 → 5 / 4; the three pads touched are instances 1213, 1214 and 1256).
- Keyframed objects in the casts (row E7c3b): each `Mover` (`collide.rs`) gives the course its animated parts'
  local matrices (`Mover::frames`, `ObjAnim::local` = `cMeshAnimFrame::vf1` 0x1cb498, in part order) whenever its
  clock moves (`CollisionWorld::tick_movers`, `set_mover_time`, and once when the course is set,
  `pose_course`); `CourseGround::pose` keeps them for `WorldEnv::frame` (`MeshAnimNode_GetFrameMatrix`
  0x1998d0), sets the object's controller animated (+0x14 bit 0x100) so `World_RayCastInstance` poses the
  parts with them, and puts its box (+0xcc) round the posed parts' mesh boxes (`world::object_box` with frames).
  The keyframed objects are no longer skipped, and `ground`, the landing and the walls take them from the
  course's own collision meshes, not the drawn triangles. STANDINs: the controller is only that bit (row E7b2),
  the box is ours (row E7a6), and the clocks are the port's (`Mover`, `src/worldanim.rs`). The objects' INode
  lists stay where the course put them (the game's do too). `tests/course_ground.rs`: 8 objects over the 11
  courses whose meshes all move with an animated part are hit at rest, the same once posed at rest, and not
  once lifted 1 km. In the sims the casts hit animated parts (kind 2) on mesablanca (object 1444, over 2,400
  hits), megaplex (1139) and elysium (154). TRICKY_SIM=600 over the 11 disc courses: falls / stuck 30 / 18 →
  32 / 18; the counts include each lap's put-back to line point 0 (megaplex 4 of 10).

## Live cells (row E7b; `tricky_game::cells`, checked: `tests/cells.rs`)

- The AI world (`cAIWorld`) keeps the grid cells (TGroups) around the riders live. It has 108 slots at +0xc
  (8 bytes each: the TGroup, +4 a short counting ticks since it was last marked, +6 a short "new") and their grid
  indices at +0x36c.
- `cAIWorld::vf1` 0x115e28, each tick: the patch cache ages (`PatchCache_Age` 0x251d70, only speed). Each rider's grid cell
  (+0x16c) comes from its position (+0x140) through `Boarder_UpdateGridCell` 0x11d438: (p − corner + size)/size − 1
  per axis, truncated and clamped. If the first rider is still in the cell it had (+0x42c), the race object's +0x1c
  decides: below 2 the camera point (`Camera_GetFocusPoint` 0x173c18: camera +0x42c, or the point at 0x3368b0 when 0x337774 is set)
  is marked too and the pass goes on; otherwise the other riders are checked in turn, and if none changed cell the
  tick stops there.
- The pass: every held slot ages (`AIWorld_AgeActiveCells` 0x1162c0, +4 less 1), `AIWorld_MarkCellsAround` 0x1160f8 marks the cells
  around each rider (and the camera point): x from cell − 2 to cell (x outer), y the same, with cell computed
  without the − 1, so the 3 × 3 block is centred on the rider.s cell; each clamped.
  `AIWorld_MarkCellActive` 0x116420 ignores index 0xffff and empty cells; a held cell gets +4 = 1, a new one the
  first free slot with +4 = 1 and +6 = 1 (the 108 slots never fill on the disc's courses; a full table drops it).
- `AIWorld_UpdateActiveCells` 0x1162f8: a new slot's cell is turned on and +6 cleared; otherwise a slot with +4
  below 1 is turned off and freed (index −1). `AIWorld_ReleaseActiveCells` 0x1163a0 turns off and frees every one.
- `AIWorld_ClearSlots` 0x115df0 (from `Game_ResetWorld` 0x115dc0, `Replay_Update` 0x191298 and `cGame::Init`): all
  108 slots to [0, 0, 0] and their grid indices to −1, without turning any cell off.
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
  +0x140 asleep, +0x150 its sphere tree (a copy of the course's, `SphTree_Copy` 0x2391e8), +0x2a0 / +0x2b0 the tree's box,
  +0x2c0 its broadphase entry (+0x84 the node).
- The constructor (0x13d6c0, from script op 0): the mass and +0xd0 from the op's record (+0xc, +0x10); the tree
  header's centre of mass (+0x2c) and inverse inertia (+0x5c) × 0.075; the body at the object's place plus the
  centre of mass turned by its orientation (`Mat4_ToPosQuat`); the object's flags `& ~0x22 | 0x45`. Then a push:
  with no velocity in the record (+0x18..+0x20 all 0) from the rider's contact (`RollerNode_PushFromContact`: v = direction ×
  speed / 2 + (0, 0, min(5000/m + 100, 600)), p = v·m, the spin 10 × the contact's +0x20), else that velocity
  (`RollerNode_PushWithVelocity`: the spin 10 × the unit vector against it). Either sets the energy and clears the knock.
- `RollerNode_UpdateTransform` 0x13e140: the world inverse inertia R·B·Rᵀ. `RollerNode_PlaceFromBody`: the object's place is
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

## The app's knockables (row E8c3; `tricky_game::knockables`, `CourseGround::knock`, tests/knockables.rs)

- Proven: the riding rider's `Boarder_InstanceCollisions` 0x125088 queries the objects with `cWorldBoarder`(rider+0x470,
  1) (`World_QueryInstances`); a hit (t ≥ 0) makes the contact record (point, unit velocity, normal, speed in; the
  normal turned off the ground in mode 2) and calls `Instance_OnPlayerContact` 0x13bd40(object, rider, contact, cast
  record), then `Boarder_InstanceBounce` for flag 0x80 and `Sfx_InstanceCollision`. The contact runs the object's
  event-1 script when it has no controller, or its controller says yes to vf 0x64 (a roller: asleep).
- Proven: the touch objects' event-1 scripts (23 of them in the test's first three per course) spawn the roller
  with op 0 sub-type 0 (`TriggerScript_Op0_SpawnController` 0x13c5d0): an asleep roller on the object is woken, an
  awake one left, else `cRollerNode::cRollerNode` on the cast record's sphere tree (+0x30) or, with a velocity in the
  op, the object's group's (`World_GetInstanceSphTree` 0x260ca0). The object's flags lose 0x20, so the rider's
  query no longer finds it; the rider then meets it only in the rollers' loop (`roller::rider_kick`).
- The port: `CourseGround` keeps the flags (now changeable), the `.ssf` and a `Knockables` (rollers newest first,
  waiting script threads, the knocks for the sound). `GroundEnv::instance_collisions` (rider/ground_tick.rs) runs
  `walls::instance_collisions` and the rollers' loop on the course; `CollisionWorld::tick_knockables` runs every
  roller's update, the pairs (`Roller_CheckCollision`) and the waiting threads once per 60 Hz frame, from the app's
  props loop and the `TRICKY_SIM` self-test; a knockable prop is drawn where its roller is (`knockable`).
- STANDINs: the rider's pairs and the rollers' pairs are boxes that meet (for the race's broadphase); the body is
  the bind pose's (G5); the rider's handle 1 / 2; the thread's rider velocity is the one at its start; only op 0
  sub-type 0 and new threads are done by these threads (the port's `logic.rs` does the rest, E9b2); no other
  controller is kept (E7b2), so a touched object is never `controller` in `Touched`; the air's
  `Boarder_InstanceCollisions` is not called (the app's air is not `AirMotion_Update` yet, E8p). Without the course's files the old ball props stay.
- Inferred: the pairs' order and pass in the frame (rollers, then pairs, then threads) is our choice.

## Bouncy fixed objects in a race, checked live (row E8c4; PINE, 2026-10-13)

What the game's computer riders do at the fixed bouncy touch objects the app's autopilot sticks on (flags 0x80 and 0x20,
SurfaceType −1, mass 1e30, bounce 0.5), read over PINE in a race (course object `gWorld` 0x346488: +4 the course
header, header +0xc the instance count, +0x48 the instances, 0x100 bytes each; rider +0x140 position, +0x150
velocity, +0x424 motion, +0x428 state, +0x2e4 impact counter; recordings kept on the PC).

- Proven (live): elysium `HalfPipeThing_Metal` 3403 / 3604 are in the race (not hidden): flags 0xa100a3, no
  controller, mass 1e30, bounce 0.5, mode 1, group 154, and their boxes (+0xcc / +0xd8) equal the port's
  (`World::from_course`) to the centimetre.
- Proven (live): 3 of the 6 computer riders ran into 3403 in one lap (one in the air, motion 1; two riding, motion
  2) and were bounced along the frame's face normal (±(0.643, −0.766, 0), its matrix's first row) by
  (1 + 0.5)·v_in: Δv 2311 against 1.5 × 1541 cm/s, and 2242 against 1.5 × 1491; then `Boarder_Wipeout` (motion 5,
  state 0x13, impact counter +0.94..1.0). They got up (motion 6) and rode on 2 to 5 s later, slower than 3 m/s for
  up to 2 s, with no reset (counter at most 4.15). The other three passed beside it or flew over. So the port's
  bounce (`walls::instance_bounce`, runner-checked in E8c2) is the game's; the port's body (bind pose, G5) meets
  3403 on the same tick as the game's within one.
- Proven (live): megaplex `Pinball_FlipperL` 75: matrix as the port's, flags 0x10a110a3 (0x10a111a3 while its
  cell is active, `World_CellActivate_RunPersistant` 0x25fed0; no cast reads 0x100), mass 1e30, mode 3 (sphere
  tree), group 2. Its box is the game's (−16774, −23326, −38619)..(−16142, −21389, −37906); the port's estimate
  (E7a6) is up to 2 m bigger (broadphase only). Twice in 10 minutes a controller was attached for exactly 300
  ticks (flags …a3 → …a5, matrix and box unchanged: the parts are posed by the controller, `World_RayCastInstance`),
  and two riders went through its box during one. At rest, the computer riders went round its tip
  (y > −21700, the paddle's long axis is its y) 18 times in 3 laps: one touch without a wipe-out, one wipe-out.
- Interpretation (inferred): the app's stucks at these objects are the autopilot's (`AiDriver`, our steering,
  row G4e) and the flipper never flipping (no controllers in the port, row E7b2), not the bounce.

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
  seeded by `Rand_SeedGame` at race init and saved in the replay snapshot by `Rand_SaveGameSnapshot`, read back by `Rand_LoadGameSnapshot` 0x23d5c0; `Rand_SaveSnapshot` / `Rand_LoadSnapshot` 0x2401c8 do the same 0x18 bytes for a given stream, F1c).
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
  - `cMeshAnimNode` start `MeshAnimNode_Start` 0x146f40 (broken pieces and path markers): nine draws a piece, in two loops over all
    the pieces: first the tumble rate (`8 u + 2`) and its axis (x, y, z each `2 u - 1`, normalised), then the spread: across
    (`u`, then a sign draw `& 1`), along (the same) and up (`u`). Ported in `props::debris_draws` / `throw_all` (test
    `props::draw_tests`); the port had a per-piece xorshift and drew the spread twice for a scripted piece.
  - `cAnimObjectNode` ctor 0x198810 (`Rand_Game`, `Rand_GameRange` 0x23d4f8 / `Rand_GameJitter` 0x23d560): the random start
    first (`start + (end - start) u`, only when the script asks), then the rate (`speed ± jitter u'`, `u'` in -1..1, only
    when the jitter word is set; the jitter is a spread around the speed, not a second speed). The port had it the other
    way round and as a second speed. `cGame::Init` seeds `Rand_Game` from `DAT_0032de94` (0x17e1c4) before the objects are
    built and the race init reseeds it, so the riders never see these draws.
  - The other `Rand_Fx` users (row E1b2e, read; ported as `tricky_game::fxrand`, tests in the module):
    - `ParticleSys_RandomizeFactors` 0x1d5180: nine draws, each a float in [1, 2) (`(r & 0x7fffff | 0x3f800000)`), stored at
      emitter offsets 0x28, 0x2c, 0x30, 0x34, 0x38, 0x40, 0x44, 0x48, 0x4c (called from `ParticleSys_Create` 0x1d85c4 and twice
      in `Fx_SurfaceSpray`). They are then the emitter's nine separate random streams: `ParticleSys_StepEmitter` 0x1d5a30 runs
      `Rand_VuNext` 0x1d5a08 (the vector unit's R register: `VRINIT` from the word's low 23 bits, `VRNEXT`) on each, in that
      order, once per emission step. So the spray, trail and splash randomness is a third generator, seeded from `Rand_Fx`.
    - `cGame::Init`'s snow: after `ParticleMan_Init`, when the game mode is neither 2 nor 4, the race has at most one rider
      (`+0x7c`) and fewer than three entries (`+0x88`) one draw decides `snow = u < course record +0x54`; then
      `SnowFall_InitLayers` 0x1c8ec8 (in the `cSnowFallMan` constructor) makes 8 layers x 4 draws (32; counted in the runner in G7c1, an earlier reading said 7) (words 4, 6, 7, 5 of the
      0x90-byte layer record, each in [1, 2)); then density `0.05 + 0.15 u` (`+0x78`, `SnowFall_SetDensity`); then two draws
      for `FUN_001c91c0`: `5 + u` and `7 - u'` (`+0x4bc`, `+0x4c0`, which set each layer's words 1 and 2).
    - `Crowd_LoadTexturesRandomize` 0x1458e0 (right after, from `cGame::Init`): for every instance whose model flags have
      bit 0 set, in instance order, one draw `& 15` per sub-item (the crowd's cells) picks one of 16 textures.
    - `ParticleGroup_UpdateTrail` 0x1d9208 (from the rider's effect, `vf7_1356f8`): per call one draw for the spawn count
      (`trunc(+0x10 + +0x14 (2u - 1) + carry +0x18)`, the fraction kept), then per new particle five draws: its life
      (`+0x1c + trunc(+0x20 (2u - 1))`), a speed (`+0x70 + +0x74 (2u - 1)`), a lateral factor (`2u - 1`, with `+0x78`,
      `+0x7c`), and the two offsets along the vectors at +0x40 and +0x60 (`2u - 1` each). Not ported (the trail itself is
      G15 / G19's).
    - `cUVScrollTexFlipNode::Update` 0x1462d0: the flip stepping of `texanim::Flip` (its random frame `ceil(u (n - 1))` and
      the flicker's `rate (0.75 u + 0.25)`), already in place.
  - `DAT_0032de94` (the `Rand_SeedGame` seed at `cGame::Init`) has no writer in the executable and is zero in the image:
    `GAME_INIT_SEED` is 0. The node order stays open (E1b2h).

- The intro's and scenes' draws (row E1b2c, read): the camera director's random branch (`CamScript_Update` 0x175f50 case 10,
  0x176278) draws once from `Rand_Fx`, `divu` by 100 (unsigned, 0x176284), and takes the branch when the remainder is below
  `|p|` (`c.lt` at 0x1762e4). Ported: `intro::Director::step` takes the draw. The pre-race scene variants are
  `Rand_Fx % count` (`FUN_00169378` 0x169378: per rider slot, from the 10-byte records at 0x3cc028, the first scene row
  at +0x2e, the count at +0x30; one draw each unless a variant is forced) and `PreRaceSel_ChooseFromTable` 0x168a18 (one
  draw `% matches`); `Career_PrepareRace` 0x163a40 draws twice and stores them at career +0x2b04 then +0x2b00 (nothing
  reads them back with those offsets in the exe). The port's staging, gate and post-race picks are `% n` draws on the
  global `Rand_Fx` (`fx_pick`); STANDIN: the game's draws before them (the riders' picks, the career's) are not made.
- The music pickers ARE `Rand_Fx` (row E1b2f, read; the E1b2c note that said otherwise was wrong): `Music_StartCourseSong`
  0x214e60 and `FUN_002246a8` (the air loop's phrase, `Music_PickAirLoopPhrase` 0x2246a8) call the sound system's virtual at
  vptr+0x24 (`DAT_003436f8`, a `cBXAudio` whose class chain is cBXAudio -> cController -> cCrowdSys -> cSpeechSys -> cMusicSys
  -> cStreamSys -> cMidiSys -> cBankSys -> ... -> cSoundSys, where the slot is pure), and `cBXAudio::vf3` 0x21e6c0 is a
  tail call of `Rand_Fx`. (The vtable entries are 8 bytes; the vptr points at the table's start, so +0x24 is the entry at
  +0x20, the fifth word pair: delta, then the function.) The formulas, all on the raw word:
  - course song: A = `1 + (r & 0x7fff) * 3 / 0x7fff` (1..=4: the top value only when the low 15 bits are all set), then two B
    picks `4 + (r & 0x7fff) * 4 / 0x7fff` (4..=8) when the race is not a single-event restart, one B pick otherwise; none
    in courses 3, 7, 8 and 0xb. Ported in `coursemusic.rs` via `fxrand::segment_pick`.
  - the air loop's phrase: `Rand_Fx % (phrases - 1) * size` (no draw when a fixed phrase is asked for); a second form
    `Rand_Fx % count2 + count1`. Ported: `pathmusic.rs` takes `Rand_Fx % 3`.
  - `FUN_002256c0` 0x2256c0 (level sound banks): `Rand_Fx % variants` (`FUN_00224658`) picks which variant of a bank is loaded.
  - `Speech_RollChance` 0x235d84: `(Rand_Fx & 0x3ff) < table[min(level, +0x383c)][kind]` (table at 0x3a3528, 0x28 a row).
  - `Audio_RaceInit` 0x2116d8 / `Audio_OnRaceGo` 0x215288: `Rand_Fx % 100` for `Music_SetPathVar`, game mode 6 only.
  - the C part of the course intro (row E1b2q, read): `Audio_InGameUpdate` 0x20ee58 (0x20ef98), whenever fewer than two
    segments wait in the stream queue, draws once and queues `8 + (r & 0x7fff) * 8 / 0x7fff` (unsigned; 8..=16, the
    closing segment only when the low fifteen bits are all set); in courses 3, 7, 8 and 0xb it queues instead a counter
    (+0x6e44, 0 at the start) that wraps from 16 to 8, with no draw other than the call. Ported: `coursemusic.rs`.
  STANDIN: the ambient and speech draws of `worldsound.rs` are the port's own picks on its own generator, not the
  game's draws (E1b2i).

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

## Skies (`Course_LoadSky` 0x185c28; F1c, proven from the code, not ported)

- The sky set at 0x337ce8 holds up to 10 layers of 0x14 bytes from +4 (`Sky_GetLayer` 0x185d50, count
  `Sky_GetLayerCount` 0x185dc0) and, at +0xd0, a table from sky number to layer (−1 none).
  `Sky_GetModelByIndex` 0x185d68 looks the layer up and returns its first model (`Course_LookupInstanceById`).
- `cSkyBoxObjNode::cSkyBoxObjNode` 0x1c7900 builds the name "<course>_sky" (format at 0x390bf0), finds it among the
  12 names at 0x337cb8 ("gari_sky", ...) and keeps that sky's model at +0xc.
- `Sky_Delete` 0x186078: destroys the 10 layers last to first (each layer's destructor), then frees the set when
  bit 0 of the flags is set.

## The falling snow (row G7c1; `game/src/snowfall.rs`, checked: `tests/snowfall.rs`)

`cSnowFallMan` 0x1c8db8 has eight layers of flakes, 0x90 bytes each from +0x10. The grid's origin is at +0x490.
- **Its fields:** +0x4a0 the fall velocity (a frame's step is 1/60 of it), +0x4b0 the alpha, +0x4b4 the density,
  +0x4b8 the cell size, and +0x4bc / +0x4c0 the fall range.
- **The constructor:** origin (0, 0, 0, 1), velocity (0, 0, −300, 0), alpha 0.1875, density 0.5, size 350, range
  2..3. Then `SnowFall_InitLayers`.
- **A layer:**
  - its flake count: density × size³ / 10⁶, with the EE's division, truncated;
  - 2a − b and b − a from the range;
  - four `Rand_Fx` factors in [1, 2), drawn into +0x10, +0x18, +0x1c, +0x14 in that order;
  - the cell's axes (size on each), its colour (128, 128, 128, alpha × 128), a zero row and (1, 1, 0, 0).
- **The setters:** density (all counts), scale (the axes, then the origin snapped), range, velocity and alpha.
  Snapping takes the origin's whole multiples of twice the size away (each axis, truncated).
- **The per-frame step (vf1):** the origin moves by 1/60 of the velocity and is snapped.
- **The draw (vf2):** only while it snows (the game's +0x74).
  - The density follows the camera's weather (+0xa4's +0x18) when it changes.
  - The camera's cell is its offset from the origin over the size, rounded half up.
  - The renderer is set up (+0x1c0 1; +0x1c8 (0, 0, 0.08); +0x1e0 5; +0x1f0 (gApp+0x30's +0x4c, 0)), the walk runs
    from that cell's centre, and the renderer is put back.
- **The walk (0x1c9388), four levels deep:**
  - Each level has eight octants at half the extent (child bits: −x, −y, −z).
  - Each octant is tested against the view (the renderer's +0x280 on its box) unless its parent was wholly inside.
    1 = outside skips it; 0 = inside stops further tests below it.
  - **At the bottom** is one cell. Its layer is chosen by the cell's parity (x 0x240, y 0x120, z 0x90 bytes apart).
    It is drawn (+0x260) when the camera is within 7 sizes (the distance includes the w lanes, on VU0), and the
    alpha fades over the last size.
- **`StaticInit_SnowFallMan`** writes three colour rows at 0x3d0da0..0x3d0dc8.

Checked in the runner on 6,000 random managers, games, cameras and weather, about 430 calls for each of 14 kinds.
The renderer was a fake vtable whose box test answered inside, outside or partly. 32 of 36 mutants were caught. The
others: (x − 1) + 1 is exact for the factors, the parity's remainder −1 gives 1 either way, and two distance
boundaries (exactly 7 sizes, or exactly 6).

## Fog (row G7c2; `game/src/fog.rs`, checked: `tests/fog.rs`)

- **The fog object manager** (`cFogObjMan`: vtable 0x3916f0, up to 64 volume objects from +0x10):
  - `FogObjMan_Build` makes a `cPS2FogVolume` (0x1c bytes from the allocator, named "FogVolume" and filled with
    0xdeadc0de) for each of the world's fog definitions (the world's +4's +0x30 of them; `0x252318` reads the list
    at +0x70). It notes the object on the definition (+0x14) and builds it from the definition (slot +0x10).
  - `DeleteAll` deletes each object (slot +8, flags 3).
- **Building a volume** (vf1 0x1ca3a8):
  - The point count is the definition's first group's count, once for each of its groups. The game reads the same
    group every time.
  - One block holds 0x34 bytes a point ("FogVolData", 0xdeadc0ed): the points (w 1), their screen positions,
    colours (128 and three channels × 128) and sizes.
- **The manager's draw (vf2):**
  - It sets the renderer up: +0x1a0; +0x1d0 4; +0x1f0 (gApp+0x30's +0x88, 0); +0x1f8 (1, 0); +0x1f0 (−1, 1); +0x1e0 3;
    +0x1c0 1; +0x1c8 (0, 2, 0).
  - The camera is the camera matrix's last row negated and run through the matrix again (+0xf8).
  - It walks the world's cells (the list at 0x3464c4) up to 0xa2 or the first empty one. For each fog slot
    (the world's +0x48) the cell's mask (+0x36) has, the cell's slot records (+0x38, 0x4c bytes) give placed
    volumes (a matrix, the definition at +0x40, the box at +0x44 / +0x50).
  - When the cell's other mask (+0x32) asks, each volume is tested against the view; outside skips it. Within
    20000.09 of the camera it is drawn (slot +0x20) with that answer and an alpha: 1 within 12500.37, then fading to
    0. Then +0x1a8.
  - Without a test, the answer passed is the last one made: a stack word in the game (STANDIN: the caller's `last`).
- **The volume's draw (vf3 0x1ca5a0):**
  - The volume's position goes through the renderer's +0x5b0 rows, with w scaled by +0x630's w, clamped to 0..x
    (`vmaxx` / `vminix`) and made an integer (`vftoi0`, `vmtir`). The low 16 bits go to the renderer's +0x1e8 (2, v).
  - Each point goes through the volume's matrix and the renderer's +0x1770 rows to its screen position. The points
    are sorted by screen w with the C library's `qsort` (`data::disc::qsort`, Bentley–McIlroy; the comparison
    answers `a < b`, 1 or 0).
  - Each point deeper than 500 is projected (x and y over w, w its reciprocal) into a sprite of its size × the
    renderer's scale (+0x1730, +0x1744).
  - With clipping, a sprite whose two corners share an outcode bit is dropped. The bits are x 2 / 1, y 8 / 4, z
    0x20 / 0x10 against +0x17b0..+0x17c8; with w below 0 the code is flipped.
  - Its colour is copied, and its alpha × the draw's alpha fades by the square of (depth − 500) / 2101.95 until
    2601.95.
  - The sprites go into one GIF packet at the renderer's +0x680: a tag, the register settings, then ST, RGBAQ, XYZ2,
    ST, XYZ2 a sprite, with integer fields OR-ed from sign-extended values as the game does. The packet is sent with
    0x1e8838.
- **The runner and the disassembler** now know `vmaxbc`, `vminibc`, `vftoi0/4/12/15` and `vmtir`.

Checked in the runner on 12,000 random worlds, about 1,000 calls for each of 12 functions. Volume objects were built
from random definitions; renderers had random matrices, a clip box about a sprite's size, and a fake vtable; the
world had holes in its cell list. About 700 packets were built. 33 of 40 mutants were caught. The other 7 are
equivalent (the w < 0 outcode, since a sprite's w is 1/depth; the delete's flags 3 or 1) or boundaries (depth
exactly 500 or 2601.95, z on the clip plane, distance exactly 12500.37 or 20000.09).

## The sky (row G7c3; `game/src/sky.rs`, checked: `tests/sky.rs`)

Ported on the bytes at their addresses. The models' own functions, the files, the course info, the texture table's
build, `Lights_Build` and the renderer are calls out. This adds to the F1c section "Skies" above.

- **The sky set** (0x337ce8): +0 the course sky table's row count; +4 ten layers (models of 0x14 bytes, the vtable
  at +0x10, the loaded data at +4); +0xcc the layers loaded; +0xd0 twelve layer numbers by sky id.
  - `Sky_InitLayers` 0x1859c0 resets each layer (slot +0x10) and counts none. `Sky_FreeLayer` 0x185a78 frees the
    layer's data (`Mem_Free`) if any, resets it, and lowers the count when it was the last one counted.
    `Sky_FreeLayers` 0x185a28 does that for 9 down to 0, then counts none.
  - `Sky_GetModelByIndex` 0x185d68 answers 0 for a negative layer number. An id of −1 reads +0xcc, the count, as its
    layer number: `cSkyBoxObjNode` hands it −1 when no name matches.
  - The static set-up 0x185f90 makes the ten layers (0x2555a8, then the layer vtable 0x388350), counts the table's
    rows by their **count** word (+4) up to a −1, and fills +0xd0 with −1. 0x185f80 is its destructor
    (`Sky_Delete` with 2).
  - `Sky_Delete` 0x186078 calls slot +8 with 0 on layers 9..0, then deletes the set with flag bit 0.
- **The loads:**
  - `Course_LoadSkies` 0x185b18 finds the course's row by its **course** word (+0) up to a negative one; the row
    count is re-read each pass and the id re-read after the load.
  - `Course_LoadSky` 0x185c28 formats both paths first ("data/models/%s%s" with ".pbd" and ".ssh"). With 10 layers
    or no such file it answers −1. Otherwise the loaded file goes to the next layer's slot +0x30. A non-zero answer
    there leaves the count as it was but still answers the layer's number. On 0 the textures are remapped, the
    layer counted, and the file released (0x255df0).
  - `Course_RemapSkyTextures` 0x185dd0: 0x2609d0 (the world 0x346488, the .ssh path, a word it fills) gives the
    table. Nothing is done when it answers 0. Each part's four shorts (not negative) take the low half of their
    table word. Each word of its +0x44 list (count first) takes its table word, and then the part's first short is
    set to the low half of the list's first word. A material's four shorts at +0x1b0 are looked up unless their
    3-bit field of +0x1ac is 5. The table is freed when it is not 0.
- **`cSkyBoxObjNode`:**
  - The constructor 0x1c7900 compares "<course info>_sky" with the 12 names using `stricmp` (case ignored).
  - The draw 0x1c79e8 needs a model at +0xc. The matrix is 0x346440's three rows plus the camera's position (the
    game's +0xb0 + 0x80 × camera). The lights are an ambient (1, 1.99, 1.99, 1.99) and three all-zero lights. The
    renderer's state is set around the model's draw (+0x2a8 with the model, its +0xc, the matrix, the lights, 0, 0)
    and put back after, the fog value 0.08 with it.
  - The static set-up 0x1c7c58 writes the light constants at 0x3d0d20..0x3d0d6c.

Checked in the runner on 16,000 random sky sets, about 1,000 calls for each of 16 functions (with `stricmp` running
as the original and names differing only in case). 340 layers were loaded, 1,669 sky boxes drawn, and 817 found
their sky. 37 of 38 mutants were caught. The one left is equivalent (the count lowered to `i` inside the test that
`i` is the last one).

## The renderer's box test, the list cull and the grid lookup (row G7c4; `game/src/cull.rs`, `bxlist.rs`, `worldgrid.rs`)

- **`cPS2GraphicsMan::CullBox` 0x1e1b10** (the renderer's slot +0x280, which the snow's and the fog's walks call):
  - The first test of a frame (+0x17d0 clear; it is then set) makes the frame's matrices: +0x1770 the camera's
    (+0x3f4) times +0x1730; +0x490 a copy of the camera's; +0x510 that times +0x4d0; +0x5b0 that times +0x570. Then
    0x1c4650(+0x480) and 0x1c4c38(+0x480, +0x610).
  - The test itself is VU0 micro program 0x320 (byte address; `vcallmsr` with CMSAR0 0x64) in the game's
    215-instruction upload. It makes the box's eight corners from its min and max quads (each corner's w is
    min.w), puts each one through +0x5b0's rows, and `vclipw.xyz`s it against its own w. Each `fcget` reads its
    corner's flags (four instructions after the clip, when they are ready). The flags are OR-ed and AND-ed into vi1
    and vi2 and masked to 6 bits.
  - The answer is 1 when every corner is beyond one same plane (AND not 0), 0 when no corner is beyond any (OR
    0), else 2. A flag is set for x > |w| or x < −|w| (the same for y and z), so a corner exactly on a plane is
    inside.
  - The runner now has a VU0 micro mode for this (`docs/checking.md`). The test loads the program from the game's
    own upload chain (0x31cac0) and runs the original.
- **`BxList_CullToCount` 0x17db00**(set, list, n): while the live list holds n nodes or more, it deletes the one with
  the least vf6 (+0x3c; unsigned; of equal ones the last) with vf0(3), then walks the list again. Nothing in the
  game calls it. With a negative n it never ends once the list is empty.
- **`WorldGrid_CellAtPos` 0x260b18**(&grid, p, &x, &y): with no grid, 0, and the coordinates are not written.
  Otherwise an INode's size is the TGroup size (+0x28) over the INodes a side (+0x44), made a whole number. The
  coordinates are (p − the grid's corner (+4, +8)) over that size, toward zero (the EE's division and
  conversions). They are written out, and the INode there is `WorldGrid_CellAt`'s (`data::ltg::Ltg::inode_at`).
  Checked on all twelve courses' grids at 4,000 points each, some exactly on INode corners.
- The other users of the `.ltg` grid are the world's walks over its cells. The snow's and the fog's are ported
  (G7c1, G7c2) and the line queries' too (E7a). Drawing the terrain and the objects belongs to G7a and G7b.

Checked in the runner. `CullBox` on 30,000 random renderers and boxes (all three answers well represented; a
third of them make the frame's matrices, some with −0 in the camera's and an exact identity clip matrix with
corners on the planes): 16 of 17 mutants caught, the one left equivalent (+0x510 from the camera's matrix instead of
its copy at +0x490). `BxList_CullToCount` in the list test's random sequences (1,232 nodes culled, keys often
shared): 6 of 6 caught. `WorldGrid_CellAtPos`: 5 of 6 caught. The one left (the INode size not made whole) is
equivalent on the game's courses, whose sizes are already whole numbers.

## The lights (row G7d1; `game/src/lights.rs`, checked: `tests/lights.rs`)

- **A light** is listed by each `.ltg` INode it touches (+0x44 its list, +0x2e the count). Its fields: +0 the kind
  (0 directional, 1 spot, 2 point, 3 ambient), +4 flags (0x100 local, 0x200 not dimmed), +0x10 r, g, b, +0x1c the
  direction, +0x28 the position, +0x4c the spot's cone (a cosine), +0x50 the falloff, +0x54 the spot's power (made a
  whole number toward zero).
- **The falloff** with distance d (VU0's `vsqrt` of all four lanes squared, so the point's w counts) is 1 below 100,
  else 100/d, 10⁴/d² or 10⁶/d³ for kinds 1, 2 and 3 (any other kind: 1). All three are exactly 1 at d = 100. A spot
  lights only where the cone (+0x4c) is below −(the way to the light · its direction). It is then × that value to its
  power, by repeated squaring: 1 for power 0, of 1/value for a negative power, 0 for a value of 0.
- **`World_GetLightsAt` 0x25a160**(world, pos, ambient, colours, dirs):
  - It starts with the ambient (1, 0, 0, 0) and three dark lights: colours 0, directions (1, 0, 0, 0), strengths 0.
  - The TGroup is (p − corner + size) / size − 1 per axis, clamped. With none, that is the answer. The INode is
    `World_GetCellAt` 0x260d08 of p's offset from the middle of the **unclamped** cell: n × (d + size/2) / size,
    clamped to 0..n−1.
  - Pass 1, the local lights (flag 0x100), point and spot only: their colour × falloff (× the spot's part) summed.
    With any found, each of r, g, b becomes clamp(sum + 1, 0, 1). Local lights only ever dim the rest, when their
    colours are negative.
  - Pass 2, every other light. An ambient one is added to the ambient's r, g, b. Directional (the direction
    negated), point and spot (the normalised way to the light) become colour quads (0, r, g, b) × falloff (× the
    spot's part) for `Lights_InsertStrongest`.
  - With local lights found, a light without flag 0x200 is first dimmed by the clamped sum. For an ambient light it
    is the whole ambient, again after each one is added.
  - The point's x and y are read as words, its quad (`lqc2`) at the address rounded down to 16.
- **`Lights_InsertStrongest` 0x25a028**(strengths, colours, dirs, dir, colour): the strength is |(r + g) + b| (the
  colour quad's last three). The three kept are in ascending order. A light only stronger than the weakest replaces
  it; one stronger than more of them moves those down a place. On a tie the old light stays.
- **`Lights_Build` 0x1ce140**(out, ambient, d0, c0, d1, c1, d2, c2) makes the renderer's block. +0 is a matrix with
  the three directions as its columns (w row (0, 0, 0, 1)). +0x40 holds four rows: the colours as (r, g, b, quad x)
  for c0, c1, c2 and the ambient, all × 128. `Lights_Clear` 0x1ce278: no lights, an ambient row of 128s, the
  matrix's w 1.
- `Mat4_RotateBy` 0x1ce310(a, b): a = b × a (b's rows through a's), its last row set to (0, 0, 0, 1).
- `cLightMan` 0x1ce0f0 keeps itself at 0x33efb8 (0x1ce400 answers it; the destructor clears it). Its vtable is
  0x391e20 and its four small virtuals do nothing.

Checked in the runner on 40,000 random cases: grids of 3 × 3 TGroups (some missing) with 2 × 2 INodes listing 24
lights of every kind, flag, falloff and power; points off 16-byte alignment, whole-number points with lights exactly
100 away. About 8,000 gatherings kept 7,500 lights. 35 of 42 mutants were caught. Five of the rest are equivalent:
the falloff boundary at 100, the clamp at exactly 1, the clamp without local lights (never used), a 0 base in the
power (it differs only for −0), and the ambient's × 1. Two are exact boundaries the random data does not reach: the
INode division's rounding, and a cone exactly at its cosine.
(G7d2a fixed `lights.rs` to compare the EE's values (`F::ee`, where Inf and NaN patterns are large numbers), as the
FPU does, instead of IEEE floats.)

## The lens flares' node (row G7d2a; `game/src/lensfx.rs`, checked: `tests/lensfx.rs`)

- **`cLensFXObjNode`** (0x2fdc bytes, vtable 0x391bd8): +0xc the frame's half, flipped as view 0 is drawn; +0x10 the
  half last finished; +0x14 the view; +0x20 the views of each half.
  - +0x30 + 0xc0 × half + 0x60 × view is a view's block: the renderer's +0xc0 quad (a struct return: the quad
    comes back through a0, `this` in a1), +0xf8's camera matrix copied, and +0xd0's three values (it writes them
    through three pointers).
  - +0x1b0 + 8 × half + 4 × view counts the flares. Their entries are at +0x1c0 + 0x1040 × half + 0x820 × view +
    0xd0 × k (+0 1, +0xc8 the depth, +0xcc the light).
  - +0x2fc0 is the flare texture (gApp+0x30's +0x70), +0x2fd8 a colour scale (1).
  - The constructor sets both halves to −1, registers the renderer callback 0x1cdc78 with the node (+0x318), and
    the destructor removes it (+0x318 with 0, 0). Its empty nested loops (constructing trivial members) change
    nothing.
- **`Draw` 0x1cbe20** works in this order:
  1. the view's block (each address taken before its call) and the half's view count (game+0x298);
  2. the flare count cleared;
  3. the sun's colours: its +0x3c, +0x40, +0x44, +0x24 and +0x10, each × +0x2fd8, to +0x2fc4..+0x2fd4;
  4. the renderer's +0x190 and +0x198 on the texture, to +0x18 and +0x1c;
  5. `LensFX_Draw`, the sun's part (0x1cd040, G7d2b), the flares' part (0x1cc190, G7d2c);
  6. after the half's last view, +0x10 is set to that half.
  - A half of −1 (as made) is only safe on view 0, which flips it to 0, and the game always draws view 0 first.
    On another view the block would land on the node's own first words.
- **`LensFX_Draw` 0x2006e8**(world, node): it walks the live cells in order and stops at the first empty one (the
  cells after a hole are never reached). It visits each slot the cell's mask names, and in it each light with a
  flare flag (+4 & 0x70) that the renderer's +0x270 does not hide (non-zero hides it). The light's depth is the
  third lane of its position through +0xf8's rows. When that depth is below the sun's +0x5c, the light goes to
  `LensFX_AddFlare`.
- **`LensFX_AddFlare` 0x1cc088**(node, light, depth): the view takes up to ten flares. When it is full, a nearer one
  replaces the farthest (the last of the deepest). With none deeper than 0 that index is −1, so a negative depth is
  written just before the first entry.

Checked in the runner on 20,000 random nodes (halves −1, 0 and 1; nodes off 16-byte alignment; full views; random
words as depths) and worlds (cells with holes). 25 of 27 mutants were caught. Of the two left, one is equivalent
(the position's w, always 1, in the depth) and one is a boundary (a depth exactly at the sun's limit).

## The sun's flare and the flares (rows G7d2b, G7d2c; `lensfx::sun_part`, `lensfx::flares_part`)

These are the two parts `Draw` runs after `LensFX_Draw`. They fill the blocks the renderer's callback (G7d2d) draws,
and are checked in `tests/lensfx.rs` with `Math_SinCos`, `Math_Atan`, `Math_Sin`, `Math_Cos`, `Vec3_Dot` and
`Vec3_Div` running as the original.

- **The sun, 0x1cd040** (block +0x2240 + 0x6c0 × half + 0x360 × view): nothing (+0 = 0) when the course's sun
  (game+0x2c) is off or the renderer's +0x270 hides it.
  - Its direction is (cos a cos b, sin a cos b, sin b) for its angles +0x28 and +0x2c (degrees × 0.017453292).
    It sits at its distance (+0x30) from the camera's origin in the camera's frame, or at the view's +0x88 − 1
    when that is nearer, with its size (+0x14) shrunk by the same ratio.
  - The block holds the projection (+0x278) and a 16 × 16 sprite box from it, clamped to the view. A glow box comes
    from the sun moved by its size along the camera's x axis and −y axis (twice the projected distance across).
    It also holds halves of the texture's two answers and the sun's six colour bytes over 255.
  - The outline: the sun's position in −1..1 screen terms, the four screen corners' angles from it (`Math_Atan`; π
    added for x < 0, ±π/2 straight up or down, 2π added below 0), and the 37 (angle, length) pairs at 0x391cf8 with
    the corners put in by angle. A corner takes the length of the pair before it; for the first pair that is the
    word at 0x391cf4, 0.
  - Each ray (`Math_SinCos`) runs to the nearest screen edge (10 when parallel). Its point is in pixels, then its
    length, then its fade, 1 − min(d², 3) ÷ 3 by the screen distance. The first point is repeated at the end;
    +0x88 counts them.
- **The flares, 0x1cc190**, for each kept flare entry:
  - The light's projection; a 16 × 8 sprite box (8 and 4 back), clamped; and the depth of the light moved toward
    the camera (the matrix's z column) by its kind's reach.
  - The kinds (flags & 0x70): 0x10 size 100.74 and reach 300; 0x20 200.08 and 500; 0x40 500 and 800. Any other
    value keeps the last flare's (the first takes the two registers as the caller left them: STANDIN 0 from
    `Draw`).
  - Two glow boxes: the size, then the size × (the sun's +0x34 + 1.0095 × ((√2 − r) × 0.7071)⁴), where r is the
    flare's distance from the screen's middle in −1..1 terms.
  - A streak's four corners (Q ∓ U ∓ V): U and V are the camera's axes turned by −(screen x × π/2) and that + π/2
    (`Math_Cos`, `Math_Sin`, the second pair negated), sized by the sun's +0x38 + 1.2523 × the fourth power.
  - The light's colour (+0x10.. × +8) normalised (left as it is at length 0). An alpha of 1 that, past half the
    sun's limit (+0x5c), falls to 0 at the limit.
  - The streak reads the projected quad with `lqc2`, rounded down to 16 bytes, while the boxes read its words as
    they are (a node off a 16-byte boundary showed the difference).

Checked in the runner on 20,000 cases (3,400 suns drawn, flares of every kind, sizes left in the registers, nodes off
16-byte alignment). The sun: 25 of 31 mutants caught. The six left are three equivalent (the sun's w, set to 1 after;
a fade capped at exactly 3) and three exact boundaries (the view's near value equal to the sun's distance, a sun
exactly on the screen's edge, an angle exactly 0). The flares: 27 of 29 caught. The two left are a depth exactly at
half the limit (1 either way) and a colour length of exactly 0.

## The lens FX callback (row G7d2d; `lensfx::callback`)

- **0x1cdc78** (a jump to 0x1cdc80) is what the node registers with the renderer (+0x318). It works on the half
  last finished (+0x10; none while −1).
  - First pass: for the sun's block (when on) and for each kept flare entry with a non-zero +0 (flags 0 skipped),
    it asks the node's own +0x50. The arguments are the sprite box clamped (+0x34) and not (+0x24), and f12 the
    depth (+0x20); the float answer goes to +0x44. Any non-zero answer (the FPU's compare, so −0 counts as 0)
    means something shows.
  - Then, only if something shows: +0x58. For each view: +0x60 with the view's block +0x30, the sun's +0x70 (when
    on), +0x68 for **every** kept flare (shown or not, +0 not checked), +0x78. Then +0x80.
  - The base class's slots there are pure virtuals. They are the PS2 node's (vtable 0x393ea8, row G7d2e): +0x50
    0x1ea520, +0x58 0x1ea990, +0x60 0x1eaa68, +0x68 0x1eab90, +0x70 0x1eb0a8, +0x78 0x1eab88 (empty), +0x80
    0x1eaa40.
- 0x1cdfc0 sets the colour scale (+0x2fd8) clamped to 0..1; 0x1ce000 reads it. 0x1ce058, 0x1ce088 and 0x1ce0c0 are
  copies of `Vec4_Scale`, `Vec3_Div` and `Vec3_Dot`.

Checked in the runner (`tests/lensfx.rs`, 826 callbacks drawing, answers of 0 and −0 among them). 14 of 15 mutants
were caught; the one left is equivalent (a scale of exactly 1).

## The PS2 lens FX node (row G7d2e; `game/src/ps2lensfx.rs`, checked: `tests/ps2lensfx.rs`)

- **`LensFX_Create` 0x1ea408**(a0, flags) gets 0x33c0 bytes. With flag 0x4000_0000 they come from the pool
  (0x23d3b0 with 0x345e48, the size, the flags, the class name 0x393ea0, 0, 0); otherwise from `EAMem_Alloc` with
  the low 26 bits of the flags. They are filled with 0 (flag bit 31) or 0xdeadc0de. The node is then constructed
  with a0 even when nothing was got (its vtable written at address 8). `cPS2LensFXObjNode` is the lens FX node with
  vtable 0x393ea8.
- **The packets** go into the renderer's buffer (+0x694), kept at the node's +0x3350 with doublewords counted at
  +0x3354. Screen positions are 12.4 fixed point about 2048 − half the screen (+0x698, +0x69c). The 32-bit sums
  and shifts are sign-extended into the 64-bit fields, as the EE does.
  - begin (+0x58): `cnt` 5, `DIRECT` 4, A+D TEST_1 0x30000, ZBUF_1 (+0x1710 | (+0x6a4 − 0x30) << 24 | the mask
    bit), ALPHA_1 0x48.
  - set view (+0x60): the view's block kept at +0x3360..; `cnt` 3, FLUSHE, `DIRECT` 2, SCISSOR_1 from its box.
  - a flare (+0x68): `cnt` 15. Two textured blended sprites (PRIM 0x156) for its glow boxes, then a four-corner
    strip (PRIM 0x154) for its streak (corners in order 0, 1, 3, 2). The colour is +0x4c.. × 128 with Q 1. The
    alphas are +0x48 × the part showing × the colour scales +0x2fc4, +0x2fc8, +0x2fcc × 128. The glows sit at the
    flare's depth, the streak at 1.
  - the sun (+0x70): its glow sprite, then a PRIM tag and a fan of untextured Gouraud triangles (PRIM 0x14b), one
    per neighbouring pair of outline points: the centre and the two points. The alphas are the part showing ×
    +0x2fd4 × 128 × the first point's length, × each point's fade at the points. The headers are written even
    with fewer than two points.
  - end view (+0x78) does nothing; end (+0x80) writes the DMA `end` tag.
- **The visibility (+0x50, 0x1ea520)** reads the depth buffer back:
  1. a FINISH packet (+0x2fe0) and a BITBLTBUF/TRXPOS/TRXREG/TRXDIR packet (+0x3000) for the flare's box, from the
     depth buffer (+0x1710, and the frame's +0x80 and +0x8c of +0x654 + 0x94 × +0x650);
  2. `FlushCache`, then waits until DMA channels 1 and 2, VIF1 and the GS FIFO are idle;
  3. FINISH sent and waited for, then the transfer packet sent;
  4. VIF1's FIFO and the GS bus turned toward the EE, channel 1 into +0x3050 for w × 3h ÷ 16 quadwords, waited
     for, and turned back.
  - It then counts the box's 24-bit depths inside the clamped box that are not nearer than the flare's own
    (`__fixunssfsi` of its depth). When the clamped box is the whole box, the answer is that share. Otherwise it is
    (n − half the box) ÷ half, at least 0.
  - The runner treats the hardware registers as plain memory, and a DMA start finishes at once (the test supplies
    the pixels the read-back would bring). The port does its register reads and writes through the env.

Checked in the runner on 20,000 cases: all ten functions, packets under way, NaN and huge depths, negative frame
widths, ZBUF formats below 0x30, boxes of no size, views off 16-byte alignment. 28 of 34 mutants were caught. Five
of the rest are equivalent: sign bits a shift of 32 or 48 drops, an all-ones GIF tag either way, the node's matrix
always on 16 bytes, a width's arithmetic shift whose top bits the next shift drops. One is unreached: the sign of a
read-back size beyond 2³¹.

## The objects' draws (row G7b1; `game/src/objdraw.rs`, checked: `tests/objdraw.rs`)

- **The model draw** is the renderer's +0x2a8 (`cPS2GraphicsMan::DrawModel`, G7b2), with (model, part, matrix,
  light block, bones, 0). The part is the model's +0xc for the course's instances, the instance's +0xf0 for animated
  ones. A course instance keeps its own light block at +0x40 and its matrix at +0.
- **`Instance_DrawVisibleCells` 0x200390**(world):
  - It sets the renderer's +0x1c8 (0, 2, 0.01) and +0x1e0 (3), and clears the first words of the 128 12-byte
    entries at 0x31d938 (from the last).
  - It walks the live cells in order, stopping at the first empty one, and in each the slots its mask (+0x36)
    names. A slot's instances are counted by +0x26, or +0x28 in show-off modes 3 and 5 (0x32de8c).
  - It draws each instance with +0xe8 bits 0 and 1 both set. The box test (+0x280, outside = 1) is skipped when
    the cell's +0x32 bit says the slot is wholly in view.
  - Then +0x1c8 (0, 2, 0.08), and the light manager's (0x33efb8) +0x78 with 0.
- **`Instance_Draw` 0x200680**(world, inst, matrix, lights) draws the instance's model (+0xc0, part its +0xc) when
  +0xe8 bit 0 is set.
- **`cRollerNode::Draw` 0x13f9f8** needs its instance shown (+0xe8 bit 2) and its box (+0x2a0, +0x2b0) not
  outside. Its matrix is `Mat4_FromPosQuat` of `RollerNode_PlaceFromBody` (both in `roller.rs`). The lights at the
  matrix's place are gathered (`World_GetLightsAt`), built, and turned by the matrix (`Mat4_RotateBy`), then the
  instance goes through `Instance_Draw` with them.
- **`cMeshAnimNode::Draw` 0x147518** needs to be on (+0x6cc). The box test is skipped once it has shown (+0x6d0,
  then set).
  - Each bone is a matrix from the quaternion at +0x38 + 16k (w, x, y, z) and the position at +0x3c0 + 16k.
    The rows are (1 − 2y² − 2z², 2xy + 2wz, 2xz − 2wy), (2xy − 2wz, 1 − 2x² − 2z², 2yz + 2wx) and (2xz + 2wy,
    2yz − 2wx, 1 − 2x² − 2y²), w 0, made from the doubled components as the game orders them.
  - It is drawn with the world's matrix 0x346440. The light block is the instance's own when the model's +0x10
    bit 1 is set. Otherwise it is the lights at +0x6c0's matrix's place (or the instance's), turned by the
    instance's matrix.
- **`cAnimObjectNode` vf28 0x199320**(node, matrix, test) tests its instance's (+0x58) box only with `test`.
  - Its bones are the model's +8 table, 0x18 a bone: +0 the parent (−1 none), +0x10 animated, +0x14 the bind
    pose. Each is the animation's next matrix (0x1998d0(node, the count of animated ones so far)) or the bind
    pose, times its parent's.
  - It is drawn between +0x1c8 (0, 2, 0.01) and (0, 2, 0.08). The light block is the instance's own when its
    +0x30 object's +0x80 answers non-zero or the model asks; otherwise the lights at the matrix's place, turned by
    it.
- The game builds these matrices and blocks on its stack. The port builds them in a caller's scratch area
  (STANDIN for the frame; the same bytes), and the test compares what the model draw is given by content.

Checked in the runner on 12,000 random worlds, instances, models with bone tables, a roller, an animated mesh and
an animated object, with the light gathering running as the original. 7,000 model draws were compared, and 26 of 27
mutants caught. The one left is equivalent: x × 2z against z × 2x, both exact.


## Names from F1c batch 17 (proven)

- `Crowd_ClearTextures` 0x145ae8 (on course unload): clears `DAT_0031d370` and the 16 crowd texture ids at 0x31d4b8
  (0x48 apart).
- `ParticleNode_InitMem` 0x1d8488 (the identity constructor that pairs with `ParticleNode_FreeMem`),
  `SplinePathNode_FreeMem` 0x1fb3c8.
- Not settled: the empty 0x259028, 0x2520a0; draws 0x1ab028, 0x244ee8, 0x249250.

## The renderer's model draw (row G7b2; `game/src/drawmodel.rs`, checked: `tests/drawmodel.rs`)

- **`cPS2GraphicsMan::DrawModel` 0x1e1f70**(r, model, part, matrix, lights, bones, scroll) is the renderer's
  +0x2a8. It begins with the frame's matrices, shared with `CullBox` (`cull::frame_matrices`). It resets the texture
  state cache (+0x470: its +0x10 byte to 5 and +0 to 0, with +0x474 1) unless it is already so, and calls +0x1e0
  (3).
- **The matrices**:
  - Without bones: the matrix with its rows scaled by the model's scale (+0x18, +0x1c, +0x20, and 1 for the last
    row). Unless the model has fewer than two bones and its first is posed at the world's matrix 0x346440, it also
    builds bones from the bind poses (+0x14 of each 0x18-byte record), each times its parent's (+0 the parent,
    −1 none).
  - With bones (given or built), for each bone with a mesh: the matrix times that bone with its rows scaled.
- **Each batch** of a bone's mesh (+0x20 of them, listed at +0x28):
  - The texture (+0x1f0 with the id at +0 of the part's texture, the batch's +4 into the part's table at +4, and
    0).
  - The blend (+0x1e8):
    - fogged (texture flags 0x40000) with fog on (+0x640): (2, the fog value). The matrix's place goes through the
      clip matrix +0x5b0 after the renderer's matrix update (0x1e85b0); its w × +0x630's w, clamped to 0..+0x630's
      x, made whole, low 16 bits.
    - fogged with fog off: (2, 1).
    - the second blend (0x20000): (1, 0).
    - otherwise: (0, 0).
  - With a scroll, the batch's own packet (+0x10) gets the VIF code 0x3000_0000_0500_0001 and the scroll × 4096 as
    two integers. The first is sign-extended into the doubleword, as the EE does.
  - The packet at the renderer's +0x680:
    - `cnt` 12, then UNPACK (0x6c0b_8000) of the matrix (4 quads) and of the light block's rows 0..2 and 4..7
      (the directions' matrix without its last row, the colours);
    - a DMA `call` (0x5000_0000 | the batch's packet +0xc << 32);
    - submitted (0x1e8838 with the renderer, the packet, its end + 0xe0, 0), and the place moved on 0xf0.
- The material's blend is only the mode given to +0x1e8. The GS's ALPHA registers come from that virtual and from
  the models' own prebuilt packets.

Checked in the runner on 20,000 cases: 55,000 packets and 13,500 fogged blends, with bind poses at the world's
matrix, negative bone counts, given bones, scrolls and fog caps beyond 16 bits. 23 of 26 mutants were caught. The
three left are equivalent: one bone posed at the world's identity matrix gives the same scaled matrix whether or
not the bones are built; skipping a mesh with no batches only saves the matrix work; and a w of exactly 0 clamps to
0 either way.

## The terrain's draw (row G7a1; `game/src/terraindraw.rs`, checked: `tests/terraindraw.rs`)

- **`cTerrainObjNode::Draw` 0x1c7810** sets the draw distance: 30000, or the course's (game+0x2c) +0x60, or +0x64
  with two views or more. Without a game it still reads the course from address 0x2c. It visits the TGroups near
  the camera (`World_VisitCellsNear` 0x260278 with (distance − 1) ÷ 10000 + 1, G7a2), then their visibility
  (`World_CullVisibility` 0x260458), then tail-calls `Patch_DrawVisibleCells`.
- **`Patch_DrawVisibleCells` 0x2008f8**(world):
  - The LOD line comes from the Bezier manager (the renderer's +0x20 object; its +0x30 and +0x38 the near and far
    distances): k = −2 ÷ (2 far² − 2 near²) (the EE's division), base = 3 − k × 2 near².
  - The camera's place in its own frame is the camera matrix's rows times its last row negated (`Vec4_Scale`).
    Then the renderer's +0x2c0.
  - It walks the live cells and the slots their masks name. Each slot's patches are +0x24 of them, listed at +0x34.
    Show-off patches (+0x1ac negative) are drawn only in show-off modes 3 and 5. Each patch is box-tested (+0x150,
    +0x15c) unless the cell's +0x32 bit says the slot is wholly in view: 1 skips it; 2 asks +0x290, clipped when
    that is non-zero; 3 counts as clipped.
  - Each edge's LOD is base + k × (the two corners' distances² from the camera; corners at +0x168 + 0x10 k),
    clamped to 1..3 and made whole (toward zero). The edges go in the order 0–1, 2–3, 0–2, 1–3. The steps come
    from the 4-byte table 0x39afb8: 4, 4, 6, 8.
  - The renderer's patch draw +0x2d0 (G7a3) gets the patch, the four steps, its two shorts +0x1b0 and +0x1b2
    (sign-extended) and the clipped flag.

Checked in the runner on 20,000 random worlds and cameras (44,000 patches, edges at all three steps). 19 of 23
mutants were caught. Two of the rest are equivalent (the clamps at exactly 1 and exactly 3). Two are exact
boundaries: the division rounded to nearest, and the distance summed in another order, move the LOD by an ulp,
which matters only at a whole number.

## The visible list of TGroups (row G7a2; `terraindraw::visit_cells_near`, `cull_visibility`)

- The world's visible list (`VisList`, world+0x3c) holds up to 0xa2 TGroup pointers with a count at +0x288. The
  "cells" the draws walk are TGroups. `VisList_Clear` 0x25ec88 zeroes the entries from the last and the count;
  `VisList_Add` 0x25ecc8 appends at the count with no bound. `CellList_Clear` 0x260f90 and `CellList_Add` 0x260f98
  are those on world+0x3c. The walks stop at the first empty entry, and what lies past it is never read.
- **`World_VisitCellsNear` 0x260278**(world, pos, r) clears the list.
  - The camera's TGroup is (p − corner + size) ÷ size − 1 per axis, using the grid of the world at 0x346488,
    whatever world is passed.
  - The square of ±r around it is clamped to 0..w−1 and 0..h−1. Its TGroups are put in the list column by
    column, using the passed world's grid and skipping empty ones.
- **`World_CullVisibility` 0x260458**(world, cam, distance) box-tests each listed TGroup (its +0 and +0xc) and
  first clears its slot marks (+0x36 shown, +0x32 wholly in view):
  - wholly in view: both set to all slots;
  - partly in view: each slot holding anything (the words +0x24 and +0x28, the half +0x2c, the word +0x48 not all
    0) is box-tested on its own +0 and +0xc. In view sets both of its bits, partly in view only the shown one;
  - outside, or any other answer: nothing.
  - The camera and the distance are not used.

Checked in the runner (`tests/terraindraw.rs`), on their own and inside the terrain's draw, which now runs them as
the game does. 16 of 17 mutants were caught; the one left is equivalent (a square's low corner at exactly 0).


## The Bezier manager's queue (row G7a3a; `game/src/bezier.rs`, checked: `tests/bezier.rs`)

- **`cPS2BezierMan`** (vtable 0x392fd8; `cBezierMan`'s is 0x3930d8) is the renderer's object at +0x484, which the
  renderer's +0x20 (0x1ea068) hands out. Its constructor 0x1d9be0 sets the vtable, spins an empty 8-turn loop, and
  runs vf1. The destructor 0x1d9c40 sets the vtable, runs vf2, then `cBezierMan`'s (0x1db908): that vtable, and
  the delete 0x114f80 (this, 1) when flag 1 is set.
- **vf1 0x1d9c88** stores +0x24 0, +0x20 1, +0x28 8.0, the near and far LOD distances +0x10 3000 and +0x14 15000,
  +0x18 1 and +0x1c 1. Then `RendPatch_Alloc` 0x1db150, +0x530 (the tables) 0, and a tail call to
  `Render_BuildBezierTables` 0x1db1e8 (row G7a3c).
  - `RendPatch_Alloc` asks the allocator 0x2ccf70 for 0xc000 bytes under the name at 0x392fb8. When it gets them it
    fills them with 0xdeadc0ed (0x2cc808). It keeps the pointer at +0x544 either way, then calls its own +0x78
    (begin) through the vtable.
- **vf2 0x1d9cf8** frees the records (0x1db1c8, when the pointer isn't 0; the pointer is left) and the tables
  (0x1db4f8: freed when not 0, then 0). Both go through `Mem_Free` 0x14a040.
- **Accessors 0x1db840..0x1db894:** set and get +0x18, +0x1c, +0x20 (+0x24 only set), the two distances as floats,
  and words +0x534..+0x540 (get only).
- **Begin, vf14 0x1d9d20 (+0x78):** zeroes the view count +0x2c, the record count +0x548, and four lists, each a
  count and a head: +0x54c/+0x550, +0x554/+0x558, +0x55c/+0x560, +0x564/+0x568.
- **The queue, vf16 0x1d9d50 (+0x88)**(patch, steps, shorts, clip) takes the next 0x18-byte record and counts it.
  - The record gets +4 3 when clipped (else 0), +0x10 the patch, and +8..+0xb the four step bytes.
  - +0xc gets the first short's low half. +0xe gets the low half of the word at shorts+4, or −1 when that word is
    negative or switch +0x1c is off. The sign is read before +0xc is written.
  - When all four steps are equal it links the record at the head of the list for 4 (kind 0), 6 (1) or 8 (2). Any
    other equal value links it nowhere; the record is still counted. Mixed steps go to list +0x564 (kind 3).
  - Linking stores the old head at +0x14 and the kind at +0, bumps the list's count, and makes the record the head.
- **The views, vf17 0x1d9eb8**(quad, a, b, word, f) fill view n = +0x2c, at this + 0xa0·n:
  - +0x30 the quad, +0x40 the matrix a, +0x80 the matrix b, all copied with lq/sq so both sides round down to 16;
  - +0xc4 the word, +0xc0 the float. Then the count goes up.
- **The renderer's +0x2c0 0x1e9068** calls the manager's +0x78 with the manager as its second word too. **Its
  +0x2d0 0x1e9008** passes the patch, steps, shorts and clip through to the manager's +0x88. The terrain's draw
  reaches the queue that way.

Checked in the runner (`tests/bezier.rs`, 40,000 cases), with the game's vtable or a stand-in, and the real begin
and queue or logging stand-ins. 46 of 49 mutants were caught. The three left are equivalent:
- the alloc and the +0x530 clear swapped;
- the destructor's first vtable store, overwritten before it is read;
- begin's this when the slot is the real one, whose adjust is 0.

## The Bezier queue drawn (row G7a3b; `bezier::draw`, `bezier::draw_views`, checked: `tests/bezierdraw.rs`)

- **vf15 0x1d9f58**(this, renderer) lays the four lists out at +0x56c as four entries of four words: the count, the
  head, a program kind (3, 2, 1 and 4: the texture state's byte at +0x10) and a program table (0x3d0f58, 0x3d0f40,
  0x3d0f28, 0x3d0f10). Then it runs:
  1. the frame's matrices (0x1e85b0);
  2. the views' draw when +0x20 is set;
  3. the renderer's +0x1c8 (1, 0, 0.08), +0x200 (1, 1), +0x1f8 (0, 0) and +0x1f8 (2, 1);
  4. each non-empty list in turn, then the close below.
- **Each list:** when the texture state's byte isn't the list's kind, or its word is set, the byte takes the kind,
  the word goes to 0 and the renderer's +0x474 to 1. Then each record in turn:
  - Its two textures (+0xc and +0xe) are bound through the renderer's +0x1f0 (texture, slot 0 or 1) when they
    differ from the last ones bound. The last ones start at 0xffff and are kept as halves, but each new one is read
    as signed, so a texture of 0x8000 or more is bound every time.
  - Its packet is written at the renderer's +0x680, which then moves on. A uniform patch's packet is 0x90 bytes; a
    mixed one's is 0xa0 and has its four steps as words at +0x20.
  - The packet holds the patch's address, then a word built from the patch's first three floats ×128 made whole (x,
    y, z): x << 4, (x + z − 1) << 14 | 0xa, y << 24, (y + z − 1) << 34.
  - The program is the list's table word at the record's clip kind + 1, or'd with 0x14000000.
  - It is sent with 0x1e8838 (renderer, packet, packet + 0x80 or + 0x90, 0).
- **The close:** +0x1f0 (−1, 0), +0x1f0 (−1, 1), +0x1c8 (0, 2, 0.08), +0x1f8 (0, 1). The texture state's program is
  cleared (word 0, +0x474 1, byte 0) when either the byte or the word is set.
- **The views' draw 0x1da5c8**(this, renderer) does nothing when there are no views. Otherwise it runs +0x1f8 (0, 0),
  +0x1f8 (2, 1) and the frame's matrices, then goes through the same lists with textures tracked on their own.
  - Each patch's box (+0x150 and +0x15c, each with w 1) is box-tested with the renderer's +0x288 against each view's
    first matrix (+0x40 of the view). Views that don't answer 1 (outside) are kept. The game builds the box on its
    stack; the port uses a scratch area (STANDIN).
  - A patch seen by no view is left in its list.
  - A patch seen by some view is drawn once as vf15 would, between +0x200 (1, 1) and +0x200 (2, 1). Then each seeing
    view gets a pass: the view's second matrix (+0x80, eight doublewords) and the program at the clip kind + 2, or'd
    with 0x15000000. The view's texture (+0xc4) is bound in slot 1 when it changed, compared as a full word.
  - Finally the patch is taken out of its list: the entry's count goes down, and the head, or the last kept record's
    +0x14, skips it.
  - It ends with the same close.

Checked in the runner (`tests/bezierdraw.rs`, 20,000 cases). The random lists are linked over shared patches, and
up to eight views each have a texture. The box test answers by a hash of what it is given. A submit also logs the
texture state, since the close would hide it otherwise. 60 of 61 mutants were caught; the last one pointed at a
redundant `break` on the loop's final pass, which is now gone.

## The Bezier tables (row G7a3c; `bezier::build_tables`, `fill_weights`, `static_init`, checked: `tests/beziertables.rs`)

- **`Render_BuildBezierTables` 0x1db1e8**(this) is what the manager's init calls last.
  - It frees the old tables (+0x530, then 0) and asks the allocator for 0xe00 bytes under the name at 0x392fc8,
    filling them with 0xdeadc0ed when it gets them.
  - It keeps the start at +0x530 and the four parts at +0x534 (the start), +0x538 (+0x220), +0x53c (+0x580) and
    +0x540 (+0xaa0).
  - Each part is a copy of the weights followed by a grid of (u, v, 1, 1). The grids are 4 × 4 points d = +0x28 ÷
    128 ÷ 3 apart, 6 × 6 at ÷ 5, 8 × 8 at ÷ 7, and the 6 × 6 inside of the 8 (from d, not 0).
  - Each u and v is summed step by step, and d is the two multiplies in that order. With the default +0x28 of 8.0,
    d is 1/48, 1/80 and 1/112.
  - The place it fills from is a word on the game's stack; the port takes it as an argument (STANDIN).
- **`Render_FillBezierWeights` 0x1db530**(this, &at) writes, at the place `at` holds, the cubic's powers (t³, t²,
  t, 1). t runs from 0 in steps of 1/7 (8 values), then 1/5 (6) and 1/3 (4), summed, with t² = t·t and t³ = t²·t.
  It then moves `at` on by 0x120. `this` is not used.
- **`StaticInit_Render` 0x1db640** fills the four lists' VU1 program tables (0x3d0f58, 0x3d0f40, 0x3d0f28,
  0x3d0f10) with micro program places in 8-byte units: 0xa, 0, 0x10, 0xd, 5, and 0x1c (0x30 for the mixed list's
  table).
  - Where a record's clip kind 0 or 3 indexes them, the patch draws at +1 use 0 or 5, and the view passes at +2 use
    0x10 or 0x1c / 0x30.
  - The manager's init still reaches the tables' build as a call out (`BUILD_TABLES`), as its own test checks.

Checked in the runner (`tests/beziertables.rs`, 6000 cases), with +0x28 any float (denormal, huge, NaN bits) and the
allocator answering 0 or a place. 26 of 29 mutants were caught. The three left are equivalent:
- t²·t as t·t²;
- the two multiplies regrouped: by 2⁻⁷ it is exact while normal, and both sides flush to 0 otherwise;
- the +0x530 clear, overwritten before it is read.
