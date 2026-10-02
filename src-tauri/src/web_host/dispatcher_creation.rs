//! A server-reserved project identity is the durable creation commit witness.
use super::*;

impl HostDispatcher {
    pub(super) fn create_project(&self, payload: &Value) -> Result<Value, String> {
        self.create_project_with_nonce(payload, &uuid::Uuid::new_v4().simple().to_string())
    }

    pub(super) fn create_project_with_nonce(
        &self,
        payload: &Value,
        nonce: &str,
    ) -> Result<Value, String> {
        let mut project: VideoProject = serde_json::from_value(
            payload
                .get("project")
                .cloned()
                .ok_or_else(|| "project is required".to_string())?,
        )
        .map_err(|_| "project is invalid".to_string())?;
        project.id = format!("project-{nonce}");
        let path = self.projects()?.create_path_with_nonce(nonce)?;
        let saved =
            save_split_project(&path, &project).map_err(|_| "project could not be saved")?;
        Ok(
            json!({"catalogProjectId": self.projects()?.id_for_path(&path)?, "project": saved.project}),
        )
    }
}
