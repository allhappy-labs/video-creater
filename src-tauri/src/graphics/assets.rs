//! Approved image asset resolution for graphics rendering.

use super::error::{ActionableError, ActionableResult, GraphicsErrorCode};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageAsset {
    pub asset_id: String,
    pub relative_path: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedImageAsset {
    pub asset_id: String,
    pub absolute_path: PathBuf,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct AssetRegistry {
    project_root: PathBuf,
    image_assets: BTreeMap<String, ImageAsset>,
}

impl AssetRegistry {
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            project_root,
            image_assets: BTreeMap::new(),
        }
    }

    pub fn register(&mut self, asset: ImageAsset) {
        self.image_assets.insert(asset.asset_id.clone(), asset);
    }

    pub fn resolve(&self, asset_id: &str) -> ActionableResult<ResolvedImageAsset> {
        let Some(asset) = self.image_assets.get(asset_id) else {
            return Err(vec![ActionableError::new(
                GraphicsErrorCode::GraphicsImageRefMissing,
                "assetId",
                "Image asset reference is not registered.",
                "Register the image asset or replace assetId with an approved imageRef.",
            )]);
        };

        let relative_path = Path::new(&asset.relative_path);
        if !is_authorized_relative_path(relative_path) {
            return Err(vec![ActionableError::new(
                GraphicsErrorCode::GraphicsImageRefUnauthorized,
                "assetId",
                "Image asset path is not approved for project graphics.",
                "Use a project-relative image path without parent-directory segments.",
            )]);
        }

        Ok(ResolvedImageAsset {
            asset_id: asset.asset_id.clone(),
            absolute_path: self.project_root.join(relative_path),
            width: asset.width,
            height: asset.height,
        })
    }
}

fn is_authorized_relative_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path.components().all(|component| {
            !matches!(
                component,
                Component::ParentDir | Component::Prefix(_) | Component::RootDir
            )
        })
}
