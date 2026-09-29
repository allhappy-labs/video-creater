use dotlottie_rs::slots::{extract_slots_from_animation, parse_slot_from_json};
use dotlottie_rs::{ColorSpace, Fit, Layout, Player, Segment, SlotType, TvgRenderer};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::ffi::CString;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use video_creater_precompose_protocol::{
    LottieRuntimeInputs, LottieSlotValue, LottieSourceFormat, LottieStateInputValue,
    PrecomposeRequest, PrecomposeResult, WorkerError, WorkerErrorCode, WorkerPhase,
};
use zip::ZipArchive;

const MANIFEST_SCHEMA_VERSION: u32 = 1;
const PIXEL_CONTRACT: &str = "rgba8-srgb-straight-v1";
const RNG_POLICY: &str = "expression-code-frame-xorshift64star-v1";

#[derive(Debug)]
pub(crate) struct BakeFailure {
    pub code: WorkerErrorCode,
    pub message: String,
    pub field: Option<String>,
}

impl BakeFailure {
    fn new(code: WorkerErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            field: None,
        }
    }

    fn field(mut self, field: impl Into<String>) -> Self {
        self.field = Some(field.into());
        self
    }

    pub(crate) fn into_worker_error(self) -> WorkerError {
        WorkerError {
            code: self.code,
            message: self.message,
            field: self.field,
            retryable: false,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BakeManifest<'a> {
    schema_version: u32,
    request_id: &'a str,
    cache_key: &'a str,
    source_sha256: &'a str,
    animation_id: Option<&'a str>,
    inputs: &'a LottieRuntimeInputs,
    width: u32,
    height: u32,
    fps_numerator: u32,
    fps_denominator: u32,
    first_frame: u64,
    frame_count: u32,
    source_start_micros: u64,
    playback_rate_micros: u32,
    looping: bool,
    expressions_enabled: bool,
    pixel_contract: &'static str,
    rng_policy: &'static str,
    renderer: RendererManifest,
    frames: Vec<FrameManifest>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RendererManifest {
    dotlottie_rs_revision: &'static str,
    thorvg_revision: &'static str,
    quality: u8,
    fit: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FrameManifest {
    index: u32,
    source_frame: f32,
    path: String,
    rgba_sha256: String,
    png_sha256: String,
    png_bytes: u64,
}

pub(crate) fn bake_lottie(
    request: &PrecomposeRequest,
    mut progress: impl FnMut(WorkerPhase, u32, u32, &str) -> Result<(), BakeFailure>,
) -> Result<PrecomposeResult, BakeFailure> {
    let started = Instant::now();
    let wall_budget = Duration::from_millis(request.budgets.max_wall_time_ms);
    let source_path = Path::new(&request.source.path);
    let metadata = fs::symlink_metadata(source_path).map_err(|error| {
        BakeFailure::new(
            WorkerErrorCode::SourceInvalid,
            format!("unable to inspect Lottie source: {error}"),
        )
        .field("source.path")
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(BakeFailure::new(
            WorkerErrorCode::SourceInvalid,
            "Lottie source must be a regular non-symlink file",
        )
        .field("source.path"));
    }
    if metadata.len() > request.budgets.max_source_bytes {
        return Err(BakeFailure::new(
            WorkerErrorCode::BudgetExceeded,
            "Lottie source exceeds maxSourceBytes",
        )
        .field("budgets.maxSourceBytes"));
    }
    let source = fs::read(source_path).map_err(|error| {
        BakeFailure::new(
            WorkerErrorCode::Io,
            format!("unable to read Lottie source: {error}"),
        )
    })?;
    if sha256_hex(&source) != request.source.sha256 {
        return Err(BakeFailure::new(
            WorkerErrorCode::ChecksumMismatch,
            "Lottie source SHA-256 does not match request",
        )
        .field("source.sha256"));
    }

    match request.source.format {
        LottieSourceFormat::LottieJson => validate_json_source(&source)?,
        LottieSourceFormat::DotLottie => validate_dotlottie_archive(request, &source)?,
    }
    check_wall_budget(started, wall_budget)?;

    let staging_dir = PathBuf::from(&request.output.staging_dir);
    if staging_dir.exists() {
        return Err(BakeFailure::new(
            WorkerErrorCode::Io,
            "staging directory must not already exist",
        )
        .field("output.stagingDir"));
    }
    let frames_dir = staging_dir.join("frames");
    fs::create_dir_all(&frames_dir).map_err(|error| {
        BakeFailure::new(
            WorkerErrorCode::Io,
            format!("unable to create staging directory: {error}"),
        )
    })?;
    progress(
        WorkerPhase::Loading,
        0,
        request.render.frame_count,
        "loading expression-enabled Lottie runtime",
    )?;

    let pixel_count = u64::from(request.render.width) * u64::from(request.render.height);
    let rgba_bytes = pixel_count.checked_mul(4).ok_or_else(|| {
        BakeFailure::new(WorkerErrorCode::BudgetExceeded, "RGBA buffer size overflow")
    })?;
    if rgba_bytes > request.budgets.max_memory_bytes {
        return Err(BakeFailure::new(
            WorkerErrorCode::BudgetExceeded,
            "one RGBA frame exceeds maxMemoryBytes",
        )
        .field("budgets.maxMemoryBytes"));
    }
    let mut pixels = vec![
        0_u32;
        usize::try_from(pixel_count).map_err(|_| {
            BakeFailure::new(
                WorkerErrorCode::BudgetExceeded,
                "pixel count is not addressable",
            )
        })?
    ];
    let mut player = Player::new();
    player.set_autoplay(false);
    player.set_loop(false);
    player
        .set_sw_target(
            &mut pixels,
            request.render.width,
            request.render.height,
            ColorSpace::ABGR8888S,
        )
        .map_err(|error| renderer_failure("unable to configure software target", error))?;
    player
        .set_layout(Layout::new(Fit::Contain, [0.5, 0.5]))
        .map_err(|error| renderer_failure("unable to configure Lottie layout", error))?;
    TvgRenderer::reset_expression_errors();
    match request.source.format {
        LottieSourceFormat::LottieJson => {
            let text = CString::new(source.as_slice()).map_err(|_| {
                BakeFailure::new(
                    WorkerErrorCode::SourceInvalid,
                    "Lottie JSON contains an interior NUL byte",
                )
            })?;
            let load_result = player.load_animation_data(&text);
            fail_on_expression_errors("loading animation")?;
            load_result.map_err(|error| renderer_failure("unable to load Lottie JSON", error))?;
        }
        LottieSourceFormat::DotLottie => {
            let load_result = player.load_dotlottie_data(&source);
            fail_on_expression_errors("loading animation")?;
            load_result
                .map_err(|error| renderer_failure("unable to load dotLottie archive", error))?;
            if let Some(animation_id) = request.source.animation_id.as_deref() {
                let animation_id = CString::new(animation_id).map_err(|_| {
                    BakeFailure::new(
                        WorkerErrorCode::SourceInvalid,
                        "animationId contains an interior NUL byte",
                    )
                })?;
                TvgRenderer::reset_expression_errors();
                let selection_result = player.load_animation(&animation_id);
                fail_on_expression_errors("selecting animation")?;
                selection_result.map_err(|error| {
                    renderer_failure("unable to select dotLottie animation", error)
                })?;
            }
        }
    }
    if !player.is_loaded() {
        return Err(BakeFailure::new(
            WorkerErrorCode::SourceInvalid,
            "dotLottie runtime did not report a loaded animation",
        ));
    }
    let mut active_animation_id = request.source.animation_id.clone();
    let archive_controls = match request.source.format {
        LottieSourceFormat::DotLottie => Some(read_archive_controls(request, &source)?),
        LottieSourceFormat::LottieJson => None,
    };
    if let Some(machine) = request.inputs.state_machine.as_ref() {
        let controls = archive_controls.as_ref().ok_or_else(|| {
            BakeFailure::new(
                WorkerErrorCode::SourceInvalid,
                "state machines require a dotLottie source",
            )
            .field("inputs.stateMachine")
        })?;
        let state = resolve_state_machine(controls, machine)?;
        if let Some(animation) = state.animation {
            let id = CString::new(animation.as_str()).map_err(|_| {
                BakeFailure::new(
                    WorkerErrorCode::SourceInvalid,
                    "state animation contains an interior NUL byte",
                )
            })?;
            player
                .load_animation(&id)
                .map_err(|error| renderer_failure("unable to select state animation", error))?;
            active_animation_id = Some(animation);
        }
        if request.inputs.marker.is_none() && request.inputs.segment.is_none() {
            if let Some(marker) = state.marker {
                select_marker(&mut player, &marker, "inputs.stateMachine")?;
            }
        }
    }
    apply_runtime_inputs(
        &mut player,
        &request.inputs,
        archive_controls.as_ref(),
        active_animation_id.as_deref(),
        request.source.format,
        &source,
    )?;
    player
        .set_quality(100)
        .map_err(|error| renderer_failure("unable to configure renderer quality", error))?;
    let total_frames = player.total_frames();
    // dotlottie-rs exposes ThorVG duration in milliseconds.
    let duration_millis = player.duration();
    if !total_frames.is_finite()
        || !duration_millis.is_finite()
        || total_frames <= 0.0
        || duration_millis <= 0.0
    {
        return Err(BakeFailure::new(
            WorkerErrorCode::SourceInvalid,
            "animation must have positive finite frames and duration",
        ));
    }
    let selected_segment = player.segment().ok();
    let segment_start = selected_segment.map_or(0.0, |segment| segment.start);
    let segment_frames = selected_segment
        .map(|segment| segment.end - segment.start)
        .filter(|frames| frames.is_finite() && *frames > 0.0)
        .unwrap_or(total_frames);
    let segment_duration_millis = duration_millis * segment_frames / total_frames;

    let output_fps =
        f64::from(request.render.fps.numerator) / f64::from(request.render.fps.denominator);
    let mut frame_manifests = Vec::with_capacity(request.render.frame_count as usize);
    let mut output_bytes = 0_u64;
    for output_index in 0..request.render.frame_count {
        check_wall_budget(started, wall_budget)?;
        let output_frame = request.render.first_frame + u64::from(output_index);
        let output_seconds = output_frame as f64 / output_fps;
        let playback_rate = f64::from(request.render.playback_rate_micros) / 1_000_000.0;
        let mut source_millis = request.render.source_start_micros as f64 / 1_000.0
            + output_seconds * playback_rate * 1000.0;
        if request.render.looping {
            source_millis %= f64::from(segment_duration_millis);
        } else {
            source_millis = source_millis.min(f64::from(segment_duration_millis));
        }
        let source_frame = segment_start
            + (source_millis * f64::from(segment_frames) / f64::from(segment_duration_millis))
                .min(f64::from(segment_frames) - f64::EPSILON) as f32;

        TvgRenderer::reset_expression_errors();
        if (player.current_frame() - source_frame).abs() > f32::EPSILON {
            pixels.fill(0);
            player
                .set_frame(source_frame)
                .map_err(|error| renderer_failure("unable to set Lottie frame", error))?;
            player
                .render()
                .map_err(|error| renderer_failure("unable to render Lottie frame", error))?;
        }
        fail_on_expression_errors("rendering frame")?;

        let rgba = abgr_words_to_rgba(&pixels);
        let encoded = encode_png(request.render.width, request.render.height, &rgba)?;
        output_bytes = output_bytes
            .checked_add(encoded.len() as u64)
            .ok_or_else(|| {
                BakeFailure::new(WorkerErrorCode::BudgetExceeded, "output size overflow")
            })?;
        if output_bytes > request.budgets.max_output_bytes {
            return Err(BakeFailure::new(
                WorkerErrorCode::BudgetExceeded,
                "PNG frames exceed maxOutputBytes",
            )
            .field("budgets.maxOutputBytes"));
        }
        let filename = format!("frame-{output_index:06}.png");
        fs::write(frames_dir.join(&filename), &encoded).map_err(|error| {
            BakeFailure::new(
                WorkerErrorCode::Io,
                format!("unable to write PNG frame: {error}"),
            )
        })?;
        frame_manifests.push(FrameManifest {
            index: output_index,
            source_frame,
            path: format!("frames/{filename}"),
            rgba_sha256: sha256_hex(&rgba),
            png_sha256: sha256_hex(&encoded),
            png_bytes: encoded.len() as u64,
        });
        progress(
            WorkerPhase::Rasterizing,
            output_index + 1,
            request.render.frame_count,
            "rendered deterministic RGBA frame",
        )?;
    }

    progress(
        WorkerPhase::Checksumming,
        request.render.frame_count,
        request.render.frame_count,
        "validated frame checksums",
    )?;
    let manifest = BakeManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        request_id: &request.request_id,
        cache_key: &request.cache_key,
        source_sha256: &request.source.sha256,
        animation_id: request.source.animation_id.as_deref(),
        inputs: &request.inputs,
        width: request.render.width,
        height: request.render.height,
        fps_numerator: request.render.fps.numerator,
        fps_denominator: request.render.fps.denominator,
        first_frame: request.render.first_frame,
        frame_count: request.render.frame_count,
        source_start_micros: request.render.source_start_micros,
        playback_rate_micros: request.render.playback_rate_micros,
        looping: request.render.looping,
        expressions_enabled: true,
        pixel_contract: PIXEL_CONTRACT,
        rng_policy: RNG_POLICY,
        renderer: RendererManifest {
            dotlottie_rs_revision: "2ce5e48f5786c3e60301d66db4cfdff56c896895",
            thorvg_revision: "73045df5398690eb1c6c0946e0b2301032337776",
            quality: 100,
            fit: "contain-center",
        },
        frames: frame_manifests,
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|error| {
        BakeFailure::new(
            WorkerErrorCode::Internal,
            format!("unable to serialize bake manifest: {error}"),
        )
    })?;
    output_bytes = output_bytes
        .checked_add(manifest_bytes.len() as u64)
        .ok_or_else(|| BakeFailure::new(WorkerErrorCode::BudgetExceeded, "output size overflow"))?;
    if output_bytes > request.budgets.max_output_bytes {
        return Err(BakeFailure::new(
            WorkerErrorCode::BudgetExceeded,
            "manifest exceeds maxOutputBytes",
        ));
    }
    let manifest_path = staging_dir.join("manifest.json");
    fs::write(&manifest_path, manifest_bytes).map_err(|error| {
        BakeFailure::new(
            WorkerErrorCode::Io,
            format!("unable to write manifest: {error}"),
        )
    })?;
    progress(
        WorkerPhase::WritingManifest,
        request.render.frame_count,
        request.render.frame_count,
        "wrote manifest last",
    )?;

    Ok(PrecomposeResult {
        manifest_path: manifest_path.to_string_lossy().into_owned(),
        frame_count: request.render.frame_count,
        output_bytes,
    })
}

fn validate_json_source(source: &[u8]) -> Result<(), BakeFailure> {
    let json: serde_json::Value = serde_json::from_slice(source).map_err(|error| {
        BakeFailure::new(
            WorkerErrorCode::SourceInvalid,
            format!("invalid Lottie JSON: {error}"),
        )
    })?;
    reject_external_assets(&json)
}

#[derive(Default)]
struct ArchiveControls {
    animations: BTreeMap<String, String>,
    themes: BTreeMap<String, String>,
    state_machines: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StateMachineDefinition {
    initial: String,
    #[serde(default)]
    states: Vec<StateDefinition>,
    #[serde(default)]
    inputs: Vec<StateInputDefinition>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StateDefinition {
    name: String,
    #[serde(default)]
    animation: Option<String>,
    #[serde(default)]
    segment: Option<String>,
    #[serde(default)]
    transitions: Vec<StateTransition>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StateTransition {
    to_state: String,
    #[serde(default)]
    guards: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
enum StateInputDefinition {
    Numeric { name: String, value: f64 },
    String { name: String, value: String },
    Boolean { name: String, value: bool },
    Event { name: String },
}

#[derive(Clone)]
enum StateValue {
    Numeric(f64),
    String(String),
    Boolean(bool),
    Event,
}

struct ResolvedState {
    animation: Option<String>,
    marker: Option<String>,
}

fn read_archive_controls(
    request: &PrecomposeRequest,
    source: &[u8],
) -> Result<ArchiveControls, BakeFailure> {
    let mut archive = ZipArchive::new(Cursor::new(source)).map_err(|error| {
        BakeFailure::new(
            WorkerErrorCode::SourceInvalid,
            format!("invalid dotLottie ZIP: {error}"),
        )
    })?;
    let mut controls = ArchiveControls::default();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| {
            BakeFailure::new(
                WorkerErrorCode::SourceInvalid,
                format!("unable to read dotLottie controls: {error}"),
            )
        })?;
        if entry.size() > request.budgets.max_expanded_archive_bytes
            || !entry.name().ends_with(".json")
        {
            continue;
        }
        let name = entry.name().to_string();
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut bytes).map_err(|error| {
            BakeFailure::new(
                WorkerErrorCode::SourceInvalid,
                format!("unable to read {name}: {error}"),
            )
        })?;
        let text = String::from_utf8(bytes).map_err(|_| {
            BakeFailure::new(
                WorkerErrorCode::SourceInvalid,
                format!("{name} is not UTF-8 JSON"),
            )
        })?;
        let id = Path::new(&name)
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_string();
        if name.starts_with("animations/") || name.starts_with("a/") {
            controls.animations.insert(id, text);
        } else if name.starts_with("t/") {
            controls.themes.insert(id, text);
        } else if name.starts_with("s/") {
            controls.state_machines.insert(id, text);
        }
    }
    Ok(controls)
}

fn resolve_state_machine(
    controls: &ArchiveControls,
    requested: &video_creater_precompose_protocol::LottieStateMachineInput,
) -> Result<ResolvedState, BakeFailure> {
    let source = controls.state_machines.get(&requested.id).ok_or_else(|| {
        BakeFailure::new(
            WorkerErrorCode::SourceInvalid,
            format!("unknown state machine {:?}", requested.id),
        )
        .field("inputs.stateMachine.id")
    })?;
    let machine: StateMachineDefinition = serde_json::from_str(source).map_err(|error| {
        BakeFailure::new(
            WorkerErrorCode::SourceInvalid,
            format!("invalid state machine {:?}: {error}", requested.id),
        )
        .field("inputs.stateMachine.id")
    })?;
    let mut values = BTreeMap::new();
    for input in machine.inputs {
        let (name, value) = match input {
            StateInputDefinition::Numeric { name, value } => (name, StateValue::Numeric(value)),
            StateInputDefinition::String { name, value } => (name, StateValue::String(value)),
            StateInputDefinition::Boolean { name, value } => (name, StateValue::Boolean(value)),
            StateInputDefinition::Event { name } => (name, StateValue::Event),
        };
        values.insert(name, value);
    }
    for input in &requested.inputs {
        let (name, value) = match input {
            LottieStateInputValue::Numeric { name, value_micros } => (
                name,
                StateValue::Numeric(*value_micros as f64 / 1_000_000.0),
            ),
            LottieStateInputValue::String { name, value } => {
                (name, StateValue::String(value.clone()))
            }
            LottieStateInputValue::Boolean { name, value } => (name, StateValue::Boolean(*value)),
        };
        let Some(declared) = values.get(name) else {
            return Err(BakeFailure::new(
                WorkerErrorCode::SourceInvalid,
                format!("unknown state input {name:?}"),
            )
            .field("inputs.stateMachine.inputs"));
        };
        if std::mem::discriminant(declared) != std::mem::discriminant(&value) {
            return Err(BakeFailure::new(
                WorkerErrorCode::SourceInvalid,
                format!("state input {name:?} has the wrong type"),
            )
            .field("inputs.stateMachine.inputs"));
        }
        values.insert(name.clone(), value);
    }
    for event in &requested.events {
        if !matches!(values.get(event), Some(StateValue::Event)) {
            return Err(BakeFailure::new(
                WorkerErrorCode::SourceInvalid,
                format!("unknown state event {event:?}"),
            )
            .field("inputs.stateMachine.events"));
        }
    }
    let mut current = machine.initial;
    let mut events = requested
        .events
        .iter()
        .map(Some)
        .chain(std::iter::once(None));
    for event in &mut events {
        let state = machine
            .states
            .iter()
            .find(|state| state.name == current)
            .ok_or_else(|| {
                BakeFailure::new(
                    WorkerErrorCode::SourceInvalid,
                    format!("state machine references unknown state {current:?}"),
                )
            })?;
        if let Some(transition) = state.transitions.iter().find(|transition| {
            transition
                .guards
                .iter()
                .all(|guard| state_guard_matches(guard, &values, event))
        }) {
            current = transition.to_state.clone();
        }
    }
    let state = machine
        .states
        .into_iter()
        .find(|state| state.name == current)
        .ok_or_else(|| {
            BakeFailure::new(
                WorkerErrorCode::SourceInvalid,
                format!("state machine resolved unknown state {current:?}"),
            )
        })?;
    Ok(ResolvedState {
        animation: state.animation,
        marker: state.segment,
    })
}

fn state_guard_matches(
    guard: &serde_json::Value,
    values: &BTreeMap<String, StateValue>,
    event: Option<&String>,
) -> bool {
    let kind = guard
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let name = guard
        .get("inputName")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let compare = guard.get("compareTo");
    let condition = guard
        .get("conditionType")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("Equal");
    let equal = match (kind, values.get(name), compare) {
        ("Event", Some(StateValue::Event), _) => event.is_some_and(|event| event == name),
        ("Boolean", Some(StateValue::Boolean(value)), Some(compare)) => {
            compare.as_bool() == Some(*value)
        }
        ("String", Some(StateValue::String(value)), Some(compare)) => {
            compare.as_str() == Some(value.as_str())
        }
        ("Numeric", Some(StateValue::Numeric(value)), Some(compare)) => {
            compare.as_f64().is_some_and(|compare| match condition {
                "GreaterThan" => *value > compare,
                "GreaterThanOrEqual" => *value >= compare,
                "LessThan" => *value < compare,
                "LessThanOrEqual" => *value <= compare,
                "NotEqual" => *value != compare,
                _ => *value == compare,
            })
        }
        _ => false,
    };
    if matches!(kind, "Boolean" | "String") && condition == "NotEqual" {
        !equal
    } else {
        equal
    }
}

fn apply_runtime_inputs(
    player: &mut Player,
    inputs: &LottieRuntimeInputs,
    archive: Option<&ArchiveControls>,
    animation_id: Option<&str>,
    format: LottieSourceFormat,
    source: &[u8],
) -> Result<(), BakeFailure> {
    let animation = match format {
        LottieSourceFormat::LottieJson => std::str::from_utf8(source).ok(),
        LottieSourceFormat::DotLottie => archive.and_then(|archive| {
            animation_id.and_then(|id| archive.animations.get(id).map(String::as_str))
        }),
    };
    let known_slots = animation
        .map(extract_slots_from_animation)
        .unwrap_or_default();
    if let Some(theme_id) = inputs.theme_id.as_deref() {
        let theme = archive
            .and_then(|archive| archive.themes.get(theme_id))
            .ok_or_else(|| {
                BakeFailure::new(
                    WorkerErrorCode::SourceInvalid,
                    format!("unknown theme {theme_id:?}"),
                )
                .field("inputs.themeId")
            })?;
        let value: serde_json::Value = serde_json::from_str(theme).map_err(|error| {
            BakeFailure::new(
                WorkerErrorCode::SourceInvalid,
                format!("invalid theme {theme_id:?}: {error}"),
            )
            .field("inputs.themeId")
        })?;
        for rule in value
            .get("rules")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
        {
            let id = rule
                .get("id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            if let Some(animations) = rule.get("animations").and_then(serde_json::Value::as_array) {
                if !animations
                    .iter()
                    .any(|value| value.as_str() == animation_id)
                {
                    continue;
                }
            }
            let kind = rule
                .get("type")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_ascii_lowercase();
            let value = rule
                .get("value")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            apply_slot_json(player, &known_slots, id, &kind, value, "inputs.themeId")?;
        }
    }
    for slot in &inputs.slots {
        let (kind, value) = match &slot.value {
            LottieSlotValue::Color { rgba } => (
                "color",
                serde_json::json!(rgba
                    .iter()
                    .take(3)
                    .map(|value| *value as f64 / 1_000_000.0)
                    .collect::<Vec<_>>()),
            ),
            LottieSlotValue::Scalar { value_micros } => (
                "scalar",
                serde_json::json!(*value_micros as f64 / 1_000_000.0),
            ),
            LottieSlotValue::Vector { x_micros, y_micros } => (
                "vector",
                serde_json::json!([
                    *x_micros as f64 / 1_000_000.0,
                    *y_micros as f64 / 1_000_000.0
                ]),
            ),
        };
        apply_slot_json(player, &known_slots, &slot.id, kind, value, "inputs.slots")?;
    }
    if let Some(marker) = inputs.marker.as_deref() {
        select_marker(player, marker, "inputs.marker")?;
    }
    if let Some(segment) = &inputs.segment {
        player
            .set_segment(Some(Segment {
                start: segment.start_frame_micros as f32 / 1_000_000.0,
                end: segment.end_frame_micros as f32 / 1_000_000.0,
            }))
            .map_err(|error| renderer_failure("unable to set Lottie segment", error))?;
    }
    Ok(())
}

fn apply_slot_json(
    player: &mut Player,
    known: &BTreeMap<String, SlotType>,
    id: &str,
    kind: &str,
    value: serde_json::Value,
    field: &str,
) -> Result<(), BakeFailure> {
    if !known.contains_key(id) {
        return Err(BakeFailure::new(
            WorkerErrorCode::SourceInvalid,
            format!("unknown Lottie slot {id:?}"),
        )
        .field(field));
    }
    let property = serde_json::json!({"a": 0, "k": value});
    let slot = parse_slot_from_json(kind, &property.to_string()).ok_or_else(|| {
        BakeFailure::new(
            WorkerErrorCode::SourceInvalid,
            format!("invalid {kind} value for slot {id:?}"),
        )
        .field(field)
    })?;
    let result = match slot {
        SlotType::Color(value) => player.set_color_slot(id, value),
        SlotType::Scalar(value) => player.set_scalar_slot(id, value),
        SlotType::Vector(value) => player.set_vector_slot(id, value),
        _ => {
            return Err(BakeFailure::new(
                WorkerErrorCode::SourceInvalid,
                format!("unsupported slot type {kind:?}"),
            ))
        }
    };
    result.map_err(|error| renderer_failure("unable to apply Lottie slot", error))
}

fn select_marker(player: &mut Player, marker: &str, field: &str) -> Result<(), BakeFailure> {
    let marker = CString::new(marker).map_err(|_| {
        BakeFailure::new(
            WorkerErrorCode::SourceInvalid,
            "marker contains an interior NUL byte",
        )
        .field(field)
    })?;
    if !player
        .markers()
        .iter()
        .any(|candidate| candidate.name.as_c_str() == marker.as_c_str())
    {
        return Err(BakeFailure::new(
            WorkerErrorCode::SourceInvalid,
            format!("unknown Lottie marker {:?}", marker.to_string_lossy()),
        )
        .field(field));
    }
    player.set_marker(Some(&marker));
    Ok(())
}

fn reject_external_assets(json: &serde_json::Value) -> Result<(), BakeFailure> {
    if let Some(assets) = json.get("assets").and_then(serde_json::Value::as_array) {
        for asset in assets {
            if let Some(path) = asset.get("p").and_then(serde_json::Value::as_str) {
                if !path.is_empty() && !path.starts_with("data:") {
                    return Err(BakeFailure::new(
                        WorkerErrorCode::SourceInvalid,
                        "external Lottie assets are forbidden; inline them as data URIs",
                    )
                    .field("source.path"));
                }
            }
        }
    }
    Ok(())
}

fn validate_dotlottie_archive(
    request: &PrecomposeRequest,
    source: &[u8],
) -> Result<(), BakeFailure> {
    let mut archive = ZipArchive::new(Cursor::new(source)).map_err(|error| {
        BakeFailure::new(
            WorkerErrorCode::SourceInvalid,
            format!("invalid dotLottie ZIP: {error}"),
        )
    })?;
    if archive.len() > request.budgets.max_archive_entries as usize {
        return Err(BakeFailure::new(
            WorkerErrorCode::BudgetExceeded,
            "dotLottie archive exceeds maxArchiveEntries",
        ));
    }
    let mut expanded_bytes = 0_u64;
    let mut archive_animation_ids = None;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| {
            BakeFailure::new(
                WorkerErrorCode::SourceInvalid,
                format!("unable to inspect dotLottie entry: {error}"),
            )
        })?;
        if entry.enclosed_name().is_none() {
            return Err(BakeFailure::new(
                WorkerErrorCode::SourceInvalid,
                "dotLottie archive contains an unsafe path",
            ));
        }
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(BakeFailure::new(
                WorkerErrorCode::SourceInvalid,
                "dotLottie archive contains a symlink",
            ));
        }
        expanded_bytes = expanded_bytes.checked_add(entry.size()).ok_or_else(|| {
            BakeFailure::new(
                WorkerErrorCode::BudgetExceeded,
                "expanded archive size overflow",
            )
        })?;
        if expanded_bytes > request.budgets.max_expanded_archive_bytes {
            return Err(BakeFailure::new(
                WorkerErrorCode::BudgetExceeded,
                "dotLottie archive exceeds maxExpandedArchiveBytes",
            ));
        }
        if entry.size() > 0
            && (entry.compressed_size() == 0
                || entry.size()
                    > entry
                        .compressed_size()
                        .saturating_mul(u64::from(request.budgets.max_compression_ratio)))
        {
            return Err(BakeFailure::new(
                WorkerErrorCode::BudgetExceeded,
                "dotLottie entry exceeds maxCompressionRatio",
            ));
        }
        if entry.name().ends_with(".json") {
            let mut json_bytes = Vec::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut json_bytes).map_err(|error| {
                BakeFailure::new(
                    WorkerErrorCode::SourceInvalid,
                    format!("unable to validate dotLottie JSON entry: {error}"),
                )
            })?;
            if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&json_bytes) {
                reject_external_assets(&json)?;
                if entry.name() == "manifest.json" {
                    let ids = json
                        .get("animations")
                        .and_then(serde_json::Value::as_array)
                        .map(|animations| {
                            animations
                                .iter()
                                .filter_map(|animation| {
                                    animation
                                        .get("id")
                                        .and_then(serde_json::Value::as_str)
                                        .map(str::to_string)
                                })
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    archive_animation_ids = Some(ids);
                }
            }
        }
    }
    let animation_ids = archive_animation_ids.ok_or_else(|| {
        BakeFailure::new(
            WorkerErrorCode::SourceInvalid,
            "dotLottie archive is missing a valid manifest.json",
        )
    })?;
    if animation_ids.len() > 1 && request.source.animation_id.is_none() {
        return Err(BakeFailure::new(
            WorkerErrorCode::SourceInvalid,
            "dotLottie archives with multiple animations require an explicit animationId",
        )
        .field("source.animationId"));
    }
    if let Some(selected) = request.source.animation_id.as_deref() {
        if !animation_ids
            .iter()
            .any(|animation_id| animation_id == selected)
        {
            return Err(BakeFailure::new(
                WorkerErrorCode::SourceInvalid,
                format!("dotLottie animationId `{selected}` is not present in manifest.json"),
            )
            .field("source.animationId"));
        }
    }
    Ok(())
}

fn check_wall_budget(started: Instant, budget: Duration) -> Result<(), BakeFailure> {
    if started.elapsed() > budget {
        Err(BakeFailure::new(
            WorkerErrorCode::BudgetExceeded,
            "precompose worker exceeded maxWallTimeMs",
        )
        .field("budgets.maxWallTimeMs"))
    } else {
        Ok(())
    }
}

fn fail_on_expression_errors(context: &str) -> Result<(), BakeFailure> {
    let count = TvgRenderer::expression_error_count();
    if count == 0 {
        Ok(())
    } else {
        Err(BakeFailure::new(
            WorkerErrorCode::ExpressionEvaluationFailed,
            format!("{count} Lottie expression evaluation error(s) occurred while {context}"),
        ))
    }
}

fn renderer_failure(context: &str, error: impl std::fmt::Debug) -> BakeFailure {
    BakeFailure::new(
        WorkerErrorCode::SourceInvalid,
        format!("{context}: {error:?}"),
    )
}

fn abgr_words_to_rgba(pixels: &[u32]) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(pixels.len() * 4);
    for pixel in pixels {
        rgba.push((pixel & 0xff) as u8);
        rgba.push(((pixel >> 8) & 0xff) as u8);
        rgba.push(((pixel >> 16) & 0xff) as u8);
        rgba.push(((pixel >> 24) & 0xff) as u8);
    }
    rgba
}

fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, BakeFailure> {
    let mut encoded = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut encoded, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|error| {
            BakeFailure::new(
                WorkerErrorCode::Internal,
                format!("unable to initialize PNG encoder: {error}"),
            )
        })?;
        writer.write_image_data(rgba).map_err(|error| {
            BakeFailure::new(
                WorkerErrorCode::Internal,
                format!("unable to encode PNG frame: {error}"),
            )
        })?;
    }
    Ok(encoded)
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::Mutex;
    use tempfile::tempdir;
    use video_creater_precompose_protocol::{
        AlphaMode, ColorSpace as ProtocolColorSpace, ExpressionPolicy, FrameRate, LottieRenderSpec,
        LottieRuntimeInputs, LottieSlotInput, LottieSlotValue, LottieSource, LottieStateInputValue,
        LottieStateMachineInput, PrecomposeOperation, WorkerBudgets, WorkerOutput,
        PRECOMPOSE_PROTOCOL_NAME, PRECOMPOSE_PROTOCOL_VERSION,
    };

    static NATIVE_RENDER_LOCK: Mutex<()> = Mutex::new(());

    fn fixture_request(fixture: &str, staging_dir: &Path) -> PrecomposeRequest {
        let source_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(fixture);
        source_request(&source_path, staging_dir)
    }

    fn source_request(source_path: &Path, staging_dir: &Path) -> PrecomposeRequest {
        let source = fs::read(source_path).expect("read expression fixture");
        PrecomposeRequest {
            protocol: PRECOMPOSE_PROTOCOL_NAME.to_string(),
            schema_version: PRECOMPOSE_PROTOCOL_VERSION,
            request_id: "expression-fixture".to_string(),
            cache_key: "a".repeat(64),
            operation: PrecomposeOperation::BakeLottieRgba,
            source: LottieSource {
                path: source_path.to_string_lossy().into_owned(),
                sha256: sha256_hex(&source),
                format: LottieSourceFormat::LottieJson,
                animation_id: None,
            },
            render: LottieRenderSpec {
                width: 64,
                height: 64,
                fps: FrameRate {
                    numerator: 2,
                    denominator: 1,
                },
                first_frame: 0,
                frame_count: 3,
                source_start_micros: 0,
                playback_rate_micros: 1_000_000,
                looping: true,
                alpha_mode: AlphaMode::Straight,
                color_space: ProtocolColorSpace::Srgb,
            },
            inputs: Default::default(),
            expressions: ExpressionPolicy { enabled: true },
            budgets: WorkerBudgets {
                max_frames: 3,
                max_pixels_per_frame: 64 * 64,
                max_source_bytes: 1024 * 1024,
                max_archive_entries: 64,
                max_expanded_archive_bytes: 4 * 1024 * 1024,
                max_compression_ratio: 100,
                max_wall_time_ms: 10_000,
                max_memory_bytes: 16 * 1024 * 1024,
                max_output_bytes: 4 * 1024 * 1024,
            },
            output: WorkerOutput {
                staging_dir: staging_dir.to_string_lossy().into_owned(),
            },
        }
    }

    fn remove_expression_fields(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(object) => {
                object.remove("x");
                for nested in object.values_mut() {
                    remove_expression_fields(nested);
                }
            }
            serde_json::Value::Array(values) => {
                for nested in values {
                    remove_expression_fields(nested);
                }
            }
            _ => {}
        }
    }

    fn write_multi_animation_dotlottie(path: &Path) {
        let file = fs::File::create(path).expect("dotLottie fixture");
        let mut archive = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        let manifest = serde_json::json!({
            "version": "1.0",
            "revision": 1,
            "animations": [
                { "id": "red", "direction": 1, "speed": 1, "playMode": "normal", "loop": true, "autoplay": false },
                { "id": "blue", "direction": 1, "speed": 1, "playMode": "normal", "loop": true, "autoplay": false }
            ]
        });
        archive
            .start_file("manifest.json", options)
            .expect("manifest entry");
        archive
            .write_all(
                serde_json::to_string(&manifest)
                    .expect("manifest JSON")
                    .as_bytes(),
            )
            .expect("manifest bytes");
        for (id, color) in [
            ("red", [1.0, 0.0, 0.0, 1.0]),
            ("blue", [0.0, 0.0, 1.0, 1.0]),
        ] {
            let animation = serde_json::json!({
                "v": "5.7.4", "fr": 2, "ip": 0, "op": 4, "w": 64, "h": 64,
                "nm": id, "ddd": 0, "assets": [],
                "layers": [{
                    "ddd": 0, "ind": 1, "ty": 4, "nm": id, "sr": 1,
                    "ks": { "o": {"a":0,"k":100}, "r":{"a":0,"k":0}, "p":{"a":0,"k":[32,32,0]}, "a":{"a":0,"k":[0,0,0]}, "s":{"a":0,"k":[100,100,100]} },
                    "shapes": [
                        { "ty":"rc", "d":1, "s":{"a":0,"k":[32,32]}, "p":{"a":0,"k":[0,0]}, "r":{"a":0,"k":0}, "nm":"Rectangle" },
                        { "ty":"fl", "c":{"a":0,"k":color}, "o":{"a":0,"k":100}, "r":1, "nm":"Fill" }
                    ],
                    "ip":0, "op":4, "st":0, "bm":0
                }]
            });
            archive
                .start_file(format!("animations/{id}.json"), options)
                .expect("animation entry");
            archive
                .write_all(
                    serde_json::to_string(&animation)
                        .expect("animation JSON")
                        .as_bytes(),
                )
                .expect("animation bytes");
        }
        archive.finish().expect("finish dotLottie fixture");
    }

    fn write_control_dotlottie(path: &Path) {
        let file = fs::File::create(path).expect("dotLottie control fixture");
        let mut archive = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        let manifest = serde_json::json!({
            "version": "2", "animations": [{"id":"main","themes":["dark"]}],
            "themes": [{"id":"dark"}], "stateMachines": [{"id":"button"}]
        });
        let animation = serde_json::json!({
            "v":"5.7.4","fr":2,"ip":0,"op":4,"w":64,"h":64,"nm":"controls","ddd":0,"assets":[],
            "markers":[{"cm":"idle","tm":0,"dr":1},{"cm":"active","tm":2,"dr":1}],
            "slots":{"brand":{"p":{"a":0,"k":[1.0,0.0,0.0]}}},
            "layers":[{"ddd":0,"ind":1,"ty":4,"nm":"badge","sr":1,
                "ks":{"o":{"a":0,"k":100},"r":{"a":0,"k":0},
                    "p":{"a":1,"k":[{"t":0,"s":[16,32,0],"e":[48,32,0]},{"t":2,"s":[48,32,0]}]},
                    "a":{"a":0,"k":[0,0,0]},"s":{"a":0,"k":[100,100,100]}},
                "shapes":[{"ty":"rc","d":1,"s":{"a":0,"k":[20,20]},"p":{"a":0,"k":[0,0]},"r":{"a":0,"k":0},"nm":"Rectangle"},
                    {"ty":"fl","c":{"a":0,"k":[1.0,0.0,0.0],"sid":"brand"},"o":{"a":0,"k":100},"r":1,"nm":"Fill"}],
                "ip":0,"op":4,"st":0,"bm":0}]
        });
        let theme =
            serde_json::json!({"rules":[{"type":"Color","id":"brand","value":[0.0,0.0,1.0]}]});
        let machine = serde_json::json!({
            "initial":"idle","inputs":[{"type":"Boolean","name":"enabled","value":false},{"type":"Event","name":"activate"}],
            "states":[
                {"type":"PlaybackState","name":"idle","animation":"main","segment":"idle","transitions":[{"type":"Transition","toState":"active","guards":[{"type":"Boolean","inputName":"enabled","conditionType":"Equal","compareTo":true},{"type":"Event","inputName":"activate"}]}]},
                {"type":"PlaybackState","name":"active","animation":"main","segment":"active","transitions":[]}
            ]
        });
        for (name, value) in [
            ("manifest.json", manifest),
            ("a/main.json", animation),
            ("t/dark.json", theme),
            ("s/button.json", machine),
        ] {
            archive.start_file(name, options).expect("fixture entry");
            archive
                .write_all(
                    serde_json::to_string(&value)
                        .expect("fixture JSON")
                        .as_bytes(),
                )
                .expect("fixture bytes");
        }
        archive.finish().expect("finish control fixture");
    }

    fn first_rgba_hash(path: &Path) -> String {
        let manifest: serde_json::Value = serde_json::from_slice(
            &fs::read(path.join("manifest.json")).expect("control manifest"),
        )
        .expect("control manifest JSON");
        manifest["frames"][0]["rgbaSha256"]
            .as_str()
            .expect("RGBA hash")
            .to_string()
    }

    #[test]
    fn abgr_straight_words_convert_to_explicit_rgba_bytes() {
        assert_eq!(
            abgr_words_to_rgba(&[0x44332211, 0xff0000ff]),
            vec![0x11, 0x22, 0x33, 0x44, 0xff, 0x00, 0x00, 0xff]
        );
    }

    #[test]
    fn external_json_assets_are_rejected() {
        let error = validate_json_source(br#"{"assets":[{"p":"https://example.test/a.png"}]}"#)
            .expect_err("external URL must fail");
        assert_eq!(error.code, WorkerErrorCode::SourceInvalid);
    }

    #[test]
    fn expression_frames_are_deterministic_across_fresh_players() {
        let _guard = NATIVE_RENDER_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let temporary = tempdir().expect("temporary directory");
        let first_dir = temporary.path().join("first");
        let second_dir = temporary.path().join("second");
        let baseline_dir = temporary.path().join("baseline");
        let first = fixture_request("time-seeded-random.json", &first_dir);
        let second = fixture_request("time-seeded-random.json", &second_dir);

        let mut baseline_json: serde_json::Value = serde_json::from_slice(
            &fs::read(&first.source.path).expect("expression fixture source"),
        )
        .expect("decode expression fixture");
        remove_expression_fields(&mut baseline_json);
        let baseline_path = temporary.path().join("without-expressions.json");
        fs::write(
            &baseline_path,
            serde_json::to_vec(&baseline_json).expect("encode baseline fixture"),
        )
        .expect("write baseline fixture");
        let baseline = source_request(&baseline_path, &baseline_dir);

        bake_lottie(&first, |_, _, _, _| Ok(())).expect("first expression bake");
        bake_lottie(&second, |_, _, _, _| Ok(())).expect("second expression bake");
        bake_lottie(&baseline, |_, _, _, _| Ok(())).expect("baseline bake");
        let first_manifest: serde_json::Value = serde_json::from_slice(
            &fs::read(first_dir.join("manifest.json")).expect("first manifest"),
        )
        .expect("decode first manifest");
        let second_manifest: serde_json::Value = serde_json::from_slice(
            &fs::read(second_dir.join("manifest.json")).expect("second manifest"),
        )
        .expect("decode second manifest");
        let baseline_manifest: serde_json::Value = serde_json::from_slice(
            &fs::read(baseline_dir.join("manifest.json")).expect("baseline manifest"),
        )
        .expect("decode baseline manifest");
        let first_hashes = first_manifest["frames"]
            .as_array()
            .expect("first frames")
            .iter()
            .map(|frame| frame["rgbaSha256"].as_str().expect("RGBA hash"))
            .collect::<Vec<_>>();
        let second_hashes = second_manifest["frames"]
            .as_array()
            .expect("second frames")
            .iter()
            .map(|frame| frame["rgbaSha256"].as_str().expect("RGBA hash"))
            .collect::<Vec<_>>();
        let sampled_source_frames = first_manifest["frames"]
            .as_array()
            .expect("first frames")
            .iter()
            .map(|frame| frame["sourceFrame"].as_f64().expect("source frame"))
            .collect::<Vec<_>>();
        let baseline_hashes = baseline_manifest["frames"]
            .as_array()
            .expect("baseline frames")
            .iter()
            .map(|frame| frame["rgbaSha256"].as_str().expect("RGBA hash"))
            .collect::<Vec<_>>();

        assert_eq!(first_hashes, second_hashes);
        assert_eq!(sampled_source_frames, vec![0.0, 15.0, 30.0]);
        assert_ne!(
            first_hashes, baseline_hashes,
            "expressions must affect pixels"
        );
        assert_ne!(
            first_hashes[0], first_hashes[1],
            "expression fixture must produce visible frame changes at {sampled_source_frames:?}"
        );
    }

    #[test]
    fn value_layer_references_and_wiggle_are_deterministic() {
        let _guard = NATIVE_RENDER_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let temporary = tempdir().expect("temporary directory");
        let first_dir = temporary.path().join("first");
        let second_dir = temporary.path().join("second");
        let baseline_dir = temporary.path().join("baseline");
        let first = fixture_request("value-layer-wiggle.json", &first_dir);
        let second = fixture_request("value-layer-wiggle.json", &second_dir);

        let mut baseline_json: serde_json::Value = serde_json::from_slice(
            &fs::read(&first.source.path).expect("expression fixture source"),
        )
        .expect("decode expression fixture");
        remove_expression_fields(&mut baseline_json);
        let baseline_path = temporary.path().join("without-expressions.json");
        fs::write(
            &baseline_path,
            serde_json::to_vec(&baseline_json).expect("encode baseline fixture"),
        )
        .expect("write baseline fixture");
        let baseline = source_request(&baseline_path, &baseline_dir);

        bake_lottie(&first, |_, _, _, _| Ok(())).expect("first expression bake");
        bake_lottie(&second, |_, _, _, _| Ok(())).expect("second expression bake");
        bake_lottie(&baseline, |_, _, _, _| Ok(())).expect("baseline bake");
        let manifest_hashes = |path: &Path| {
            let manifest: serde_json::Value = serde_json::from_slice(
                &fs::read(path.join("manifest.json")).expect("expression manifest"),
            )
            .expect("decode expression manifest");
            manifest["frames"]
                .as_array()
                .expect("expression frames")
                .iter()
                .map(|frame| frame["rgbaSha256"].as_str().expect("RGBA hash").to_string())
                .collect::<Vec<_>>()
        };
        let first_hashes = manifest_hashes(&first_dir);
        let second_hashes = manifest_hashes(&second_dir);
        let baseline_hashes = manifest_hashes(&baseline_dir);

        assert_eq!(first_hashes, second_hashes);
        assert_ne!(
            first_hashes, baseline_hashes,
            "value, layer references, and wiggle must affect pixels"
        );
        assert!(
            first_hashes.windows(2).any(|frames| frames[0] != frames[1]),
            "value, layer references, and wiggle must produce visible frame changes: {first_hashes:?}"
        );
    }

    #[test]
    fn long_lottie_bake_keeps_sequential_evidence_beyond_600_frames() {
        let _guard = NATIVE_RENDER_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let temporary = tempdir().expect("temporary directory");
        let output = temporary.path().join("long");
        let mut request = fixture_request("time-seeded-random.json", &output);
        request.render.width = 8;
        request.render.height = 8;
        request.render.frame_count = 601;
        request.budgets.max_frames = 601;
        request.budgets.max_pixels_per_frame = 64;
        request.budgets.max_wall_time_ms = 30_000;
        request.budgets.max_output_bytes = 8 * 1024 * 1024;

        bake_lottie(&request, |_, _, _, _| Ok(())).expect("long Lottie bake");
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(output.join("manifest.json")).expect("long manifest"))
                .expect("decode long manifest");
        let frames = manifest["frames"].as_array().expect("long frames");
        assert_eq!(frames.len(), 601);
        assert_eq!(frames[0]["index"], serde_json::json!(0));
        assert_eq!(frames[600]["index"], serde_json::json!(600));
        assert_eq!(
            frames[600]["path"],
            serde_json::json!("frames/frame-000600.png")
        );
    }

    #[test]
    fn expression_exceptions_fail_closed() {
        let _guard = NATIVE_RENDER_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let temporary = tempdir().expect("temporary directory");
        let request = fixture_request("invalid-expression.json", &temporary.path().join("invalid"));

        let error = bake_lottie(&request, |_, _, _, _| Ok(()))
            .expect_err("invalid expression must fail the bake");
        assert_eq!(error.code, WorkerErrorCode::ExpressionEvaluationFailed);
    }

    #[test]
    fn dotlottie_animation_selection_changes_rendered_pixels() {
        let _guard = NATIVE_RENDER_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let temporary = tempdir().expect("temporary directory");
        let source_path = temporary.path().join("multi.lottie");
        write_multi_animation_dotlottie(&source_path);
        let mut red = source_request(&source_path, &temporary.path().join("red"));
        red.source.format = LottieSourceFormat::DotLottie;
        red.source.animation_id = Some("red".to_string());
        red.render.frame_count = 1;
        red.budgets.max_frames = 1;
        let mut blue = source_request(&source_path, &temporary.path().join("blue"));
        blue.source.format = LottieSourceFormat::DotLottie;
        blue.source.animation_id = Some("blue".to_string());
        blue.render.frame_count = 1;
        blue.budgets.max_frames = 1;

        bake_lottie(&red, |_, _, _, _| Ok(())).expect("red animation bake");
        bake_lottie(&blue, |_, _, _, _| Ok(())).expect("blue animation bake");
        let red_manifest: serde_json::Value = serde_json::from_slice(
            &fs::read(temporary.path().join("red/manifest.json")).expect("red manifest"),
        )
        .expect("red manifest JSON");
        let blue_manifest: serde_json::Value = serde_json::from_slice(
            &fs::read(temporary.path().join("blue/manifest.json")).expect("blue manifest"),
        )
        .expect("blue manifest JSON");

        assert_ne!(
            red_manifest["frames"][0]["rgbaSha256"],
            blue_manifest["frames"][0]["rgbaSha256"]
        );
    }

    #[test]
    fn theme_marker_and_state_inputs_change_deterministic_frames() {
        let _guard = NATIVE_RENDER_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let temporary = tempdir().expect("temporary directory");
        let source_path = temporary.path().join("controls.lottie");
        write_control_dotlottie(&source_path);
        let request = |name: &str| {
            let mut request = source_request(&source_path, &temporary.path().join(name));
            request.source.format = LottieSourceFormat::DotLottie;
            request.source.animation_id = Some("main".to_string());
            request.render.first_frame = 1;
            request.render.frame_count = 1;
            request.budgets.max_frames = 1;
            request
        };
        let baseline = request("baseline");
        let mut theme = request("theme");
        theme.inputs.theme_id = Some("dark".to_string());
        let mut marker = request("marker");
        marker.inputs.marker = Some("active".to_string());
        let mut state = request("state");
        state.inputs.state_machine = Some(LottieStateMachineInput {
            id: "button".to_string(),
            inputs: vec![LottieStateInputValue::Boolean {
                name: "enabled".to_string(),
                value: true,
            }],
            events: vec!["activate".to_string()],
        });

        for request in [&baseline, &theme, &marker, &state] {
            bake_lottie(request, |_, _, _, _| Ok(())).expect("control bake");
        }
        let baseline_hash = first_rgba_hash(&temporary.path().join("baseline"));
        let theme_hash = first_rgba_hash(&temporary.path().join("theme"));
        let marker_hash = first_rgba_hash(&temporary.path().join("marker"));
        let state_hash = first_rgba_hash(&temporary.path().join("state"));
        assert_ne!(baseline_hash, theme_hash, "theme must change pixels");
        assert_ne!(baseline_hash, marker_hash, "marker must change pixels");
        assert_eq!(
            marker_hash, state_hash,
            "state event must resolve the active marker deterministically"
        );
    }

    #[test]
    fn unknown_typed_inputs_fail_closed() {
        let _guard = NATIVE_RENDER_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let temporary = tempdir().expect("temporary directory");
        let source_path = temporary.path().join("controls.lottie");
        write_control_dotlottie(&source_path);
        let mut request = source_request(&source_path, &temporary.path().join("unknown"));
        request.source.format = LottieSourceFormat::DotLottie;
        request.source.animation_id = Some("main".to_string());
        request.render.frame_count = 1;
        request.budgets.max_frames = 1;
        request.inputs = LottieRuntimeInputs {
            slots: vec![LottieSlotInput {
                id: "missing".to_string(),
                value: LottieSlotValue::Color {
                    rgba: [1_000_000, 0, 0, 1_000_000],
                },
            }],
            ..Default::default()
        };
        let error = bake_lottie(&request, |_, _, _, _| Ok(())).expect_err("unknown slot must fail");
        assert_eq!(error.code, WorkerErrorCode::SourceInvalid);
        assert_eq!(error.field.as_deref(), Some("inputs.slots"));
    }

    #[test]
    fn dotlottie_multi_animation_archive_requires_explicit_selection() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let source_path = temporary.path().join("multi.lottie");
        write_multi_animation_dotlottie(&source_path);
        let mut request = source_request(&source_path, &temporary.path().join("output"));
        request.source.format = LottieSourceFormat::DotLottie;

        let error =
            bake_lottie(&request, |_, _, _, _| Ok(())).expect_err("ambiguous archive must fail");
        assert_eq!(error.code, WorkerErrorCode::SourceInvalid);
        assert_eq!(error.field.as_deref(), Some("source.animationId"));
        assert!(error.message.contains("explicit animationId"));
    }
}
