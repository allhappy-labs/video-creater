//! Offscreen GPU graphics renderer.

use super::error::{GpuGraphicsError, GpuGraphicsErrorCode, GpuGraphicsResult};
use super::ir::{GpuGraphicRole, GpuGraphicsLayer};
use super::shader::{fullscreen_vertex_glsl, wrap_glsl_fragment_source};
use super::validation::validate_gpu_graphics_layer;
use crate::graphics::manifest::{graphics_playback_manifest, GraphicsArtifactManifest};
use image::{ImageBuffer, Rgba};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use wgpu::util::DeviceExt;

const PREVIEW_FILE_NAME: &str = "preview.png";
const MANIFEST_FILE_NAME: &str = "manifest.json";
const FRAMES_DIR_NAME: &str = "frames";
const FRAMES_PATTERN: &str = "frames/frame-%06d.png";
const TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuRenderOptions {
    pub output_dir: PathBuf,
}

pub fn render_gpu_graphics_layer(
    layer: &GpuGraphicsLayer,
    options: GpuRenderOptions,
) -> GpuGraphicsResult<GraphicsArtifactManifest> {
    render_gpu_graphics_layer_cancellable(layer, options, || false)
}

pub fn render_gpu_graphics_layer_cancellable(
    layer: &GpuGraphicsLayer,
    options: GpuRenderOptions,
    is_cancelled: impl Fn() -> bool,
) -> GpuGraphicsResult<GraphicsArtifactManifest> {
    if is_cancelled() {
        return Err(cancelled_gpu_errors(&options.output_dir));
    }
    validate_gpu_graphics_layer(layer)?;
    std::fs::create_dir_all(&options.output_dir).map_err(|error| {
        vec![artifact_error(
            "outputDir",
            "Could not create GPU graphics output directory.",
            "Choose a writable outputDir for GPU graphics artifacts.",
            error,
        )]
    })?;

    let frames_dir = options.output_dir.join(FRAMES_DIR_NAME);
    if frames_dir.exists() {
        std::fs::remove_dir_all(&frames_dir).map_err(|error| {
            vec![artifact_error(
                "frames",
                "Could not clear stale GPU graphics frames.",
                "Remove stale graphics artifacts or choose a fresh outputDir.",
                error,
            )]
        })?;
    }
    std::fs::create_dir_all(&frames_dir).map_err(|error| {
        vec![artifact_error(
            "frames",
            "Could not create GPU graphics frames directory.",
            "Choose a writable outputDir for GPU graphics artifacts.",
            error,
        )]
    })?;

    let frame_count = frame_count(layer);
    let renderer = BlockingGpuRenderer::new(layer)?;
    for frame_index in 0..frame_count {
        if is_cancelled() {
            return Err(cancelled_gpu_errors(&options.output_dir));
        }
        let rgba = renderer.render_frame(layer, frame_index, frame_count)?;
        let frame_path = frames_dir.join(frame_file_name(frame_index));
        write_rgba_png(
            layer.dimensions.width,
            layer.dimensions.height,
            rgba,
            &frame_path,
        )?;
        if frame_index == 0 {
            let preview_path = options.output_dir.join(PREVIEW_FILE_NAME);
            std::fs::copy(&frame_path, &preview_path).map_err(|error| {
                vec![artifact_error(
                    "preview",
                    "Could not write GPU preview frame.",
                    "Choose a writable outputDir for GPU graphics artifacts.",
                    error,
                )]
            })?;
        }
    }

    if is_cancelled() {
        return Err(cancelled_gpu_errors(&options.output_dir));
    }
    let manifest = GraphicsArtifactManifest {
        schema_version: 1,
        artifact_id: format!("{}.preview", layer.id),
        kind: "rgbaFrameSequence".to_string(),
        dimensions: layer.dimensions.clone(),
        fps: layer.fps,
        duration_seconds: layer.duration_seconds,
        alpha: layer.alpha,
        frame_count,
        frames_pattern: FRAMES_PATTERN.to_string(),
        playback: graphics_playback_manifest(
            frame_count,
            layer.fps,
            layer.duration_seconds,
            true,
            FRAMES_PATTERN,
        ),
        preview_path: PathBuf::from(PREVIEW_FILE_NAME),
        source_layer_ids: vec![layer.id.clone()],
        checksums: BTreeMap::new(),
    };
    let manifest_json = serde_json::to_string_pretty(&manifest).map_err(|error| {
        vec![GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsArtifactWriteFailed,
            "manifest",
            "Could not serialize GPU graphics manifest.",
            "Inspect manifest fields for unsupported values.",
        )
        .with_detail("serdeError", error.to_string())]
    })?;
    std::fs::write(options.output_dir.join(MANIFEST_FILE_NAME), manifest_json).map_err(
        |error| {
            vec![artifact_error(
                "manifest",
                "Could not write GPU graphics manifest.",
                "Choose a writable outputDir for GPU graphics artifacts.",
                error,
            )]
        },
    )?;

    Ok(manifest)
}

