# PCTX proposals — TASK-032

- 2026-09-26 (implementer, gates): a gate about traffic BEHIND a driving player needs the camera turned
  back as a named mutation: off frame the bubble despawns traffic 25 m behind a forward-looking driver,
  so the queue under test vanishes within 2 s (G6 fixture, TASK-032). Trigger: a gate with a driving
  player and `set_view(chase_view(..))`.
- 2026-09-26 (implementer, bevy-ecs/gates): "standing time" from velocity alone misses teleports (QA
  `put`, respawn keep velocity 0): the t15 player teleported in front of a car counted as a person
  standing for minutes and the car went around him at once. Reset on displacement > hold_speed x dt
  (`occupancy::snapshot_road`). Trigger: any `standing`/`stood` timer read by an AI rule.
- 2026-09-26 (implementer, gates): the TASK-016 rest-point lesson also covers virtual obstacles: an
  IDM obstacle at `v^2/2b + s0` recomputed every tick makes the car creep towards s0 forever (it never
  reaches speed 0); a "stop" obstacle is `v^2/2b` (TASK-032 siren yield).

- 2026-09-26 (fixer, TASK-032 R-A) — domain bevy-ecs or game-design, risk lesson: an autopilot car cannot shove
  a parked sedan sideways: an unmanned car holds its wheels below `hold_speed` and its tyres grip laterally
  (~mu·m·g ≈ 16 kN), while the sedan drive force is m·a ≈ 6.9 kN; the push only moves the body by ramming
  (autopilot stuck → reverse → ram cycles, backing into the follower). A kinematic pusher moves anything but
  drives through a pinned body (the G1 class). Trigger: any "push through" / "shove" traffic design.

- 2026-09-26 (qa, gates domain): a flip restored by `mv file.bak file` (or `cp` without a later edit) gives the file an
  mtime OLDER than the flipped build, so cargo does not rebuild and the gate stays RED after a correct restore (QA saw
  `a_hijacked_passer_claims_nothing` RED with sha256 already matching). Rule: after restoring a flipped file, `touch` it
  (and `crates/*/src/lib.rs`) before the GREEN run; sha256 proves the content, not the build.
- 2026-09-26 (qa, universal invariants): the orchestrator's binding QA note asked for `git worktree add` of the pre-task
  commit plus a second `--target-dir` under the task scratch (R1 main-vs-branch). The project law forbids both for
  stage work. Proposal: name the exception ("a pre-task baseline build for an A/B runtime comparison, removed at the
  end") so QA does not have to pick between the note and the law. Cost here: 2.7 GB target, 8 min cold release build.

- 2026-09-26 (fixer round 2, game-design/gates): a give-up timeout in an AI state machine is also the breaker of
  every lock it resolved silently. Narrowing the traffic give-up to "off its lane with nothing ahead" (to stop
  Abandoned columns) turned in-view box locks and walker-pinned box holders into `Dynamic` stands of 53-100 s
  in four city gates that were green only through the give-up. Before narrowing or removing such a timeout,
  run every city gate's stand bounds and list which locks it was ending. Trigger: edits to `give_up_seconds`,
  `stood`, or any "never gives up while ..." rule.
- 2026-09-26 (fixer round 2, traffic): never reset `TrafficCar.lateral` on a car the kinematic law still moves
  (Bailing kinematic): the next tick places it on the path line, a 3.25 m sideways jump for a passer (probe in
  `a_bailing_passer_claims_nothing`). Drop derived state (claims) at the reader instead. Trigger:
  `clear_ai_state` or a `lateral = 0.0` outside `abandon()`/hijack.
