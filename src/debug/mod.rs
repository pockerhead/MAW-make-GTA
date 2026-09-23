use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_inspector_egui::{bevy_egui::EguiPlugin, quick::WorldInspectorPlugin};

#[derive(Resource, Default)]
struct InspectorVisible(bool);

pub struct DebugToolsPlugin;

impl Plugin for DebugToolsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(InspectorVisible::default())
            .add_plugins((
                EguiPlugin::default(),
                WorldInspectorPlugin::new().run_if(inspector_visible),
                PhysicsDebugPlugin,
            ))
            .add_systems(Update, toggle_debug);
        app.world_mut()
            .resource_mut::<GizmoConfigStore>()
            .config_mut::<PhysicsGizmos>()
            .0
            .enabled = false;
    }
}

fn inspector_visible(visible: Res<InspectorVisible>) -> bool {
    visible.0
}

fn toggle_debug(
    keys: Res<ButtonInput<KeyCode>>,
    mut visible: ResMut<InspectorVisible>,
    mut gizmos: ResMut<GizmoConfigStore>,
) {
    if keys.just_pressed(KeyCode::F1) {
        visible.0 = !visible.0;
    }
    if keys.just_pressed(KeyCode::F2) {
        let (config, _) = gizmos.config_mut::<PhysicsGizmos>();
        config.enabled = !config.enabled;
    }
}
