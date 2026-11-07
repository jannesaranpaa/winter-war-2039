//! # Sprite sheets
//!
//! Art is drawn in Aseprite and exported as two files side by side:
//!
//! - `name.png`  — every animation frame packed into one image (a *sheet*).
//! - `name.json` — where each frame is in that image, how long it lasts, and
//!   the named frame ranges ("tags") such as `idle` or `move`.
//!
//! This module teaches Bevy to load the pair as one asset, `SpriteSheet`, and
//! keeps a registry so the rest of the game can ask for a sheet by name.
//! Because it is a proper asset, Bevy loads it in the background and reloads it
//! when the file is edited.

use bevy::{
    asset::{AssetLoader, LoadContext, io::Reader},
    prelude::*,
};
use serde::Deserialize;
use std::collections::HashMap;

use crate::state::AppState;

/// The sheets the game loads at startup. Add a name here to make a new sheet
/// available (its files must be `assets/<name>.png` and `assets/<name>.json`).
const SHEET_NAMES: &[&str] = &["placeholder"];

pub struct SheetsPlugin;

impl Plugin for SheetsPlugin {
    fn build(&self, app: &mut App) {
        app
            // Tell Bevy the asset type and its loader exist.
            .init_asset::<SpriteSheet>()
            .init_asset_loader::<SpriteSheetLoader>()
            .init_resource::<Sheets>()
            // `PreStartup` runs before `Startup`, so the requests are already
            // issued by the time anything else starts.
            .add_systems(PreStartup, load_sheets)
            // While `Loading`, check every frame whether we are done yet.
            .add_systems(Update, finish_loading.run_if(in_state(AppState::Loading)));
    }
}

/// Asks the asset server to load every sheet. This returns immediately; the
/// files are read in the background.
fn load_sheets(mut sheets: ResMut<Sheets>, asset_server: Res<AssetServer>) {
    for name in SHEET_NAMES {
        sheets.load(name, &asset_server);
    }
}

/// Moves the app from `Loading` to `Running` once every sheet is ready.
///
/// "Loaded with dependencies" matters: a sheet is not usable until its PNG has
/// arrived too, and the PNG is a dependency the loader registered below.
fn finish_loading(
    sheets: Res<Sheets>,
    asset_server: Res<AssetServer>,
    mut next: ResMut<NextState<AppState>>,
) {
    if sheets
        .by_name
        .values()
        .all(|h| asset_server.is_loaded_with_dependencies(h))
    {
        next.set(AppState::Running);
    }
}

// ---------- The Aseprite JSON format ----------
//
// These structs mirror only the parts of Aseprite's export that we use.
// `serde` fills them in from the file; anything we do not name is ignored.

#[derive(Deserialize)]
struct SheetJson {
    frames: Vec<FrameJson>,
    meta: MetaJson,
}

/// One animation frame: its rectangle inside the PNG, and how long to show it.
#[derive(Deserialize)]
struct FrameJson {
    frame: RectJson,
    /// Milliseconds.
    duration: u32,
}

#[derive(Deserialize)]
struct RectJson {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

#[derive(Deserialize)]
struct MetaJson {
    /// File name of the PNG, relative to the JSON.
    image: String,
    /// Size of the whole PNG.
    size: SizeJson,
    /// Named frame ranges. `default`: a sheet without tags is still valid.
    #[serde(rename = "frameTags", default)]
    frame_tags: Vec<TagJson>,
}

#[derive(Deserialize)]
struct SizeJson {
    w: u32,
    h: u32,
}

/// A named range of frames, `from..=to`, played in a given `direction`.
#[derive(Deserialize, Clone)]
struct TagJson {
    name: String,
    from: usize,
    to: usize,
    /// "forward", "reverse" or "pingpong".
    direction: String,
}

// ---------- The asset ----------

/// A loaded sprite sheet, ready to use.
#[derive(Asset, TypePath)]
pub struct SpriteSheet {
    /// The packed image holding all frames.
    pub image: Handle<Image>,
    /// Describes where each frame is in the image. A sprite picks a frame by
    /// index into this layout.
    pub layout: Handle<TextureAtlasLayout>,
    /// Per-frame duration in milliseconds, indexed by frame number.
    durations: Vec<u32>,
    /// Tags by name.
    tags: HashMap<String, TagJson>,
}

impl SpriteSheet {
    /// The frame numbers to play for a tag, in order.
    ///
    /// Aseprite tags can run backwards or bounce; expanding that into a plain
    /// list here means the animator never has to know about directions.
    pub fn sequence(&self, tag: &str) -> Vec<usize> {
        // An unknown tag plays the whole sheet rather than crashing.
        let Some(t) = self.tags.get(tag) else {
            return (0..self.durations.len()).collect();
        };
        let fwd: Vec<usize> = (t.from..=t.to).collect();
        match t.direction.as_str() {
            "reverse" => fwd.into_iter().rev().collect(),
            // Ping-pong goes forward, then back down *excluding both ends*,
            // so the end frames are not shown twice in a row when it loops.
            "pingpong" => {
                let back = (t.from + 1..t.to).rev();
                fwd.iter().copied().chain(back).collect()
            }
            _ => fwd,
        }
    }

