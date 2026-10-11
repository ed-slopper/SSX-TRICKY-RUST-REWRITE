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
| 3 | message the controller: its vf 0xc4(p3, p2), if the instance has one | (inline) |
| 4 | wait p2 seconds (thread+0x40) | (inline) |
| 5 | condition p2: 0 speed (p3 0: |v| > p4 cm/s, 1: |v| < p4 km/h), 1 random (`Rand_GameRange`(0, 1) against p4; p3 picks which side passes), 2 human only (rider+0x41c), 3 stop if the instance already has a controller | (inline) |
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
