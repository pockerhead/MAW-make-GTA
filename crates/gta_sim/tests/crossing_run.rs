mod common;

use bevy::prelude::*;
use common::*;
use gta_sim::world::PlayerSpawn;

/// Camera yaw of the TASK-022 QA runtime probe: pi - 0.0349, the longest open street from the
/// seed-1 spawn; its heading drifts x by -0.035 m per metre of z, as in the runtime log.
const QA_YAW: f32 = std::f32::consts::PI - 0.0349;

/// Seed 1, hold "W" from the spawn along the QA yaw with no civilians: the runtime path of the
/// TASK-022 stall reports (stops at z -19.6 and z 4.5). The player must keep running past both.
#[test]
fn player_runs_through_seed1_crossings_on_qa_path() {
    let mut app = city_app(1);
    set_population(&mut app, |p| p.max_civilians = 0);
    settle(&mut app);
    let start = position(&mut app);
    let spawn = app.world().resource::<PlayerSpawn>().0;
    assert!(
        (Vec2::new(start.x, start.z) - Vec2::new(spawn.x, spawn.z)).length() < 0.05,
        "GATE BROKEN: player not at the spawn {spawn} after settle: {start}"
    );
    set_intent(&mut app, |intent| {
        intent.axis = Vec2::Y;
        intent.yaw = QA_YAW;
    });
    let hz = (1.0
        / app
            .world()
            .resource::<Time<Fixed>>()
            .timestep()
            .as_secs_f32())
    .round() as u32;
    let mut track = vec![start];
    for _ in 0..20 {
        run_ticks(&mut app, hz);
        track.push(position(&mut app));
    }
    let run_speed = app
        .world()
        .resource::<gta_sim::character::LocomotionConfig>()
        .run_speed;
    for (second, pair) in track.windows(2).enumerate().skip(1) {
        let step = (pair[1] - pair[0]).xz().length();
        assert!(
            step > 0.8 * run_speed,
            "second {second}: moved {step:.2} m from {} to {} (run speed {run_speed}); track {track:?}",
            pair[0],
            pair[1]
        );
    }
    let end = *track.last().unwrap();
    assert!(
        end.z > 30.0,
        "player did not run past the crossings at z -19.6 and z 4.5: end {end}; track {track:?}"
    );
}