fn cancelled_gpu_errors(output_dir: &Path) -> Vec<GpuGraphicsError> {
    let mut errors = vec![GpuGraphicsError::new(
        GpuGraphicsErrorCode::GpuGraphicsRenderFailed,
        "cancelled",
        "GPU graphics rendering was cancelled.",
        "Start a new render attempt to retry.",
    )];
    if let Err(error) = std::fs::remove_dir_all(output_dir) {
        if error.kind() != std::io::ErrorKind::NotFound {
            errors.push(artifact_error(
                "cancelled.cleanup",
                "Partial GPU graphics could not be removed.",
                "Remove the partial graphics directory before retrying.",
                error,
            ));
        }
    }
    errors
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct FrameUniforms {
    time: f32,
    duration: f32,
    resolution: [f32; 2],
    frame: f32,
    progress: f32,
    padding: [f32; 2],
}

struct BlockingGpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    texture: wgpu::Texture,
    texture_view: wgpu::TextureView,
    readback_buffer: wgpu::Buffer,
    width: u32,
    height: u32,
    unpadded_bytes_per_row: u32,
    padded_bytes_per_row: u32,
}

impl BlockingGpuRenderer {
    fn new(layer: &GpuGraphicsLayer) -> GpuGraphicsResult<Self> {
        let background = match layer.role {
            GpuGraphicRole::ShaderBackground | GpuGraphicRole::HybridScene => {
                layer.background.as_ref()
            }
            GpuGraphicRole::PrimitiveScene => None,
        };
        let fragment_source = background.map(|shader| shader.fragment_source.as_str());
        let wrapped_glsl = match fragment_source {
            Some(source) => wrap_glsl_fragment_source(source)?,
            None => default_primitive_fragment_source(),
        };

        pollster::block_on(Self::new_async(layer, wrapped_glsl))
    }

