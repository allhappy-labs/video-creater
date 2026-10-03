#[cfg(feature = "temporal-worker")]
use video_creater_lib::render_runtime::start_render_process_runtime;
use video_creater_lib::workflows::temporal_worker_environment_report;
#[cfg(feature = "temporal-worker")]
use video_creater_lib::workflows::temporal_worker_manifest;
#[cfg(feature = "temporal-worker")]
use video_creater_lib::workflows::temporal_worker_options;
#[cfg(feature = "temporal-worker")]
use video_creater_lib::workflows::temporal_worker_registration_plan;

#[cfg(feature = "temporal-worker")]
fn main() -> anyhow::Result<()> {
    start_render_process_runtime().map_err(|error| anyhow::anyhow!(error))?;
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(run())
}

#[cfg(feature = "temporal-worker")]
async fn run() -> anyhow::Result<()> {
    let manifest = temporal_worker_manifest();
    let registration_plan = temporal_worker_registration_plan();
    let environment = temporal_worker_environment_report();
    let manifest_json = serde_json::json!({
        "taskQueue": manifest.task_queue,
        "localServiceTarget": manifest.local_service_target,
        "localWebUiUrl": manifest.local_web_ui_url,
        "localDevCommand": manifest.local_dev_command,
        "workerRunCommand": manifest.worker_run_command,
        "featureName": manifest.feature_name,
        "sdkCrates": manifest.sdk_crates,
        "requiredTools": manifest.required_tools,
        "workflowTypes": manifest.workflow_types,
        "activityTypes": manifest.activity_types,
        "registrationPlan": registration_plan,
        "environment": environment,
    });

    if std::env::args().any(|arg| arg == "--print-manifest") {
        println!("{}", serde_json::to_string_pretty(&manifest_json)?);
        return Ok(());
    }

    let worker_options = temporal_worker_options()?;
    let runtime = temporalio_sdk_core::CoreRuntime::new_assume_tokio(
        temporalio_sdk_core::RuntimeOptions::builder()
            .telemetry_options(temporalio_common::telemetry::TelemetryOptions::builder().build())
            .build()
            .map_err(|error| anyhow::anyhow!(error))?,
    )?;
    let (connection_options, client_options) = temporalio_client::ClientOptions::load_from_config(
        temporalio_client::envconfig::LoadClientConfigProfileOptions::default(),
    )
    .map_err(|error| anyhow::anyhow!("{error}"))?;
    let connection = temporalio_client::Connection::connect(connection_options).await?;
    let client = temporalio_client::Client::new(connection, client_options)?;
    let mut worker = temporalio_sdk::Worker::new(&runtime, client, worker_options)
        .map_err(|error| anyhow::anyhow!("{error}"))?;

    eprintln!(
        "Temporal worker started on task queue {}. Local service target: {}.",
        manifest_json["taskQueue"]
            .as_str()
            .unwrap_or("video-creater-workflows"),
        manifest_json["localServiceTarget"]
            .as_str()
            .unwrap_or("http://localhost:7233")
    );
    worker.run().await?;
    Ok(())
}

#[cfg(not(feature = "temporal-worker"))]
fn main() {
    let report = temporal_worker_environment_report();

    eprintln!(
        "Temporal worker support is disabled. Rebuild with `--features {}` to target {} on task queue {}. Run `{}` after starting `{}`.",
        report.feature_name,
        report.local_service_target,
        report.task_queue,
        report.worker_run_command,
        report.local_dev_command
    );
}
