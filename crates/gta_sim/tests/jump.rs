mod common;

use common::*;
use gta_sim::character::{LocomotionConfig, MoveIntent};

#[test]
fn jump_apex_matches_jump_height() {
    let mut app = headless_app();
    settle(&mut app);
    let rest = position(&mut app).y;
    set_intent(&mut app, |intent| intent.jump_held = true);
    let mut max = rest;
    for _ in 0..96 {
        run_ticks(&mut app, 1);
        max = max.max(position(&mut app).y);
    }
    let height = app.world().resource::<LocomotionConfig>().jump_height;
    assert!(
        ((max - rest) - height).abs() <= 0.1 * height,
        "rise={}",
        max - rest
    );
}

#[test]
fn tapped_jump_fires() {
    let mut app = headless_app();
    settle(&mut app);
    let rest = position(&mut app).y;
    set_intent(&mut app, |intent| intent.jump_requested = true);
    run_ticks(&mut app, 1);
    let entity = player(&mut app);
    assert!(
        !app.world()
            .get::<MoveIntent>(entity)
            .unwrap()
            .jump_requested
    );
    let mut max = position(&mut app).y;
    for _ in 0..32 {
        run_ticks(&mut app, 1);
        max = max.max(position(&mut app).y);
    }
    assert!(
        (0.15..0.35).contains(&(max - rest)),
        "ground tap rise={}",
        max - rest
    );
}

#[test]
fn late_tap_is_buffered_until_landing() {
    fn trajectory(tap_at: Option<usize>) -> Vec<f32> {
        let mut app = headless_app();
        settle(&mut app);
        let rest = position(&mut app).y;
        set_intent(&mut app, |intent| intent.jump_held = true);
        let mut rise = Vec::new();
        for tick in 0..120 {
            if tick == 30 {
                set_intent(&mut app, |intent| intent.jump_held = false);
            }
            if tap_at == Some(tick) {
                set_intent(&mut app, |intent| intent.jump_requested = true);
            }
            run_ticks(&mut app, 1);
            rise.push(position(&mut app).y - rest);
        }
        rise
    }

    let baseline = trajectory(None);
    let apex = baseline
        .iter()
        .position(|&rise| rise == baseline.iter().copied().fold(f32::MIN, f32::max))
        .unwrap();
    let landing = (apex..baseline.len())
        .find(|&tick| baseline[tick] < 0.02)
        .expect("GATE BROKEN: jump never landed");
    let tapped = trajectory(Some(landing - 4));
    let second_rise = tapped[landing + 1..]
        .iter()
        .copied()
        .fold(f32::MIN, f32::max);
    assert!(second_rise > 0.15, "buffered jump rise={second_rise}");
    assert!(
        second_rise < 0.5,
        "tap became a held jump: rise={second_rise}"
    );
}
