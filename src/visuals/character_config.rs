use bevy::prelude::*;
use gta_sim::config::manifest::{THIRD_PARTY_MANIFEST, ThirdPartyManifest};
use serde::Deserialize;
use std::time::Duration;

pub const CHARACTER_VISUAL_CONFIG: &str = "character/visual.ron";

#[derive(Resource, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct CharacterVisualConfig {
    /// Asset path of the glTF model.
    pub(crate) model: String,
    /// Head top of the model in model units (feet at 0).
    pub(super) model_height: f32,
    /// Target body height, m.
    pub(super) height: f32,
    /// glTF mesh whose material is multiplied by `tint`.
    pub(super) tinted_mesh: String,
    /// Linear multiplier of the base colour.
    pub(super) tint: (f32, f32, f32),
    /// Cross-fade between states, s.
    pub(super) blend_seconds: f32,
    pub(super) idle: String,
    pub(super) jump: String,
    pub(super) fall: String,
    pub(super) walk: LocomotionClip,
    pub(super) run: LocomotionClip,
    pub(super) sprint: LocomotionClip,
    /// Rig joints the armed arm layer animates; locomotion keeps every other joint.
    pub(super) arm_joints: Vec<String>,
    /// Joint the held gun is attached to.
    pub(super) hand_joint: String,
    /// Arm clips while holding a one-handed gun (pistol).
    pub(super) one_hand: ArmClipNames,
    /// Arm clips while holding a two-handed gun (SMG, shotgun).
    pub(super) two_hands: ArmClipNames,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub(super) struct ArmClipNames {
    /// Looped while the gun is held.
    pub(super) hold: String,
    /// Played once per shot.
    pub(super) shoot: String,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub(super) struct LocomotionClip {
    pub(super) clip: String,
    /// Body speed at playback rate 1, model units/s.
    pub(super) native_speed: f32,
}

/// glTF animation indices of the character clips.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CharacterClips {
    /// Clip of each `AnimState`, in `AnimState` declaration order.
    pub locomotion: [usize; 6],
    /// Arm hold clip per `ArmPose` (one hand, two hands).
    pub hold: [usize; 2],
    /// Arm shoot clip per `ArmPose`.
    pub shoot: [usize; 2],
}

impl CharacterVisualConfig {
    pub(super) fn scale(&self) -> f32 {
        self.height / self.model_height
    }

    pub fn validate(&self) -> Result<(), String> {
        for (field, value) in [
            ("model_height", self.model_height),
            ("height", self.height),
            ("walk.native_speed", self.walk.native_speed),
            ("run.native_speed", self.run.native_speed),
            ("sprint.native_speed", self.sprint.native_speed),
        ] {
            if !(value.is_finite() && value > 0.0) {
                return Err(format!("{field} {value} must be finite and > 0"));
            }
        }
        if Duration::try_from_secs_f32(self.blend_seconds).is_err() {
            return Err(format!(
                "blend_seconds {} must be finite, >= 0 and fit a Duration",
                self.blend_seconds
            ));
        }
        let scale = self.scale();
        if !(scale.is_finite() && scale > 0.0) {
            return Err(format!(
                "height / model_height = {scale} must be finite and > 0"
            ));
        }
        for (field, clip) in [
            ("walk.native_speed", &self.walk),
            ("run.native_speed", &self.run),
            ("sprint.native_speed", &self.sprint),
        ] {
            let metres = clip.native_speed * scale;
            if !(metres.is_finite() && metres > 0.0) {
                return Err(format!("{field} * scale = {metres} must be finite and > 0"));
            }
        }
        let (r, g, b) = self.tint;
        if ![r, g, b].iter().all(|c| c.is_finite() && *c >= 0.0) {
            return Err(format!(
                "tint {:?} components must be finite and >= 0",
                self.tint
            ));
        }
        Ok(())
    }

    fn clip_names(&self) -> [&str; 6] {
        [
            &self.idle,
            &self.walk.clip,
            &self.run.clip,
            &self.sprint.clip,
            &self.jump,
            &self.fall,
        ]
    }

    /// Resolves every clip name against the manifest rig of `model`; collects all errors.
    pub fn resolve(&self, manifest: &ThirdPartyManifest) -> Result<CharacterClips, Vec<String>> {
        let model = &self.model;
        let Some(rig) = manifest.rig_for(model) else {
            return Err(vec![format!(
                "model {model} has no rig in {THIRD_PARTY_MANIFEST}"
            )]);
        };
        let mut errors = Vec::new();
        if !rig.skinned_meshes.contains(&self.tinted_mesh) {
            errors.push(format!(
                "tinted_mesh {:?} is not a skinned mesh of {model}",
                self.tinted_mesh
            ));
        }
        for joint in self.arm_joints.iter().chain([&self.hand_joint]) {
            if !rig.joints.contains(joint) {
                errors.push(format!("joint {joint:?} is not in the rig of {model}"));
            }
        }
        let mut index = |name: &str| {
            rig.clip_index(name).unwrap_or_else(|| {
                errors.push(format!("clip {name:?} is not in the rig of {model}"));
                0
            })
        };
        let locomotion = self.clip_names().map(&mut index);
        let hold = [index(&self.one_hand.hold), index(&self.two_hands.hold)];
        let shoot = [index(&self.one_hand.shoot), index(&self.two_hands.shoot)];
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(CharacterClips {
            locomotion,
            hold,
            shoot,
        })
    }
}
