# IMPL_REVIEW — TASK-013 (GDD T12): minimap, menus, settings

Reviewer: code-reviewer (claude opus, medium). Scope: the whole branch diff `main...fff9cdf` (49 files, +3345 −135),
so both implementer runs are covered. Loaded: TASK_FINAL.md, PLAN_FINAL.md (all 594 lines), IMPL_SUMMARY.md, log.jsonl.

## 1. Verdict

**PASS.** The plan is implemented step by step, every AC gate exists and is green on my re-run, and the silent-class
teardown is gated with 30+ recorded flips. What I found is owner-visible polish: a missing glyph on the settings toggle
buttons and a few hard-coded UI literals. None of it breaks silently.

## Disconfirmation

Counter-example chosen before evaluating: **"Resume from pause (`Paused → Playing`) re-runs a one-shot bound to
`OnEnter(Playing)` (HUD, minimap, player, pickups, city visuals, sidewalk graph), so each Esc/Esc cycle duplicates the
HUD or minimap or respawns the player."** T12 is the first slice that re-enters `Playing` from a state that must not
rebuild anything.

What I searched: a grep of `OnEnter(GameState::Playing)` / `OnExit(Playing)` / `entered: GameState::Playing` over
`src/` and `crates/`. **It did not hold.** Every city-lifetime spawn is on `OnTransition { Loading → Playing }`:
`player/mod.rs:31`, `combat/mod.rs:79`, `gang/mod.rs:533`, `navigation/mod.rs:352`, `hud/mod.rs:25`, `minimap/mod.rs:50`,
`visuals/city.rs:26`. The only `OnEnter(Playing)` is `input::capture_cursor`, which is idempotent. t12 runtime QA also
counts 1 `Hud`/1 `Minimap` after the new city.

Second counter-example: "the shader's world lookup is not the inverse of `project`, so the map turns against the
arrow and markers". Checked algebraically. `project` uses M = [[c, −s], [−s, −c]]. M·M = I, and the shader's
`w = center + (c, −s)·m.x + (−s, −c)·m.y` is exactly M·m. `uv.y` grows with z, which matches raster row 0 = smallest z.
**It held (the code is consistent).**

## Log triage

The log has no implementer `dead_end` entries. The planner/plan-reviewer dead ends were checked:
- `melee::Strike` is `pub(super)`: it is cleared in `combat/mod.rs` `clear_combat_messages` and not asserted in G3.
- `raise_heat` runs before `spawn_unit`: `new_city.rs:221` before `:248`.
- A rotated ImageNode would clip wrong: the implementation uses an unrotated `UiMaterial` node (`minimap/material.rs`).

## Commands re-run on this tree

- `touch crates/*/src/lib.rs; cargo test -j 4 -p gta_sim -p citygen`: every `test result` ok, 0 failed. This covers
  `new_city` 3/3, `minimap` 5 + 1 ignored bless, `pause_request_table` and `search_circle_per_star`.
- `cargo test -j 4 -p gta_like --bin gta_like`: 50 passed. `city_gate` ran 3×: 5/5 each time, including P1/P2.
- `cargo clippy -j 4 -- -D warnings`: clean.
- The diff adds no `unsafe`. The only new `unwrap()` is in a test helper.
- I did not re-run t12 at runtime. The two PASS runs in `scratch/t12_run1`, `scratch/t12_run2` and
  `scratch/main_menu_probe` are the implementer's evidence.

## 2. Confirmed correct

- **Pause rule**
  - `crates/gta_sim/src/flow/mod.rs:370-379`: `pause_request` refuses while `NextState` is not `Unchanged`, so a death
    or arrest set in the same frame's fixed tick wins.
  - `pause_request_table` has one assert per row.
  - `pause_time` on `OnEnter(Paused)` and `resume_time` on `OnExit(Paused)`. With Exit → Transition → Enter ordering,
    a new city never starts frozen (G3 A23).
- **Teardown**
  - `CityScoped` is required by `Character`, `Pickup`, `WeaponPickup`, `BatPickup`, `CityBuilding/Ground/EdgeWall/Block`,
    `CityChunk` and `CityProp`.
  - `despawn_city` uses `try_despawn` (`world/mod.rs:832`). `drop_city` removes `City`, `CityLayoutHash` and
    `CityLandmarks`.
  - Each domain resets its own state on `NEW_CITY`: gang, navigation, perception, police, population, wanted, combat
    and flow. The resets match the plan's table row for row.
  - The G3 A1 leak detector (every `Transform` entity outside baseline B0) is generic and carries the claim well.
- **Reseed**: 4 RNGs are reseeded on `OnEnter(Loading)` with `run_if(resource_exists::<CitySeed>)`, so TestArea keeps
  seed 0. G2 has one row per RNG, each flipped separately.
- **Client cancel**: `cancel_city_build` (`src/visuals/city.rs:89`) drops `CityMeshTask` and `PendingCitySpawn`. P2
  has an honest precondition: pending spawn still present after `pause`.
- **Raster** (`crates/citygen/src/minimap.rs`)
  - Pure and deterministic, with precomputed convex edges. Blending uses integer rounding as specified.
  - The hash covers size, origin in mm, scale and pixels.
  - M3 reads texels from the contract in the test (no raster helper), so a mirrored raster turns RED (flip recorded).
- **Projection AC**: M4 uses an independent `Quat` forward at yaw 0/90/180 plus the right rows, and `map_px` lands at
  (0, −20). The yaw-90 row is the one the flip `s = −s` hits.
