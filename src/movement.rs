//! # Click to move
//!
//! Left-click a cell and the unit walks to it in a straight line. The walk is
//! a small state machine that is driven by, and drives, the unit's animation:
//!
//! ```text
//! Idle --click--> Starting --(start_move done)--> Moving
//!   ^                                               |
//!   +-- Ending <------------(arrived)---------------+
//!        (end_move done)
//! ```
//!
//! Only `GridPos` is written here; `iso.rs` turns it into a screen position.

use bevy::prelude::*;

use crate::anim::Animator;
use crate::cursor::{cell_in_bounds, mouse_cell};
use crate::iso::{GridPos, ProjectSet};
use crate::sheets::SpriteSheet;
use crate::unit::Unit;

pub struct MovementPlugin;

impl Plugin for MovementPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            // Click first so a new target is seen in the same frame. Both run
            // before the projection because the second one writes `GridPos`.
            (set_target_on_click, drive_movers)
                .chain()
                .before(ProjectSet),
        );
    }
}

/// Walking speed in grid cells per second.
const SPEED: f32 = 3.0;

/// Where a mover is in its walk cycle; see the diagram in the module docs.
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum MoveState {
    /// Standing still, playing `idle`.
    #[default]
    Idle,
    /// Playing the one-shot `start_move` before setting off.
    Starting,
    /// Travelling toward the target, looping `move`.
    Moving,
    /// Arrived; playing the one-shot `end_move` before going idle.
    Ending,
}

/// Lets a unit walk. Added to every unit when it is spawned.
#[derive(Component, Default)]
pub struct Mover {
    /// Current phase of the walk.
    state: MoveState,
    /// The cell to walk to, if any. Kept until reached, and may be replaced
    /// by a later click.
    target: Option<Vec2>,
}

/// Moves `pos` toward `target` by at most `speed * dt` cells, in a straight
/// line. Returns the new position and whether it has arrived. Snaps exactly
/// onto the target rather than overshooting it.
pub fn step_toward(pos: Vec2, target: Vec2, speed: f32, dt: f32) -> (Vec2, bool) {
    let delta = target - pos;
    let dist = delta.length();
    let max_step = speed * dt;
    if dist <= max_step {
        (target, true)
    } else {
        (pos + delta / dist * max_step, false)
    }
}

/// On a left click over the map, tells every unit to walk to that cell.
fn set_target_on_click(
    buttons: Res<ButtonInput<MouseButton>>,
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera2d>>,
    mut movers: Query<(&mut Mover, &GridPos), With<Unit>>,
) {
    // Step 1: only react to the frame in which the button goes down.
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }

    // Step 2: find the cell under the mouse, ignoring clicks off the map.
    let (camera, camera_transform) = *camera;
    let Some(cell) = mouse_cell(&window, camera, camera_transform) else {
        return;
    };
    if !cell_in_bounds(cell) {
        return;
    }

    // Step 3: hand the target to each unit, skipping units already standing
    // exactly there (a walk to nowhere would still play both animations).
    for (mut mover, pos) in &mut movers {
        if pos.0 != cell {
            mover.target = Some(cell);
        }
    }
}

/// Advances each mover's state machine and animation, and its position while
/// it is travelling.
fn drive_movers(
    time: Res<Time>,
    sheets: Res<Assets<SpriteSheet>>,
    mut q: Query<(&mut Mover, &mut GridPos, &mut Animator)>,
) {
    for (mut mover, mut pos, mut anim) in &mut q {
        // The sheet data is needed to switch tags; wait until it is loaded.
        let Some(sheet) = sheets.get(anim.sheet()) else {
            continue;
        };

        match mover.state {
            // Idle is handled after the match, so a unit that just finished
            // `Ending` can set off again in the same frame.
            MoveState::Idle => {}

            // Wind-up finished: start walking.
            MoveState::Starting => {
                if anim.finished() {
                    mover.state = MoveState::Moving;
                    anim.play(sheet, "move");
                }
            }

            MoveState::Moving => match mover.target {
                Some(target) => {
                    let (next, arrived) = step_toward(pos.0, target, SPEED, time.delta_secs());
                    // `set_if_neq` keeps change detection quiet when nothing moved.
                    pos.set_if_neq(GridPos(next));
                    if arrived {
                        mover.target = None;
                        mover.state = MoveState::Ending;
                        anim.play_once(sheet, "end_move");
                    }
                }
                // No target should not happen while moving; recover by stopping.
                None => {
                    mover.state = MoveState::Ending;
                    anim.play_once(sheet, "end_move");
                }
            },

            // Wind-down finished: stand still.
            MoveState::Ending => {
                if anim.finished() {
                    mover.state = MoveState::Idle;
                    anim.play(sheet, "idle");
                }
            }
        }

        // A target while idle starts the walk with the wind-up animation.
        if mover.state == MoveState::Idle && mover.target.is_some() {
            mover.state = MoveState::Starting;
            anim.play_once(sheet, "start_move");
        }
    }
}

#[cfg(test)]
mod tests {
    //! The step function is pure maths, so it is tested without an `App`.

    use super::*;

    #[test]
    fn partial_step_moves_along_the_line() {
        let (p, arrived) = step_toward(Vec2::ZERO, Vec2::new(10.0, 0.0), 2.0, 0.5);
        assert_eq!(p, Vec2::new(1.0, 0.0));
        assert!(!arrived);
    }

    #[test]
    fn exact_arrival_is_reported() {
        let (p, arrived) = step_toward(Vec2::ZERO, Vec2::new(1.0, 0.0), 2.0, 0.5);
        assert_eq!(p, Vec2::new(1.0, 0.0));
        assert!(arrived);
    }

    /// A step bigger than the remaining distance snaps to the target.
    #[test]
    fn never_overshoots() {
        let target = Vec2::new(3.0, 4.0);
        let (p, arrived) = step_toward(Vec2::new(2.0, 3.0), target, 100.0, 1.0);
        assert_eq!(p, target);
        assert!(arrived);
    }

    /// Diagonal steps cover `speed * dt` in total, not per axis.
    #[test]
    fn diagonal_speed_matches_straight_speed() {
        let (p, _) = step_toward(Vec2::ZERO, Vec2::new(10.0, 10.0), 1.0, 1.0);
        assert!((p.length() - 1.0).abs() < 1e-5);
    }
}
