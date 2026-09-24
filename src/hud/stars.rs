//! Wanted stars under the ammo counter (GDD §6.4, §7): earned stars blink while no cop sees the
//! player. The sim owns `WantedLevel`; this only draws it.

use super::{rgb, rgba};
use crate::{
    juice::{JuiceConfig, StarPulseConfig, StarsRaised},
    menu::{StarsConfig, UiConfig, UiFonts},
};
use bevy::prelude::*;
use gta_sim::{
    wanted::{STARS, WantedLevel},
    world::CityScoped,
};

#[derive(Component)]
pub(super) struct StarRow;

/// Real seconds left of the pulse that plays when the wanted level rises.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct StarPulse {
    pub left: f32,
}

/// One star; `0` is its index from the left.
#[derive(Component)]
pub(super) struct StarSlot(u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StarLook {
    Lit,
    Gray,
    /// An earned star in the dark phase of the blink.
    Dim,
    Off,
}

/// Look of star `slot` at `real_secs` real seconds, blinking with phases of `blink` seconds.
pub(super) fn star_look(slot: u8, wanted: &WantedLevel, real_secs: f32, blink: f32) -> StarLook {
    if slot >= wanted.stars {
        return StarLook::Off;
    }
    if wanted.seen {
        return StarLook::Lit;
    }
    if ((real_secs / blink).floor() as u64).is_multiple_of(2) {
        StarLook::Gray
    } else {
        StarLook::Dim
    }
}

/// Fill and drop-shadow colours of a star `look`.
pub(super) fn star_style(look: StarLook, cfg: &StarsConfig) -> (Color, Color) {
    match look {
        StarLook::Lit => (rgb(cfg.lit_color), rgba(cfg.shadow_color)),
        StarLook::Gray => (rgb(cfg.gray_color), rgba(cfg.shadow_color)),
        StarLook::Dim => (rgb(cfg.dim_color), rgba(cfg.shadow_color)),
        StarLook::Off => (rgba(cfg.off_color), Color::NONE),
    }
}

pub(super) fn spawn_stars(mut commands: Commands, ui: Res<UiConfig>, fonts: Res<UiFonts>) {
    let hud = &ui.hud;
    let stars = &hud.stars;
    let font = TextFont {
        font: FontSource::Handle(fonts.regular.clone()),
        font_size: FontSize::from(stars.size),
        ..default()
    };
    commands
        .spawn((
            Name::new("Wanted stars"),
            CityScoped,
            StarRow,
            StarPulse { left: 0.0 },
            Node {
                position_type: PositionType::Absolute,
                top: px(hud.margin
                    + 2.0 * (hud.bar_height + hud.bar_gap)
                    + hud.ammo_size
                    + hud.bar_gap),
                right: px(hud.margin),
                column_gap: px(stars.gap),
                ..default()
            },
            Visibility::Hidden,
        ))
        .with_children(|row| {
            for slot in 0..STARS as u8 {
                row.spawn((
                    StarSlot(slot),
                    Text::new(stars.glyph.clone()),
                    font.clone(),
                    TextColor(rgba(stars.off_color)),
                    TextShadow {
                        offset: Vec2::splat(stars.shadow_offset),
                        color: Color::NONE,
                    },
                ));
            }
        });
}

pub(super) fn update_stars(
    ui: Res<UiConfig>,
    real: Res<Time<Real>>,
    wanted: Res<WantedLevel>,
    mut rows: Query<&mut Visibility, With<StarRow>>,
    mut slots: Query<(&StarSlot, &mut TextColor, &mut TextShadow)>,
) {
    let cfg = &ui.hud.stars;
    let shown = if wanted.stars == 0 {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for mut visibility in &mut rows {
        visibility.set_if_neq(shown);
    }
    let now = real.elapsed_secs();
    for (slot, mut color, mut shadow) in &mut slots {
        let look = star_look(slot.0, &wanted, now, cfg.blink_seconds);
        let (wanted_color, wanted_shadow) = star_style(look, cfg);
        if color.0 != wanted_color {
            color.0 = wanted_color;
        }
        if shadow.color != wanted_shadow {
            shadow.color = wanted_shadow;
        }
    }
}

/// Row scale `elapsed` real seconds into a pulse: `scale` at 0, eases back-out to 1.
pub(super) fn star_pulse_scale(elapsed: f32, cfg: &StarPulseConfig) -> f32 {
    1.0 + (cfg.scale - 1.0) * (1.0 - EaseFunction::BackOut.sample_clamped(elapsed / cfg.seconds))
}

pub(super) fn pulse_stars(
    mut raised: MessageReader<StarsRaised>,
    juice: Res<JuiceConfig>,
    real: Res<Time<Real>>,
    mut rows: Query<(&mut StarPulse, &mut UiTransform)>,
) {
    let cfg = &juice.star_pulse;
    let restart = raised.read().count() > 0;
    for (mut pulse, mut transform) in &mut rows {
        if restart {
            pulse.left = cfg.seconds;
        }
        pulse.left = (pulse.left - real.delta_secs()).max(0.0);
        let scale = Vec2::splat(star_pulse_scale(cfg.seconds - pulse.left, cfg));
        if transform.scale != scale {
            transform.scale = scale;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::UI_CONFIG;
    use gta_sim::config::{ConfigRoot, load_config};
    use std::path::Path;

    #[test]
    fn star_look_table() {
        use StarLook::*;
        let blink = 0.25;
        let seen = WantedLevel {
            heat: 180,
            stars: 2,
            seen: true,
            ..default()
        };
        let unseen = WantedLevel {
            seen: false,
            ..seen
        };
        let looks = |w: &WantedLevel, t: f32| {
            (0..5)
                .map(|s| star_look(s, w, t, blink))
                .collect::<Vec<_>>()
        };
        assert_eq!(looks(&seen, 0.1), vec![Lit, Lit, Off, Off, Off]);
        assert_eq!(looks(&unseen, 0.1), vec![Gray, Gray, Off, Off, Off]);
        assert_eq!(looks(&unseen, 0.3), vec![Dim, Dim, Off, Off, Off]);
        assert_eq!(looks(&WantedLevel::default(), 0.1), vec![Off; 5]);
    }

    /// G-J3 (pure math): the pulse starts at `scale`, eases back-out (dipping just under 1) and
    /// ends at exactly 1.
    #[test]
    fn star_pulse_curve() {
        let cfg = StarPulseConfig {
            scale: 1.3,
            seconds: 0.35,
        };
        assert!((star_pulse_scale(0.0, &cfg) - 1.3).abs() < 1e-5);
        assert_eq!(star_pulse_scale(0.35, &cfg), 1.0);
        assert_eq!(star_pulse_scale(3.5, &cfg), 1.0);
        let min = (0..=1000)
            .map(|i| star_pulse_scale(0.35 * i as f32 / 1000.0, &cfg))
            .fold(f32::INFINITY, f32::min);
        assert!((0.96..=0.98).contains(&min), "min scale {min}");
    }

    /// With the shipped colours an earned star never looks like an empty slot, and the blink shows.
    #[test]
    fn earned_star_styles_differ_from_empty() {
        use StarLook::*;
        let root = ConfigRoot(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets"));
        let ui = load_config::<UiConfig>(&root, UI_CONFIG)
            .unwrap_or_else(|e| panic!("GATE BROKEN: {e}"));
        let cfg = &ui.hud.stars;
        let off = star_style(Off, cfg);
        for look in [Lit, Gray, Dim] {
            assert_ne!(
                star_style(look, cfg),
                off,
                "{look:?} looks like an empty slot"
            );
        }
        assert_ne!(
            star_style(Gray, cfg),
            star_style(Dim, cfg),
            "the blink has one phase"
        );
    }
}
