//! # Frame animation
//!
//! An animation is a list of frames from a sprite sheet, each shown for its own
//! duration, then looping. Sprites name ranges of frames with Aseprite *tags*
//! ("idle", "move", ...); an `Animator` plays one tag at a time.
//!
//! The animator only decides *which frame* to show. It writes that to the
//! sprite's `TextureAtlas::index`, the field that selects a frame from the
//! sheet's single image.

use bevy::prelude::*;

use crate::sheets::SpriteSheet;

pub struct AnimationPlugin;

impl Plugin for AnimationPlugin {
    fn build(&self, app: &mut App) {
        // `.chain()` fixes the order: first react to edited sheets, then
        // advance the frames, so a reload takes effect in the same frame.
        app.add_systems(Update, (reload_animators, animate).chain());
    }
}

/// The playback state of one animated sprite.
#[derive(Component)]
pub struct Animator {
    /// Which sheet the frames come from. A handle, so we can look up the
    /// current data each frame (it can change when the file is edited).
    sheet: Handle<SpriteSheet>,
    /// The tag being played, kept so we can rebuild after a reload.
    tag: String,
    /// Frame indices in playback order. Precomputed from the tag so a tag that
    /// plays backwards or ping-pongs is just a different list.
    seq: Vec<usize>,
    /// Where we are in `seq` (not a frame index, an index *into the list*).
    pos: usize,
    /// Counts down the current frame's duration.
    timer: Timer,
    /// Set when the sprite still shows the old animation's frame and must be
    /// updated to ours straight away, rather than after the first timer tick.
    snap: bool,
}

impl Animator {
    /// Starts playing `tag` from its first frame.
    pub fn new(handle: Handle<SpriteSheet>, sheet: &SpriteSheet, tag: &str) -> Self {
        let seq = sheet.sequence(tag);
        let secs = sheet.frame_secs(seq[0]);
        Self {
            sheet: handle,
            tag: tag.into(),
            seq,
            pos: 0,
            // `Once`: we re-arm it ourselves per frame, because each frame can
            // have a different duration.
            timer: Timer::from_seconds(secs, TimerMode::Once),
            snap: true,
        }
    }

    /// Switches to `tag`. Does nothing if it is already playing, so it is safe
    /// to call every frame (e.g. "while moving, play `move`") without
    /// restarting the animation each time.
    #[allow(dead_code)] // not used yet; will be once units can move
    pub fn play(&mut self, sheet: &SpriteSheet, tag: &str) {
        if self.tag != tag {
            *self = Self::new(self.sheet.clone(), sheet, tag);
        }
    }
}

/// Hot reload. When the sheet's JSON is edited, the asset server announces
/// `Modified`. The old frame list may now be wrong (frames added, durations
/// changed), so every animator using that sheet restarts its current tag.
fn reload_animators(
    mut events: MessageReader<AssetEvent<SpriteSheet>>,
    sheets: Res<Assets<SpriteSheet>>,
    mut q: Query<&mut Animator>,
) {
    for event in events.read() {
        // We only care about edits; ignore added/removed/etc.
        let AssetEvent::Modified { id } = event else {
            continue;
        };
        let Some(sheet) = sheets.get(*id) else {
            continue;
        };
        for mut a in q.iter_mut().filter(|a| a.sheet.id() == *id) {
            *a = Animator::new(a.sheet.clone(), sheet, &a.tag);
        }
    }
}

/// Advances every animation by the time elapsed since the last frame.
fn animate(
    time: Res<Time>,
    sheets: Res<Assets<SpriteSheet>>,
    mut q: Query<(&mut Animator, &mut Sprite)>,
) {
    for (mut a, mut sprite) in &mut q {
        // Skip until we have both the sheet data and an atlas to write into.
        let (Some(sheet), Some(atlas)) = (sheets.get(&a.sheet), &mut sprite.texture_atlas) else {
            continue;
        };

        // A brand-new or just-switched animator: show its first frame now.
        if a.snap {
            atlas.index = a.seq[a.pos];
            a.snap = false;
        }

        // When the current frame's time is up, step to the next one (wrapping
        // to the start, so the animation loops) and arm the timer with that
        // frame's own duration.
        if a.timer.tick(time.delta()).just_finished() {
            a.pos = (a.pos + 1) % a.seq.len();
            let frame = a.seq[a.pos];
            atlas.index = frame;
            a.timer = Timer::from_seconds(sheet.frame_secs(frame), TimerMode::Once);
        }
    }
}
