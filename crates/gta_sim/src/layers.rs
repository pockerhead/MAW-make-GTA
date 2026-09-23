use avian3d::prelude::*;

/// Collision layers (GDD §12 law). `World` is the default layer of all static geometry.
#[derive(PhysicsLayer, Default, Clone, Copy, Debug)]
pub enum GameLayer {
    #[default]
    World,
    Character,
    Hitbox,
}
