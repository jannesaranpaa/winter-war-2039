//! # Isometric projection
//!
//! The game world is a square grid, but we draw it from a 45° angle so that it
//! looks like a diamond. Every object has *two* positions:
//!
//! 1. A **grid position** (`GridPos`): which cell it is in, as the game logic sees it.
//! 2. A **screen position** (`Transform`): where its sprite is drawn.
//!
//! This module owns the conversion between the two. `GridPos` is the source of
//! truth; the `Transform` is derived from it and must not be edited elsewhere.
//!
//! ## The projection
//!
//! Moving one cell along grid-x goes right and down on screen; moving one cell
//! along grid-y goes left and down. A tile is a 2:1 diamond, `TILE_W` wide and
//! `TILE_H` tall, so one step is half a tile in each direction:
//!
//! ```text
//!   screen_x =  (gx - gy) * TILE_W / 2
//!   screen_y = -(gx + gy) * TILE_H / 2      (Bevy's y axis points up)
//! ```
//!
//! Note that `gx - gy` and `gx + gy` are the two diagonals of the grid. They
//! appear again below when we invert the formula for mouse picking.

use bevy::prelude::*;

/// Width of a tile's diamond in pixels.
pub const TILE_W: f32 = 32.0;
/// Height of a tile's diamond in pixels (half the width: the classic 2:1 look).
pub const TILE_H: f32 = 16.0;

/// The tile image is a diamond *plus* some thickness hanging below it, so the
/// image's centre is not the diamond's centre. The top face sits this many
/// pixels above the image centre (the thickness is 4px, so half of it).
/// Mouse picking must correct for this, or clicks land half a cell off.
const TILE_FACE_OFFSET_Y: f32 = 2.0;

/// ## Depth ("y-sorting")
///
/// Bevy draws 2D sprites in order of their z coordinate, higher z on top. In an
/// isometric view, what is lower on screen is closer to the viewer and must
/// cover what is behind it. So we derive z from screen y: lower y, higher z.
///
/// This constant is how much z one screen pixel is worth. One grid row is
/// `TILE_H / 2` = 8px tall, which comes out to 0.001 of z. `ScreenOffset::z`
/// biases (for layering things inside one cell) must stay below that, or an
/// object would jump over the row in front of it. The total z used is
/// `map height in pixels * Z_PER_PIXEL`, which must fit in the camera's range.
const Z_PER_PIXEL: f32 = 0.001 / (TILE_H / 2.0);

/// Where an entity is on the grid, in *continuous* coordinates.
///
/// Cells are centred on whole numbers, so (3.0, 2.0) is the middle of a cell and
/// (3.5, 2.0) is halfway towards its neighbour. Being continuous means a unit
/// can walk smoothly between cells without special cases.
#[derive(Component, Clone, Copy, PartialEq)]
pub struct GridPos(pub Vec2);

/// Adjusts how an entity is drawn relative to its cell, without changing where
/// it logically is.
#[derive(Component)]
pub struct ScreenOffset {
    /// Pixels to lift the sprite. A character's art is taller than a tile; this
    /// raises the picture so its feet land in the cell's centre.
    pub y: f32,
    /// Tiny depth bias to layer objects sharing a cell: e.g. the cursor above
    /// the floor tile, a unit above the cursor.
    pub z: f32,
}

/// A label for the projection system. Systems that *change* a `GridPos`
/// (the cursor, later unit movement) order themselves `.before(ProjectSet)`, so
/// the new position is drawn this frame instead of one frame late.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProjectSet;

pub struct IsoPlugin;

impl Plugin for IsoPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, project_grid_entities.in_set(ProjectSet));
    }
}

/// Grid -> world: where is the centre of this grid position on screen?
///
/// Returns x and y from the projection formulas above. z is computed from y
/// (see [`Z_PER_PIXEL`]), so depth varies smoothly even between cells.
/// Any `ScreenOffset` is applied later, by the caller.
pub fn grid_to_world(grid: Vec2) -> Vec3 {
    let x = (grid.x - grid.y) * TILE_W / 2.0;
    let y = -(grid.x + grid.y) * TILE_H / 2.0;
    Vec3::new(x, y, -y * Z_PER_PIXEL)
}

/// World -> grid: which grid position is under this point on screen?
///
/// This is the algebraic inverse of [`grid_to_world`], used for mouse picking.
/// The forward formulas turn the grid into two diagonals:
///
/// ```text
///   u = gx - gy = screen_x / (TILE_W / 2)
///   v = gx + gy = -screen_y / (TILE_H / 2)
/// ```
///
/// so we compute `u` and `v` from the screen point and then solve for the two
/// unknowns: `gx = (u + v) / 2` and `gy = (v - u) / 2`.
pub fn world_to_grid(world: Vec2) -> Vec2 {
    let u = world.x / (TILE_W / 2.0);
    // Undo the thickness correction first: the diamond we want to hit is the
    // top face, which is above the image's centre.
    let v = -(world.y - TILE_FACE_OFFSET_Y) / (TILE_H / 2.0);
    Vec2::new((u + v) / 2.0, (v - u) / 2.0)
}