    async fn new_async(layer: &GpuGraphicsLayer, wrapped_glsl: String) -> GpuGraphicsResult<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .map_err(|error| {
                vec![GpuGraphicsError::new(
                    GpuGraphicsErrorCode::GpuGraphicsDeviceUnavailable,
                    "adapter",
                    "No compatible GPU adapter is available for offscreen graphics rendering.",
                    "Run on a system with a compatible Metal/Vulkan/DX12/WebGPU adapter or skip GPU visuals for this render.",
                )
                .with_detail("adapterError", error.to_string())]
            })?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("video-creater-gpu-graphics-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await
            .map_err(|error| {
                vec![GpuGraphicsError::new(
                    GpuGraphicsErrorCode::GpuGraphicsDeviceUnavailable,
                    "device",
                    "Could not create a GPU device for offscreen graphics rendering.",
                    "Run on a system with a compatible GPU device or skip GPU visuals for this render.",
                )
                .with_detail("deviceError", error.to_string())]
            })?;

        let vertex_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("video-creater-fullscreen-vertex"),
            source: wgpu::ShaderSource::Glsl {
                shader: Cow::Borrowed(fullscreen_vertex_glsl()),
                stage: wgpu::naga::ShaderStage::Vertex,
                defines: &[],
            },
        });
        let fragment_shader = create_fragment_shader(&device, wrapped_glsl).await?;

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("video-creater-frame-uniforms"),
            contents: bytemuck::bytes_of(&FrameUniforms::new(layer, 0, frame_count(layer))),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("video-creater-gpu-graphics-bind-group-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("video-creater-gpu-graphics-bind-group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("video-creater-gpu-graphics-pipeline-layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline =
            create_render_pipeline(&device, &pipeline_layout, &vertex_shader, &fragment_shader)
                .await?;

        let width = layer.dimensions.width;
        let height = layer.dimensions.height;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("video-creater-gpu-graphics-output"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TEXTURE_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let unpadded_bytes_per_row = width * 4;
        let padded_bytes_per_row =
            align_to(unpadded_bytes_per_row, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("video-creater-gpu-graphics-readback"),
            size: padded_bytes_per_row as u64 * height as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        Ok(Self {
            device,
            queue,
            pipeline,
            uniform_buffer,
            bind_group,
            texture,
            texture_view,
            readback_buffer,
            width,
            height,
            unpadded_bytes_per_row,
            padded_bytes_per_row,
        })
    }

    fn render_frame(
        &self,
        layer: &GpuGraphicsLayer,
        frame_index: u32,
        total_frames: u32,
    ) -> GpuGraphicsResult<Vec<u8>> {
        let uniforms = FrameUniforms::new(layer, frame_index, total_frames);
        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("video-creater-gpu-graphics-frame-encoder"),
            });
        {
            let color_attachment = Some(wgpu::RenderPassColorAttachment {
                view: &self.texture_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            });
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("video-creater-gpu-graphics-render-pass"),
                color_attachments: &[color_attachment],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &self.readback_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.padded_bytes_per_row),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(Some(encoder.finish()));
        let mut rgba = self.readback_rgba()?;
        if let Some(scene) = &layer.scene {
            super::primitive_renderer::draw_projected_wireframes_into_rgba(
                &mut rgba,
                self.width,
                self.height,
                scene,
                uniforms.progress,
            )?;
        }
        Ok(rgba)
    }

    fn readback_rgba(&self) -> GpuGraphicsResult<Vec<u8>> {
        let buffer_slice = self.readback_buffer.slice(..);
        let (sender, receiver) = mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|error| {
                vec![GpuGraphicsError::new(
                    GpuGraphicsErrorCode::GpuGraphicsReadbackFailed,
                    "device.poll",
                    "GPU readback polling failed.",
                    "Retry GPU rendering or skip GPU visuals for this render.",
                )
                .with_detail("pollError", error.to_string())]
            })?;
        let map_result = receiver.recv().map_err(|error| {
            vec![GpuGraphicsError::new(
                GpuGraphicsErrorCode::GpuGraphicsReadbackFailed,
                "readback",
                "GPU readback callback did not complete.",
                "Retry GPU rendering or skip GPU visuals for this render.",
            )
            .with_detail("channelError", error.to_string())]
        })?;
        map_result.map_err(|error| {
            vec![GpuGraphicsError::new(
                GpuGraphicsErrorCode::GpuGraphicsReadbackFailed,
                "readback",
                "GPU readback mapping failed.",
                "Retry GPU rendering or skip GPU visuals for this render.",
            )
            .with_detail("mapError", error.to_string())]
        })?;

        let padded = buffer_slice.get_mapped_range();
        let mut rgba = Vec::with_capacity((self.width * self.height * 4) as usize);
        for row in padded.chunks(self.padded_bytes_per_row as usize) {
            rgba.extend_from_slice(&row[..self.unpadded_bytes_per_row as usize]);
        }
        drop(padded);
        self.readback_buffer.unmap();
        Ok(rgba)
    }
}