    /// How long a frame is shown, in seconds.
    pub fn frame_secs(&self, frame: usize) -> f32 {
        self.durations[frame] as f32 / 1000.0
    }
}

// ---------- The loader ----------

/// Teaches the asset server how to turn a `.json` file into a `SpriteSheet`.
#[derive(Default, TypePath)]
struct SpriteSheetLoader;

impl AssetLoader for SpriteSheetLoader {
    type Asset = SpriteSheet;
    type Settings = ();
    type Error = std::io::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        load_context: &mut LoadContext<'_>,
    ) -> Result<SpriteSheet, Self::Error> {
        // Read the whole file, then parse it. A malformed file becomes a load
        // error naming the file, instead of a crash.
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let data: SheetJson = serde_json::from_slice(&bytes).map_err(|e| {
            std::io::Error::other(format!(
                "invalid aseprite json {}: {e}",
                load_context.path()
            ))
        })?;

        // Build the atlas layout: one rectangle per frame, in frame order, so
        // frame number N in the JSON is index N for the sprite.
        let mut layout =
            TextureAtlasLayout::new_empty(UVec2::new(data.meta.size.w, data.meta.size.h));
        for f in &data.frames {
            let r = &f.frame;
            layout.add_texture(URect::new(r.x, r.y, r.x + r.w, r.y + r.h));
        }

        // The PNG sits next to the JSON; its name is in `meta.image`. Loading it
        // from inside the loader makes it a *dependency* of this asset, so the
        // sheet counts as ready only once the image has arrived as well.
        let image_path = load_context.path().path().with_file_name(&data.meta.image);

        Ok(SpriteSheet {
            image: load_context.load(image_path),
            // The layout is created here, not on disk, so it is registered as
            // a labeled sub-asset ("name.json#layout") of this file.
            layout: load_context.add_labeled_asset("layout".to_string(), layout),
            durations: data.frames.iter().map(|f| f.duration).collect(),
            tags: data
                .meta
                .frame_tags
                .into_iter()
                .map(|t| (t.name.clone(), t))
                .collect(),
        })
    }

    /// Files with this extension are handled by this loader.
    fn extensions(&self) -> &[&str] {
        &["json"]
    }
}

// ---------- The registry ----------

/// Looks up sheets by name. It holds handles, not data: the data lives in
/// `Assets<SpriteSheet>` and may not have arrived yet (see `AppState`).
#[derive(Resource, Default)]
pub struct Sheets {
    by_name: HashMap<String, Handle<SpriteSheet>>,
}

impl Sheets {
    /// Requests `assets/<name>.json` and remembers the handle under `name`.
    fn load(&mut self, name: &str, asset_server: &AssetServer) {
        self.by_name
            .insert(name.to_string(), asset_server.load(format!("{name}.json")));
    }

    /// The handle for a sheet. Panics on a typo, since an unknown sheet name is
    /// a bug in the code, not a runtime condition to recover from.
    pub fn handle(&self, name: &str) -> &Handle<SpriteSheet> {
        self.by_name
            .get(name)
            .unwrap_or_else(|| panic!("sheet '{name}' not registered"))
    }
}
