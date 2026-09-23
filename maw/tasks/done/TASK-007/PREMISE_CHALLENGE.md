# Counter-example tested

A damage message queued on the last `Wasted` frame is already discarded before respawn, so the stated TASK-006 B1 root cause does not exist in the current game.

# Primary-source investigation

- `crates/gta_sim/src/player/mod.rs:35-42,62-76`: `apply_debug_damage` reads `DebugDamage` only in `PlayingSystems`; its target query excludes `Dead`, but it does not discard messages outside `Playing`.
- `crates/gta_sim/src/flow/mod.rs:44-58`: `PlayingSystems` runs only in `GameState::Playing`; `respawn_player` runs on exit from `Wasted`.
- `crates/gta_sim/src/flow/wasted.rs:115-121`: respawn restores full `Health` on the same entity and removes `Dead`.
- `crates/gta_sim/src/combat/mod.rs:1-27`: the current combat plugin registers only health/armor pickups, so the shooting slice is absent.
- Ran `cargo test -p gta_sim --test respawn -- --nocapture`. Output: `3 passed; 0 failed`; the test reported `playing_at: 288`. This test exercises respawn, but does not queue damage during `Wasted`.

# Did it hold

No. The code contains no discard of `DebugDamage` during `Wasted`; the reader is disabled until `Playing`, when respawn has restored health and removed `Dead`. The existing respawn test does not establish that such a queued message is dropped. I found no primary-source evidence that the proposed counter-example occurs.

# Verdict

PREMISE HOLDS — `crates/gta_sim/src/player/mod.rs:35-42,62-76` and `crates/gta_sim/src/flow/wasted.rs:115-121` show the attempted counter-example is not enforced by the current damage and respawn paths.
