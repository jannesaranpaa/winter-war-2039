//! # Winter War 2039
//!
//! An isometric 2D game build with Bevy. The program is organised as a set of
//! plugins, each owning a concern. This file wires them together.
//!
//! - [`state`]  — the coarse lifecycle: `Loading`, then `Running`.
//! - [`sheets`] — loads Aseprite sprite sheets as assets and names them.
//! - [`iso`]    — the isometric math: grid position -> screen position + depth.
//! - [`anim`]   — plays frame animations from a sheet's Aseprite tags.
//! - [`world`]  — the camera and the static tile map.
//! - [`cursor`] — the highlight that follows the mouse over grid cells.
//! - [`mod@unit`] — characters standing on the grid.
//!
//! Data flows in one direction. Gameplay code decides *where* things are by
//! writing a `GridPos`; `iso` turns that into a `Transform`; Bevy draws it.
//! Nothing outside `iso` should ever write a sprite's `Transform` by hand.

mod anim;
mod cursor;
mod iso;
mod sheets;
mod state;
mod unit;
mod world;

use bevy::prelude::*;

fn main() {
    App::new()
        // Pixel art must be scaled without smoothing, otherwise the sprites
        // turn blurry when the camera zooms. "Nearest" keeps hard pixel edges.
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        // The state machine starts in `AppState::Loading`; see `state.rs`.
        .init_state::<state::AppState>()
        // Plugin order is not significant here: each plugin states its own
        // ordering needs (e.g. the cursor says it must run before projection).
        .add_plugins((
            sheets::SheetsPlugin,
            iso::IsoPlugin,
            anim::AnimationPlugin,
            world::WorldPlugin,
            cursor::CursorPlugin,
            unit::UnitPlugin,
        ))
        .run();
}
