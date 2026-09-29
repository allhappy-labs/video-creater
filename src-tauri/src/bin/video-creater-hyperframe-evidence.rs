use serde_json::{json, Value};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use video_creater_lib::edit::render_plan::RenderQualityProfile;
use video_creater_lib::project::action::{apply_project_action, ProjectAction};
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::render_pipeline::gstreamer_backend::generate_fixture_source_with_gstreamer;
use video_creater_lib::render_pipeline::proposal::{
    run_render_proposal, CodexProposalReport, RenderProposalConfig,
};
use video_creater_lib::render_runtime::start_render_process_runtime;

#[cfg(target_os = "macos")]
fn main() {
    if let Err(error) = start_render_process_runtime() {
        eprintln!("{error}");
        std::process::exit(1);
    }
    let code = gstreamer::macos_main(run);
    std::process::exit(code);
}
#[cfg(not(target_os = "macos"))]
fn main() {
    if let Err(error) = start_render_process_runtime() {
        eprintln!("{error}");
        std::process::exit(1);
    }
    std::process::exit(run());
}

fn run() -> i32 {
    match build_evidence() {
        Ok(path) => {
            println!("retained HyperFrame evidence at {}", path.display());
            0
        }
        Err(error) => {
            eprintln!("HyperFrame evidence failed: {error}");
            1
        }
    }
}

fn build_evidence() -> Result<PathBuf, String> {
    let root = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("output/hyperframe-evidence-20260711"));
    fs::create_dir_all(root.join("media")).map_err(string_error)?;
    let root = root.canonicalize().map_err(string_error)?;
    let source = root.join("media/input.mp4");
    generate_fixture_source_with_gstreamer(&source, 320, 180, 12.0, 6.0, Duration::from_secs(60))
        .map_err(|errors| format!("fixture source failed: {errors:?}"))?;
    let report: CodexProposalReport = serde_json::from_value(json!({
        "generatedAt":"2026-07-11T00:00:00Z",
        "proposal":{
            "mediaId":"media-1",
            "clips":[
                {"mediaId":"media-1","sourceIn":0.25,"sourceOut":2.25,"reason":"hook with visible action"},
                {"mediaId":"media-1","sourceIn":3.0,"sourceOut":5.0,"reason":"payoff after a real cut"}
            ],
            "captions":[],"overlays":[],"gpuVisuals":[],"projectActions":[],
            "hyperframes":[
                hyperframe(HyperframeBrief { kind: "title_card", start: 0.0, duration: 0.6, headline: "Launch proof", subline: "Opening beat", visual: "kinetic editorial title with dimensional accent layers", motion: "fast type-on and camera push", safe: "keep title inside 10% margins", avoid: "static text-only cards" }),
                hyperframe(HyperframeBrief { kind: "lower_third", start: 0.65, duration: 0.65, headline: "Olha API", subline: "Editor", visual: "compact translucent lower third with cyan accent and strong hierarchy", motion: "slide in, hold, soft fade", safe: "keep essential text inside lower safe margins", avoid: "full-width opaque black slabs" }),
                hyperframe(HyperframeBrief { kind: "diagram", start: 1.35, duration: 0.65, headline: "Three steps", subline: "Cut Render", visual: "transparent process diagram with connected labeled nodes", motion: "stagger nodes and wipe connectors", safe: "keep labels inside 10% margins", avoid: "dense labels and plain boxes" }),
                hyperframe(HyperframeBrief { kind: "transition", start: 2.05, duration: 0.45, headline: "Next", subline: "Payoff", visual: "short full-frame kinetic color wipe with sparse readable cue", motion: "fast panel wipe with clean exit", safe: "keep cue inside title safe margins", avoid: "long static holds" }),
                hyperframe(HyperframeBrief { kind: "immersive_scene", start: 2.55, duration: 1.4, headline: "Scene", subline: "Dimensional payoff", visual: "full-frame immersive editorial scene with dimensional color panels", motion: "slow parallax drift with clean entrance and exit", safe: "keep cue text inside 10% margins", avoid: "static text-only cards" })
            ],
            "renderReview":{"durationSeconds":4.0,"streamCheckRequired":true,"captionAlignmentRequired":true,
                "overlayTimingRequired":true,"visualFrameEvidenceRequired":true,"artifactPathsRequired":true,"logReferenceRequired":true}
        }
    })).map_err(string_error)?;
    let report_path = root.join("proposal.json");
    fs::write(
        &report_path,
        serde_json::to_vec_pretty(&report).map_err(string_error)?,
    )
    .map_err(string_error)?;
    let render_dir = root.join("render");
    let result = run_render_proposal(&RenderProposalConfig {
        project_root: root.clone(),
        source_video_path: source,
        codex_report_path: report_path,
        output_dir: render_dir.clone(),
        final_name: "final.webm".to_string(),
        quality_profile: RenderQualityProfile::FinalWebm,
    })
    .map_err(|errors| format!("native proposal render failed: {errors:?}"))?;
    let graphics = verify_graphics(&render_dir.join("graphics"))?;
    let recovery = mock_failure_retry_recovery()?;
    fs::write(
        root.join("mock-provider-recovery.json"),
        serde_json::to_vec_pretty(&recovery).map_err(string_error)?,
    )
    .map_err(string_error)?;
    let render_report: Value =
        serde_json::from_slice(&fs::read(&result.render_report_path).map_err(string_error)?)
            .map_err(string_error)?;
    let log_path = render_dir.join("render.log");
    fs::write(
        &log_path,
        format!(
            "{}\n{}",
            render_report
                .get("stdout")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            render_report
                .get("stderr")
                .and_then(Value::as_str)
                .unwrap_or_default()
        ),
    )
    .map_err(string_error)?;
    fs::write(root.join("evidence.json"), serde_json::to_vec_pretty(&json!({
        "schemaVersion":1,"spend":"none","edlClipCount":2,"edlFirst":true,
        "hyperframeKinds":["title_card","lower_third","diagram","transition","immersive_scene"],
        "graphics":graphics,"finalRender":result.final_path,"renderReport":result.render_report_path,
        "renderChecks":render_report.get("summary"),"renderLog":log_path,"mockProviderRecovery":"mock-provider-recovery.json"
    })).map_err(string_error)?).map_err(string_error)?;
    Ok(root)
}

