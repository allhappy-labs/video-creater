use std::collections::BTreeMap;
use std::time::Instant;

use super::error::PipelineResult;
use super::report::{RenderPerformanceSummary, RenderStageReport};

#[derive(Debug)]
pub struct RenderPerformanceRecorder {
    started_at: Instant,
    stages: Vec<RenderStageReport>,
}

impl RenderPerformanceRecorder {
    pub fn start() -> Self {
        Self {
            started_at: Instant::now(),
            stages: Vec::new(),
        }
    }

    pub fn measure_stage<T, F>(
        &mut self,
        name: impl Into<String>,
        details: BTreeMap<String, String>,
        stage: F,
    ) -> PipelineResult<T>
    where
        F: FnOnce() -> PipelineResult<T>,
    {
        let name = name.into();
        let started_at = Instant::now();
        match stage() {
            Ok(value) => {
                self.stages.push(RenderStageReport {
                    name,
                    status: "succeeded".to_string(),
                    duration_ms: started_at.elapsed().as_millis(),
                    details,
                });
                Ok(value)
            }
            Err(errors) => {
                self.stages.push(RenderStageReport {
                    name,
                    status: "failed".to_string(),
                    duration_ms: started_at.elapsed().as_millis(),
                    details,
                });
                Err(errors)
            }
        }
    }

    pub fn push_stage(&mut self, stage: RenderStageReport) {
        self.stages.push(stage);
    }

    pub fn finish(self) -> RenderPerformanceSummary {
        RenderPerformanceSummary {
            total_duration_ms: self.started_at.elapsed().as_millis(),
            stages: self.stages,
        }
    }
}