/// Selects only entities whose position or offset changed since last frame.
/// Newly spawned entities count as changed, so everything is placed once, and
/// then the 64 static tiles cost nothing: only moving things are touched again.
type NeedsProjection = Or<(Changed<GridPos>, Changed<ScreenOffset>)>;

/// The system that keeps `Transform` in sync with `GridPos`.
fn project_grid_entities(
    mut q: Query<(&GridPos, &mut Transform, Option<&ScreenOffset>), NeedsProjection>,
) {
    for (pos, mut tf, offset) in &mut q {
        // Not every entity has an offset (tiles do not); treat missing as zero.
        let (dy, dz) = offset.map_or((0.0, 0.0), |o| (o.y, o.z));

        let mut world = grid_to_world(pos.0);
        // The lift is sprite art: it moves the picture, but not the sort depth.
        // Depth was fixed from the cell (the unit's feet), before this line.
        world.y += dy;
        world.z += dz;
        tf.translation = world;
    }
}

#[cfg(test)]
mod tests {
    //! The projection is pure maths, so it is tested without an `App`.
    //! The properties that matter: picking inverts drawing, and depth orders
    //! things the way the eye expects.

    use super::*;

    /// Picking must invert drawing. The sprite is placed at `grid_to_world`, but
    /// the diamond we click is the *top face*, `TILE_FACE_OFFSET_Y` above the
    /// image centre, so that is the point we feed to `world_to_grid`.
    #[test]
    fn picking_inverts_projection() {
        for gx in -3..10 {
            for gy in -3..10 {
                let grid = Vec2::new(gx as f32, gy as f32);
                let face = grid_to_world(grid).truncate() + Vec2::new(0.0, TILE_FACE_OFFSET_Y);
                let back = world_to_grid(face);
                assert!(back.abs_diff_eq(grid, 1e-4), "{grid} came back as {back}");
            }
        }
    }

    /// Rounding a point anywhere inside a tile's diamond must select that cell,
    /// which is how the cursor chooses a cell. We test points near the diamond's
    /// four corners (pulled slightly inward so they are unambiguously inside).
    #[test]
    fn points_inside_a_diamond_pick_its_cell() {
        let grid = Vec2::new(4.0, 2.0);
        let face = grid_to_world(grid).truncate() + Vec2::new(0.0, TILE_FACE_OFFSET_Y);
        let (hw, hh) = (TILE_W / 2.0 * 0.9, TILE_H / 2.0 * 0.9);
        for corner in [
            Vec2::new(hw, 0.0),
            Vec2::new(-hw, 0.0),
            Vec2::new(0.0, hh),
            Vec2::new(0.0, -hh),
        ] {
            assert_eq!(world_to_grid(face + corner).round(), grid);
        }
    }

    /// One step along grid-x goes right and down; along grid-y, left and down.
    #[test]
    fn axes_point_the_isometric_way() {
        let origin = grid_to_world(Vec2::ZERO);
        let dx = grid_to_world(Vec2::X) - origin;
        let dy = grid_to_world(Vec2::Y) - origin;
        assert_eq!((dx.x, dx.y), (TILE_W / 2.0, -TILE_H / 2.0));
        assert_eq!((dy.x, dy.y), (-TILE_W / 2.0, -TILE_H / 2.0));
    }

    /// Lower on screen means closer, so it must have a higher z.
    #[test]
    fn lower_on_screen_draws_on_top() {
        let behind = grid_to_world(Vec2::new(2.0, 2.0)).z;
        let in_front = grid_to_world(Vec2::new(3.0, 2.0)).z;
        assert!(in_front > behind);
    }

    /// Y-sorting is continuous: a position halfway between two cells sorts
    /// halfway between them, not snapped to either.
    #[test]
    fn depth_is_continuous_between_cells() {
        let a = grid_to_world(Vec2::new(2.0, 2.0)).z;
        let b = grid_to_world(Vec2::new(3.0, 2.0)).z;
        let mid = grid_to_world(Vec2::new(2.5, 2.0)).z;
        assert!(a < mid && mid < b);
        assert!((mid - (a + b) / 2.0).abs() < 1e-6);
    }

    /// Layer biases (cursor 0.0002, unit 0.0005) must not lift an object over
    /// the row in front of it; the row step is the safe upper bound.
    #[test]
    fn biases_stay_below_one_row_step() {
        let row = grid_to_world(Vec2::new(1.0, 0.0)).z - grid_to_world(Vec2::ZERO).z;
        assert!((row - 0.001).abs() < 1e-7, "row step is {row}");
        for bias in [0.0002, 0.0005] {
            assert!(bias < row);
        }
    }
}
