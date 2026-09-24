//! Wanted stars under the ammo counter (GDD §6.4, §7): earned stars blink while no cop sees the
//! player. The sim owns `WantedLevel`; this only draws it.

use super::{rgb, rgba};
use crate::menu::{StarsConfig, UiConfig, UiFonts};
use bevy::prelude::*;
use gta_sim::wanted::{STARS, WantedLevel};

#[derive(Component)]
pub(super) struct StarRow;

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
            StarRow,
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
