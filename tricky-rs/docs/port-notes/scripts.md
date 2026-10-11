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
