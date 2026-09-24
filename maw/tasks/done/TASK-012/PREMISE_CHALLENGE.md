## Counter-example tested

At one wanted star, the existing heat-decay system clears the wanted level while police are responding, before a nearby passive player can be arrested after 1.5 seconds. If this occurs, the stated arrest acceptance case assumes a pursuit state the game does not maintain.

## Primary-source investigation

(Appended by the orchestrator from the stage's final message. The codex sandbox began rejecting file writes after `rust-lld.exe` exited with `0xc0000142` under host memory pressure, and the harness reaped its wrapper.)

- `crates/gta_sim/src/wanted/search.rs:80` resets the hidden timer while the player remains inside the search circle.
- The one-star circle radius is 40 m (`assets/wanted/wanted.ron:6`).
- So a passive player near a responding cop stays inside the circle and keeps the wanted level. The arrest case can run.

## Did it hold

No. The test could not run because the linker failed; the verdict comes from reading the code.

## Verdict

PREMISE HOLDS — `crates/gta_sim/src/wanted/search.rs:80`, `assets/wanted/wanted.ron:6`.