impl FrameUniforms {
    fn new(layer: &GpuGraphicsLayer, frame_index: u32, total_frames: u32) -> Self {
        let time = frame_time_seconds(frame_index, layer.fps, layer.duration_seconds);
        let progress = if total_frames <= 1 {
            0.0
        } else {
            frame_index as f32 / (total_frames - 1) as f32
        };
        Self {
            time,
            duration: layer.duration_seconds as f32,
            resolution: [
                layer.dimensions.width as f32,
                layer.dimensions.height as f32,
            ],
            frame: frame_index as f32,
            progress,
            padding: [0.0, 0.0],
        }
    }
}

async fn create_fragment_shader(
    device: &wgpu::Device,
    wrapped_glsl: String,
) -> GpuGraphicsResult<wgpu::ShaderModule> {
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("video-creater-gpu-graphics-fragment"),
        source: wgpu::ShaderSource::Glsl {
            shader: Cow::Owned(wrapped_glsl),
            stage: wgpu::naga::ShaderStage::Fragment,
            defines: &[],
        },
    });
    shader_error(scope).await?;
    Ok(shader)
}

async fn create_render_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    vertex_shader: &wgpu::ShaderModule,
    fragment_shader: &wgpu::ShaderModule,
) -> GpuGraphicsResult<wgpu::RenderPipeline> {
    let targets = [Some(wgpu::ColorTargetState {
        format: TEXTURE_FORMAT,
        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
        write_mask: wgpu::ColorWrites::ALL,
    })];
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("video-creater-gpu-graphics-pipeline"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: vertex_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: fragment_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &targets,
        }),
        multiview_mask: None,
        cache: None,
    });
    shader_error(scope).await?;
    Ok(pipeline)
}

async fn shader_error(scope: wgpu::ErrorScopeGuard) -> GpuGraphicsResult<()> {
    match scope.pop().await {
        Some(error) => Err(vec![GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsShaderCompileFailed,
            "fragmentSource",
            "GPU shader or render pipeline compilation failed.",
            "Revise the GLSL fragment function so it compiles under the Video Creater shader contract.",
        )
        .with_detail("wgpuError", error.to_string())]),
        None => Ok(()),
    }
}

fn write_rgba_png(width: u32, height: u32, rgba: Vec<u8>, path: &Path) -> GpuGraphicsResult<()> {
    let image = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, rgba).ok_or_else(|| {
        vec![GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsReadbackFailed,
            "frame",
            "GPU readback buffer size did not match output dimensions.",
            "Ensure the renderer returns width * height * 4 bytes.",
        )]
    })?;
    image.save(path).map_err(|error| {
        vec![artifact_error(
            "frame",
            "Could not write GPU frame PNG.",
            "Choose a writable outputDir and retry GPU rendering.",
            error,
        )]
    })
}

fn artifact_error(
    path: impl Into<String>,
    message: impl Into<String>,
    fix: impl Into<String>,
    error: impl std::fmt::Display,
) -> GpuGraphicsError {
    GpuGraphicsError::new(
        GpuGraphicsErrorCode::GpuGraphicsArtifactWriteFailed,
        path,
        message,
        fix,
    )
    .with_detail("ioError", error.to_string())
}

fn default_primitive_fragment_source() -> String {
    wrap_glsl_fragment_source(
        "vec4 video_creater_fragment(vec2 uv, float time, float progress) { return vec4(uv.x, 0.35 + 0.35 * progress, uv.y, 1.0); }",
    )
    .expect("default primitive shader should satisfy the fixed contract")
}

fn frame_count(layer: &GpuGraphicsLayer) -> u32 {
    (layer.duration_seconds * layer.fps).ceil().max(1.0) as u32
}

fn frame_time_seconds(frame_index: u32, fps: f64, duration_seconds: f64) -> f32 {
    ((frame_index as f64 / fps).min(duration_seconds)) as f32
}

fn frame_file_name(frame_index: u32) -> String {
    format!("frame-{frame_index:06}.png")
}

fn align_to(value: u32, alignment: u32) -> u32 {
    value.div_ceil(alignment) * alignment
}
