use bevy::prelude::*;
use bevy_brp_extras::BrpExtrasPlugin;

pub struct QaRemotePlugin;

impl Plugin for QaRemotePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(BrpExtrasPlugin::default());
    }
}
