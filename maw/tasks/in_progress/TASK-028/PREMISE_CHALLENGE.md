# PREMISE_CHALLENGE — TASK-028

## 1. Counter-example tested

The premise assumes a zip of "exe + `assets/` next to it" is a self-contained, bootable release.
Concrete counter-example: the game (or its data-first RON loaders / city generator) resolves asset or
config paths through a compile-time absolute path (`env!("CARGO_MANIFEST_DIR")`, `concat!(env!(...))`,
`include_str!` of something outside the zip, or a hard-coded repo-relative path used via `std::fs`),
so the unpacked zip on another machine fails to find its data even though CI built it and the success
predicate "zip contains exe + assets/" is met. A second angle of the same class: the release build
cannot be made without the `fast`/`bevy_dylib` dynamic-linking path (e.g. `default` features pull it in),
so the exe needs a `.so`/`.dll` that is not in the zip.

## 2. Primary-source investigation

- `grep -rn "CARGO_MANIFEST_DIR\|include_str!\|include_bytes!" src crates` (non-`tests/`): 30+ hits.
  Every hit sits inside a `#[cfg(test)]` module or a `#[cfg(test)] mod *_gate;` file:
  `src/audio/mod.rs:7-10`, `src/camera/mod.rs:2-3`, `src/hud/mod.rs:5-6`, `src/juice/mod.rs:4-7`,
  `src/visuals/mod.rs:3-17`; `src/bench/mod.rs:284`, `src/hud/stars.rs:159`, `src/juice/config.rs:316`,
  `src/menu/screens.rs:358`, `crates/citygen/src/params.rs:321`; gta_sim `include_str!`/`concat!(env!)`
  users all after `#[cfg(test)]` (anim.rs:67, reaction.rs:44, melee.rs:571, gang/fsm.rs:150,
  territory.rs:172, police/cars.rs:341, police/fsm.rs:158, wanted/mod.rs:259, wanted/search.rs:156,
  traffic/graph.rs:470, vehicle/chassis.rs:206).
- The only runtime `ConfigRoot` is `src/main.rs:194`:
  `ConfigRoot(FileAssetReader::get_base_path().join("assets"))`.
- Engine truth, `~/.cargo/registry/src/*/bevy_asset-0.19.1/src/io/file/mod.rs:19-29`: `get_base_path()` =
  `BEVY_ASSET_ROOT` env, else runtime `CARGO_MANIFEST_DIR` env, else `current_exe().parent()`.
  Outside cargo neither env var is set, so configs and Bevy assets resolve to `<exe dir>/assets`.
- Second angle: `Cargo.toml` `fast = ["bevy/dynamic_linking"]` is an opt-in feature;
  `bevy-0.19.1/Cargo.toml:2742-2747` `default = ["2d","3d","ui","audio"]` has no `dynamic_linking`.
  A plain `cargo build --release` produces a statically linked exe.
- Extra premise checks: `.cargo/config.toml` only sets `[target.x86_64-pc-windows-msvc] linker = "rust-lld.exe"`
  (no Linux section, so it cannot break a Linux build). `git check-ignore -v assets/third_party/x` ->
  `.gitignore:65:/assets/third_party/*`, and `assets/third_party/manifest.ron` has per-pack URL,
  `archive_sha256`, per-file sha256 and `license_file` — so "binary assets are not in git, fetch via
  manifest" matches reality. `.github/workflows/` holds 5 workflows (citygen/client/sim gates, clippy,
  repo-checks), consistent with the TASK-029 dependency being done.

## 3. Did it hold

No. The counter-example does not occur. Every compile-time absolute path is test-only, the game resolves
its whole data root through Bevy's exe-relative fallback, and the release build is static without `fast`.
"exe next to `assets/`" is the layout the code actually expects. The other premises (Windows-only
linker config, git-ignored third-party assets fetched by manifest with sha256 and licence files,
existing CI to reuse) match the repo too.

One residual note for the planner, not a mis-framing: if a CI job exports `CARGO_MANIFEST_DIR` or
`BEVY_ASSET_ROOT` while running the built binary, the boot smoke would read the checkout's `assets/`
and not the zip's. The smoke check should run the unpacked zip without those vars.

## 4. Verdict

PREMISE HOLDS — runtime data root is `src/main.rs:194` `FileAssetReader::get_base_path().join("assets")`, which per `bevy_asset-0.19.1/src/io/file/mod.rs:19-29` falls back to the exe directory; all `env!("CARGO_MANIFEST_DIR")`/`include_str!` asset paths are `#[cfg(test)]`-only; `dynamic_linking` is not in bevy 0.19.1 defaults (`bevy-0.19.1/Cargo.toml:2742-2747`) and `.cargo/config.toml` is Windows-target-only.
