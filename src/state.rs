//! The application lifecycle.
//!
//! Assets load asynchronously: asking the asset server for a file returns a
//! handle immediately, but the data arrives some frames later. Anything that
//! needs the *contents* of an asset (a unit needs its sprite sheet to pick a
//! first frame) cannot be built until then.
//!
//! Rather than checking "is it loaded yet?" in every system, we keep one global
//! flag. Systems that need loaded data are scheduled for `OnEnter(Running)`,
//! which is guaranteed to happen after loading has finished.

use bevy::prelude::*;

#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum AppState {
    /// Sheets are still being read from disk. The app starts here.
    #[default]
    Loading,
    /// Everything is loaded; the game proper runs.
    Running,
}
