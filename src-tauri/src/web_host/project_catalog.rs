use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Serialize;
use sha2::{Digest, Sha256};

const MANIFEST: &str = "video-creater.project.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteProjectSummary {
    pub project_id: String,
    pub name: String,
    pub updated_at_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProjectCatalog {
    roots: Vec<PathBuf>,
}

impl ProjectCatalog {
    pub fn new(roots: Vec<PathBuf>) -> Result<Self, String> {
        if roots.is_empty() {
            return Err("at least one remote project root is required".into());
        }
        let mut canonical = Vec::with_capacity(roots.len());
        for root in roots {
            fs::create_dir_all(&root).map_err(|_| "project root could not be created")?;
            canonical
                .push(fs::canonicalize(root).map_err(|_| "project root could not be resolved")?);
        }
        canonical.sort();
        canonical.dedup();
        Ok(Self { roots: canonical })
    }

    pub fn list(&self) -> Result<Vec<RemoteProjectSummary>, String> {
        let mut projects = BTreeMap::new();
        for root in &self.roots {
            let entries = fs::read_dir(root).map_err(|_| "project root could not be read")?;
            for entry in entries.flatten() {
                let path = entry.path();
                if is_hidden_project_path(&path) {
                    continue;
                }
                let Ok(canonical) = fs::canonicalize(&path) else {
                    continue;
                };
                if is_hidden_project_path(&canonical)
                    || !beneath(&canonical, root)
                    || !canonical.join(MANIFEST).is_file()
                {
                    continue;
                }
                let Ok(summary) = self.summary(&canonical) else {
                    continue;
                };
                projects
                    .entry(summary.project_id.clone())
                    .or_insert(summary);
            }
        }
        Ok(projects.into_values().collect())
    }

    pub fn resolve(&self, project_id: &str) -> Result<PathBuf, String> {
        if project_id.len() != 64 || !project_id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("project ID is invalid".into());
        }
        for root in &self.roots {
            let entries = fs::read_dir(root).map_err(|_| "project root could not be read")?;
            for entry in entries.flatten() {
                let path = entry.path();
                if is_hidden_project_path(&path) {
                    continue;
                }
                let Ok(canonical) = fs::canonicalize(path) else {
                    continue;
                };
                if is_hidden_project_path(&canonical)
                    || !beneath(&canonical, root)
                    || !canonical.join(MANIFEST).is_file()
                {
                    continue;
                }
                if opaque_id(&canonical) == project_id {
                    return Ok(canonical);
                }
            }
        }
        Err("project is not available".into())
    }

    pub fn id_for_path(&self, path: &Path) -> Result<String, String> {
        let canonical = fs::canonicalize(path).map_err(|_| "project could not be resolved")?;
        if is_hidden_project_path(&canonical)
            || !self.roots.iter().any(|root| beneath(&canonical, root))
        {
            return Err("project is outside allowed roots".into());
        }
        Ok(opaque_id(&canonical))
    }

    pub fn primary_root(&self) -> &Path {
        &self.roots[0]
    }

    pub fn create_path(&self) -> Result<PathBuf, String> {
        let path = self
            .primary_root()
            .join(format!("{}.palmier", uuid::Uuid::new_v4().simple()));
        fs::create_dir(&path).map_err(|_| "project folder could not be created")?;
        Ok(path)
    }

    fn summary(&self, path: &Path) -> Result<RemoteProjectSummary, String> {
        let manifest = fs::read(path.join(MANIFEST)).map_err(|_| "project could not be read")?;
        let value: serde_json::Value =
            serde_json::from_slice(&manifest).map_err(|_| "project manifest is invalid")?;
        let name = value
            .get("name")
            .and_then(serde_json::Value::as_str)
            .filter(|name| !name.trim().is_empty())
            .ok_or_else(|| "project name is invalid".to_string())?;
        let updated_at_ms = fs::metadata(path.join(MANIFEST))
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
            .unwrap_or(0);
        Ok(RemoteProjectSummary {
            project_id: opaque_id(path),
            name: name.chars().take(200).collect(),
            updated_at_ms,
            thumbnail_id: None,
        })
    }
}

fn beneath(path: &Path, root: &Path) -> bool {
    path != root && path.starts_with(root)
}

fn is_hidden_project_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with('.'))
}

fn opaque_id(path: &Path) -> String {
    format!("{:x}", Sha256::digest(path.as_os_str().as_encoded_bytes()))
}
