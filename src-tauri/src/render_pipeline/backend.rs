use crate::edit::render_plan::RenderPlan;
use crate::graphics::manifest::GraphicsArtifactManifest;
use std::path::PathBuf;
use std::time::Duration;

use super::error::PipelineResult;
use super::process::{CommandSpec, ProcessOutput, ProcessRunner};

pub trait RenderBackend {
    fn build_command(
        &self,
        plan: &RenderPlan,
        graphics: &[(GraphicsArtifactManifest, PathBuf, f64)],
    ) -> PipelineResult<CommandSpec>;

    fn render(
        &self,
        runner: &dyn ProcessRunner,
        plan: &RenderPlan,
        graphics: &[(GraphicsArtifactManifest, PathBuf, f64)],
        timeout: Duration,
    ) -> PipelineResult<ProcessOutput> {
        let spec = self.build_command(plan, graphics)?;
        runner.run(&spec, timeout)
    }
}
