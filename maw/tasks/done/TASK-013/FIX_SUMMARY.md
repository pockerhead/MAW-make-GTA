# FIX_SUMMARY — TASK-013 (small fixer run: minors 1, 2, 4)

Preflight: scratch/ read as a coverage map. Riskiest review prescription checked first: "use `set_if_neq`" (item 4).
`GameSettings: PartialEq`, and `DetectChangesMut::set_if_neq` only marks on a difference, so it is safe; verified by the
new unit test and its flip below. Item 1 diagnosis re-checked with fontTools: U+21C4 is absent from both Inter fonts.

## Fixed
- **1 (missing glyph)**: toggle label is now `↔` (U+2194, present in Inter-Regular and InterDisplay-Black). New gate
  `tools/qa/font_check.py`: reads `font`/`title_font` from `strings.ron`, checks every character of every string literal
  in `assets/ui/strings.ron` and `src/menu/*.rs` (comments stripped) against both font cmaps; exit 1 on a missing glyph,
  2 on broken setup (< 20 literals or font key missing). `strings.ron` comment points to it.
  - Green: `font_check: 153 literals, 2 fonts, 0 missing glyphs`, exit 0.
  - Flip 1 (`toggle: "⇄"` in strings.ron): 2 missing (U+21C4 in both fonts), exit 1 — RED.
  - Flip 2 (literal `"⇄".into()` back in screens.rs): 2 missing, exit 1 — RED. Both reverted.
- **2 (hard-coded texts)**: `MenuConfig` gained `step_down "−"`, `step_up "+"`, `toggle "↔"`, `sensitivity_value "×{value}"`,
  `volume_value "{value}%"`. Validation: the three labels non-empty, both `*_value` must contain `{value}`
  (`current_seed` precedent). Number precision (2 decimals / integer percent) stays in code as formatting.
  `src/menu/screens.rs` uses the config for labels and values.
- **4 (rewrite at a bound)**: `apply_step` in `screens.rs` builds the stepped copy and writes through `set_if_neq`;
  `SaveSettingsDeferred` is queued only when it returns true. Unit test `step_at_a_bound_leaves_settings_unmarked`:
  4 bound rows (sensitivity + at max, − at min, volume + at 1, − at 0) each assert (reported change, change mark,
  value) == (false, false, start), plus an in-range row that must change and mark.
  - Flip (unconditional `set_changed()` + write, return true): RED on "sensitivity + at max" `(true, true, ..)`. Reverted.

## Skipped
- 3, 5, 6: accepted as-is per the binding OPEN_DECISIONS entry. Nits: not in scope.

## Runtime check
`scratch/fixer_settings_probe.py` (release, `--seed 1`, QA id `com.github.pockerhead.maw-make-gta.qa` via brp.py):
Esc → Paused, click on the "Настройки" text node (position from `UiGlobalTransform`) → `PauseMenu == Settings`,
screenshot `scratch/fixer_settings/settings.png`. Looked at it: rows show `×1.00`, `100%`, `−`/`+` and `↔` render
correctly in Inter, `вкл/выкл` values next to them. Owner feel of the screen is still owner-run.

## Test results
- `cargo clippy -j 4 -- -D warnings`: clean.
- `cargo test -j 4 -p gta_like --bin gta_like` ×3: `51 passed; 0 failed` each (scratch/fixer_client_3runs.txt).
- `python tools/qa/font_check.py`: exit 0.
- Sim crates untouched by this run; `cargo test -p gta_sim -p citygen` not re-run.