- **Minimap client** (`src/minimap/`)
  - Rasterizes in `AsyncComputeTaskPool` and polls with `block_on(poll_once)` (no blocking wait).
  - The task is dropped on `NEW_CITY`. The root is `CityScoped`, so image and material are freed with it.
  - `MAX_CONES` = 16 matches the WGSL array length. `textureSampleLevel` is called before any branch.
  - Marker centring with `UiTransform` percent translation is correct: bevy_ui 0.19.1 resolves `Val2` against the
    node's own `layout_size` (`layout/mod.rs:294-299`, `ui_transform.rs:180-186`).
- **Settings** (`src/settings/mod.rs`)
  - bevy-settings 0.19.1 loads synchronously inside `SettingsPlugin::build` (`lib.rs:97-124`), so `sanitize_settings`
    at `Startup` sees the loaded file. It writes only on a difference.
  - `SaveSettingsDeferred` after each change, `SaveSettings::IfChanged` when the settings screen closes, and
    `SaveSettingsSync::IfChanged` on `AppExit` in `Last.after(ExitSystems)`.
  - The `--settings-id` flag isolates QA and showcase.
- **Input**
  - `ContextActivity::<OnFoot>::INACTIVE` while paused: bevy_enhanced_input 0.26 `context.rs:715-720` resets action
    states to None.
  - The `wait_release` latch is kept.
  - `in_state(PauseMenu::Main)` is false while the substate does not exist (`bevy_state condition.rs:103-108`).
- **Data**: the new tuning values are in `strings.ron` (`hud.minimap`, `menu`) and `juice.ron` (`shake.reduced_scale`),
  with strict validation. The consts are identity or law only: `SETTINGS_APP_ID`, `MAX_CONES`, the 19-digit seed field
  and the 2 px/m raster cap.
- File sizes are below the 750-line warning (largest touched: `population/mod.rs` at 645).

## 3. Issues

1. **minor (owner-visible)**: `src/menu/screens.rs:177`, toggle button label `"⇄"` (U+21C4).
   - The UI font `assets/third_party/inter/Inter-Regular.ttf` has no such glyph. Checked with fontTools: `0x21c4`
     missing; `0x2212` "−", `+`, `×` and `П` present.
   - The three toggle buttons (invert Y, reduce shake, no flashes) will render a missing-glyph box or nothing. The
     value text next to them still shows вкл/выкл.
   - No QA screenshot covers the settings screen, so the tests could not catch this.
   - Fix: move the button labels (`−`, `+`, the toggle label) into `MenuConfig` in `strings.ron`, pick a glyph Inter
     has (or a word such as "сменить"), and add a preflight check that every configured glyph exists in the font (the
     minimap `+`/`П` glyphs have the same exposure).
2. **minor (data-first, texts)**: display texts and formats are hard-coded in `src/menu/screens.rs`.
   - `"−"`, `"+"`, `"⇄"`, `"×{:.2}"`, `"{:.0}%"` at `:106-107` and `:162-178`.
   - All other menu text lives in `strings.ron`.
   - Fix: move them to `menu` in `strings.ron`, next to `on`/`off`.
3. **minor (tuning literals)**:
   - `src/menu/widgets.rs:127`: seed-field padding `px(6.0)`.
   - `assets/shaders/minimap.wgsl:109`: arrow shape ratios `0.4 / 0.35 / 0.6`.
   - These are look values the owner may want to adjust. They are not `const`s, so this is not a BLOCK, but they
     belong in `menu`/`hud.minimap`.
   - Fix: add `field_padding` and `arrow_width` to the configs, or state in a one-line comment that they are fixed
     shape.
4. **minor (cosmetic persistence)**: `src/menu/screens.rs:281-305`.
   - `Step`/`Toggle` take `&mut *settings` even when `step_value` clamps to the same value (a "+" at max).
   - That marks `GameSettings` changed and triggers an identical file rewrite.
   - Fix: compare before writing, or use `set_if_neq`.
5. **minor (per-frame asset churn)**: `src/minimap/mod.rs:204`.
   - `materials.get_mut` runs every `PostUpdate`, even when nothing moved (paused, standing still). Each call
     re-extracts and re-prepares the material bind group.
   - The raster image is not modified, so the cost is small.
   - Measure before touching. The t12 frame report (2.6 ms no-vsync) shows no problem.
6. **minor (documented, accepted)**: the frame that enters `Paused` still runs one fixed tick.
   - Physics (avian, not state-gated) steps once with gameplay sets off.
   - This is documented in the `NpcSystems` doc and the plan. It is harmless, and I note it only for the record.

## 4. Missing coverage

- No gate or screenshot covers the settings screen. This is where the Issue 1 defect lives. Adding
  `settings.png` to t12 is cheap: open settings with one `send_keys` is not possible while paused (see the BRP key-hold
  PCTX proposal), so this stays for the owner run. Put it explicitly on the QA_REPORT owner checklist.
- The WGSL inverse projection has no gate. It is the algebraic inverse of the gated `project` (checked above). The
  screenshot `minimap_wanted.png` plus the owner run carry the look. Acceptable per the cost-of-error law.
- The `CityMeshTask` removal has no flip (timing-dependent), as the plan says. Covered by this review: dropping a
  bevy `Task` cancels it.
- `sanitize` has no row where the configured sensitivity range excludes the default 1.0. Low value.

## 5. Nits

- `src/hud/wasted.rs:282`: Busted passes `ui.wasted_size`. This is unchanged behaviour (the old `title_screen` did the
  same), but the tuple makes it look deliberate. Fine.
- `src/minimap/mod.rs:113`: a missing gang tint falls back silently to white. `GangConfig` validation already
  guarantees 2 gangs, so this branch is dead.

children: 0 launched / 0 reported.
