pub mod cancel;
pub mod capabilities;
pub mod catalog;
pub mod download;
pub mod elevenlabs;
pub mod fal;
pub mod google;
pub mod minimax;
pub mod mock;
pub mod openai;
pub mod orphan_watch;
pub mod replicate;
pub mod xai;

use std::path::Path;

use crate::project::model::GeneratedAsset;

#[derive(Clone, Copy, Debug)]
pub struct GenerationTarget<'a> {
    pub project_dir: &'a Path,
    pub asset: &'a GeneratedAsset,
    pub updated_at: &'a str,
    pub run_id: Option<&'a str>,
    pub replacement_item_id: Option<&'a str>,
}

impl<'a> GenerationTarget<'a> {
    pub const fn new(
        project_dir: &'a Path,
        asset: &'a GeneratedAsset,
        updated_at: &'a str,
        run_id: Option<&'a str>,
        replacement_item_id: Option<&'a str>,
    ) -> Self {
        Self {
            project_dir,
            asset,
            updated_at,
            run_id,
            replacement_item_id,
        }
    }
}
