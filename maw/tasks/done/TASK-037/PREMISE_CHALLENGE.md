# PREMISE_CHALLENGE — TASK-037

## 1. Counter-example tested

The premise says the lock is **geometry**: "two bodies in a box leave no pass corridor, and nothing owns
that case", so the fix is a new re-route / U-turn / box-pass mechanism.

Counter-example: in the G4 liveness / R1 scene, the approach cars standing at the box entries
(`Connector(719)` s 0 at (-4, -86), end of lane 299 at (0, -72)) are **not** geometrically blocked by the
left body. They stand because a *rule* holds them: the left (abandoned) car's box occupancy / junction
grant conflicts with connectors whose swept path does not physically touch its body (an over-broad
conflict set), or a grant is never released to them. If that is true, a car could drive a free exit today,
the "no pass corridor" framing is wrong, and the root cause is a conflict/lease rule, not missing
geometry handling.

(Section 1 was written before reading any code.)

## 2. Primary-source investigation

Ran (HEAD be5d8cf, branch feature/box-lock-in-view, clean tree):

```
cargo test -p gta_sim --test traffic_junction_box seed_1_box_keeps_moving_liveness -- --ignored --nocapture
```

Real output (verbatim, abridged to the verdict lines):

```
seed 1: worst stale holder 0.67 s, worst stand 100.0 s at (11.4, -81.2), extra car stood None s, 6401 grant ticks at the box, G1 max depth 0.000
panicked at crates\gta_sim\tests\traffic_junction_box.rs:284:5:
seed 1: AI cars stood > 40 s: [(1935v0, 100.0, (11.42, -81.15)), (2029v0, 99.3, (17.50, -80.99)),
 (2109v0, 97.7, (-4.43, -86.73)), (2108v0, 97.6, (23.58, -80.83)), ... (2021v0, 93.9, (-0.71, -72.78)),
 ... 13 cars on the east, south-west and north approaches]; an AI car stood 95.5 s in Dynamic (> 30 s)
test result: FAILED. 0 passed; 1 failed
```

So the lock reproduces as the spec says (13 cars, three approaches, 95.5 s in `Dynamic`). The lease is
fine: worst stale holder 0.67 s. My counter-example (an over-broad conflict set or a grant not
released) is **not** supported: the lease works and the grants keep turning over (6401 grant ticks).

Code read at the rules the premise's evidence points at:

- `crates/gta_sim/src/traffic/box_rules.rs:1-3` (module doc), `:96-219` `plan_box_pass`,
  `crates/gta_sim/src/traffic/manoeuvre.rs:160-181` (a car "standing behind a body in the box, alone
  there: around it, holding the whole box"), `crates/gta_sim/src/traffic/junction.rs:315-325` (grant
  of the whole box when "a body standing on every way out"), `junction.rs:222-250` + `box_rules.rs:52-67`
  (`repick`, another exit). **A box pass and a re-route already exist** (TASK-032, commit c787db3).
- Every one of those mechanisms is gated to `Kinematic` cars:
  - `manoeuvre.rs:115` `let idle = !snap.dynamic && car.mode == TrafficMode::Kinematic && ...`; the pass
    / box pass only runs for `idle` (`manoeuvre.rs:158`).
  - `junction.rs:204-205`: a `Dynamic` car on a connector is skipped outright
    (`Segment::Connector(_) => continue`): no queue, no repick.
- The only way back from `Dynamic` to `Kinematic` is `recover.rs:154` (`Recovery::Recover`, applied at
  `drive.rs:616-626`, no other `mode = TrafficMode::Kinematic` in `traffic/`). It requires
  `on_lane = matches!(snap.car.segment, Segment::Lane(_))` (`recover.rs:144-145`). **A `Dynamic` car on
  a connector can never recover**, whatever the geometry around it.
- `led` (`recover.rs:115-135`) gates only the give-up (`recover.rs:156`), not recovery. The premise's
  "both led, so they never give up or recover" is wrong for recovery: the connector head at
  `Connector(719)` s 0 (test output (-4.43, -86.73)) cannot recover because it is on a connector, and a
  `Dynamic` car on its lane line (not `off_lane`) never gives up either (`recover.rs:156`).
- A kinematic car switches to `Dynamic` when it comes within the switch skin of any dynamic body
  (`contact.rs:170-205`); the left car is a plain `Dynamic` vehicle (`tests/traffic_support/mod.rs:557-571`,
  `vehicle_bundle`), so the car that stops next to it at the box entry turns `Dynamic`.

## 3. Did it hold

My specific counter-example did not hold: no evidence of an over-broad conflict set or an unreleased
grant (worst stale holder 0.67 s).

But the investigation found positive evidence against the premise's framing of the root cause:

1. "Nothing owns that case" is false at code level: `plan_box_pass` + whole-box grant and `repick`
   own it (box_rules.rs:1-3). They exist and do not fire. Redesign direction 2 ("pass through the box
   ... as lane passes do") describes a mechanism already in the tree.
2. They do not fire because the box-entry heads are `Dynamic` (the run: 95.5 s in `Dynamic`; the spec's
   own G4 evidence: 95.5 s of 97.7 s), and every box rule, existing or proposed in the spec (re-route
   of queue heads, box pass), acts only on `Kinematic` cars (manoeuvre.rs:115, junction.rs:204-205).
3. A `Dynamic` car on a connector has no path back to `Kinematic` at all (recover.rs:144-145), and one
   on its line never gives up (recover.rs:156). That is a permanent state by rule, independent of whether
   the box geometry leaves a corridor. The spec explains non-recovery with `led`, which the recovery
   code does not read.

So a planner that follows the spec's direction (extend re-route to cars further back, add a U-turn, add
a box pass) can ship all of it and the gated stands stay: the heads that hold every queue are in a mode
none of those rules touch. The "geometry" diagnosis may still be true for the physical corridor, but it
is not the whole cause of the gated failure.

## 4. Verdict

PREMISE SUSPECT — `cargo test -p gta_sim --test traffic_junction_box seed_1_box_keeps_moving_liveness -- --ignored` fails with the box-entry heads standing 95.5 s in `Dynamic`; the box pass and re-route the spec proposes already exist (`box_rules.rs:1-3`, `manoeuvre.rs:160-181`, `junction.rs:222-250,315-325`) but act only on `Kinematic` cars (`manoeuvre.rs:115`, `junction.rs:204-205`), and a `Dynamic` car on a connector can never recover (`recover.rs:144-145`; `led` gates only give-up, `recover.rs:156`, contrary to the spec's "led, so they never give up or recover") ; smallest implied reframing: the lock is the box-entry head that turns `Dynamic` next to the left car and then drops out of every existing box rule (no recovery on a connector, no repick or box pass while `Dynamic`), so the task must own that head's `Dynamic` state and explain why the existing box pass / repick never engage, before (or instead of) adding new routing geometry.
