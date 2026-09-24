# PCTX proposals from TASK-026 (implementer)

- 2026-09-25 (TASK-026), domain game-design or bevy-ecs: a visible corpse is re-offered to every civilian
  in corpse sight on each perception cycle (16 Hz). Any FSM branch that "refreshes on a threat" therefore
  refreshes forever while a body lies in view: before TASK-026 a cowering witness stayed crouched until the
  corpse despawned (30 s). Refresh only on a NEW cause (`civilian::already_fleeing`); trigger: a new
  `(State, Some(threat))` arm in `civilian/mod.rs::next_state`.
- 2026-09-25 (TASK-026), domain gates: `graph_app(10)` square corner (10,0,10) sits on the test-area 1 m
  box: two civilians fleeing around the square jam head-on there, and a stuck fleer never ends its flight
  (`left` counts travelled metres). Delayed-call fixtures use separate dead-end runs east of x=20
  (`wanted.rs::delayed_calls_about_one_kill_count_once`).
