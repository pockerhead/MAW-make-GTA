use bevy::prelude::*;
use gta_sim::config::manifest::{THIRD_PARTY_MANIFEST, ThirdPartyManifest};
use serde::Deserialize;
use std::time::Duration;

pub const CHARACTER_VISUAL_CONFIG: &str = "character/visual.ron";

#[derive(Resource, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct CharacterVisualConfig {
    /// Asset path of the glTF model (player and dummies).
    pub(crate) model: String,
    /// Asset paths of the civilian models; each builds its own animation graph from its own clips.
    pub(crate) civilian_models: Vec<String>,
    /// Asset paths of the gang models; the tint comes from `gang/gangs.ron`.
    pub(crate) gang_models: Vec<String>,
    /// Asset paths of the police models; patrol and SWAT differ by tint.
    pub(crate) police_models: Vec<String>,
    /// Base colour multipliers of a patrol cop and a SWAT unit.
    pub(crate) police_tint: (f32, f32, f32),
    pub(super) swat_tint: (f32, f32, f32),
    /// Civilian base colour multipliers of `tinted_mesh`.
    pub(super) civilian_tints: Vec<(f32, f32, f32)>,
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
    /// Full-body clip per fist combo step, in step order (exactly 3).
    pub(super) fists: Vec<String>,
    /// Full-body clip of a bat swing.
    pub(super) bat: String,
    /// Full-body clip of a dead character, played once.
    pub(super) death: String,
    /// Full-body clip looped while a civilian cowers.
    pub(super) cower: String,
    /// Full-body clip of a knockdown.
    pub(super) knockdown: String,
    /// Rest-pose layer under every other clip.
    pub(super) rest: RestClip,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub(super) struct RestClip {
    pub(super) clip: String,
    /// Graph node weight; the other clips play at 1.
    pub(super) weight: f32,
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
    /// Full-body clip per fist combo step.
    pub melee: [usize; 3],
    pub bat: usize,
    pub death: usize,
    pub cower: usize,
    pub knockdown: usize,
    /// Rest-pose clip that keys every joint.
    pub rest: usize,
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
            ("rest.weight", self.rest.weight),
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
        if self.civilian_models.is_empty() {
            return Err("civilian_models must name at least one model".into());
        }
        if self.gang_models.is_empty() {
            return Err("gang_models must name at least one model".into());
        }
        if self.police_models.is_empty() {
            return Err("police_models must name at least one model".into());
        }
        if self.civilian_tints.is_empty() {
            return Err("civilian_tints must name at least one tint".into());
        }
        for (field, (r, g, b)) in [
            ("tint", self.tint),
            ("police_tint", self.police_tint),
            ("swat_tint", self.swat_tint),
        ]
        .into_iter()
        .chain(self.civilian_tints.iter().map(|&t| ("civilian_tints", t)))
        {
            if ![r, g, b].iter().all(|c| c.is_finite() && *c >= 0.0) {
                return Err(format!(
                    "{field} {:?} components must be finite and >= 0",
                    (r, g, b)
                ));
            }
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

    /// Resolves every clip name against the manifest rig of `model`, and checks that every civilian,
    /// gang and police model resolves to the same clips; collects all errors.
    pub fn resolve(&self, manifest: &ThirdPartyManifest) -> Result<CharacterClips, Vec<String>> {
        let clips = self.resolve_model(manifest, &self.model)?;
        let mut errors = Vec::new();
        for civilian in &self.civilian_models {
            match self.resolve_model(manifest, civilian) {
                Ok(own) if own == clips => {}
                Ok(_) => errors.push(format!(
                    "civilian model {civilian} resolves clips differently from {}",
                    self.model
                )),
                Err(own) => errors.extend(own),
            }
        }
        for gang in &self.gang_models {
            match self.resolve_model(manifest, gang) {
                Ok(own) if own == clips => {}
                Ok(_) => errors.push(format!(
                    "gang model {gang} resolves clips differently from {}",
                    self.model
                )),
                Err(own) => errors.extend(own),
            }
        }
        for police in &self.police_models {
            match self.resolve_model(manifest, police) {
                Ok(own) if own == clips => {}
                Ok(_) => errors.push(format!(
                    "police model {police} resolves clips differently from {}",
                    self.model
                )),
                Err(own) => errors.extend(own),
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(clips)
    }

    fn resolve_model(
        &self,
        manifest: &ThirdPartyManifest,
        model: &str,
    ) -> Result<CharacterClips, Vec<String>> {
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
        let fists = self
            .fists
            .iter()
            .map(|name| index(name))
            .collect::<Vec<_>>();
        let bat = index(&self.bat);
        let death = index(&self.death);
        let cower = index(&self.cower);
        let knockdown = index(&self.knockdown);
        let rest = index(&self.rest.clip);
        let melee = <[usize; 3]>::try_from(fists).unwrap_or_else(|fists| {
            errors.push(format!(
                "fists must name exactly 3 clips, got {}",
                fists.len()
            ));
            [0; 3]
        });
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(CharacterClips {
            locomotion,
            hold,
            shoot,
            melee,
            bat,
            death,
            cower,
            knockdown,
            rest,
        })
    }
}
