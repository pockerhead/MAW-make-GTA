# PCTX proposals — TASK-007 (planner)

## 2026-09-23 — bevy-ecs, Risk lessons (proposed)

A `MessageReader` inside a state-gated set (`PlayingSystems`) does not consume while the state is off; the
messages stay buffered and are read in the FIRST fixed tick after re-entry (after `OnExit` respawn already ran).
Probe `maw/tasks/in_progress/TASK-007/scratch/probe/tests/probe.rs`: one `DebugDamage(30)` written only before
the last `Wasted` update hits the respawned player (health 70, not 100) — TASK-006 QA B1. A read-time state check
cannot catch it (the state is already `Playing` when the message is read). Fix pattern: `Messages<T>::clear()`
on `OnExit` of the state that must not leak (bevy_ecs 0.19.1 `message/messages.rs:228`).
Why a lesson: silent state corruption across a state boundary; applies to every future gated reader
(melee hits, wanted heat, police arrest messages).

## 2026-09-23 — gates or bevy-ecs (proposed)

A character hitbox child inside its own capsule is never the closest ray hit (probe: head sphere inside the
capsule → every ray reports the capsule). A hitbox that must win a closest-hit ray has to enclose the body
surface where it is meant to count. Any camera/AI shape cast with mask ALL now also hits hitbox sensors:
new spatial queries name their `SpatialQueryFilter` mask explicitly.

## 2026-09-23 — gates, Risk lessons (proposed, implementer)

`bevy_brp_extras` 0.22.6 `screenshot` publishes the PNG before its capture flag clears: a second
`screenshot` issued right after the first file appears fails with "A screenshot capture is already in
progress" (t6.py, first SMG-burst run). Space consecutive screenshots by >= 0.15 s. Also: a transient effect
shorter than the screenshot latency (60 ms tracer seen nearly end-on from a TPS camera) rarely shows up in a
capture; gate its mechanism headless and leave its look to the owner run.

## 2026-09-23 — gates (proposed, implementer)

Validation boundaries in data (e.g. a head sphere that exactly touches the capsule top, 1.6 + 0.2 = 1.8) are
decided by f32 rounding (1.8000001 > 1.8 passed). A config-sabotage fixture must sit strictly on the failing
side of the rule, never on its boundary.

## 2026-09-23 — bevy-ecs, Risk lessons (proposed, fixer)

Bevy 0.19.1 `AnimationGraph` node masks live in the shared graph asset: toggling a node's mask changes every
character that plays the graph. A per-character layer (arms holding a gun over locomotion) needs duplicate nodes
(locomotion with and without the arm mask) chosen per `AnimationPlayer`, plus mask groups keyed by the
`AnimationTargetId`s the glTF loader puts on the spawned nodes (path from the scene root name, so read them at
runtime rather than rebuilding the path). Also: anything parented to a glTF joint inherits the model scale
(Kenney mini characters x2.68) and must undo it.

> RESOLVED 2026-09-23: all five folded — message/latch leak + animation mask lessons -> domains/bevy-ecs.md; boundary fixture, hitbox/explicit mask, BRP screenshot spacing -> domains/gates.md.