struct HyperframeBrief<'a> {
    kind: &'a str,
    start: f64,
    duration: f64,
    headline: &'a str,
    subline: &'a str,
    visual: &'a str,
    motion: &'a str,
    safe: &'a str,
    avoid: &'a str,
}

fn hyperframe(brief: HyperframeBrief<'_>) -> Value {
    json!({"kind":brief.kind,"role":brief.kind,"startSeconds":brief.start,"durationSeconds":brief.duration,
        "brief":format!("{}: {}", brief.headline, brief.subline),"sourceBeat":brief.subline,
        "fields":{"headline":brief.headline,"subline":brief.subline},"visualTreatment":brief.visual,
        "motion":brief.motion,"safeZone":brief.safe,"avoid":brief.avoid})
}

fn verify_graphics(root: &Path) -> Result<Vec<Value>, String> {
    let mut evidence = Vec::new();
    for index in 1..=5 {
        let directory = root.join(format!("proposal-hyperframe-{index}"));
        let manifest: Value = serde_json::from_slice(
            &fs::read(directory.join("manifest.json")).map_err(string_error)?,
        )
        .map_err(string_error)?;
        if manifest.get("alpha").and_then(Value::as_bool) != Some(true)
            || manifest
                .get("frameCount")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                < 2
        {
            return Err(format!(
                "HyperFrame {index} lacks alpha or animated frame evidence"
            ));
        }
        evidence.push(json!({"id":format!("proposal-hyperframe-{index}"),"manifest":directory.join("manifest.json"),
            "preview":directory.join("preview.png"),"alpha":true,"frameCount":manifest["frameCount"]}));
    }
    Ok(evidence)
}

fn mock_failure_retry_recovery() -> Result<Value, String> {
    let mut project = sample_project();
    for action in [
        json!({"type":"recordGeneratedAsset","asset":{"id":"mock-scene-failed","kind":"generated","status":"queued","name":"Mock scene","placementIntent":"replaceTimeline","prompt":"no-spend mock scene","model":{"provider":"mock","id":"hyperframe-scene-v1"},"references":{"mediaIds":[],"firstFrameMediaId":null,"lastFrameMediaId":null},"settings":{"width":320,"height":180,"durationSeconds":1.0,"fps":12.0},"outputs":[],"createdAt":"2026-07-11T00:00:00Z","parentAssetId":null,"retryOfAssetId":null}}),
        json!({"type":"updateGeneratedAssetStatus","assetId":"mock-scene-failed","status":"failed"}),
        json!({"type":"recordGeneratedAsset","asset":{"id":"mock-scene-retry","kind":"generated","status":"queued","name":"Mock scene retry","placementIntent":"replaceTimeline","prompt":"no-spend mock scene retry","model":{"provider":"mock","id":"hyperframe-scene-v1"},"references":{"mediaIds":[],"firstFrameMediaId":null,"lastFrameMediaId":null},"settings":{"width":320,"height":180,"durationSeconds":1.0,"fps":12.0},"outputs":[],"createdAt":"2026-07-11T00:00:01Z","parentAssetId":"mock-scene-failed","retryOfAssetId":"mock-scene-failed"}}),
        json!({"type":"completeGeneratedAsset","assetId":"mock-scene-retry","outputs":[{"mediaId":"mock-scene-output","relativePath":"generated/mock-scene-retry/output.mp4","sourceUrl":null,"width":320,"height":180,"durationSeconds":1.0,"fps":12.0}],"replacement":{"itemId":"item-1","mediaId":"mock-scene-output"}}),
    ] {
        apply_project_action(
            &mut project,
            serde_json::from_value::<ProjectAction>(action).map_err(string_error)?,
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(
        json!({"failedAsset":project.generated_assets.iter().find(|asset| asset.id=="mock-scene-failed"),
        "retryAsset":project.generated_assets.iter().find(|asset| asset.id=="mock-scene-retry"),
        "timelineItem":project.timeline.tracks.iter().flat_map(|track| &track.items).find(|item| item.id=="item-1")}),
    )
}

fn string_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}
