use crate::project::model::VideoProject;
use crate::project::split::{save_split_project, SPLIT_PROJECT_SCHEMA_VERSION};
use tempfile::TempDir;

pub struct SchemaV2ProjectFixture {
    directory: TempDir,
}

impl SchemaV2ProjectFixture {
    pub fn materialize() -> Result<Self, String> {
        let directory = tempfile::tempdir()
            .map_err(|error| format!("could not create temporary MCP project: {error}"))?;
        let project = VideoProject::new_empty(
            "settings-mcp-probe".to_string(),
            "Settings MCP Probe".to_string(),
            "2026-01-01T00:00:00Z".to_string(),
        );
        save_split_project(directory.path(), &project)
            .map_err(|error| format!("could not materialize temporary MCP project: {error}"))?;
        Ok(Self { directory })
    }

    pub fn path(&self) -> &std::path::Path {
        self.directory.path()
    }

    pub const fn schema_version(&self) -> u32 {
        SPLIT_PROJECT_SCHEMA_VERSION
    }
}
