//! # The cell cursor
//!
//! A diamond outline that sits on whichever grid cell the mouse is over. It is
//! an ordinary grid entity: this module only decides *which cell*, and the
//! projection in `iso.rs` takes care of drawing it there.

use bevy::prelude::*;

use crate::iso::{GridPos, ProjectSet, ScreenOffset, world_to_grid};
use crate::world::GRID;

pub struct CursorPlugin;

impl Plugin for CursorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_cursor)
            // We change `GridPos`, so we must run before the projection reads
            // it; otherwise the cursor would trail the mouse by one frame.
            .add_systems(Update, cursor_follow_mouse.before(ProjectSet));
    }
}

/// Marks the one cursor entity, so the system below can find it.
#[derive(Component)]
struct Cursor;

fn spawn_cursor(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        Cursor,
        GridPos(Vec2::ZERO),
        // Sink the outline 1px so it hugs the tile's top face. The z bias puts
        // it just above the floor tile of its own cell, but below any unit.
        ScreenOffset { y: -1.0, z: 0.0002 },
        Sprite::from_image(asset_server.load("cursor.png")),
        Transform::default(), // filled in by the projection system
    ));
}

/// Whether a (rounded) grid cell lies on the map. Shared by hover and click
/// handling so both agree on what "on the map" means.
pub fn cell_in_bounds(cell: Vec2) -> bool {
    cell.x >= 0.0 && cell.y >= 0.0 && cell.x < GRID as f32 && cell.y < GRID as f32
}

/// The grid cell currently under the mouse, or `None` if the mouse is outside
/// the window or the camera cannot project it. The cell may be off the map;
/// check with [`cell_in_bounds`]. Shared by hovering and clicking.
pub fn mouse_cell(
    window: &Window,
    camera: &Camera,
    camera_transform: &GlobalTransform,
) -> Option<Vec2> {
    // Step 1: mouse position in window pixels. There is none while the mouse
    // is outside the window.
    let screen_pos = window.cursor_position()?;

    // Step 2: undo the camera (position and zoom) to get a point in the world.
    let world_pos = camera
        .viewport_to_world_2d(camera_transform, screen_pos)
        .ok()?;

    // Step 3: undo the isometric projection to get a grid coordinate. Cells are
    // centred on whole numbers, so rounding picks the cell the point is in.
    Some(world_to_grid(world_pos).round())
}

/// Each frame: mouse position -> grid cell -> move the cursor.
fn cursor_follow_mouse(
    // `Single` means "exactly one of these exists"; the system is skipped
    // otherwise. There is one window, one 2D camera, and one cursor.
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera2d>>,
    cursor: Single<(&mut GridPos, &mut Visibility), With<Cursor>>,
) {
    let (camera, camera_transform) = *camera;
    let (mut pos, mut vis) = cursor.into_inner();

    // Step 1: find the cell under the mouse. With the mouse outside the window
    // there is none, and we leave the cursor where it is.
    let Some(cell) = mouse_cell(&window, camera, camera_transform) else {
        return;
    };
    let in_bounds = cell_in_bounds(cell);

    // Step 2: move, but only if the mouse is over the map. `set_if_neq` writes
    // only when the value really differs, so Bevy's change detection stays
    // quiet while the mouse moves within one cell, and the projection (which
    // reacts to changes) does no work.
    if in_bounds {
        pos.set_if_neq(GridPos(cell));
    }

    // Hide the cursor when the mouse is off the map instead of leaving it
    // stranded on the last cell it touched.
    vis.set_if_neq(if in_bounds {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    });
}
