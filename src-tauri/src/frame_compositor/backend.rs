use super::{composite_rgba8_srgb, BlendMode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlendBackendReport {
    pub backend: String,
    pub gpu_adapter: Option<String>,
    pub gpu_verified_frames: u32,
    pub cpu_fallback_frames: u32,
    pub fallback_reason: Option<String>,
}

pub struct FrameBlendCompositor {
    #[cfg(feature = "gpu-render")]
    gpu: Option<super::GpuFrameCompositor>,
    report: BlendBackendReport,
}

impl FrameBlendCompositor {
    pub fn new(prefer_gpu: bool) -> Self {
        #[cfg(feature = "gpu-render")]
        if prefer_gpu {
            match super::GpuFrameCompositor::new() {
                Ok(gpu) => {
                    let adapter = gpu.adapter_name().to_string();
                    return Self {
                        gpu: Some(gpu),
                        report: BlendBackendReport {
                            backend: "wgpu-verified-v1".to_string(),
                            gpu_adapter: Some(adapter),
                            gpu_verified_frames: 0,
                            cpu_fallback_frames: 0,
                            fallback_reason: None,
                        },
                    };
                }
                Err(error) => {
                    return Self {
                        gpu: None,
                        report: BlendBackendReport {
                            backend: "cpu-v1".to_string(),
                            gpu_adapter: None,
                            gpu_verified_frames: 0,
                            cpu_fallback_frames: 0,
                            fallback_reason: Some(format!("GPU initialization failed: {error}")),
                        },
                    };
                }
            }
        }
        #[cfg(not(feature = "gpu-render"))]
        let _ = prefer_gpu;
        Self {
            #[cfg(feature = "gpu-render")]
            gpu: None,
            report: BlendBackendReport {
                backend: "cpu-v1".to_string(),
                gpu_adapter: None,
                gpu_verified_frames: 0,
                cpu_fallback_frames: 0,
                fallback_reason: None,
            },
        }
    }

    pub fn fingerprint_identity(&self) -> (&str, Option<&str>) {
        (&self.report.backend, self.report.gpu_adapter.as_deref())
    }

    pub fn report(&self) -> &BlendBackendReport {
        &self.report
    }

    pub fn composite_frame(
        &mut self,
        backdrop: &[u8],
        source: &[u8],
        mode: BlendMode,
    ) -> Result<Vec<u8>, &'static str> {
        if backdrop.len() != source.len() || !backdrop.len().is_multiple_of(4) {
            return Err("blend frame inputs must be equally sized RGBA8 buffers");
        }
        let cpu = backdrop
            .chunks_exact(4)
            .zip(source.chunks_exact(4))
            .flat_map(|(backdrop, source)| {
                composite_rgba8_srgb(
                    [backdrop[0], backdrop[1], backdrop[2], backdrop[3]],
                    [source[0], source[1], source[2], source[3]],
                    mode,
                )
            })
            .collect::<Vec<_>>();

        #[cfg(feature = "gpu-render")]
        if let Some(gpu) = &self.gpu {
            match gpu.composite_rgba8_srgb(backdrop, source, mode) {
                Ok(candidate) if gpu_matches_cpu(&candidate, &cpu) => {
                    self.report.gpu_verified_frames += 1;
                    return Ok(candidate);
                }
                Ok(_) => self.disable_gpu("GPU output exceeded CPU comparison tolerance"),
                Err(super::GpuFrameCompositorError::UnsupportedBlendMode(_)) => {}
                Err(error) => self.disable_gpu(&format!("GPU execution failed: {error}")),
            }
        }
        self.report.cpu_fallback_frames += 1;
        Ok(cpu)
    }

    #[cfg(feature = "gpu-render")]
    fn disable_gpu(&mut self, reason: &str) {
        self.gpu = None;
        self.report.backend = "cpu-fallback-v1".to_string();
        self.report.fallback_reason = Some(reason.to_string());
    }
}

#[cfg(feature = "gpu-render")]
fn gpu_matches_cpu(gpu: &[u8], cpu: &[u8]) -> bool {
    gpu.len() == cpu.len()
        && gpu.iter().zip(cpu).enumerate().all(|(index, (gpu, cpu))| {
            if index % 4 == 3 {
                gpu == cpu
            } else {
                (i16::from(*gpu) - i16::from(*cpu)).abs() <= 1
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_backend_composites_whole_frames_and_reports_usage() {
        let mut compositor = FrameBlendCompositor::new(false);
        let result = compositor
            .composite_frame(
                &[100, 80, 60, 255, 10, 20, 30, 255],
                &[180, 100, 220, 128, 255, 0, 0, 0],
                BlendMode::Multiply,
            )
            .expect("CPU frame blend");
        assert_eq!(result.len(), 8);
        assert_eq!(&result[4..], &[10, 20, 30, 255]);
        assert_eq!(compositor.report().backend, "cpu-v1");
        assert_eq!(compositor.report().cpu_fallback_frames, 1);
    }

    #[cfg(feature = "gpu-render")]
    #[test]
    fn preferred_gpu_is_verified_against_cpu_or_reports_fallback() {
        let mut compositor = FrameBlendCompositor::new(true);
        compositor
            .composite_frame(
                &[100, 80, 60, 255],
                &[180, 100, 220, 128],
                BlendMode::Overlay,
            )
            .expect("verified frame blend");
        let report = compositor.report();
        assert!(
            report.gpu_verified_frames == 1
                || (report.cpu_fallback_frames == 1 && report.fallback_reason.is_some())
        );
    }
}
