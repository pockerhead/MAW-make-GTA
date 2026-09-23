# PCTX proposals — TASK-008

## 2026-09-23 (planner) — bevy-ecs risk lesson: Tnua knockback re-hit
bevy-tnua 0.32 `TnuaController::action_interrupt` on an action that is already running only replaces
`state.input` (`bevy-tnua-macros-0.3.0/src/scheme_derive/codegen.rs`, `update_in_action_state`); the
`TnuaBuiltinKnockback` memory stays `Pushback` and the new `shove` is never applied. Probe (TASK-008
scratch/probe_second_shove.log): 3 m/s then 5 m/s four ticks later moved the body 0.363 m (= the first shove
alone) without a reset, 1.108 m with `state.memory = TnuaBuiltinKnockbackMemory::Shove` set first.
Why: silent defect (second hit has no push), invisible in a single-hit test; any T9+ multi-attacker fight hits it.

## 2026-09-23 (plan-reviewer-2) — gates risk lesson: `AnimationPlayer::all_paused()` on an empty player
bevy_animation 0.19.1 `AnimationPlayer::all_paused()` is `playing_animations().all(is_paused)` (`lib.rs:914-917`):
it returns `true` when nothing is playing. A hit-stop / pause gate that asserts `all_paused()` on a hand-spawned
animator with no started node is GREEN with the pause code deleted. Assert `animation(node).is_some_and(|a| a.is_paused())`
on a node the test started, and assert the node is active first.
Why: a silent tautology in any presentation gate over animation pause state (TASK-008 hit-stop).

## 2026-09-23 (implementer) — bevy-ecs risk lesson: a nested tuple inside `.chain()` is not chained
`((a, (b, c, d).before(X), e).chain())` orders a -> {b, c, d} -> e, but b, c, d stay unordered among
themselves. TASK-008: `apply_strikes` (reader of a same-tick `Strike` message) sometimes ran before
`swing_melee` (its writer), so hits landed one tick late — flaky (T0+8 vs T0+9 in the same test binary).
Write `(b, c, d).chain().before(X)`. Gate with an exact-tick assertion (`tests/melee.rs`
`hit_lands_only_in_window`), which caught it.
Why: silent, nondeterministic one-tick delay; any same-tick writer/reader pair inside a nested group.

## 2026-09-23 (implementer) — bevy-ecs risk lesson: a clip writes only the joints it keys
bevy_animation blends per property over the clips that key it; a joint no active clip keys keeps its last
written value. Kenney mini-characters: `die` keys root rotation, `attack-kick-right` keys legs and root
translation, `idle` keys neither (TASK-008 scratch/probe_glb_channels.log), so after a knockdown the model
stayed lying while the sim said `Steady`. Fix used: a rest-pose layer (`static` clip, every joint keyed) at a
tiny node weight under all clips (`visual.ron` `rest`). Before adding a clip that keys a joint the base
locomotion does not, check the channels (the probe script lists them).
Why: visible only in the windowed game (headless gates see node indices, not poses); every new full-body clip.

## 2026-09-23 (qa) — gates/bevy-ecs risk lesson: a sweep from inside the caster hits what touches it
avian 0.7 `ShapeCastConfig::from_max_distance` keeps `ignore_origin_penetration: false`: a shape that already
overlaps something at the origin returns that collider at distance 0, whatever the cast direction. TASK-008 melee
sweeps a 0.35 m sphere from the attacker's axis (capsule 0.3 m), so a wall or character within 0.05 m of the
capsule on ANY side (behind, beside) eats the punch aimed forward (QA probe scratch/qa_a1/qa_a1_probe.rs).
Every gate of the slice used an attacker in open space, so all stayed GREEN. Any attack/interaction sweep
needs one gate with a collider touching the caster from behind or the side.
Why: silent in tests, and in a city characters stand against walls all the time.

> RESOLVED 2026-09-23: Tnua knockback re-hit, nested chain, clip-writes-only-keyed-joints -> domains/bevy-ecs.md; all_paused tautology -> domains/gates.md; plus a scratch-probe CARGO_TARGET_DIR rule in planner/implementer/fixer/qa overlays (QA found a 2.5 GB second target dir).
