//! # The world: camera and tile map
//!
//! Everything that exists before any unit does: the camera that looks at the
//! scene, and the floor of `GRID` x `GRID` tiles.

use bevy::prelude::*;

use crate::iso::GridPos;

/// The map is `GRID` cells wide and `GRID` cells tall. Other modules read this
/// to know where the map ends (the cursor hides outside it).
pub const GRID: i32 = 8;

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        // Needs no sheet data, so it can run at `Startup` while loading goes on.
        app.add_systems(Startup, setup_world);
    }
}

/// Marks a floor tile, so tiles can be found and queried later.
#[derive(Component)]
struct Tile;

fn setup_world(mut commands: Commands, asset_server: Res<AssetServer>) {
    // The camera. We scale its view to 0.5: the projection shows half as much
    // of the world, which draws everything twice as large (0.25 would be 4x).
    // Integer zoom keeps pixel art crisp.
    commands.spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scale: 0.5,
            ..OrthographicProjection::default_2d()
        }),
    ));

    // One tile image, shared by all tiles. `load` returns a handle at once and
    // the image streams in; cloning a handle is cheap and points to the same data.
    let tile = asset_server.load("tile.png");

    // One entity per cell. Tiles only declare their *grid* position, exactly
    // like units do; the `Transform` is left at its default because the
    // projection system in `iso.rs` fills it in. That way tiles, units and the
    // cursor all share one projection and one depth rule.
    for x in 0..GRID {
        for y in 0..GRID {
            commands.spawn((
                Tile,
                GridPos(Vec2::new(x as f32, y as f32)),
                Sprite::from_image(tile.clone()),
                Transform::default(),
            ));
        }
    }
}
