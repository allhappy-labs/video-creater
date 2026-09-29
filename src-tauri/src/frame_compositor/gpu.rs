use super::BlendMode;
use std::borrow::Cow;
use std::sync::mpsc;
use thiserror::Error;
use wgpu::util::DeviceExt;

const WORKGROUP_SIZE: u32 = 64;

#[derive(Debug, Error)]
pub enum GpuFrameCompositorError {
    #[error("GPU frame inputs must be equally sized RGBA8 buffers")]
    InvalidInput,
    #[error("GPU frame compositor does not implement blend mode {0:?}")]
    UnsupportedBlendMode(BlendMode),
    #[error("GPU frame compositor device is unavailable: {0}")]
    Device(String),
    #[error("GPU frame compositor shader failed validation: {0}")]
    Shader(String),
    #[error("GPU frame compositor readback failed: {0}")]
    Readback(String),
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    pixel_count: u32,
    mode: u32,
    padding: [u32; 2],
}

pub struct GpuFrameCompositor {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    adapter_name: String,
}

impl GpuFrameCompositor {
    pub fn new() -> Result<Self, GpuFrameCompositorError> {
        pollster::block_on(Self::new_async())
    }

    async fn new_async() -> Result<Self, GpuFrameCompositorError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .map_err(|error| GpuFrameCompositorError::Device(error.to_string()))?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("video-creater-frame-compositor-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await
            .map_err(|error| GpuFrameCompositorError::Device(error.to_string()))?;
        let adapter_name = adapter.get_info().name;
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("video-creater-frame-compositor-wgsl"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(COMPOSITOR_WGSL)),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("video-creater-frame-compositor-pipeline"),
            layout: None,
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        if let Some(error) = scope.pop().await {
            return Err(GpuFrameCompositorError::Shader(error.to_string()));
        }
        Ok(Self {
            device,
            queue,
            pipeline,
            adapter_name,
        })
    }

    pub fn adapter_name(&self) -> &str {
        &self.adapter_name
    }

    pub fn composite_rgba8_srgb(
        &self,
        backdrop: &[u8],
        source: &[u8],
        mode: BlendMode,
    ) -> Result<Vec<u8>, GpuFrameCompositorError> {
        if backdrop.len() != source.len()
            || !backdrop.len().is_multiple_of(4)
            || backdrop.is_empty()
        {
            return Err(GpuFrameCompositorError::InvalidInput);
        }
        let pixel_count =
            u32::try_from(backdrop.len() / 4).map_err(|_| GpuFrameCompositorError::InvalidInput)?;
        let backdrop_words = backdrop
            .iter()
            .map(|value| u32::from(*value))
            .collect::<Vec<_>>();
        let source_words = source
            .iter()
            .map(|value| u32::from(*value))
            .collect::<Vec<_>>();
        let backdrop_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("frame-compositor-backdrop"),
                contents: bytemuck::cast_slice(&backdrop_words),
                usage: wgpu::BufferUsages::STORAGE,
            });
        let source_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("frame-compositor-source"),
                contents: bytemuck::cast_slice(&source_words),
                usage: wgpu::BufferUsages::STORAGE,
            });
        let output_size = (backdrop_words.len() * std::mem::size_of::<u32>()) as u64;
        let output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame-compositor-output"),
            size: output_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame-compositor-readback"),
            size: output_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let params = Params {
            pixel_count,
            mode: mode_code(mode)?,
            padding: [0; 2],
        };
        let params_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("frame-compositor-params"),
                contents: bytemuck::bytes_of(&params),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let layout = self.pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frame-compositor-bind-group"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: backdrop_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: source_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: output_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame-compositor-encoder"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("frame-compositor-pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(pixel_count.div_ceil(WORKGROUP_SIZE), 1, 1);
        }
        encoder.copy_buffer_to_buffer(&output_buffer, 0, &readback_buffer, 0, output_size);
        self.queue.submit(Some(encoder.finish()));

        let slice = readback_buffer.slice(..);
        let (sender, receiver) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|error| GpuFrameCompositorError::Readback(error.to_string()))?;
        receiver
            .recv()
            .map_err(|error| GpuFrameCompositorError::Readback(error.to_string()))?
            .map_err(|error| GpuFrameCompositorError::Readback(error.to_string()))?;
        let mapped = slice.get_mapped_range();
        let words = bytemuck::cast_slice::<u8, u32>(&mapped);
        let rgba = words
            .iter()
            .map(|value| (*value).min(255) as u8)
            .collect::<Vec<_>>();
        drop(mapped);
        readback_buffer.unmap();
        Ok(rgba)
    }
}

fn mode_code(mode: BlendMode) -> Result<u32, GpuFrameCompositorError> {
    Ok(match mode {
        BlendMode::Source => 0,
        BlendMode::Over => 1,
        BlendMode::Add => 2,
        BlendMode::Multiply => 3,
        BlendMode::Screen => 4,
        BlendMode::Overlay => 5,
        _ => return Err(GpuFrameCompositorError::UnsupportedBlendMode(mode)),
    })
}

const COMPOSITOR_WGSL: &str = r#"
struct Params {
    pixel_count: u32,
    mode: u32,
    padding0: u32,
    padding1: u32,
};

