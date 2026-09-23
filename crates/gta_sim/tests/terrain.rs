mod common;

use bevy::prelude::*;
use common::*;

#[test]
fn walking_into_arena_box_stays_blocked() {
    let mut app = headless_app();
    settle(&mut app);
    place_player(&mut app, Vec3::new(10.0, 1.05, 12.0));
    set_intent(&mut app, |intent| intent.axis = Vec2::Y);
    run_ticks(&mut app, 90);
    let at = position(&mut app);
    assert!(at.z > 10.75, "walk crossed box front: {at:?}");
    assert!(at.y < 1.2, "walk climbed arena box: {at:?}");
}

#[test]
fn walking_climbs_stairs() {
    let mut app = headless_app();
    settle(&mut app);
    place_player(&mut app, Vec3::new(10.0, 1.05, -10.0));
    set_intent(&mut app, |intent| intent.axis = Vec2::Y);
    run_ticks(&mut app, 200);
    let at = position(&mut app);
    assert!(at.z < -14.5, "walk stopped on stairs: {at:?}");
    assert!(at.y > 2.3, "walk did not climb stairs: {at:?}");
}
