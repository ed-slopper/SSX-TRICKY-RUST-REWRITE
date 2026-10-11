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
