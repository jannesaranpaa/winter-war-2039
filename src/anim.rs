//! # Frame animation
//!
//! An animation is a list of frames from a sprite sheet, each shown for its own
//! duration, then looping (or, for one-shots like a wind-up, stopping on the
//! last frame and reporting that it finished). Sprites name ranges of frames with Aseprite *tags*
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
    /// Whether the animation wraps around at the end. A one-shot (`false`)
    /// instead holds its last frame and reports [`Animator::finished`].
    looping: bool,
    /// Set once a one-shot has shown its last frame for its full duration.
    /// Always `false` for looping animations.
    done: bool,
}

impl Animator {
    /// Starts playing `tag` from its first frame, looping.
    pub fn new(handle: Handle<SpriteSheet>, sheet: &SpriteSheet, tag: &str) -> Self {
        Self::with_looping(handle, sheet, tag, true)
    }

    /// Starts playing `tag` from its first frame, looping or as a one-shot.
    fn with_looping(
        handle: Handle<SpriteSheet>,
        sheet: &SpriteSheet,
        tag: &str,
        looping: bool,
    ) -> Self {
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
            looping,
            done: false,
        }
    }

    /// The sheet this animator plays from, so callers can look up its data
    /// (e.g. to pass to [`Animator::play`]).
    pub fn sheet(&self) -> &Handle<SpriteSheet> {
        &self.sheet
    }

    /// Switches to `tag`, looping. Does nothing if it is already playing, so it
    /// is safe to call every frame (e.g. "while moving, play `move`") without
    /// restarting the animation each time.
    pub fn play(&mut self, sheet: &SpriteSheet, tag: &str) {
        if self.tag != tag {
            *self = Self::with_looping(self.sheet.clone(), sheet, tag, true);
        }
    }

    /// Switches to `tag` and plays it once, holding the last frame. Poll
    /// [`Animator::finished`] to know when to move on. Like [`Animator::play`],
    /// it does nothing if `tag` is already the current tag.
    pub fn play_once(&mut self, sheet: &SpriteSheet, tag: &str) {
        if self.tag != tag {
            *self = Self::with_looping(self.sheet.clone(), sheet, tag, false);
        }
    }

    /// True once a one-shot animation has fully played. Never true for a
    /// looping animation.
    pub fn finished(&self) -> bool {
        self.done
    }

    /// Advances playback by `delta`. Returns the new frame index if the shown
    /// frame changed. Pure logic, so it can be unit tested without an `App`.
    fn advance(&mut self, sheet: &SpriteSheet, delta: std::time::Duration) -> Option<usize> {
        // A finished one-shot holds its last frame forever.
        if self.done || !self.timer.tick(delta).just_finished() {
            return None;
        }

        // The frame's time is up. At the end of the list, a looping animation
        // wraps to the start; a one-shot stops and marks itself finished.
        if self.pos + 1 >= self.seq.len() && !self.looping {
            self.done = true;
            return None;
        }

        // Otherwise step to the next frame and arm the timer with that frame's
        // own duration.
        self.pos = (self.pos + 1) % self.seq.len();
        let frame = self.seq[self.pos];
        self.timer = Timer::from_seconds(sheet.frame_secs(frame), TimerMode::Once);
        Some(frame)
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
            *a = Animator::with_looping(a.sheet.clone(), sheet, &a.tag, a.looping);
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

        // Advance the clock; if the shown frame changed, display it.
        if let Some(frame) = a.advance(sheet, time.delta()) {
            atlas.index = frame;
        }
    }
}

#[cfg(test)]
mod tests {
    //! Playback is pure logic over a hand-built sheet; no `App` is needed.

    use std::time::Duration;

    use super::*;

    /// 4 frames of 100 ms each, with a 3-frame tag `walk` (frames 1..=3).
    fn sheet() -> SpriteSheet {
        SpriteSheet::for_test(vec![100; 4], &[("walk", 1, 3)])
    }

    /// A tick just over one 100 ms frame: timers hold seconds as `f32`, so exactly
    /// 100 ms would fall a hair short of finishing the frame.
    const FRAME: Duration = Duration::from_millis(101);

    #[test]
    fn looping_animation_wraps_and_never_finishes() {
        let s = sheet();
        let mut a = Animator::with_looping(Handle::default(), &s, "walk", true);
        assert_eq!(a.advance(&s, FRAME), Some(2));
        assert_eq!(a.advance(&s, FRAME), Some(3));
        assert_eq!(a.advance(&s, FRAME), Some(1)); // wrapped
        assert!(!a.finished());
    }

    #[test]
    fn one_shot_holds_last_frame_then_finishes() {
        let s = sheet();
        let mut a = Animator::with_looping(Handle::default(), &s, "walk", false);
        assert_eq!(a.advance(&s, FRAME), Some(2));
        assert_eq!(a.advance(&s, FRAME), Some(3));
        // The last frame must be shown for its full duration before "finished".
        assert!(!a.finished());
        assert_eq!(a.advance(&s, FRAME), None);
        assert!(a.finished());
        // Further time changes nothing.
        assert_eq!(a.advance(&s, FRAME), None);
    }

    #[test]
    fn play_once_restarts_only_when_the_tag_changes() {
        let s = sheet();
        let mut a = Animator::new(Handle::default(), &s, "idle");
        a.play_once(&s, "walk");
        assert!(!a.looping);
        a.advance(&s, FRAME);
        a.play_once(&s, "walk"); // same tag: no restart
        assert_eq!(a.pos, 1);
    }
}
