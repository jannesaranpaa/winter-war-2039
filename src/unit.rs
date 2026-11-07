//! # Units
//!
//! A unit is a character standing on the grid. It is assembled from parts that
//! each belong to another module: a position (`iso`), an animation (`anim`) and
//! a sprite from a sheet (`sheets`). This module just puts them together.

use bevy::prelude::*;

use crate::anim::Animator;
use crate::iso::{GridPos, ScreenOffset};
use crate::sheets::{Sheets, SpriteSheet};
use crate::state::AppState;

pub struct UnitPlugin;

impl Plugin for UnitPlugin {
    fn build(&self, app: &mut App) {
        // Building a unit reads its sheet's contents (frame layout, image), so
        // it must wait until loading is done: `OnEnter(Running)` is exactly that.
        app.add_systems(OnEnter(AppState::Running), spawn_units);
    }
}

/// Marks an entity as a unit, for queries that should act on units only.
#[derive(Component)]
pub struct Unit;

/// Creates one unit of the given sheet at a grid position.
///
/// A Bevy entity is just a bundle of components; each line below adds one.
pub fn spawn_unit(
    commands: &mut Commands,
    sheets: &Sheets,
    assets: &Assets<SpriteSheet>,
    sheet_name: &str,
    pos: Vec2,
) {
    // Look the sheet up by name, then fetch its loaded data. The `expect` holds
    // because this only runs after the `Loading` state has completed.
    let handle = sheets.handle(sheet_name);
    let sheet = assets.get(handle).expect("sheet loaded before Running");

    commands.spawn((
        Unit,
        // Where it stands. The projection turns this into a screen position.
        GridPos(pos),
        // The art is taller than a tile: lift it 8px so the feet are on the
        // cell's centre. The small z bias keeps it above the cursor and floor.
        ScreenOffset { y: 8.0, z: 0.0005 },
        // Starts playing the "idle" tag of the sheet.
        Animator::new(handle.clone(), sheet, "idle"),
        // The sprite shows one frame of the sheet's texture atlas (a single
        // image holding every frame); the animator changes `index` over time.
        Sprite::from_atlas_image(
            sheet.image.clone(),
            TextureAtlas {
                layout: sheet.layout.clone(),
                index: 0,
            },
        ),
        Transform::default(), // filled in by the projection system
    ));
}

/// The initial population of the world.
fn spawn_units(mut commands: Commands, sheets: Res<Sheets>, assets: Res<Assets<SpriteSheet>>) {
    spawn_unit(
        &mut commands,
        &sheets,
        &assets,
        "placeholder",
        Vec2::new(5.0, 5.0),
    );
}
