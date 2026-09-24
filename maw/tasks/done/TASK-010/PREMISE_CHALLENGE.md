## Counter-example tested

If the city generator does not produce gang territory or headquarters positions, the task's premise that T9 can use territories from the generator is incomplete.

## Primary-source investigation

- `crates/citygen/src/pois.rs:26-51` selects two Residential or Industrial districts that contain buildings and marks one building in each as `GangHq(0)` or `GangHq(1)`.
- `crates/citygen/src/lib.rs:31-49` carries those assignments into `CityLayout`; `crates/citygen/src/layout.rs:14,67-74,84-89` exposes the district IDs and headquarters building positions.
- Ran `cargo test -p citygen pois_exist -- --exact`. Output included `test pois_exist ... ok` and `1 passed; 0 failed`. The test at `crates/citygen/tests/properties.rs:241-257` checks distinct districts, one headquarters per gang, headquarters in the assigned district, and the district type.

## Did it hold

No. The generator produces both gang district IDs and locatable headquarters buildings. The executed property test passed for its configured layouts.

## Verdict

PREMISE HOLDS — `crates/citygen/src/pois.rs:26-51`, `crates/citygen/src/layout.rs:14,67-74,84-89`; `cargo test -p citygen pois_exist -- --exact` returned `test pois_exist ... ok`.