@group(0) @binding(0) var<storage, read> backdrop: array<u32>;
@group(0) @binding(1) var<storage, read> source: array<u32>;
@group(0) @binding(2) var<storage, read_write> output: array<u32>;
@group(0) @binding(3) var<uniform> params: Params;

fn srgb_to_linear(value: f32) -> f32 {
    if value <= 0.04045 {
        return value / 12.92;
    }
    return pow((value + 0.055) / 1.055, 2.4);
}

fn linear_to_srgb(value: f32) -> f32 {
    if value <= 0.0031308 {
        return value * 12.92;
    }
    return 1.055 * pow(value, 1.0 / 2.4) - 0.055;
}

fn blend_channel(backdrop_value: f32, source_value: f32, mode: u32) -> f32 {
    if mode == 3u {
        return backdrop_value * source_value;
    }
    if mode == 4u {
        return backdrop_value + source_value - backdrop_value * source_value;
    }
    if backdrop_value <= 0.5 {
        return 2.0 * backdrop_value * source_value;
    }
    return 1.0 - 2.0 * (1.0 - backdrop_value) * (1.0 - source_value);
}

fn encode_channel(value: f32, alpha: f32) -> u32 {
    if alpha <= 0.0000001192092896 {
        return 0u;
    }
    let straight = clamp(value / alpha, 0.0, 1.0);
    return u32(round(linear_to_srgb(straight) * 255.0));
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let pixel = id.x;
    if pixel >= params.pixel_count {
        return;
    }
    let base = pixel * 4u;
    let ba = f32(backdrop[base + 3u]) / 255.0;
    let sa = f32(source[base + 3u]) / 255.0;
    let b = vec3<f32>(
        srgb_to_linear(f32(backdrop[base]) / 255.0),
        srgb_to_linear(f32(backdrop[base + 1u]) / 255.0),
        srgb_to_linear(f32(backdrop[base + 2u]) / 255.0)
    );
    let s = vec3<f32>(
        srgb_to_linear(f32(source[base]) / 255.0),
        srgb_to_linear(f32(source[base + 1u]) / 255.0),
        srgb_to_linear(f32(source[base + 2u]) / 255.0)
    );
    var out_rgb: vec3<f32>;
    var out_alpha: f32;
    if params.mode == 0u {
        out_rgb = s * sa;
        out_alpha = sa;
    } else if params.mode == 2u {
        out_rgb = min(b * ba + s * sa, vec3<f32>(1.0));
        out_alpha = min(ba + sa, 1.0);
    } else {
        out_alpha = sa + ba * (1.0 - sa);
        if params.mode == 1u {
            out_rgb = s * sa + b * ba * (1.0 - sa);
        } else {
            let blended = vec3<f32>(
                blend_channel(b.r, s.r, params.mode),
                blend_channel(b.g, s.g, params.mode),
                blend_channel(b.b, s.b, params.mode)
            );
            out_rgb = (1.0 - sa) * b * ba + (1.0 - ba) * s * sa + sa * ba * blended;
        }
    }
    output[base] = encode_channel(out_rgb.r, out_alpha);
    output[base + 1u] = encode_channel(out_rgb.g, out_alpha);
    output[base + 2u] = encode_channel(out_rgb.b, out_alpha);
    output[base + 3u] = u32(round(clamp(out_alpha, 0.0, 1.0) * 255.0));
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame_compositor::composite_rgba8_srgb;

    #[test]
    fn richer_cpu_only_modes_are_rejected_before_shader_dispatch() {
        assert!(matches!(
            mode_code(BlendMode::ColorBurn),
            Err(GpuFrameCompositorError::UnsupportedBlendMode(
                BlendMode::ColorBurn
            ))
        ));
    }

    #[test]
    fn gpu_matches_canonical_cpu_blends_with_documented_tolerance() {
        let gpu = GpuFrameCompositor::new().expect("GPU compositor");
        let pixels = [
            ([20, 80, 160, 200], [255, 0, 0, 0]),
            ([128, 128, 128, 255], [128, 64, 192, 255]),
            ([100, 80, 60, 255], [180, 100, 220, 128]),
        ];
        let backdrop = pixels
            .iter()
            .flat_map(|(backdrop, _)| backdrop)
            .copied()
            .collect::<Vec<_>>();
        let source = pixels
            .iter()
            .flat_map(|(_, source)| source)
            .copied()
            .collect::<Vec<_>>();
        for mode in [
            BlendMode::Source,
            BlendMode::Over,
            BlendMode::Add,
            BlendMode::Multiply,
            BlendMode::Screen,
            BlendMode::Overlay,
        ] {
            let actual = gpu
                .composite_rgba8_srgb(&backdrop, &source, mode)
                .expect("GPU composite");
            let expected = pixels
                .iter()
                .flat_map(|(backdrop, source)| composite_rgba8_srgb(*backdrop, *source, mode))
                .collect::<Vec<_>>();
            for (channel, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
                assert!(
                    (i16::from(*actual) - i16::from(*expected)).abs() <= 1,
                    "mode={mode:?} channel={channel} actual={actual} expected={expected}"
                );
            }
        }
    }
}
