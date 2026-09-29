use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::error::ServiceError;
use super::events::{EventSink, ServiceEvent};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobProgress {
    pub project_id: String,
    pub job_id: String,
    pub fraction: f64,
    pub state: String,
}

#[derive(Clone)]
pub struct JobService {
    events: Arc<dyn EventSink>,
}

impl JobService {
    pub fn new(events: Arc<dyn EventSink>) -> Self {
        Self { events }
    }

    pub fn publish_progress(&self, progress: JobProgress) -> Result<(), ServiceError> {
        if !progress.fraction.is_finite() || !(0.0..=1.0).contains(&progress.fraction) {
            return Err(ServiceError::invalid_input(
                "job progress must be between zero and one",
            ));
        }
        self.events.publish(ServiceEvent::new(
            "job.progress",
            Some(&progress.project_id),
            json!({
                "jobId": progress.job_id,
                "fraction": progress.fraction,
                "state": progress.state,
            }),
        ))?;
        Ok(())
    }
}
