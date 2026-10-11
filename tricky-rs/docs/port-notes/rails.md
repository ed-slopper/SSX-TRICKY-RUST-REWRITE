# Rails: how the original works

Part of the port notes (index: [README.md](README.md)); units and wording as described there.

## Rails (`RailSlideMotion_Update` 0x10b0d0, `RailSlideControl_Update` 0x1073e0, `Rail_TryCapture` 0x125cb8)

- Stick alone balances. Boost held + left/right starts a quarter turn (clips 0x258–0x25f), committed at clip end;
  holding chains turns. Types cycle +90°: 1→4→2→3→1 (1 = 50-50, 2 = fakie 50-50 toggling switch, 3/4 slides).
- Balance: g = v<555.56 ? 0.5 : 0.0009v; gc = clamp(g,0.5,2); h = clamp(1/g²,1,4); k = −5 for the first 0.6 s or
  steep rails (|tan.z|>0.92) else +1.8153523; |off|<2.5 cm → k = −1 (if k > −1); k ≥ 0 → off clamped ±20 cm.
  lateral velocity = −s·145.07674·gc − k·h·off cm/s (s = smoothed steer).
- Fall off when |d·row0| ≥ 150w+80(1−w), |d·row1| ≥ 50, |d·row2| ≥ 50w+30(1−w) (w = 1 until 0.6 s then → 0 at
  1.667/s): v −= 277.8 cm/s away from the rail, into the air, not a crash; trick still scored.
- Motion: velocity blended to the tangent at 30·dt; gravity along the tangent; no friction, cap or minimum (slow
  riders roll back). Boost: per press Δv = 0.408·level m/s, cost 0.00075 meter; held boost only while crouched.
- Capture: same box ×0.9; v = tangent·min(|v|, max(555.56, v·t)); vertical ×0.1.

## Rails (more), board classes, circuit

- Hop-off: prewind works on rails (wind-up rate doubled, 4.001), jump Δv = max(630.88, ...) along the board's up
  (rail normal tilted by the balance offset); spins/flips from rails as on the ground. Running off the end: no
  velocity change, into the air. Grind scored the same on every exit. Re-capture weight −0.25 → 1 at 1.667/s.
- Balance sign: positive steer moves the rider to the left of travel (same as turning left on snow).
- Board class: 1 freestyle, 0 BX, 2 alpine. Switch penalty (drag & grip) none / ×0.85 / ×0.70. Yaw-to-velocity
  (a, b): BX (0.30019, 0.34919), freestyle (0.4, 0.52398), alpine (0.20834, 0.26221). Alpine lean 65° (else 50°).
  Per character boards: index 1 alpine, index 6 BX, others freestyle. Board unlock points 0, 0, 5, 20, 35, 60, 90,
  120, 160, 200, 240, 999.
- World Circuit: tracks gari, snowdream, elysium, mesablanca, merqury, tokyo, aloha, alaska. Race: 3 rounds of 6
  riders, top 3 advance; medal = place in the final (gold/silver/bronze = 15/10/5 points). Show-off: medal by score.
  A medal unlocks the next track of that event; Alaska race medal → Untracked, Alaska show-off medal → Pipedream.
