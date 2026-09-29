use std::{fs, path::Path};

#[cfg(target_os = "macos")]
use std::os::unix::fs::PermissionsExt;

use tempfile::tempdir;
use video_creater_lib::edit::preset::{CaptionStyle, EditJobRequest, EditPreset, LanguageMode};
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::{Transcript, TranscriptWord};
use video_creater_lib::transcription::acquisition::{
    HuggingFaceHubProvider, ModelArtifactProvider,
};
use video_creater_lib::transcription::job::{
    validate_transcription_ready_for_generate_edit, TranscriptionJobError,
};
use video_creater_lib::transcription::model::{
    catalog_entry_runtime_id, parakeet_v3_catalog_entry, parakeet_v3_coreml_catalog_entry,
    parakeet_v3_sherpa_onnx_catalog_entry, required_files_dir_for_entry, safe_model_dir_name,
    InstalledModelManifest, ModelArtifactProviderKind, ModelArtifactSource, ModelInstallStatus,
    ModelModality as CatalogModelModality, TranscriptionModelArtifactFormat,
    TranscriptionModelCatalogEntry, TranscriptionModelFamily, TranscriptionModelFile,
    TranscriptionRuntimeId, FLUID_AUDIO_COREML_RUNTIME_ID,
};
use video_creater_lib::transcription::runtime::{
    coreml_unsupported_platform_diagnostic, select_transcription_runtime, CoreMlParakeetRuntime,
    ModelArtifactFormat, ModelModality, ModelRuntime, RuntimeCapability, RuntimeSelection,
    TranscriptToken, TranscriptionRuntime, TranscriptionRuntimeRegistry,
};
#[cfg(target_os = "macos")]
use video_creater_lib::transcription::runtime::{
    CoreMlParakeetBackend, TranscriptionBackend, TranscriptionRuntimeEngine,
    TranscriptionRuntimeJob,
};
#[cfg(target_os = "macos")]
use video_creater_lib::transcription::store::{
    global_transcription_model_root_for_home, VIDEO_CREATER_APP_IDENTIFIER,
};
use video_creater_lib::transcription::store::{
    StartDownload, TranscriptionModelStore, COREML_INSPECTION_FILE_NAME, MODEL_MANIFEST_FILE_NAME,
};

fn sample_edit_request_for_gate() -> EditJobRequest {
    EditJobRequest {
        media_id: "media-1".to_string(),
        preset: EditPreset::TrailerCut,
        prompt: "Make a trailer".to_string(),
        target_duration_seconds: None,
        language_mode: LanguageMode::English,
        caption_style: CaptionStyle::Bold,
        created_at: "2026-06-13T00:00:00Z".to_string(),
    }
}

#[test]
fn generate_edit_gate_blocks_missing_model_when_project_has_no_transcript() {
    let project = sample_project();
    let request = sample_edit_request_for_gate();

    let result = validate_transcription_ready_for_generate_edit(
        &project,
        &request,
        ModelInstallStatus::Missing,
        RuntimeSelection::Native,
    );

    assert_eq!(result, Err(TranscriptionJobError::ModelNotReady));
}

#[cfg(target_os = "macos")]
#[test]
fn global_transcription_model_root_for_home_matches_tauri_app_data_dir() {
    let home = std::path::Path::new("/Users/tester");

    let root = global_transcription_model_root_for_home(home);

    assert_eq!(
        root,
        home.join("Library")
            .join("Application Support")
            .join(VIDEO_CREATER_APP_IDENTIFIER)
            .join("models")
    );
}

#[test]
fn generate_edit_gate_allows_existing_transcript_without_ready_model() {
    let mut project = sample_project();
    project.transcripts.push(sample_transcript_for_gate());
    let request = sample_edit_request_for_gate();

    let result = validate_transcription_ready_for_generate_edit(
        &project,
        &request,
        ModelInstallStatus::Missing,
        RuntimeSelection::Unavailable,
    );

    assert_eq!(result, Ok(()));
}

#[test]
fn generate_edit_gate_allows_ready_model_and_runtime() {
    let project = sample_project();
    let request = sample_edit_request_for_gate();

    let native_result = validate_transcription_ready_for_generate_edit(
        &project,
        &request,
        ModelInstallStatus::Ready,
        RuntimeSelection::Native,
    );

    assert_eq!(native_result, Ok(()));
}

#[test]
fn generate_edit_gate_blocks_unavailable_runtime_when_model_is_ready() {
    let project = sample_project();
    let request = sample_edit_request_for_gate();

    let result = validate_transcription_ready_for_generate_edit(
        &project,
        &request,
        ModelInstallStatus::Ready,
        RuntimeSelection::Unavailable,
    );

    assert_eq!(result, Err(TranscriptionJobError::RuntimeUnavailable));
}

#[test]
fn catalog_entry_declares_coreml_bundle_without_transformers_runtime() {
    let entry = parakeet_v3_coreml_catalog_entry();

    assert_eq!(entry.family, TranscriptionModelFamily::Parakeet);
    assert_eq!(
        entry.artifact_format,
        TranscriptionModelArtifactFormat::CoreMlBundle
    );
    assert_eq!(
        entry.supported_runtimes,
        vec![TranscriptionRuntimeId::FluidAudioCoreMl]
    );
    assert_eq!(
        serde_json::to_value(&entry).expect("serialize catalog entry")["supportedRuntimes"],
        serde_json::json!([FLUID_AUDIO_COREML_RUNTIME_ID])
    );
    assert!(!entry
        .required_files
        .iter()
        .any(|file| file.path == "model.safetensors"));
}

#[test]
fn platform_catalog_entry_uses_sherpa_onnx_on_linux_and_coreml_elsewhere() {
    let entry = parakeet_v3_catalog_entry();

    #[cfg(target_os = "linux")]
    assert_eq!(entry, parakeet_v3_sherpa_onnx_catalog_entry());
    #[cfg(not(target_os = "linux"))]
    assert_eq!(entry, parakeet_v3_coreml_catalog_entry());
}

#[test]
fn sherpa_onnx_catalog_entry_declares_pinned_hugging_face_onnx_source() {
    let entry = parakeet_v3_sherpa_onnx_catalog_entry();

    assert_eq!(entry.id, "nvidia/parakeet-tdt-0.6b-v3");
    assert_eq!(entry.family, TranscriptionModelFamily::Parakeet);
    assert_eq!(
        entry.artifact_format,
        TranscriptionModelArtifactFormat::SherpaOnnxTransducer
    );
    assert_eq!(
        entry.supported_runtimes,
        vec![TranscriptionRuntimeId::SherpaOnnx]
    );
    assert_eq!(catalog_entry_runtime_id(&entry), "sherpa_onnx");
    let required_paths = entry
        .required_files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        required_paths,
        vec![
            "encoder.int8.onnx",
            "decoder.int8.onnx",
            "joiner.int8.onnx",
            "tokens.txt"
        ]
    );
    assert!(entry
        .required_files
        .iter()
        .all(|file| file.size_bytes.is_some_and(|size| size > 0)
            && file.sha256.as_deref().is_some_and(|hash| hash.len() == 64)));

    assert_eq!(entry.artifact_sources.len(), 1);
    let source = &entry.artifact_sources[0];
    assert_eq!(source.provider, ModelArtifactProviderKind::HuggingFaceHub);
    assert_eq!(
        source.repo_id,
        "csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8"
    );
    assert_eq!(source.revision, "2bda32ec70b097a55adaa07d9a7173915b43cc78");
    assert_eq!(source.license.as_deref(), Some("cc-by-4.0"));
    assert_eq!(source.path_prefix, None);
    assert_eq!(source.include_files, required_paths);

    let json = serde_json::to_value(&entry).expect("serialize catalog entry");
    assert_eq!(
        json["supportedRuntimes"],
        serde_json::json!(["sherpa_onnx"])
    );
    assert_eq!(
        json["artifactFormat"],
        serde_json::json!("sherpa_onnx_transducer")
    );
}

#[test]
fn parakeet_v3_catalog_entry_lists_compiled_coreml_bundles() {
    let entry = parakeet_v3_coreml_catalog_entry();
    let required_paths = entry
        .required_files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<Vec<_>>();

    assert!(required_paths.contains(&"Encoder.mlmodelc/coremldata.bin"));
    assert!(required_paths.contains(&"Decoder.mlmodelc/coremldata.bin"));
    assert!(required_paths.contains(&"JointDecisionv3.mlmodelc/coremldata.bin"));
    assert!(required_paths.contains(&"Preprocessor.mlmodelc/coremldata.bin"));
}

#[test]
fn parakeet_v3_catalog_entry_declares_pinned_hugging_face_coreml_source() {
    let entry = parakeet_v3_coreml_catalog_entry();

    assert_eq!(entry.revision, "main");
    assert_eq!(
        entry.modality,
        video_creater_lib::transcription::model::ModelModality::Transcription
    );
    assert_eq!(entry.artifact_sources.len(), 1);

    let source = &entry.artifact_sources[0];
    assert_eq!(
        source.provider,
        video_creater_lib::transcription::model::ModelArtifactProviderKind::HuggingFaceHub
    );
    assert_eq!(source.repo_id, "FluidInference/parakeet-tdt-0.6b-v3-coreml");
    assert_eq!(source.revision, "aed02740059203c4a87495924f685de3722ae9ce");
    assert_eq!(source.license.as_deref(), Some("cc-by-4.0"));
    assert!(source
        .include_files
        .contains(&"Preprocessor.mlmodelc/coremldata.bin".to_string()));
    assert!(source
        .include_files
        .contains(&"Encoder.mlmodelc/coremldata.bin".to_string()));
    assert!(source
        .include_files
        .contains(&"Decoder.mlmodelc/coremldata.bin".to_string()));
    assert!(source
        .include_files
        .contains(&"JointDecisionv3.mlmodelc/coremldata.bin".to_string()));
    assert!(source
        .include_files
        .contains(&"parakeet_v3_vocab.json".to_string()));
    assert!(source.include_files.contains(&"config.json".to_string()));

    let json = serde_json::to_value(&entry).expect("serialize catalog entry");
    assert_eq!(json["modality"], serde_json::json!("transcription"));
    assert_eq!(
        json["artifactSources"][0]["provider"],
        serde_json::json!("hugging_face_hub")
    );
    assert_eq!(
        json["artifactSources"][0]["repoId"],
        serde_json::json!("FluidInference/parakeet-tdt-0.6b-v3-coreml")
    );
    assert_eq!(
        json["artifactSources"][0]["revision"],
        serde_json::json!("aed02740059203c4a87495924f685de3722ae9ce")
    );
    let source_json = json["artifactSources"][0]
        .as_object()
        .expect("artifact source serializes as object");
    assert!(source_json.contains_key("pathPrefix"));
    assert_eq!(source_json["pathPrefix"], serde_json::Value::Null);
}

#[test]
fn hugging_face_provider_builds_pinned_resolve_urls() {
    let coreml_entry = parakeet_v3_coreml_catalog_entry();
    let coreml_source = coreml_entry.artifact_sources.first().expect("source");

    let url = video_creater_lib::transcription::acquisition::hugging_face_resolve_url(
        coreml_source,
        "Encoder.mlmodelc/coremldata.bin",
    )
    .expect("resolve url");

    assert_eq!(
        url,
        "https://huggingface.co/FluidInference/parakeet-tdt-0.6b-v3-coreml/resolve/aed02740059203c4a87495924f685de3722ae9ce/Encoder.mlmodelc/coremldata.bin"
    );

    let sherpa_entry = parakeet_v3_sherpa_onnx_catalog_entry();
    let sherpa_source = sherpa_entry.artifact_sources.first().expect("source");

    let url = video_creater_lib::transcription::acquisition::hugging_face_resolve_url(
        sherpa_source,
        "encoder.int8.onnx",
    )
    .expect("resolve url");

    assert_eq!(
        url,
        "https://huggingface.co/csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8/resolve/2bda32ec70b097a55adaa07d9a7173915b43cc78/encoder.int8.onnx"
    );
}

#[test]
fn hugging_face_provider_trait_object_supports_matching_sources_only() {
    let entry = parakeet_v3_catalog_entry();
    let source = entry.artifact_sources.first().expect("source");
    let provider: Box<dyn ModelArtifactProvider> =
        Box::new(HuggingFaceHubProvider::new(reqwest::blocking::Client::new()));

    assert!(provider.supports(source));

    let mut mirror_source = source.clone();
    mirror_source.provider = ModelArtifactProviderKind::HttpMirror;

    assert!(!provider.supports(&mirror_source));
}

#[test]
fn hugging_face_provider_download_rejects_unsupported_provider() {
    use video_creater_lib::transcription::acquisition::{
        HuggingFaceHubProvider, ModelAcquisitionError, ModelArtifactProvider, ModelDownloadProgress,
    };

    struct NoCancel;
    impl ModelDownloadProgress for NoCancel {
        fn is_cancelled(&self) -> Result<bool, ModelAcquisitionError> {
            Ok(false)
        }

        fn file_completed(&self, _downloaded_files: u32) -> Result<(), ModelAcquisitionError> {
            Ok(())
        }
    }

    let target_dir = tempfile::tempdir().expect("target");
    let source = ModelArtifactSource {
        provider: ModelArtifactProviderKind::HttpMirror,
        repo_id: "org/model".to_string(),
        revision: "abc123".to_string(),
        path_prefix: None,
        include_files: vec!["config.json".to_string()],
        license: None,
    };
    let provider = HuggingFaceHubProvider::with_base_urls(reqwest::blocking::Client::new(), "", "");

    let error = provider
        .download(&source, target_dir.path(), &NoCancel)
        .expect_err("download should reject unsupported provider");

    assert!(matches!(
        error,
        ModelAcquisitionError::UnsupportedProvider(ModelArtifactProviderKind::HttpMirror)
    ));
}

#[test]
fn hugging_face_provider_uses_custom_resolve_base_url() {
    let source = ModelArtifactSource {
        provider: ModelArtifactProviderKind::HuggingFaceHub,
        repo_id: "org/model".to_string(),
        revision: "abc123".to_string(),
        path_prefix: None,
        include_files: vec!["config.json".to_string()],
        license: None,
    };
    let provider =
        video_creater_lib::transcription::acquisition::HuggingFaceHubProvider::with_base_urls(
            reqwest::blocking::Client::new(),
            "https://hf.example.test/api/models/",
            "https://hf.example.test/",
        );

    let url = provider
        .resolve_url(&source, "config.json")
        .expect("resolve url");

    assert_eq!(
        url,
        "https://hf.example.test/org/model/resolve/abc123/config.json"
    );
}

#[test]
fn hugging_face_provider_maps_path_prefix_into_resolve_urls() {
    let source = ModelArtifactSource {
        provider: ModelArtifactProviderKind::HuggingFaceHub,
        repo_id: "org/model".to_string(),
        revision: "abc123".to_string(),
        path_prefix: Some("coreml".to_string()),
        include_files: vec!["Encoder.mlmodelc/coremldata.bin".to_string()],
        license: None,
    };

    let url = video_creater_lib::transcription::acquisition::hugging_face_resolve_url(
        &source,
        "Encoder.mlmodelc/coremldata.bin",
    )
    .expect("resolve url");

    assert_eq!(
        url,
        "https://huggingface.co/org/model/resolve/abc123/coreml/Encoder.mlmodelc/coremldata.bin"
    );
}

#[test]
fn hugging_face_provider_validates_prefixed_required_files_against_remote_file_paths() {
    let source = ModelArtifactSource {
        provider: ModelArtifactProviderKind::HuggingFaceHub,
        repo_id: "org/model".to_string(),
        revision: "abc123".to_string(),
        path_prefix: Some("coreml".to_string()),
        include_files: vec![
            "config.json".to_string(),
            "Encoder.mlmodelc/coremldata.bin".to_string(),
        ],
        license: None,
    };
    let remote = video_creater_lib::transcription::acquisition::RemoteModelArtifact {
        repo_id: source.repo_id.clone(),
        revision: source.revision.clone(),
        file_paths: vec![
            "coreml/config.json".to_string(),
            "coreml/Encoder.mlmodelc/coremldata.bin".to_string(),
        ],
    };

    let result = video_creater_lib::transcription::acquisition::validate_remote_required_files(
        &source, &remote,
    );

    assert!(result.is_ok());
}

#[test]
fn hugging_face_provider_rejects_missing_required_source_file() {
    for (entry, present_file, missing_file) in [
        (
            parakeet_v3_coreml_catalog_entry(),
            "config.json",
            "Encoder.mlmodelc/coremldata.bin",
        ),
        (
            parakeet_v3_sherpa_onnx_catalog_entry(),
            "tokens.txt",
            "encoder.int8.onnx",
        ),
    ] {
        let source = entry.artifact_sources.first().expect("source");
        let remote = video_creater_lib::transcription::acquisition::RemoteModelArtifact {
            repo_id: source.repo_id.clone(),
            revision: source.revision.clone(),
            file_paths: vec![present_file.to_string()],
        };

        let result = video_creater_lib::transcription::acquisition::validate_remote_required_files(
            source, &remote,
        );

        let error = result.expect_err("missing required files should fail");
        assert!(error.to_string().contains(missing_file), "{error}");
    }
}

#[test]
fn hugging_face_provider_parses_model_api_response() {
    let json = serde_json::json!({
        "id": "FluidInference/parakeet-tdt-0.6b-v3-coreml",
        "sha": "aed02740059203c4a87495924f685de3722ae9ce",
        "siblings": [
            { "rfilename": "config.json" },
            { "rfilename": "parakeet_v3_vocab.json" },
            { "rfilename": "Encoder.mlmodelc/coremldata.bin" }
        ]
    });

    let remote =
        video_creater_lib::transcription::acquisition::parse_hugging_face_model_response(&json)
            .expect("parse response");

    assert_eq!(remote.repo_id, "FluidInference/parakeet-tdt-0.6b-v3-coreml");
    assert_eq!(remote.revision, "aed02740059203c4a87495924f685de3722ae9ce");
    assert_eq!(
        remote.file_paths,
        vec![
            "config.json".to_string(),
            "parakeet_v3_vocab.json".to_string(),
            "Encoder.mlmodelc/coremldata.bin".to_string()
        ]
    );
}

#[test]
fn hugging_face_provider_inspects_pinned_revision_endpoint_with_custom_api_base_url() {
    use std::io::{ErrorKind, Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::time::Duration;

    let listener = match TcpListener::bind("127.0.0.1:0") {
        Ok(listener) => listener,
        Err(error) if error.kind() == ErrorKind::PermissionDenied => return,
        Err(error) => panic!("bind local test server: {error}"),
    };
    let address = listener.local_addr().expect("local address");
    let (request_line_sender, request_line_receiver) = mpsc::channel();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("set read timeout");

        let mut request = Vec::new();
        let mut chunk = [0; 512];
        while !request.windows(4).any(|window| window == b"\r\n\r\n") {
            let read = stream.read(&mut chunk).expect("read request");
            assert_ne!(read, 0, "request ended before headers");
            request.extend_from_slice(&chunk[..read]);
        }
        let request = String::from_utf8_lossy(&request);
        let request_line = request.lines().next().expect("request line").to_string();
        request_line_sender
            .send(request_line)
            .expect("send request line");

        let body = serde_json::json!({
            "id": "FluidInference/parakeet-tdt-0.6b-v3-coreml",
            "sha": "aed02740059203c4a87495924f685de3722ae9ce",
            "siblings": [
                { "rfilename": "config.json" }
            ]
        })
        .to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write response");
        stream.flush().expect("flush response");
    });

    let entry = parakeet_v3_coreml_catalog_entry();
    let mut source = entry.artifact_sources.first().expect("source").clone();
    source.include_files = vec!["config.json".to_string()];
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .no_proxy()
        .build()
        .expect("client");
    let provider = HuggingFaceHubProvider::with_base_urls(
        client,
        format!("http://{}", address),
        "http://unused.local",
    );

    let remote = provider.inspect_remote(&source).expect("inspect source");

    let request_line = request_line_receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("observed request line");
    server.join().expect("server thread");
    assert!(request_line.starts_with(
        "GET /FluidInference/parakeet-tdt-0.6b-v3-coreml/revision/aed02740059203c4a87495924f685de3722ae9ce "
    ));
    assert!(remote.file_paths.contains(&"config.json".to_string()));
}

#[test]
fn hugging_face_provider_downloads_included_files_and_records_sha256() {
    use std::sync::Mutex;
    use video_creater_lib::transcription::acquisition::{
        HuggingFaceHubProvider, ModelArtifactProvider, ModelDownloadProgress,
    };

    struct RecordingProgress {
        completed_files: Mutex<Vec<u32>>,
    }

    impl ModelDownloadProgress for RecordingProgress {
        fn is_cancelled(
            &self,
        ) -> Result<bool, video_creater_lib::transcription::acquisition::ModelAcquisitionError>
        {
            Ok(false)
        }

        fn file_completed(
            &self,
            downloaded_files: u32,
        ) -> Result<(), video_creater_lib::transcription::acquisition::ModelAcquisitionError>
        {
            self.completed_files
                .lock()
                .expect("progress lock")
                .push(downloaded_files);
            Ok(())
        }
    }

    let source_dir = tempfile::tempdir().expect("source");
    let target_dir = tempfile::tempdir().expect("target");
    let encoder = source_dir
        .path()
        .join("resolve")
        .join("Encoder.mlmodelc")
        .join("coremldata.bin");
    std::fs::create_dir_all(encoder.parent().expect("parent")).expect("mkdir");
    std::fs::write(&encoder, b"encoder bytes").expect("write encoder");
    let config = source_dir.path().join("resolve").join("config.json");
    std::fs::write(&config, b"{\"model\":\"fixture\"}").expect("write config");

    let mut entry = parakeet_v3_catalog_entry();
    let mut source = entry.artifact_sources.remove(0);
    source.include_files = vec![
        "Encoder.mlmodelc/coremldata.bin".to_string(),
        "config.json".to_string(),
    ];

    let client = reqwest::blocking::Client::builder()
        .build()
        .expect("client");
    let provider = HuggingFaceHubProvider::with_base_urls(
        client,
        "http://127.0.0.1/unused",
        source_dir
            .path()
            .join("resolve")
            .to_string_lossy()
            .to_string(),
    );

    let progress = RecordingProgress {
        completed_files: Mutex::new(Vec::new()),
    };
    let artifact = provider
        .download(&source, target_dir.path(), &progress)
        .expect("download local fixture files");

    assert_eq!(artifact.files.len(), 2);
    assert_eq!(
        *progress.completed_files.lock().expect("progress lock"),
        vec![1, 2]
    );
    assert_eq!(
        std::fs::read(target_dir.path().join("Encoder.mlmodelc/coremldata.bin")).expect("read"),
        b"encoder bytes"
    );
    let encoder_file = artifact
        .files
        .iter()
        .find(|file| file.relative_path == "Encoder.mlmodelc/coremldata.bin")
        .expect("encoder file");
    assert_eq!(encoder_file.size_bytes, "encoder bytes".len() as u64);
    assert_eq!(
        encoder_file.sha256,
        "f0cf98ee8f826096ea85c67028b74638b2967d815b670df19414e6e10f03b87e"
    );
}

#[test]
fn hugging_face_provider_records_absolute_paths_for_relative_target_dir() {
    use video_creater_lib::transcription::acquisition::{
        HuggingFaceHubProvider, ModelAcquisitionError, ModelArtifactProvider, ModelDownloadProgress,
    };

    struct CurrentDirGuard {
        original: std::path::PathBuf,
    }

    impl Drop for CurrentDirGuard {
        fn drop(&mut self) {
            std::env::set_current_dir(&self.original).expect("restore cwd");
        }
    }

    struct NoCancel;
    impl ModelDownloadProgress for NoCancel {
        fn is_cancelled(&self) -> Result<bool, ModelAcquisitionError> {
            Ok(false)
        }

        fn file_completed(&self, _downloaded_files: u32) -> Result<(), ModelAcquisitionError> {
            Ok(())
        }
    }

    let root = tempfile::tempdir().expect("root");
    let source_base = root.path().join("source").join("resolve");
    let config = source_base.join("config.json");
    fs::create_dir_all(config.parent().expect("parent")).expect("mkdir");
    fs::write(&config, b"config").expect("write config");
    let source = ModelArtifactSource {
        provider: ModelArtifactProviderKind::HuggingFaceHub,
        repo_id: "org/model".to_string(),
        revision: "abc123".to_string(),
        path_prefix: None,
        include_files: vec!["config.json".to_string()],
        license: None,
    };
    let provider = HuggingFaceHubProvider::with_base_urls(
        reqwest::blocking::Client::new(),
        "http://127.0.0.1/unused",
        source_base.to_string_lossy().to_string(),
    );
    let _cwd_guard = CurrentDirGuard {
        original: std::env::current_dir().expect("current dir"),
    };
    std::env::set_current_dir(root.path()).expect("set cwd");

    let artifact = provider
        .download(&source, Path::new("target"), &NoCancel)
        .expect("download to relative target");

    let downloaded_file = artifact.files.first().expect("downloaded file");
    let expected_path = root
        .path()
        .join("target")
        .canonicalize()
        .expect("canonical target")
        .join("config.json");
    assert!(downloaded_file.absolute_path.is_absolute());
    assert_eq!(downloaded_file.absolute_path, expected_path);
}

#[test]
fn hugging_face_provider_downloads_local_files_with_path_prefix() {
    use video_creater_lib::transcription::acquisition::{
        HuggingFaceHubProvider, ModelArtifactProvider, ModelDownloadProgress,
    };

    struct NoCancel;
    impl ModelDownloadProgress for NoCancel {
        fn is_cancelled(
            &self,
        ) -> Result<bool, video_creater_lib::transcription::acquisition::ModelAcquisitionError>
        {
            Ok(false)
        }

        fn file_completed(
            &self,
            _downloaded_files: u32,
        ) -> Result<(), video_creater_lib::transcription::acquisition::ModelAcquisitionError>
        {
            Ok(())
        }
    }

    let source_dir = tempfile::tempdir().expect("source");
    let target_dir = tempfile::tempdir().expect("target");
    let config = source_dir
        .path()
        .join("resolve")
        .join("coreml")
        .join("config.json");
    std::fs::create_dir_all(config.parent().expect("parent")).expect("mkdir");
    std::fs::write(&config, b"prefixed config").expect("write config");
    let source = ModelArtifactSource {
        provider: ModelArtifactProviderKind::HuggingFaceHub,
        repo_id: "org/model".to_string(),
        revision: "abc123".to_string(),
        path_prefix: Some("coreml".to_string()),
        include_files: vec!["config.json".to_string()],
        license: None,
    };
    let provider = HuggingFaceHubProvider::with_base_urls(
        reqwest::blocking::Client::new(),
        "http://127.0.0.1/unused",
        source_dir
            .path()
            .join("resolve")
            .to_string_lossy()
            .to_string(),
    );

    let artifact = provider
        .download(&source, target_dir.path(), &NoCancel)
        .expect("download prefixed local fixture");

    assert_eq!(artifact.files.len(), 1);
    assert_eq!(
        std::fs::read(target_dir.path().join("config.json")).expect("read"),
        b"prefixed config"
    );
}

#[cfg(unix)]
#[test]
fn hugging_face_provider_replaces_stale_part_symlink_without_writing_outside_target() {
    use std::os::unix::fs::symlink;
    use video_creater_lib::transcription::acquisition::{
        HuggingFaceHubProvider, ModelAcquisitionError, ModelArtifactProvider, ModelDownloadProgress,
    };

    struct NoCancel;
    impl ModelDownloadProgress for NoCancel {
        fn is_cancelled(&self) -> Result<bool, ModelAcquisitionError> {
            Ok(false)
        }

        fn file_completed(&self, _downloaded_files: u32) -> Result<(), ModelAcquisitionError> {
            Ok(())
        }
    }

    let source_dir = tempfile::tempdir().expect("source");
    let target_dir = tempfile::tempdir().expect("target");
    let outside_dir = tempfile::tempdir().expect("outside");
    let config = source_dir.path().join("resolve").join("config.json");
    fs::create_dir_all(config.parent().expect("parent")).expect("mkdir");
    fs::write(&config, b"downloaded config").expect("write config");
    let outside_file = outside_dir.path().join("outside.bin");
    fs::write(&outside_file, b"do not overwrite").expect("write outside");
    symlink(&outside_file, target_dir.path().join("config.json.part")).expect("symlink part");
    let source = ModelArtifactSource {
        provider: ModelArtifactProviderKind::HuggingFaceHub,
        repo_id: "org/model".to_string(),
        revision: "abc123".to_string(),
        path_prefix: None,
        include_files: vec!["config.json".to_string()],
        license: None,
    };
    let provider = HuggingFaceHubProvider::with_base_urls(
        reqwest::blocking::Client::new(),
        "http://127.0.0.1/unused",
        source_dir
            .path()
            .join("resolve")
            .to_string_lossy()
            .to_string(),
    );

    provider
        .download(&source, target_dir.path(), &NoCancel)
        .expect("download local fixture");

    assert_eq!(
        fs::read(&outside_file).expect("read outside"),
        b"do not overwrite"
    );
    assert_eq!(
        fs::read(target_dir.path().join("config.json")).expect("read downloaded config"),
        b"downloaded config"
    );
    assert!(fs::symlink_metadata(target_dir.path().join("config.json"))
        .expect("config metadata")
        .file_type()
        .is_file());
}

#[test]
fn hugging_face_provider_download_cancelled_before_file_does_not_write_output() {
    use video_creater_lib::transcription::acquisition::{
        HuggingFaceHubProvider, ModelAcquisitionError, ModelArtifactProvider, ModelDownloadProgress,
    };

    struct CancelBeforeStart;
    impl ModelDownloadProgress for CancelBeforeStart {
        fn is_cancelled(&self) -> Result<bool, ModelAcquisitionError> {
            Ok(true)
        }

        fn file_completed(&self, _downloaded_files: u32) -> Result<(), ModelAcquisitionError> {
            Ok(())
        }
    }

    let source_dir = tempfile::tempdir().expect("source");
    let target_dir = tempfile::tempdir().expect("target");
    let config = source_dir.path().join("resolve").join("config.json");
    std::fs::create_dir_all(config.parent().expect("parent")).expect("mkdir");
    std::fs::write(&config, b"config").expect("write config");
    let source = ModelArtifactSource {
        provider: ModelArtifactProviderKind::HuggingFaceHub,
        repo_id: "org/model".to_string(),
        revision: "abc123".to_string(),
        path_prefix: None,
        include_files: vec!["config.json".to_string()],
        license: None,
    };
    let provider = HuggingFaceHubProvider::with_base_urls(
        reqwest::blocking::Client::new(),
        "http://127.0.0.1/unused",
        source_dir
            .path()
            .join("resolve")
            .to_string_lossy()
            .to_string(),
    );

    let error = provider
        .download(&source, target_dir.path(), &CancelBeforeStart)
        .expect_err("cancelled before file starts");

    assert!(matches!(error, ModelAcquisitionError::Cancelled));
    assert!(!target_dir.path().join("config.json").exists());
}

#[test]
fn hugging_face_provider_cleans_part_when_http_stream_is_cancelled() {
    use std::io::{ErrorKind, Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use video_creater_lib::transcription::acquisition::{
        HuggingFaceHubProvider, ModelAcquisitionError, ModelArtifactProvider, ModelDownloadProgress,
    };

    struct CancelDuringStream {
        checks: AtomicUsize,
    }

    impl ModelDownloadProgress for CancelDuringStream {
        fn is_cancelled(&self) -> Result<bool, ModelAcquisitionError> {
            Ok(self.checks.fetch_add(1, Ordering::SeqCst) > 0)
        }

        fn file_completed(&self, _downloaded_files: u32) -> Result<(), ModelAcquisitionError> {
            Ok(())
        }
    }

    let listener = match TcpListener::bind("127.0.0.1:0") {
        Ok(listener) => listener,
        Err(error) if error.kind() == ErrorKind::PermissionDenied => return,
        Err(error) => panic!("bind local test server: {error}"),
    };
    let address = listener.local_addr().expect("local address");
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("set read timeout");

        let mut request = Vec::new();
        let mut chunk = [0; 512];
        while !request.windows(4).any(|window| window == b"\r\n\r\n") {
            let read = stream.read(&mut chunk).expect("read request");
            assert_ne!(read, 0, "request ended before headers");
            request.extend_from_slice(&chunk[..read]);
        }

        let body = vec![b'a'; 128 * 1024];
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream
            .write_all(response.as_bytes())
            .expect("write response headers");
        let _ = stream.write_all(&body);
        let _ = stream.flush();
    });

    let target_dir = tempfile::tempdir().expect("target");
    let source = ModelArtifactSource {
        provider: ModelArtifactProviderKind::HuggingFaceHub,
        repo_id: "org/model".to_string(),
        revision: "abc123".to_string(),
        path_prefix: None,
        include_files: vec!["config.json".to_string()],
        license: None,
    };
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .no_proxy()
        .build()
        .expect("client");
    let provider = HuggingFaceHubProvider::with_base_urls(
        client,
        "http://127.0.0.1/unused",
        format!("http://{}", address),
    );

    let error = provider
        .download(
            &source,
            target_dir.path(),
            &CancelDuringStream {
                checks: AtomicUsize::new(0),
            },
        )
        .expect_err("cancelled during HTTP stream");

    server.join().expect("server thread");
    assert!(matches!(error, ModelAcquisitionError::Cancelled));
    assert!(!target_dir.path().join("config.json.part").exists());
    assert!(!target_dir.path().join("config.json").exists());
}

#[test]
fn hugging_face_provider_rejects_drive_prefix_looking_include_file_on_all_platforms() {
    use video_creater_lib::transcription::acquisition::{
        HuggingFaceHubProvider, ModelAcquisitionError, ModelArtifactProvider, ModelDownloadProgress,
    };

    struct NoCancel;
    impl ModelDownloadProgress for NoCancel {
        fn is_cancelled(&self) -> Result<bool, ModelAcquisitionError> {
            Ok(false)
        }

        fn file_completed(&self, _downloaded_files: u32) -> Result<(), ModelAcquisitionError> {
            Ok(())
        }
    }

    for (path_prefix, include_file, unexpected_target) in [
        (None, "C:/outside.bin", "C:/outside.bin"),
        (None, "C:outside.bin", "C:outside.bin"),
        (Some("C:prefix"), "outside.bin", "outside.bin"),
    ] {
        let root = tempfile::tempdir().expect("root");
        let source_base = root.path().join("source").join("resolve");
        let target_dir = root.path().join("target");
        fs::create_dir_all(source_base.join("C:")).expect("mkdir source");
        fs::create_dir_all(source_base.join("C:prefix")).expect("mkdir prefixed source");
        fs::create_dir_all(&target_dir).expect("mkdir target");
        fs::write(source_base.join("C:").join("outside.bin"), b"drive path")
            .expect("write drive source");
        fs::write(source_base.join("C:outside.bin"), b"drive-relative path")
            .expect("write drive-relative source");
        fs::write(
            source_base.join("C:prefix").join("outside.bin"),
            b"prefixed path",
        )
        .expect("write prefixed source");
        let source = ModelArtifactSource {
            provider: ModelArtifactProviderKind::HuggingFaceHub,
            repo_id: "org/model".to_string(),
            revision: "abc123".to_string(),
            path_prefix: path_prefix.map(str::to_string),
            include_files: vec![include_file.to_string()],
            license: None,
        };
        let provider = HuggingFaceHubProvider::with_base_urls(
            reqwest::blocking::Client::new(),
            "http://127.0.0.1/unused",
            source_base.to_string_lossy().to_string(),
        );

        let error = provider
            .download(&source, &target_dir, &NoCancel)
            .expect_err("drive-looking artifact path should fail");

        assert!(matches!(error, ModelAcquisitionError::Download(_)));
        assert!(!target_dir.join(unexpected_target).exists());
    }
}

#[test]
fn hugging_face_provider_rejects_unsafe_include_file_without_writing_outside_target() {
    use video_creater_lib::transcription::acquisition::{
        HuggingFaceHubProvider, ModelAcquisitionError, ModelArtifactProvider, ModelDownloadProgress,
    };

    struct NoCancel;
    impl ModelDownloadProgress for NoCancel {
        fn is_cancelled(&self) -> Result<bool, ModelAcquisitionError> {
            Ok(false)
        }

        fn file_completed(&self, _downloaded_files: u32) -> Result<(), ModelAcquisitionError> {
            Ok(())
        }
    }

    let root = tempfile::tempdir().expect("root");
    let source_base = root.path().join("source").join("resolve");
    let target_dir = root.path().join("target");
    fs::create_dir_all(&source_base).expect("mkdir source");
    fs::create_dir_all(&target_dir).expect("mkdir target");
    fs::write(root.path().join("source").join("outside.bin"), b"outside").expect("write source");
    let source = ModelArtifactSource {
        provider: ModelArtifactProviderKind::HuggingFaceHub,
        repo_id: "org/model".to_string(),
        revision: "abc123".to_string(),
        path_prefix: None,
        include_files: vec!["../outside.bin".to_string()],
        license: None,
    };
    let provider = HuggingFaceHubProvider::with_base_urls(
        reqwest::blocking::Client::new(),
        "http://127.0.0.1/unused",
        source_base.to_string_lossy().to_string(),
    );

    let error = provider
        .download(&source, &target_dir, &NoCancel)
        .expect_err("unsafe include path should fail");

    assert!(matches!(error, ModelAcquisitionError::Download(_)));
    assert!(!root.path().join("outside.bin").exists());
}

#[test]
fn hugging_face_provider_rejects_empty_absolute_dot_and_backslash_include_files() {
    use video_creater_lib::transcription::acquisition::{
        HuggingFaceHubProvider, ModelAcquisitionError, ModelArtifactProvider, ModelDownloadProgress,
    };

    struct NoCancel;
    impl ModelDownloadProgress for NoCancel {
        fn is_cancelled(&self) -> Result<bool, ModelAcquisitionError> {
            Ok(false)
        }

        fn file_completed(&self, _downloaded_files: u32) -> Result<(), ModelAcquisitionError> {
            Ok(())
        }
    }

    for unsafe_include_file in ["", "/absolute.bin", ".", "nested\\outside.bin"] {
        let root = tempfile::tempdir().expect("root");
        let source_base = root.path().join("source").join("resolve");
        let target_dir = root.path().join("target");
        fs::create_dir_all(&source_base).expect("mkdir source");
        fs::create_dir_all(&target_dir).expect("mkdir target");
        let source = ModelArtifactSource {
            provider: ModelArtifactProviderKind::HuggingFaceHub,
            repo_id: "org/model".to_string(),
            revision: "abc123".to_string(),
            path_prefix: None,
            include_files: vec![unsafe_include_file.to_string()],
            license: None,
        };
        let provider = HuggingFaceHubProvider::with_base_urls(
            reqwest::blocking::Client::new(),
            "http://127.0.0.1/unused",
            source_base.to_string_lossy().to_string(),
        );

        let error = provider
            .download(&source, &target_dir, &NoCancel)
            .expect_err(&format!(
                "unsafe include path {unsafe_include_file:?} should fail"
            ));

        assert!(matches!(error, ModelAcquisitionError::Download(_)));
        assert!(
            fs::read_dir(&target_dir)
                .expect("read target dir")
                .next()
                .is_none(),
            "unsafe include path {unsafe_include_file:?} wrote into target"
        );
    }
}

#[test]
fn hugging_face_provider_rejects_embedded_dot_include_file_without_writing_target() {
    use video_creater_lib::transcription::acquisition::{
        HuggingFaceHubProvider, ModelAcquisitionError, ModelArtifactProvider, ModelDownloadProgress,
    };

    struct NoCancel;
    impl ModelDownloadProgress for NoCancel {
        fn is_cancelled(&self) -> Result<bool, ModelAcquisitionError> {
            Ok(false)
        }

        fn file_completed(&self, _downloaded_files: u32) -> Result<(), ModelAcquisitionError> {
            Ok(())
        }
    }

    let root = tempfile::tempdir().expect("root");
    let source_base = root.path().join("source").join("resolve");
    let target_dir = root.path().join("target");
    let source_file = source_base.join("nested").join("file.bin");
    fs::create_dir_all(source_file.parent().expect("parent")).expect("mkdir source");
    fs::create_dir_all(&target_dir).expect("mkdir target");
    fs::write(&source_file, b"normalized source").expect("write source");
    let source = ModelArtifactSource {
        provider: ModelArtifactProviderKind::HuggingFaceHub,
        repo_id: "org/model".to_string(),
        revision: "abc123".to_string(),
        path_prefix: None,
        include_files: vec!["nested/./file.bin".to_string()],
        license: None,
    };
    let provider = HuggingFaceHubProvider::with_base_urls(
        reqwest::blocking::Client::new(),
        "http://127.0.0.1/unused",
        source_base.to_string_lossy().to_string(),
    );

    let error = provider
        .download(&source, &target_dir, &NoCancel)
        .expect_err("embedded dot include path should fail");

    assert!(matches!(error, ModelAcquisitionError::Download(_)));
    assert!(
        fs::read_dir(&target_dir)
            .expect("read target dir")
            .next()
            .is_none(),
        "embedded dot include path wrote into target"
    );
}

#[cfg(unix)]
#[test]
fn hugging_face_provider_rejects_target_escape_through_symlink_parent() {
    use std::os::unix::fs::symlink;
    use video_creater_lib::transcription::acquisition::{
        HuggingFaceHubProvider, ModelAcquisitionError, ModelArtifactProvider, ModelDownloadProgress,
    };

    struct NoCancel;
    impl ModelDownloadProgress for NoCancel {
        fn is_cancelled(&self) -> Result<bool, ModelAcquisitionError> {
            Ok(false)
        }

        fn file_completed(&self, _downloaded_files: u32) -> Result<(), ModelAcquisitionError> {
            Ok(())
        }
    }

    let root = tempfile::tempdir().expect("root");
    let source_base = root.path().join("source").join("resolve");
    let target_dir = root.path().join("target");
    let outside_dir = root.path().join("outside");
    let source_file = source_base.join("linked").join("sub").join("escaped.bin");
    fs::create_dir_all(source_file.parent().expect("parent")).expect("mkdir source");
    fs::write(&source_file, b"escaped").expect("write source");
    fs::create_dir_all(&target_dir).expect("mkdir target");
    fs::create_dir_all(&outside_dir).expect("mkdir outside");
    symlink(&outside_dir, target_dir.join("linked")).expect("symlink parent");
    let source = ModelArtifactSource {
        provider: ModelArtifactProviderKind::HuggingFaceHub,
        repo_id: "org/model".to_string(),
        revision: "abc123".to_string(),
        path_prefix: None,
        include_files: vec!["linked/sub/escaped.bin".to_string()],
        license: None,
    };
    let provider = HuggingFaceHubProvider::with_base_urls(
        reqwest::blocking::Client::new(),
        "http://127.0.0.1/unused",
        source_base.to_string_lossy().to_string(),
    );

    let error = provider
        .download(&source, &target_dir, &NoCancel)
        .expect_err("symlink parent should escape target containment");

    assert!(matches!(error, ModelAcquisitionError::Download(_)));
    assert!(!outside_dir.join("sub").exists());
    assert!(!outside_dir.join("sub").join("escaped.bin").exists());
    assert!(!target_dir
        .join("linked")
        .join("sub")
        .join("escaped.bin")
        .exists());
}

#[test]
fn hugging_face_provider_rejects_unsafe_path_prefix_for_local_downloads() {
    use video_creater_lib::transcription::acquisition::{
        HuggingFaceHubProvider, ModelAcquisitionError, ModelArtifactProvider, ModelDownloadProgress,
    };

    struct NoCancel;
    impl ModelDownloadProgress for NoCancel {
        fn is_cancelled(&self) -> Result<bool, ModelAcquisitionError> {
            Ok(false)
        }

        fn file_completed(&self, _downloaded_files: u32) -> Result<(), ModelAcquisitionError> {
            Ok(())
        }
    }

    let root = tempfile::tempdir().expect("root");
    let source_base = root.path().join("source").join("resolve");
    let target_dir = root.path().join("target");
    fs::create_dir_all(&source_base).expect("mkdir source");
    fs::create_dir_all(&target_dir).expect("mkdir target");
    let outside_config = root
        .path()
        .join("source")
        .join("outside")
        .join("config.json");
    fs::create_dir_all(outside_config.parent().expect("parent")).expect("mkdir outside");
    fs::write(&outside_config, b"outside").expect("write source");
    let source = ModelArtifactSource {
        provider: ModelArtifactProviderKind::HuggingFaceHub,
        repo_id: "org/model".to_string(),
        revision: "abc123".to_string(),
        path_prefix: Some("../outside".to_string()),
        include_files: vec!["config.json".to_string()],
        license: None,
    };
    let provider = HuggingFaceHubProvider::with_base_urls(
        reqwest::blocking::Client::new(),
        "http://127.0.0.1/unused",
        source_base.to_string_lossy().to_string(),
    );

    let error = provider
        .download(&source, &target_dir, &NoCancel)
        .expect_err("unsafe path prefix should fail");

    assert!(matches!(error, ModelAcquisitionError::Download(_)));
    assert!(!target_dir.join("config.json").exists());
}

#[test]
fn hugging_face_provider_cleans_part_when_cancelled_before_rename() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use video_creater_lib::transcription::acquisition::{
        HuggingFaceHubProvider, ModelAcquisitionError, ModelArtifactProvider, ModelDownloadProgress,
    };

    struct CancelBeforeRename {
        checks: AtomicUsize,
    }

    impl ModelDownloadProgress for CancelBeforeRename {
        fn is_cancelled(&self) -> Result<bool, ModelAcquisitionError> {
            Ok(self.checks.fetch_add(1, Ordering::SeqCst) > 0)
        }

        fn file_completed(&self, _downloaded_files: u32) -> Result<(), ModelAcquisitionError> {
            Ok(())
        }
    }

    let source_dir = tempfile::tempdir().expect("source");
    let target_dir = tempfile::tempdir().expect("target");
    let config = source_dir.path().join("resolve").join("config.json");
    fs::create_dir_all(config.parent().expect("parent")).expect("mkdir");
    fs::write(&config, b"config").expect("write config");
    let source = ModelArtifactSource {
        provider: ModelArtifactProviderKind::HuggingFaceHub,
        repo_id: "org/model".to_string(),
        revision: "abc123".to_string(),
        path_prefix: None,
        include_files: vec!["config.json".to_string()],
        license: None,
    };
    let provider = HuggingFaceHubProvider::with_base_urls(
        reqwest::blocking::Client::new(),
        "http://127.0.0.1/unused",
        source_dir
            .path()
            .join("resolve")
            .to_string_lossy()
            .to_string(),
    );

    let error = provider
        .download(
            &source,
            target_dir.path(),
            &CancelBeforeRename {
                checks: AtomicUsize::new(0),
            },
        )
        .expect_err("cancelled before rename");

    assert!(matches!(error, ModelAcquisitionError::Cancelled));
    assert!(!target_dir.path().join("config.json").exists());
    assert!(!target_dir.path().join("config.json.part").exists());
}

#[test]
fn hugging_face_provider_cleans_part_when_progress_errors_before_rename() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use video_creater_lib::transcription::acquisition::{
        HuggingFaceHubProvider, ModelAcquisitionError, ModelArtifactProvider, ModelDownloadProgress,
    };

    struct ErrorBeforeRename {
        checks: AtomicUsize,
    }

    impl ModelDownloadProgress for ErrorBeforeRename {
        fn is_cancelled(&self) -> Result<bool, ModelAcquisitionError> {
            if self.checks.fetch_add(1, Ordering::SeqCst) == 0 {
                Ok(false)
            } else {
                Err(ModelAcquisitionError::Download(
                    "progress failed before rename".to_string(),
                ))
            }
        }

        fn file_completed(&self, _downloaded_files: u32) -> Result<(), ModelAcquisitionError> {
            Ok(())
        }
    }

    let source_dir = tempfile::tempdir().expect("source");
    let target_dir = tempfile::tempdir().expect("target");
    let config = source_dir.path().join("resolve").join("config.json");
    fs::create_dir_all(config.parent().expect("parent")).expect("mkdir");
    fs::write(&config, b"config").expect("write config");
    let source = ModelArtifactSource {
        provider: ModelArtifactProviderKind::HuggingFaceHub,
        repo_id: "org/model".to_string(),
        revision: "abc123".to_string(),
        path_prefix: None,
        include_files: vec!["config.json".to_string()],
        license: None,
    };
    let provider = HuggingFaceHubProvider::with_base_urls(
        reqwest::blocking::Client::new(),
        "http://127.0.0.1/unused",
        source_dir
            .path()
            .join("resolve")
            .to_string_lossy()
            .to_string(),
    );

    let error = provider
        .download(
            &source,
            target_dir.path(),
            &ErrorBeforeRename {
                checks: AtomicUsize::new(0),
            },
        )
        .expect_err("progress error before rename");

    assert!(matches!(
        error,
        ModelAcquisitionError::Download(message) if message == "progress failed before rename"
    ));
    assert!(!target_dir.path().join("config.json").exists());
    assert!(!target_dir.path().join("config.json.part").exists());
}

#[test]
fn hugging_face_provider_removes_part_when_local_copy_fails() {
    use video_creater_lib::transcription::acquisition::{
        HuggingFaceHubProvider, ModelAcquisitionError, ModelArtifactProvider, ModelDownloadProgress,
    };

    struct NoCancel;
    impl ModelDownloadProgress for NoCancel {
        fn is_cancelled(&self) -> Result<bool, ModelAcquisitionError> {
            Ok(false)
        }

        fn file_completed(&self, _downloaded_files: u32) -> Result<(), ModelAcquisitionError> {
            Ok(())
        }
    }

    let source_dir = tempfile::tempdir().expect("source");
    let target_dir = tempfile::tempdir().expect("target");
    let source = ModelArtifactSource {
        provider: ModelArtifactProviderKind::HuggingFaceHub,
        repo_id: "org/model".to_string(),
        revision: "abc123".to_string(),
        path_prefix: None,
        include_files: vec!["config.json".to_string()],
        license: None,
    };
    let part_path = target_dir.path().join("config.json.part");
    fs::write(&part_path, b"stale partial").expect("write stale part");
    let provider = HuggingFaceHubProvider::with_base_urls(
        reqwest::blocking::Client::new(),
        "http://127.0.0.1/unused",
        source_dir
            .path()
            .join("resolve")
            .to_string_lossy()
            .to_string(),
    );

    let error = provider
        .download(&source, target_dir.path(), &NoCancel)
        .expect_err("missing local source file");

    assert!(matches!(error, ModelAcquisitionError::Download(_)));
    assert!(!part_path.exists());
    assert!(!target_dir.path().join("config.json").exists());
}

#[test]
fn hugging_face_provider_can_inspect_real_parakeet_source_when_network_enabled() {
    if std::env::var("VIDEO_CREATER_TEST_HF_MODEL_METADATA")
        .ok()
        .as_deref()
        != Some("1")
    {
        eprintln!(
            "skipping Hugging Face metadata test: set VIDEO_CREATER_TEST_HF_MODEL_METADATA=1"
        );
        return;
    }

    use video_creater_lib::transcription::acquisition::{
        HuggingFaceHubProvider, ModelArtifactProvider,
    };

    let entry = parakeet_v3_catalog_entry();
    let source = entry.artifact_sources.first().expect("source");
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("client");
    let provider = HuggingFaceHubProvider::new(client);

    let remote = provider
        .inspect_remote(source)
        .expect("inspect real HF source");

    assert_eq!(remote.repo_id, source.repo_id);
    assert_eq!(remote.revision, source.revision);
    assert!(remote.file_paths.contains(&"config.json".to_string()));
    assert!(remote
        .file_paths
        .contains(&"JointDecisionv3.mlmodelc/coremldata.bin".to_string()));
}

#[derive(Debug)]
struct TestRuntime {
    capability: RuntimeCapability,
    supported: bool,
}

impl TranscriptionRuntime for TestRuntime {
    fn id(&self) -> &'static str {
        "test_runtime"
    }

    fn supports(&self, _model: &TranscriptionModelCatalogEntry) -> bool {
        self.supported
    }

    fn probe(&self, _model: &TranscriptionModelCatalogEntry) -> RuntimeCapability {
        self.capability.clone()
    }
}

#[test]
fn runtime_selection_uses_coreml_when_model_is_ready_on_macos() {
    let model = parakeet_v3_catalog_entry();
    let runtime = TestRuntime {
        capability: RuntimeCapability::Ready,
        supported: true,
    };

    let selection = select_transcription_runtime(&model, &runtime);

    assert_eq!(selection, RuntimeSelection::Native);
}

#[test]
fn runtime_selection_reports_unsupported_platform() {
    let model = parakeet_v3_catalog_entry();
    let runtime = TestRuntime {
        capability: RuntimeCapability::UnsupportedPlatform,
        supported: true,
    };

    let selection = select_transcription_runtime(&model, &runtime);

    assert_eq!(selection, RuntimeSelection::UnsupportedPlatform);
}

#[test]
fn coreml_unsupported_platform_diagnostic_names_runtime_requirements() {
    let diagnostic = coreml_unsupported_platform_diagnostic();

    assert!(diagnostic.contains("macOS"));
    assert!(diagnostic.contains("native helper"));
    assert!(diagnostic.contains("Core ML Parakeet model"));
}

#[test]
fn runtime_selection_reports_unavailable_when_coreml_cannot_load() {
    let model = parakeet_v3_catalog_entry();
    let runtime = TestRuntime {
        capability: RuntimeCapability::Unavailable,
        supported: true,
    };

    let selection = select_transcription_runtime(&model, &runtime);

    assert_eq!(selection, RuntimeSelection::Unavailable);
}

#[test]
fn runtime_registry_is_send_and_sync_for_shared_worker_state() {
    fn assert_send_sync<T: Send + Sync>() {}

    assert_send_sync::<TranscriptionRuntimeRegistry>();
}

#[test]
fn installed_model_uses_runtime_artifact_format_descriptor() {
    for (model, expected) in [
        (parakeet_v3_coreml_catalog_entry(), "core_ml_bundle"),
        (
            parakeet_v3_sherpa_onnx_catalog_entry(),
            "sherpa_onnx_transducer",
        ),
    ] {
        let artifact_format = ModelArtifactFormat::from(model.artifact_format);
        let json = serde_json::to_value(&artifact_format).expect("serialize artifact format");

        assert_eq!(json, serde_json::json!(expected));
    }
}

#[test]
fn runtime_registry_selects_transcription_runtime_by_id_and_modality() {
    let model = parakeet_v3_catalog_entry();
    let installed = video_creater_lib::transcription::runtime::InstalledModel {
        model_id: model.id.to_string(),
        model_dir: std::path::PathBuf::from("/tmp/model"),
        artifact_format: ModelArtifactFormat::from(model.artifact_format),
        modality: video_creater_lib::transcription::runtime::ModelModality::Transcription,
        runtime_family: "fluid_audio_coreml".to_string(),
    };
    let registry =
        video_creater_lib::transcription::runtime::TranscriptionRuntimeRegistry::new(vec![
            Box::new(
                video_creater_lib::transcription::runtime::FixtureTranscriptionRuntime {
                    runtime_id: "fluid_audio_coreml",
                    capability: RuntimeCapability::Ready,
                    supported: true,
                    tokens: vec![TranscriptToken {
                        token: "Hello".to_string(),
                        start: 0.0,
                        end: 0.5,
                        confidence: Some(0.9),
                    }],
                },
            ),
        ]);

    let selected = registry
        .select("fluid_audio_coreml", &installed)
        .expect("selected runtime");

    assert_eq!(selected.runtime_id(), "fluid_audio_coreml");
}

#[test]
fn runtime_registry_rejects_transcription_runtime_that_does_not_support_installed_model() {
    let model = parakeet_v3_catalog_entry();
    let installed = video_creater_lib::transcription::runtime::InstalledModel {
        model_id: model.id.to_string(),
        model_dir: std::path::PathBuf::from("/tmp/model"),
        artifact_format: ModelArtifactFormat::from(model.artifact_format),
        modality: video_creater_lib::transcription::runtime::ModelModality::Transcription,
        runtime_family: "fluid_audio_coreml".to_string(),
    };
    let registry =
        video_creater_lib::transcription::runtime::TranscriptionRuntimeRegistry::new(vec![
            Box::new(
                video_creater_lib::transcription::runtime::FixtureTranscriptionRuntime {
                    runtime_id: "fluid_audio_coreml",
                    capability: RuntimeCapability::Ready,
                    supported: false,
                    tokens: Vec::new(),
                },
            ),
        ]);

    let error = registry
        .select("fluid_audio_coreml", &installed)
        .expect_err("unsupported model should be rejected");

    assert!(error.to_string().contains("does not support"));
}

#[test]
fn fluid_audio_helper_response_converts_words_to_runtime_tokens() {
    let output = video_creater_lib::transcription::fluidaudio::runtime_output_from_helper_json(
        "nvidia/parakeet-tdt-0.6b-v3",
        serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 1,
            "runtimeId": FLUID_AUDIO_COREML_RUNTIME_ID,
            "modelId": "nvidia/parakeet-tdt-0.6b-v3",
            "text": "Hello world",
            "durationSeconds": 1.2,
            "processingSeconds": 0.4,
            "words": [
                {
                    "text": "Hello",
                    "startSeconds": 0.1,
                    "endSeconds": 0.4,
                    "confidence": 0.98
                },
                {
                    "text": "   ",
                    "startSeconds": 0.4,
                    "endSeconds": 0.5,
                    "confidence": 0.5
                },
                {
                    "text": "bad",
                    "startSeconds": 0.8,
                    "endSeconds": 0.7,
                    "confidence": 0.4
                },
                {
                    "text": "world",
                    "startSeconds": 0.5,
                    "endSeconds": 1.0,
                    "confidence": 0.91
                }
            ]
        }))
        .expect("helper response JSON")
        .as_slice(),
    )
    .expect("runtime output");

    assert_eq!(output.runtime_id, FLUID_AUDIO_COREML_RUNTIME_ID);
    assert_eq!(output.model_id, "nvidia/parakeet-tdt-0.6b-v3");
    assert_eq!(
        output.tokens,
        vec![
            TranscriptToken {
                token: "Hello".to_string(),
                start: 0.1,
                end: 0.4,
                confidence: Some(0.98),
            },
            TranscriptToken {
                token: "world".to_string(),
                start: 0.5,
                end: 1.0,
                confidence: Some(0.91),
            }
        ]
    );
}

#[test]
fn fluid_audio_helper_response_rejects_empty_words() {
    let error = video_creater_lib::transcription::fluidaudio::runtime_output_from_helper_json(
        "nvidia/parakeet-tdt-0.6b-v3",
        serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 1,
            "runtimeId": FLUID_AUDIO_COREML_RUNTIME_ID,
            "modelId": "nvidia/parakeet-tdt-0.6b-v3",
            "text": "",
            "durationSeconds": 0.0,
            "processingSeconds": 0.1,
            "words": [
                {
                    "text": " ",
                    "startSeconds": 0.0,
                    "endSeconds": 0.1,
                    "confidence": 0.5
                },
                {
                    "text": "bad",
                    "startSeconds": 0.4,
                    "endSeconds": 0.2,
                    "confidence": 0.5
                }
            ]
        }))
        .expect("helper response JSON")
        .as_slice(),
    )
    .expect_err("empty usable words should be rejected");

    assert!(error.to_string().contains("no usable words"));
}

#[test]
fn fluid_audio_helper_response_rejects_error_json() {
    let error = video_creater_lib::transcription::fluidaudio::runtime_output_from_helper_json(
        "nvidia/parakeet-tdt-0.6b-v3",
        serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 1,
            "error": "model failed inspection"
        }))
        .expect("helper error JSON")
        .as_slice(),
    )
    .expect_err("helper error response should be rejected");

    assert!(error.to_string().contains("model failed inspection"));
}

#[test]
fn fluid_audio_helper_response_rejects_error_json_schema_mismatch() {
    let error = video_creater_lib::transcription::fluidaudio::runtime_output_from_helper_json(
        "nvidia/parakeet-tdt-0.6b-v3",
        serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 2,
            "error": "model failed inspection"
        }))
        .expect("helper error JSON")
        .as_slice(),
    )
    .expect_err("helper error response with mismatched schema should be rejected");

    let message = error.to_string();
    assert!(message.contains("schemaVersion mismatch"));
    assert!(message.contains("expected 1, found 2"));
    assert!(!message.contains("model failed inspection"));
}

#[test]
fn fluid_audio_runtime_stderr_summary_truncates_utf8_safely() {
    let stderr = "é".repeat(260);

    let summary = video_creater_lib::transcription::fluidaudio::concise_stderr(stderr.as_bytes());

    assert!(summary.ends_with("..."));
    assert!(summary.len() <= 515);
    assert!(summary.is_char_boundary(summary.len() - 3));
}

#[test]
fn fluid_audio_runtime_support_rejects_wrong_runtime_family_artifact_and_modality() {
    let root = tempfile::tempdir().expect("root");
    let model = parakeet_v3_catalog_entry();
    create_fake_coreml_parakeet_tree(root.path());
    fs::write(
        root.path().join(COREML_INSPECTION_FILE_NAME),
        serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 1,
            "modelId": model.id,
            "runtimeId": FLUID_AUDIO_COREML_RUNTIME_ID,
            "bundles": []
        }))
        .expect("inspection report JSON"),
    )
    .expect("write inspection report");
    let runtime = video_creater_lib::transcription::fluidaudio::FluidAudioCoreMlRuntime::new(
        std::path::PathBuf::from("/tmp/helper"),
    );
    let mut installed = video_creater_lib::transcription::runtime::InstalledModel {
        model_id: model.id.to_string(),
        model_dir: root.path().join("metadata-only-model"),
        artifact_format: ModelArtifactFormat::from(TranscriptionModelArtifactFormat::CoreMlBundle),
        modality: ModelModality::Transcription,
        runtime_family: FLUID_AUDIO_COREML_RUNTIME_ID.to_string(),
    };

    assert!(runtime.supports_installed_model(&installed));

    installed.runtime_family = "other_runtime".to_string();
    assert!(!runtime.supports_installed_model(&installed));

    installed.runtime_family = FLUID_AUDIO_COREML_RUNTIME_ID.to_string();
    installed.artifact_format = ModelArtifactFormat("other_artifact".to_string());
    assert!(!runtime.supports_installed_model(&installed));

    installed.artifact_format =
        ModelArtifactFormat::from(TranscriptionModelArtifactFormat::CoreMlBundle);
    installed.modality = ModelModality::ImageGeneration;
    assert!(!runtime.supports_installed_model(&installed));
}

#[cfg(target_os = "macos")]
#[test]
fn fluid_audio_runtime_probe_installed_model_ready_when_helper_and_model_artifacts_exist() {
    let root = tempfile::tempdir().expect("root");
    let helper_path = root.path().join("video-creater-fluidaudio-transcribe");
    let model_dir = root.path().join("model");
    fs::write(&helper_path, b"fake helper").expect("helper file");
    chmod(&helper_path, 0o755);
    create_fake_coreml_parakeet_tree(&model_dir);
    write_fake_coreml_inspection_report(&model_dir);
    let runtime =
        video_creater_lib::transcription::fluidaudio::FluidAudioCoreMlRuntime::new(helper_path);
    let installed = fluid_audio_installed_model(&model_dir);

    assert_eq!(
        runtime.probe_installed_model(&installed),
        RuntimeCapability::Ready
    );
}

#[cfg(target_os = "macos")]
#[test]
fn fluid_audio_runtime_probe_installed_model_unavailable_when_helper_is_not_executable() {
    let root = tempfile::tempdir().expect("root");
    let helper_path = root.path().join("video-creater-fluidaudio-transcribe");
    let model_dir = root.path().join("model");
    fs::write(&helper_path, b"fake helper").expect("helper file");
    chmod(&helper_path, 0o644);
    create_fake_coreml_parakeet_tree(&model_dir);
    write_fake_coreml_inspection_report(&model_dir);
    let runtime =
        video_creater_lib::transcription::fluidaudio::FluidAudioCoreMlRuntime::new(helper_path);
    let installed = fluid_audio_installed_model(&model_dir);

    assert_eq!(
        runtime.probe_installed_model(&installed),
        RuntimeCapability::Unavailable
    );
}

#[cfg(target_os = "macos")]
#[test]
fn fluid_audio_runtime_probe_installed_model_unavailable_when_helper_missing_or_model_incomplete() {
    let root = tempfile::tempdir().expect("root");
    let helper_path = root.path().join("video-creater-fluidaudio-transcribe");
    let model_dir = root.path().join("model");
    create_fake_coreml_parakeet_tree(&model_dir);
    write_fake_coreml_inspection_report(&model_dir);
    let runtime = video_creater_lib::transcription::fluidaudio::FluidAudioCoreMlRuntime::new(
        helper_path.clone(),
    );
    let installed = fluid_audio_installed_model(&model_dir);

    assert_eq!(
        runtime.probe_installed_model(&installed),
        RuntimeCapability::Unavailable
    );
    assert!(runtime
        .readiness_diagnostic(&installed, &RuntimeCapability::Unavailable)
        .expect("helper readiness diagnostic")
        .contains("helper executable is unavailable"));

    fs::write(&helper_path, b"fake helper").expect("helper file");
    chmod(&helper_path, 0o755);
    fs::remove_dir_all(model_dir.join("Encoder.mlmodelc")).expect("remove required bundle");

    assert_eq!(
        runtime.probe_installed_model(&installed),
        RuntimeCapability::Unavailable
    );
    let diagnostic = runtime
        .readiness_diagnostic(&installed, &RuntimeCapability::Unavailable)
        .expect("missing bundle readiness diagnostic");
    assert!(diagnostic.contains("missing Core ML bundle"));
    assert!(diagnostic.contains("Encoder.mlmodelc"));
}

#[cfg(target_os = "macos")]
#[test]
fn fluid_audio_runtime_probe_installed_model_reports_missing_inspection_detail() {
    let root = tempfile::tempdir().expect("root");
    let helper_path = root.path().join("video-creater-fluidaudio-transcribe");
    let model_dir = root.path().join("model");
    fs::write(&helper_path, b"fake helper").expect("helper file");
    chmod(&helper_path, 0o755);
    create_fake_coreml_parakeet_tree(&model_dir);
    let runtime =
        video_creater_lib::transcription::fluidaudio::FluidAudioCoreMlRuntime::new(helper_path);
    let installed = fluid_audio_installed_model(&model_dir);

    assert_eq!(
        runtime.probe_installed_model(&installed),
        RuntimeCapability::Unavailable
    );
    assert!(runtime
        .readiness_diagnostic(&installed, &RuntimeCapability::Unavailable)
        .expect("missing inspection readiness diagnostic")
        .contains("missing Core ML inspection report"));
}

#[cfg(target_os = "macos")]
#[test]
fn fluid_audio_runtime_transcribe_reports_missing_inspection_before_helper_spawn() {
    let root = tempfile::tempdir().expect("root");
    let helper_path = root.path().join("missing-helper");
    let model_dir = root.path().join("model");
    create_fake_coreml_parakeet_tree(&model_dir);
    let runtime =
        video_creater_lib::transcription::fluidaudio::FluidAudioCoreMlRuntime::new(helper_path);
    let installed = fluid_audio_installed_model(&model_dir);

    let error = runtime
        .transcribe_installed(
            &TranscriptionRuntimeJob {
                media_id: "media-1".to_string(),
                source_path: root.path().join("media.wav").display().to_string(),
                model_path: model_dir.display().to_string(),
                language_mode: "en".to_string(),
                output_artifact_path: None,
            },
            &installed,
        )
        .expect_err("missing inspection should fail before helper spawn");

    assert!(error
        .to_string()
        .contains("missing Core ML inspection report"));
    assert!(!error
        .to_string()
        .contains("helper executable is unavailable"));
}

#[cfg(target_os = "macos")]
#[test]
fn fluid_audio_runtime_transcribe_reports_missing_bundle_before_helper_spawn() {
    let root = tempfile::tempdir().expect("root");
    let helper_path = root.path().join("missing-helper");
    let model_dir = root.path().join("model");
    for bundle in [
        "Preprocessor.mlmodelc",
        "Encoder.mlmodelc",
        "Decoder.mlmodelc",
    ] {
        fs::create_dir_all(model_dir.join(bundle)).expect("model bundle dir");
    }
    write_fake_coreml_inspection_report(&model_dir);
    let runtime =
        video_creater_lib::transcription::fluidaudio::FluidAudioCoreMlRuntime::new(helper_path);
    let installed = fluid_audio_installed_model(&model_dir);

    let error = runtime
        .transcribe_installed(
            &TranscriptionRuntimeJob {
                media_id: "media-1".to_string(),
                source_path: root.path().join("media.wav").display().to_string(),
                model_path: model_dir.display().to_string(),
                language_mode: "en".to_string(),
                output_artifact_path: None,
            },
            &installed,
        )
        .expect_err("missing bundle should fail before helper spawn");

    assert!(error.to_string().contains("missing Core ML bundle"));
    assert!(error.to_string().contains("JointDecisionv3.mlmodelc"));
    assert!(!error
        .to_string()
        .contains("helper executable is unavailable"));
}

#[cfg(not(target_os = "macos"))]
#[test]
fn fluid_audio_runtime_probe_installed_model_reports_unsupported_platform() {
    let root = tempfile::tempdir().expect("root");
    let helper_path = root.path().join("video-creater-fluidaudio-transcribe");
    let model_dir = root.path().join("model");
    fs::write(&helper_path, b"fake helper").expect("helper file");
    create_fake_coreml_parakeet_tree(&model_dir);
    write_fake_coreml_inspection_report(&model_dir);
    let runtime =
        video_creater_lib::transcription::fluidaudio::FluidAudioCoreMlRuntime::new(helper_path);
    let installed = fluid_audio_installed_model(&model_dir);

    assert_eq!(
        runtime.probe_installed_model(&installed),
        RuntimeCapability::UnsupportedPlatform
    );
}

#[test]
fn coreml_runtime_reports_platform_capability() {
    let runtime = CoreMlParakeetRuntime;
    let entry = parakeet_v3_coreml_catalog_entry();

    assert_eq!(runtime.id(), FLUID_AUDIO_COREML_RUNTIME_ID);
    assert!(runtime.supports(&entry));
    assert!(!runtime.supports(&parakeet_v3_sherpa_onnx_catalog_entry()));
    #[cfg(target_os = "macos")]
    assert_eq!(runtime.probe(&entry), RuntimeCapability::Ready);
    #[cfg(not(target_os = "macos"))]
    assert_eq!(
        runtime.probe(&entry),
        RuntimeCapability::UnsupportedPlatform
    );
}

#[cfg(target_os = "macos")]
#[test]
fn coreml_backend_returns_tokens_for_short_wav_fixture_when_model_is_installed() {
    let Ok(model_root) = std::env::var("VIDEO_CREATER_TEST_PARAKEET_MODEL_ROOT") else {
        eprintln!("skipping Core ML Parakeet integration test: VIDEO_CREATER_TEST_PARAKEET_MODEL_ROOT is not set");
        return;
    };
    let Ok(source_path) = std::env::var("VIDEO_CREATER_TEST_TRANSCRIPTION_WAV") else {
        eprintln!("skipping Core ML Parakeet integration test: VIDEO_CREATER_TEST_TRANSCRIPTION_WAV is not set");
        return;
    };

    let backend = CoreMlParakeetBackend;
    let output = backend
        .transcribe(
            &TranscriptionRuntimeJob {
                media_id: "media-1".to_string(),
                source_path,
                model_path: model_root,
                language_mode: "en".to_string(),
                output_artifact_path: None,
            },
            "nvidia/parakeet-tdt-0.6b-v3",
        )
        .expect("Core ML Parakeet output");

    assert_eq!(output.runtime_id, FLUID_AUDIO_COREML_RUNTIME_ID);
    assert!(!output.tokens.is_empty());
    assert!(output.tokens.iter().all(|token| token.end >= token.start));
}

#[test]
fn coreml_inspector_json_contract_is_documented_for_installed_model() {
    let Ok(model_root) = std::env::var("VIDEO_CREATER_TEST_PARAKEET_MODEL_ROOT") else {
        eprintln!("skipping Core ML Parakeet inspector JSON contract test: VIDEO_CREATER_TEST_PARAKEET_MODEL_ROOT is not set");
        return;
    };

    #[cfg(not(target_os = "macos"))]
    {
        eprintln!("skipping Core ML Parakeet inspector JSON contract test: Core ML requires macOS");
        let _ = model_root;
    }

    #[cfg(target_os = "macos")]
    {
        let output =
            std::process::Command::new(env!("CARGO_BIN_EXE_video-creater-inspect-coreml-parakeet"))
                .arg(model_root)
                .arg("--json")
                .output()
                .expect("run Core ML Parakeet inspector");

        assert!(
            output.status.success(),
            "inspector failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let report: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("parse inspector JSON report");

        assert_eq!(report["schemaVersion"], serde_json::json!(1));
        assert_eq!(
            report["runtimeId"],
            serde_json::json!(FLUID_AUDIO_COREML_RUNTIME_ID)
        );
        assert_eq!(
            report["bundles"]
                .as_array()
                .expect("bundles is an array")
                .len(),
            4
        );
    }
}

#[cfg(target_os = "macos")]
#[test]
fn coreml_backend_accepts_runtime_local_fluid_audio_inspector_report_and_uses_helper() {
    let root = tempfile::tempdir().expect("root");
    let helper_path = root.path().join("video-creater-fluidaudio-transcribe");
    let runtime_model_dir = root.path().join("fluid-audio-coreml");
    let source_path = root.path().join("media.wav");
    fs::write(&source_path, b"fake wav").expect("write media fixture");
    write_fake_fluidaudio_helper(&helper_path);
    create_fake_coreml_parakeet_tree(&runtime_model_dir);
    fs::write(
        runtime_model_dir.join(COREML_INSPECTION_FILE_NAME),
        serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 1,
            "modelId": "nvidia/parakeet-tdt-0.6b-v3",
            "runtimeId": FLUID_AUDIO_COREML_RUNTIME_ID,
            "bundles": [
                {
                    "name": "Preprocessor",
                    "path": runtime_model_dir.join("Preprocessor.mlmodelc").display().to_string(),
                    "inputs": [],
                    "outputs": []
                }
            ]
        }))
        .expect("inspection report json"),
    )
    .expect("write inspection report");

    let output = CoreMlParakeetBackend::with_helper_path(helper_path)
        .transcribe(
            &TranscriptionRuntimeJob {
                media_id: "media-1".to_string(),
                source_path: source_path.display().to_string(),
                model_path: runtime_model_dir.display().to_string(),
                language_mode: "en".to_string(),
                output_artifact_path: None,
            },
            "nvidia/parakeet-tdt-0.6b-v3",
        )
        .expect("helper-backed Core ML backend output");

    assert_eq!(output.runtime_id, FLUID_AUDIO_COREML_RUNTIME_ID);
    assert_eq!(output.model_id, "nvidia/parakeet-tdt-0.6b-v3");
    assert_eq!(output.tokens.len(), 2);
    assert_eq!(output.tokens[0].token, "hello");
    assert!(!format!("{output:?}").contains("Core ML Parakeet decode loop"));
}

#[test]
fn safe_model_dir_name_replaces_model_id_slashes() {
    assert_eq!(
        safe_model_dir_name("nvidia/parakeet-tdt-0.6b-v3"),
        "nvidia__parakeet-tdt-0.6b-v3"
    );
}

#[test]
fn manifest_validation_rejects_missing_required_files() {
    let root = tempdir().expect("temp model root");
    let entry = parakeet_v3_catalog_entry();
    let model_dir = root.path().join(safe_model_dir_name(entry.id));
    fs::create_dir_all(&model_dir).expect("model dir");
    write_required_runtime_files(&entry, &model_dir);

    let manifest = InstalledModelManifest {
        schema_version: 1,
        model_id: entry.id.to_string(),
        revision: "main".to_string(),
        modality: None,
        artifact_format: None,
        source: None,
        installed_files: vec![TranscriptionModelFile {
            path: entry.required_files[0].path.clone(),
            size_bytes: Some(1),
            sha256: None,
        }],
        installed_at: "2026-06-13T00:00:00Z".to_string(),
        verified_at: None,
    };
    assert_eq!(manifest.revision, entry.revision);

    let status = manifest.validate_against(&entry, root.path());

    assert_eq!(status, ModelInstallStatus::Failed);
}

#[test]
fn manifest_validation_rejects_wrong_model_id() {
    let root = tempdir().expect("temp model root");
    let entry = parakeet_v3_catalog_entry();
    let model_dir = root.path().join(safe_model_dir_name(entry.id));
    fs::create_dir_all(&model_dir).expect("model dir");
    write_required_runtime_files(&entry, &model_dir);

    let mut manifest = complete_manifest(&entry);
    manifest.model_id = "nvidia/other-model".to_string();

    let status = manifest.validate_against(&entry, root.path());

    assert_eq!(status, ModelInstallStatus::Failed);
}

#[test]
fn manifest_validation_rejects_wrong_revision() {
    let root = tempdir().expect("temp model root");
    let entry = parakeet_v3_catalog_entry();
    let model_dir = root.path().join(safe_model_dir_name(entry.id));
    fs::create_dir_all(&model_dir).expect("model dir");
    write_required_runtime_files(&entry, &model_dir);

    let mut manifest = complete_manifest(&entry);
    manifest.revision = "refs/pr/1".to_string();

    let status = manifest.validate_against(&entry, root.path());

    assert_eq!(status, ModelInstallStatus::Failed);
}

#[test]
fn manifest_validation_accepts_legacy_manifest_without_source_fields() {
    let root = tempdir().expect("temp model root");
    let entry = parakeet_v3_catalog_entry();
    let model_dir = root.path().join(safe_model_dir_name(entry.id));
    fs::create_dir_all(&model_dir).expect("model dir");
    write_required_runtime_files(&entry, &model_dir);

    let manifest_json = serde_json::json!({
        "modelId": entry.id,
        "revision": entry.revision,
        "installedFiles": entry
            .required_files
            .iter()
            .map(|file| serde_json::json!({ "path": file.path }))
            .collect::<Vec<_>>(),
        "installedAt": "2026-06-13T00:00:00Z",
        "verifiedAt": "2026-06-13T00:01:00Z"
    });
    let manifest: InstalledModelManifest =
        serde_json::from_value(manifest_json).expect("legacy manifest");

    let status = manifest.validate_against(&entry, root.path());

    assert_eq!(status, ModelInstallStatus::Ready);
    assert_eq!(manifest.schema_version, 1);
    assert_eq!(manifest.modality, None);
    assert_eq!(manifest.artifact_format, None);
    assert_eq!(manifest.source, None);
}

#[test]
fn manifest_validation_rejects_wrong_modality() {
    let root = tempdir().expect("temp model root");
    let entry = parakeet_v3_catalog_entry();
    let model_dir = root.path().join(safe_model_dir_name(entry.id));
    fs::create_dir_all(&model_dir).expect("model dir");
    write_required_runtime_files(&entry, &model_dir);

    let mut manifest = complete_manifest(&entry);
    manifest.modality = Some(CatalogModelModality::ImageGeneration);

    let status = manifest.validate_against(&entry, root.path());

    assert_eq!(status, ModelInstallStatus::Failed);
}

#[test]
fn manifest_deserialization_rejects_unknown_artifact_format() {
    let entry = parakeet_v3_catalog_entry();
    let manifest_json = serde_json::json!({
        "schemaVersion": 1,
        "modelId": entry.id,
        "revision": entry.revision,
        "modality": "transcription",
        "artifactFormat": "unexpected_format",
        "installedFiles": entry.required_files,
        "installedAt": "2026-06-13T00:00:00Z",
        "verifiedAt": "2026-06-13T00:01:00Z"
    });
    let manifest: Result<InstalledModelManifest, _> = serde_json::from_value(manifest_json);

    assert!(manifest.is_err());
}

#[test]
fn manifest_validation_rejects_manifest_entry_when_local_file_missing() {
    let root = tempdir().expect("temp model root");
    let entry = parakeet_v3_catalog_entry();
    let model_dir = root.path().join(safe_model_dir_name(entry.id));
    fs::create_dir_all(&model_dir).expect("model dir");

    let omitted_path = entry
        .required_files
        .last()
        .expect("required file")
        .path
        .clone();
    for file in &entry.required_files {
        if file.path != omitted_path {
            write_model_file(
                &required_files_dir_for_entry(&entry, &model_dir),
                &file.path,
                b"present",
            );
        }
    }

    let manifest = complete_manifest(&entry);

    let status = manifest.validate_against(&entry, root.path());

    assert_eq!(status, ModelInstallStatus::Failed);
}

#[test]
fn manifest_validation_rejects_zero_length_local_file() {
    let root = tempdir().expect("temp model root");
    let entry = parakeet_v3_catalog_entry();
    let model_dir = root.path().join(safe_model_dir_name(entry.id));
    fs::create_dir_all(&model_dir).expect("model dir");
    write_required_runtime_files(&entry, &model_dir);
    write_model_file(
        &required_files_dir_for_entry(&entry, &model_dir),
        &entry.required_files[0].path,
        b"",
    );

    let manifest = complete_manifest(&entry);

    let status = manifest.validate_against(&entry, root.path());

    assert_eq!(status, ModelInstallStatus::Failed);
}

#[test]
fn manifest_validation_rejects_recorded_size_mismatch() {
    let root = tempdir().expect("temp model root");
    let entry = parakeet_v3_catalog_entry();
    let model_dir = root.path().join(safe_model_dir_name(entry.id));
    fs::create_dir_all(&model_dir).expect("model dir");
    write_required_runtime_files(&entry, &model_dir);

    let mut manifest = complete_manifest(&entry);
    manifest.installed_files[0].size_bytes = Some(999);

    let status = manifest.validate_against(&entry, root.path());

    assert_eq!(status, ModelInstallStatus::Failed);
}

#[test]
fn manifest_validation_rejects_recorded_sha256_mismatch() {
    let root = tempdir().expect("temp model root");
    let entry = parakeet_v3_catalog_entry();
    let model_dir = root.path().join(safe_model_dir_name(entry.id));
    fs::create_dir_all(&model_dir).expect("model dir");
    write_required_runtime_files(&entry, &model_dir);

    let mut manifest = complete_manifest(&entry);
    manifest.installed_files[0].sha256 = Some("0".repeat(64));

    let status = manifest.validate_against(&entry, root.path());

    assert_eq!(status, ModelInstallStatus::Failed);
}

#[test]
fn manifest_validation_rejects_directory_instead_of_required_file() {
    let root = tempdir().expect("temp model root");
    let entry = parakeet_v3_catalog_entry();
    let model_dir = root.path().join(safe_model_dir_name(entry.id));
    fs::create_dir_all(&model_dir).expect("model dir");
    write_required_runtime_files(&entry, &model_dir);
    let required_path =
        required_files_dir_for_entry(&entry, &model_dir).join(&entry.required_files[0].path);
    fs::remove_file(&required_path).expect("remove model file");
    fs::create_dir(&required_path).expect("model file directory");

    let manifest = complete_manifest(&entry);

    let status = manifest.validate_against(&entry, root.path());

    assert_eq!(status, ModelInstallStatus::Failed);
}

#[test]
fn manifest_validation_accepts_complete_nonempty_install() {
    let root = tempdir().expect("temp model root");
    let entry = parakeet_v3_catalog_entry();
    let model_dir = root.path().join(safe_model_dir_name(entry.id));
    fs::create_dir_all(&model_dir).expect("model dir");
    write_required_runtime_files(&entry, &model_dir);

    let manifest = complete_manifest(&entry);

    let status = manifest.validate_against(&entry, root.path());

    assert_eq!(status, ModelInstallStatus::Ready);
}

#[test]
fn manifest_validation_returns_verifying_for_complete_unverified_install() {
    let root = tempdir().expect("temp model root");
    let entry = parakeet_v3_catalog_entry();
    let model_dir = root.path().join(safe_model_dir_name(entry.id));
    fs::create_dir_all(&model_dir).expect("model dir");
    write_required_runtime_files(&entry, &model_dir);

    let mut manifest = complete_manifest(&entry);
    manifest.verified_at = None;

    let status = manifest.validate_against(&entry, root.path());

    assert_eq!(status, ModelInstallStatus::Verifying);
}

#[test]
fn store_reports_missing_when_manifest_is_absent() {
    let root = tempdir().expect("temp model root");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    let entry = parakeet_v3_catalog_entry();
    let model_dir = root.path().join(safe_model_dir_name(entry.id));

    let status = store.status("nvidia/parakeet-tdt-0.6b-v3").expect("status");

    assert_eq!(status.install_status, ModelInstallStatus::Missing);
    assert_eq!(status.model_id, "nvidia/parakeet-tdt-0.6b-v3");
    assert_eq!(status.local_path, model_dir.display().to_string());
    assert_eq!(status.approximate_size_bytes, entry.approximate_size_bytes);
    assert_eq!(status.verified_at, None);
}

#[test]
fn store_defaults_active_model_to_first_catalog_entry() {
    let root = tempdir().expect("temp model root");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());

    let active_model_id = store.active_model_id().expect("active model");

    assert_eq!(active_model_id, "nvidia/parakeet-tdt-0.6b-v3");
}

#[test]
fn store_marks_active_model_in_list_and_can_change_active_model() {
    let root = tempdir().expect("temp model root");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    let entry = parakeet_v3_catalog_entry();

    let active_status = store.set_active_model(entry.id).expect("set active");
    let statuses = store.list().expect("list");

    assert!(active_status.is_active);
    assert!(statuses
        .iter()
        .any(|status| status.model_id == entry.id && status.is_active));
}

#[test]
fn store_reports_failed_when_model_directory_exists_without_manifest() {
    let root = tempdir().expect("temp model root");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    let entry = parakeet_v3_catalog_entry();
    let model_dir = root.path().join(safe_model_dir_name(entry.id));
    fs::create_dir_all(&model_dir).expect("model dir");

    let status = store.status(entry.id).expect("status");

    assert_eq!(status.install_status, ModelInstallStatus::Failed);
    assert_eq!(status.verified_at, None);
    assert_eq!(status.downloaded_files, 0);
    assert_eq!(status.total_files, entry.required_files.len() as u32);
}

#[test]
fn store_partial_directory_status_remains_failed_until_verified() {
    let root = tempdir().expect("temp model root");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    let entry = parakeet_v3_catalog_entry();
    let model_dir = root.path().join(safe_model_dir_name(entry.id));
    fs::create_dir_all(&model_dir).expect("model dir");
    write_model_file(&model_dir, &entry.required_files[0].path, b"present");

    let status = store.status(entry.id).expect("status");

    assert_eq!(status.install_status, ModelInstallStatus::Failed);
    assert_eq!(status.downloaded_files, 0);
    assert_eq!(status.total_files, entry.required_files.len() as u32);
}

#[test]
fn store_active_download_reports_downloading_with_progress_fields() {
    let root = tempdir().expect("temp model root");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    let entry = parakeet_v3_catalog_entry();

    let started = store
        .mark_download_started(entry.id, entry.required_files.len() as u32)
        .expect("start download");
    let status = store.status(entry.id).expect("status");
    let StartDownload::Started {
        status: started_status,
        ..
    } = started
    else {
        panic!("first start should own download");
    };

    assert_eq!(
        started_status.install_status,
        ModelInstallStatus::Downloading
    );
    assert_eq!(started_status.downloaded_files, 0);
    assert_eq!(
        started_status.total_files,
        entry.required_files.len() as u32
    );
    assert_eq!(status.install_status, ModelInstallStatus::Downloading);
    assert_eq!(status.downloaded_files, 0);
    assert_eq!(status.total_files, entry.required_files.len() as u32);
}

// Needs the unpinned Core ML catalog (every platform except Linux) so fixture bytes can verify.
#[cfg(not(target_os = "linux"))]
#[test]
fn store_download_installs_coreml_source_into_runtime_layout() {
    let root = tempfile::tempdir().expect("root");
    let source = tempfile::tempdir().expect("source");
    write_minimal_coreml_source(source.path());

    let entry = parakeet_v3_catalog_entry();
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    let status = store
        .download_from_local_source_for_test(entry.id, source.path())
        .expect("download from local source");

    assert_eq!(status.install_status, ModelInstallStatus::Ready);
    let runtime_dir = store
        .runtime_model_dir(entry.id, FLUID_AUDIO_COREML_RUNTIME_ID)
        .expect("runtime dir");
    assert!(runtime_dir
        .join("Encoder.mlmodelc/coremldata.bin")
        .is_file());
    assert!(runtime_dir
        .join("JointDecisionv3.mlmodelc/coremldata.bin")
        .is_file());
    assert!(runtime_dir.join("parakeet_v3_vocab.json").is_file());
    assert!(runtime_dir.join(COREML_INSPECTION_FILE_NAME).is_file());
}

// Needs the unpinned Core ML catalog (every platform except Linux) so fixture bytes can verify.
#[cfg(not(target_os = "linux"))]
#[test]
fn downloaded_model_manifest_records_artifact_source_and_file_hashes() {
    let root = tempfile::tempdir().expect("root");
    let source = tempfile::tempdir().expect("source");
    write_minimal_coreml_source(source.path());

    let entry = parakeet_v3_catalog_entry();
    let store = TranscriptionModelStore::new(root.path().to_path_buf());

    store
        .download_from_local_source_for_test(entry.id, source.path())
        .expect("download from local source");

    let manifest_path = root
        .path()
        .join(safe_model_dir_name(entry.id))
        .join(MODEL_MANIFEST_FILE_NAME);
    let manifest_json: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("read downloaded model manifest"))
            .expect("downloaded model manifest JSON");

    assert_eq!(manifest_json["modelId"], serde_json::json!(entry.id));
    assert_eq!(
        manifest_json["artifactFormat"],
        serde_json::json!("core_ml_bundle")
    );
    assert_eq!(
        manifest_json["modality"],
        serde_json::json!("transcription")
    );
    assert_eq!(
        manifest_json["source"]["provider"],
        serde_json::json!("hugging_face_hub")
    );
    assert_eq!(
        manifest_json["source"]["repoId"],
        serde_json::json!("FluidInference/parakeet-tdt-0.6b-v3-coreml")
    );
    assert_eq!(
        manifest_json["source"]["revision"],
        serde_json::json!("aed02740059203c4a87495924f685de3722ae9ce")
    );
    assert_eq!(
        manifest_json["source"]["license"],
        serde_json::json!("cc-by-4.0")
    );

    let encoder_file = manifest_json["installedFiles"]
        .as_array()
        .expect("installed files array")
        .iter()
        .find(|file| file["path"] == serde_json::json!("Encoder.mlmodelc/coremldata.bin"))
        .expect("encoder file manifest entry");
    assert!(
        encoder_file["sizeBytes"]
            .as_u64()
            .expect("encoder size bytes")
            > 0
    );
    assert_eq!(
        encoder_file["sha256"]
            .as_str()
            .expect("encoder sha256")
            .len(),
        64
    );
}

// Needs the unpinned Core ML catalog (every platform except Linux) so fixture bytes can verify.
#[cfg(not(target_os = "linux"))]
#[test]
fn store_import_rejects_missing_coreml_bundle_file() {
    let root = tempfile::tempdir().expect("root");
    let source = tempfile::tempdir().expect("source");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());

    create_fake_coreml_parakeet_tree(source.path());
    fs::remove_file(source.path().join("Preprocessor.mlmodelc/coremldata.bin"))
        .expect("remove preprocessor");

    let error = store
        .import_model("nvidia/parakeet-tdt-0.6b-v3", source.path())
        .expect_err("missing required files");

    assert!(error.to_string().contains("Preprocessor.mlmodelc"));
}

// Needs the unpinned Core ML catalog (every platform except Linux) so fixture bytes can verify.
#[cfg(not(target_os = "linux"))]
#[test]
fn fluid_audio_import_copies_coreml_bundles_into_runtime_local_subdir_and_writes_metadata() {
    let root = tempfile::tempdir().expect("root");
    let source = tempfile::tempdir().expect("source");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    create_fake_coreml_parakeet_tree(source.path());

    let status = store
        .import_model("nvidia/parakeet-tdt-0.6b-v3", source.path())
        .expect("import model");

    assert_eq!(status.install_status, ModelInstallStatus::Ready);
    let runtime_model_dir = root
        .path()
        .join("nvidia__parakeet-tdt-0.6b-v3")
        .join("fluid-audio-coreml");
    assert!(runtime_model_dir
        .join("Preprocessor.mlmodelc/coremldata.bin")
        .is_file());
    assert!(!root
        .path()
        .join("nvidia__parakeet-tdt-0.6b-v3/Preprocessor.mlmodelc/coremldata.bin")
        .exists());
    assert!(root
        .path()
        .join("nvidia__parakeet-tdt-0.6b-v3")
        .join(MODEL_MANIFEST_FILE_NAME)
        .is_file());

    let report_path = root
        .path()
        .join("nvidia__parakeet-tdt-0.6b-v3")
        .join(COREML_INSPECTION_FILE_NAME);
    let report_json = fs::read_to_string(report_path).expect("inspection report");
    let report: serde_json::Value =
        serde_json::from_str(&report_json).expect("inspection report JSON");
    assert_eq!(report["runtimeId"], serde_json::json!("fluid_audio_coreml"));
    assert_eq!(
        report["runtimeModelDir"],
        serde_json::json!(runtime_model_dir.display().to_string())
    );
    assert!(report["bundles"]
        .as_array()
        .expect("bundles should be an array")
        .iter()
        .any(|bundle| bundle == &serde_json::json!("Preprocessor.mlmodelc")));
}

#[test]
fn runtime_model_dir_is_under_global_store_root_not_project_path() {
    let root = tempfile::tempdir().expect("root");
    let project = tempfile::tempdir().expect("project");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    let entry = parakeet_v3_catalog_entry();
    #[cfg(target_os = "linux")]
    let runtime_subdir = "sherpa-onnx";
    #[cfg(not(target_os = "linux"))]
    let runtime_subdir = "fluid-audio-coreml";

    let runtime_model_dir = store
        .runtime_model_dir(entry.id, catalog_entry_runtime_id(&entry))
        .expect("runtime model dir");

    assert_eq!(
        runtime_model_dir,
        root.path()
            .join("nvidia__parakeet-tdt-0.6b-v3")
            .join(runtime_subdir)
    );
    assert!(!runtime_model_dir.starts_with(project.path()));
}

// Needs the unpinned Core ML catalog (every platform except Linux) so fixture bytes can verify.
#[cfg(not(target_os = "linux"))]
#[test]
fn fluid_audio_status_and_verify_detect_ready_normalized_install() {
    let root = tempdir().expect("temp model root");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    let entry = parakeet_v3_catalog_entry();
    let model_dir = root.path().join(safe_model_dir_name(entry.id));
    let runtime_model_dir = model_dir.join("fluid-audio-coreml");
    fs::create_dir_all(&runtime_model_dir).expect("runtime model dir");
    write_required_files(&entry, &runtime_model_dir);

    let verify_status = store.verify(entry.id).expect("verify");
    let status = store.status(entry.id).expect("status");

    assert_eq!(verify_status.install_status, ModelInstallStatus::Ready);
    assert_eq!(status.install_status, ModelInstallStatus::Ready);
    assert!(status.verified_at.is_some());

    let runtime_report_path = runtime_model_dir.join(COREML_INSPECTION_FILE_NAME);
    assert!(
        runtime_report_path.is_file(),
        "missing runtime-local inspection report at {}",
        runtime_report_path.display()
    );
    let report_json = fs::read_to_string(runtime_report_path).expect("inspection report");
    let report: serde_json::Value =
        serde_json::from_str(&report_json).expect("inspection report JSON");
    assert_eq!(report["runtimeId"], serde_json::json!("fluid_audio_coreml"));
    assert_eq!(
        report["runtimeModelDir"],
        serde_json::json!(runtime_model_dir.display().to_string())
    );
}

#[test]
fn fluid_audio_stale_flat_coreml_install_is_not_ready() {
    let root = tempdir().expect("temp model root");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    let entry = parakeet_v3_catalog_entry();
    let model_dir = root.path().join(safe_model_dir_name(entry.id));
    fs::create_dir_all(&model_dir).expect("model dir");
    write_required_files(&entry, &model_dir);
    fs::write(
        model_dir.join(MODEL_MANIFEST_FILE_NAME),
        serde_json::to_vec(&complete_manifest(&entry)).expect("manifest json"),
    )
    .expect("write manifest");

    let status = store.status(entry.id).expect("status");
    let verify_status = store.verify(entry.id).expect("verify");

    assert_eq!(status.install_status, ModelInstallStatus::Failed);
    assert_eq!(verify_status.install_status, ModelInstallStatus::Failed);
}

// Needs the unpinned Core ML catalog (every platform except Linux) so fixture bytes can verify.
#[cfg(not(target_os = "linux"))]
#[test]
fn store_import_copies_coreml_bundle_and_verifies_manifest() {
    let root = tempfile::tempdir().expect("root");
    let source = tempfile::tempdir().expect("source");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    create_fake_coreml_parakeet_tree(source.path());

    let status = store
        .import_model("nvidia/parakeet-tdt-0.6b-v3", source.path())
        .expect("import model");

    assert_eq!(status.install_status, ModelInstallStatus::Ready);
    let model_dir = root.path().join("nvidia__parakeet-tdt-0.6b-v3");
    assert!(model_dir
        .join("fluid-audio-coreml/Preprocessor.mlmodelc/coremldata.bin")
        .is_file());
    assert!(model_dir.join(MODEL_MANIFEST_FILE_NAME).is_file());
}

// Needs the unpinned Core ML catalog (every platform except Linux) so fixture bytes can verify.
#[cfg(not(target_os = "linux"))]
#[test]
fn store_import_writes_coreml_inspection_summary_contract() {
    let root = tempfile::tempdir().expect("root");
    let source = tempfile::tempdir().expect("source");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    create_fake_coreml_parakeet_tree(source.path());

    let status = store
        .import_model("nvidia/parakeet-tdt-0.6b-v3", source.path())
        .expect("import model");

    assert_eq!(status.install_status, ModelInstallStatus::Ready);
    let report_path = root
        .path()
        .join("nvidia__parakeet-tdt-0.6b-v3")
        .join(COREML_INSPECTION_FILE_NAME);
    let report_json = fs::read_to_string(report_path).expect("inspection report");
    let report: serde_json::Value =
        serde_json::from_str(&report_json).expect("inspection report JSON");
    assert_eq!(report["schemaVersion"], serde_json::json!(1));
    assert_eq!(
        report["modelId"],
        serde_json::json!("nvidia/parakeet-tdt-0.6b-v3")
    );
    assert_eq!(report["runtimeId"], serde_json::json!("fluid_audio_coreml"));
    assert_eq!(
        report["runtimeModelDir"],
        serde_json::json!(root
            .path()
            .join("nvidia__parakeet-tdt-0.6b-v3/fluid-audio-coreml")
            .display()
            .to_string())
    );

    let bundles = report["bundles"]
        .as_array()
        .expect("bundles should be an array");
    for expected_bundle in [
        "Encoder.mlmodelc",
        "Decoder.mlmodelc",
        "JointDecisionv3.mlmodelc",
        "Preprocessor.mlmodelc",
    ] {
        assert!(
            bundles
                .iter()
                .any(|bundle| bundle == &serde_json::json!(expected_bundle)),
            "missing {expected_bundle} in {bundles:?}"
        );
    }

    let runtime_report_path = root
        .path()
        .join("nvidia__parakeet-tdt-0.6b-v3/fluid-audio-coreml")
        .join(COREML_INSPECTION_FILE_NAME);
    assert!(
        runtime_report_path.is_file(),
        "missing runtime-local inspection report at {}",
        runtime_report_path.display()
    );
    let runtime_report_json =
        fs::read_to_string(runtime_report_path).expect("runtime inspection report");
    let runtime_report: serde_json::Value =
        serde_json::from_str(&runtime_report_json).expect("runtime inspection report JSON");
    assert_eq!(
        runtime_report["runtimeId"],
        serde_json::json!("fluid_audio_coreml")
    );
    assert_eq!(
        runtime_report["runtimeModelDir"],
        serde_json::json!(root
            .path()
            .join("nvidia__parakeet-tdt-0.6b-v3/fluid-audio-coreml")
            .display()
            .to_string())
    );
}

// Needs the unpinned Core ML catalog (every platform except Linux) so fixture bytes can verify.
#[cfg(not(target_os = "linux"))]
#[test]
fn store_import_copies_extra_coreml_bundle_resources() {
    let root = tempfile::tempdir().expect("root");
    let source = tempfile::tempdir().expect("source");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    create_fake_coreml_parakeet_tree(source.path());
    fs::write(
        source.path().join("Encoder.mlmodelc/extra-resource.bin"),
        b"extra",
    )
    .expect("extra resource");

    let status = store
        .import_model("nvidia/parakeet-tdt-0.6b-v3", source.path())
        .expect("import model");

    assert_eq!(status.install_status, ModelInstallStatus::Ready);
    assert_eq!(
        fs::read(root.path().join(
            "nvidia__parakeet-tdt-0.6b-v3/fluid-audio-coreml/Encoder.mlmodelc/extra-resource.bin"
        ))
        .expect("extra imported resource"),
        b"extra"
    );
}

// Needs the unpinned Core ML catalog (every platform except Linux) so fixture bytes can verify.
#[cfg(not(target_os = "linux"))]
#[test]
fn store_import_keeps_existing_ready_install_when_staged_validation_fails() {
    let root = tempfile::tempdir().expect("root");
    let source = tempfile::tempdir().expect("source");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    let model_dir = root.path().join("nvidia__parakeet-tdt-0.6b-v3");
    create_fake_coreml_parakeet_tree(&model_dir.join("fluid-audio-coreml"));
    let ready_status = store
        .verify("nvidia/parakeet-tdt-0.6b-v3")
        .expect("verify existing install");
    assert_eq!(ready_status.install_status, ModelInstallStatus::Ready);
    create_fake_coreml_parakeet_tree(source.path());
    fs::write(
        source.path().join("Preprocessor.mlmodelc/coremldata.bin"),
        b"",
    )
    .expect("empty required file");

    let import_status = store
        .import_model("nvidia/parakeet-tdt-0.6b-v3", source.path())
        .expect("import invalid model");

    assert_eq!(import_status.install_status, ModelInstallStatus::Failed);
    assert_eq!(
        store
            .status("nvidia/parakeet-tdt-0.6b-v3")
            .expect("status after failed import")
            .install_status,
        ModelInstallStatus::Ready
    );
    assert!(
        model_dir
            .join("fluid-audio-coreml/Preprocessor.mlmodelc/coremldata.bin")
            .metadata()
            .expect("existing preprocessor")
            .len()
            > 0
    );
}

#[test]
// Needs the unpinned Core ML catalog (every platform except Linux) so fixture bytes can verify.
#[cfg(all(unix, not(target_os = "linux")))]
fn store_import_verifies_canonical_equivalent_destination_in_place() {
    let root = tempfile::tempdir().expect("root");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    let model_dir = root.path().join("nvidia__parakeet-tdt-0.6b-v3");
    create_fake_coreml_parakeet_tree(&model_dir.join("fluid-audio-coreml"));
    let extra_file = model_dir.join("local-note.txt");
    fs::write(&extra_file, b"keep me").expect("extra file");
    let source_path = root.path().join("model-link");
    std::os::unix::fs::symlink(&model_dir, &source_path).expect("model symlink");

    let status = store
        .import_model("nvidia/parakeet-tdt-0.6b-v3", &source_path)
        .expect("import canonical equivalent path");

    assert_eq!(status.install_status, ModelInstallStatus::Ready);
    assert_eq!(
        fs::read(&extra_file).expect("extra file remains"),
        b"keep me"
    );
}

// Needs the unpinned Core ML catalog (every platform except Linux) so fixture bytes can verify.
#[cfg(not(target_os = "linux"))]
#[test]
fn store_import_does_not_reuse_stale_deterministic_importing_path() {
    let root = tempfile::tempdir().expect("root");
    let source = tempfile::tempdir().expect("source");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    create_fake_coreml_parakeet_tree(source.path());
    let deterministic_import_dir = root
        .path()
        .join("nvidia__parakeet-tdt-0.6b-v3")
        .with_extension("importing");
    fs::create_dir_all(&deterministic_import_dir).expect("deterministic import dir");
    let marker = deterministic_import_dir.join("marker.txt");
    fs::write(&marker, b"stale marker").expect("marker");

    let status = store
        .import_model("nvidia/parakeet-tdt-0.6b-v3", source.path())
        .expect("import model");

    assert_eq!(status.install_status, ModelInstallStatus::Ready);
    assert_eq!(
        fs::read(&marker).expect("stale deterministic path remains untouched"),
        b"stale marker"
    );
}

#[test]
fn store_active_download_state_is_shared_across_clones() {
    let root = tempdir().expect("temp model root");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    let cloned_store = store.clone();
    let entry = parakeet_v3_catalog_entry();

    store
        .mark_download_started(entry.id, entry.required_files.len() as u32)
        .expect("start download");
    let status = cloned_store.status(entry.id).expect("status from clone");

    assert_eq!(status.install_status, ModelInstallStatus::Downloading);
    assert_eq!(status.total_files, entry.required_files.len() as u32);
}

#[test]
fn store_cancel_download_returns_failed_status() {
    let root = tempdir().expect("temp model root");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    let entry = parakeet_v3_catalog_entry();
    store
        .mark_download_started(entry.id, entry.required_files.len() as u32)
        .expect("start download");

    let status = store.cancel_download(entry.id).expect("cancel");

    assert_eq!(status.install_status, ModelInstallStatus::Failed);
    assert_eq!(status.downloaded_files, 0);
    assert_eq!(status.total_files, entry.required_files.len() as u32);
}

#[test]
fn store_status_after_cancel_download_is_not_downloading() {
    let root = tempdir().expect("temp model root");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    let entry = parakeet_v3_catalog_entry();
    store
        .mark_download_started(entry.id, entry.required_files.len() as u32)
        .expect("start download");

    store.cancel_download(entry.id).expect("cancel");
    let status = store.status(entry.id).expect("status");

    assert_ne!(status.install_status, ModelInstallStatus::Downloading);
}

#[test]
fn store_cancel_download_without_active_download_returns_missing_status() {
    let root = tempdir().expect("temp model root");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    let entry = parakeet_v3_catalog_entry();

    let status = store.cancel_download(entry.id).expect("cancel");

    assert_eq!(status.install_status, ModelInstallStatus::Missing);
}

// Needs the unpinned Core ML catalog (every platform except Linux) so fixture bytes can verify.
#[cfg(not(target_os = "linux"))]
#[test]
fn store_cancel_download_without_active_download_returns_ready_status() {
    let root = tempdir().expect("temp model root");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    let entry = parakeet_v3_catalog_entry();
    let model_dir = root.path().join(safe_model_dir_name(entry.id));
    fs::create_dir_all(&model_dir).expect("model dir");
    write_required_runtime_files(&entry, &model_dir);
    fs::write(
        model_dir.join("video-creater-model-manifest.json"),
        serde_json::to_vec(&complete_manifest(&entry)).expect("manifest json"),
    )
    .expect("write manifest");

    let status = store.cancel_download(entry.id).expect("cancel");

    assert_eq!(status.install_status, ModelInstallStatus::Ready);
}

#[test]
fn store_duplicate_mark_download_started_keeps_existing_active_state() {
    let root = tempdir().expect("temp model root");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    let entry = parakeet_v3_catalog_entry();
    let first_start = store
        .mark_download_started(entry.id, entry.required_files.len() as u32)
        .expect("start download");
    let StartDownload::Started { token, .. } = first_start else {
        panic!("first start should own download");
    };

    let duplicate = store
        .mark_download_started(entry.id, 999)
        .expect("duplicate start");
    let status = store.status(entry.id).expect("status");
    let StartDownload::AlreadyActive(duplicate_status) = duplicate else {
        panic!("duplicate start should report active download");
    };

    assert_eq!(
        duplicate_status.install_status,
        ModelInstallStatus::Downloading
    );
    assert_eq!(
        duplicate_status.total_files,
        entry.required_files.len() as u32
    );
    assert_eq!(status.total_files, entry.required_files.len() as u32);
    assert!(!store
        .download_is_cancelled_or_stale(entry.id, token)
        .expect("first token remains active"));
}

#[test]
fn store_start_after_cancel_uses_new_token_and_marks_old_token_stale() {
    let root = tempdir().expect("temp model root");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    let entry = parakeet_v3_catalog_entry();
    let first_start = store
        .mark_download_started(entry.id, entry.required_files.len() as u32)
        .expect("start download");
    let StartDownload::Started {
        token: old_token, ..
    } = first_start
    else {
        panic!("first start should own download");
    };

    store.cancel_download(entry.id).expect("cancel");
    let second_start = store
        .mark_download_started(entry.id, entry.required_files.len() as u32)
        .expect("restart download");
    let StartDownload::Started {
        token: new_token, ..
    } = second_start
    else {
        panic!("restart should own download");
    };

    assert_ne!(old_token, new_token);
    assert!(store
        .download_is_cancelled_or_stale(entry.id, old_token)
        .expect("old token is stale"));
    assert!(!store
        .download_is_cancelled_or_stale(entry.id, new_token)
        .expect("new token is active"));
}

#[test]
fn store_progress_and_remove_are_owner_token_only() {
    let root = tempdir().expect("temp model root");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    let entry = parakeet_v3_catalog_entry();
    let first_start = store
        .mark_download_started(entry.id, entry.required_files.len() as u32)
        .expect("start download");
    let StartDownload::Started {
        token: old_token, ..
    } = first_start
    else {
        panic!("first start should own download");
    };
    store.cancel_download(entry.id).expect("cancel");
    let second_start = store
        .mark_download_started(entry.id, entry.required_files.len() as u32)
        .expect("restart download");
    let StartDownload::Started {
        token: new_token, ..
    } = second_start
    else {
        panic!("restart should own download");
    };

    assert!(!store
        .update_download_progress_if_owner(entry.id, old_token, 3)
        .expect("old token update"));
    assert!(!store
        .remove_active_download_if_owner(entry.id, old_token)
        .expect("old token remove"));
    assert_eq!(
        store.status(entry.id).expect("status").install_status,
        ModelInstallStatus::Downloading
    );
    assert!(store
        .update_download_progress_if_owner(entry.id, new_token, 2)
        .expect("new token update"));
    assert_eq!(store.status(entry.id).expect("status").downloaded_files, 2);
    assert!(store
        .remove_active_download_if_owner(entry.id, new_token)
        .expect("new token remove"));
}

#[test]
fn store_remove_during_active_download_clears_active_state() {
    let root = tempdir().expect("temp model root");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    let entry = parakeet_v3_catalog_entry();
    store
        .mark_download_started(entry.id, entry.required_files.len() as u32)
        .expect("start download");

    let status = store.remove(entry.id).expect("remove");

    assert_eq!(status.install_status, ModelInstallStatus::Missing);
    assert_ne!(
        store.status(entry.id).expect("status").install_status,
        ModelInstallStatus::Downloading
    );
}

#[test]
fn store_remove_during_active_download_makes_owner_token_stale() {
    let root = tempdir().expect("temp model root");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    let entry = parakeet_v3_catalog_entry();
    let start = store
        .mark_download_started(entry.id, entry.required_files.len() as u32)
        .expect("start download");
    let StartDownload::Started { token, .. } = start else {
        panic!("first start should own download");
    };

    store.remove(entry.id).expect("remove");

    assert!(store
        .download_is_cancelled_or_stale(entry.id, token)
        .expect("token stale after remove"));
}

#[test]
fn store_status_prefers_active_download_over_disk_manifest_state() {
    let root = tempdir().expect("temp model root");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    let entry = parakeet_v3_catalog_entry();
    let model_dir = root.path().join(safe_model_dir_name(entry.id));
    fs::create_dir_all(&model_dir).expect("model dir");
    write_required_runtime_files(&entry, &model_dir);
    fs::write(
        model_dir.join("video-creater-model-manifest.json"),
        serde_json::to_vec(&complete_manifest(&entry)).expect("manifest json"),
    )
    .expect("write manifest");
    store
        .mark_download_started(entry.id, entry.required_files.len() as u32)
        .expect("start download");

    let status = store.status(entry.id).expect("status");

    assert_eq!(status.install_status, ModelInstallStatus::Downloading);
    assert_eq!(status.downloaded_files, 0);
    assert_eq!(status.total_files, entry.required_files.len() as u32);
}

// Needs the unpinned Core ML catalog (every platform except Linux) so fixture bytes can verify.
#[cfg(not(target_os = "linux"))]
#[test]
fn store_verifies_complete_install_and_writes_manifest() {
    let root = tempdir().expect("temp model root");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    let entry = parakeet_v3_catalog_entry();
    let model_dir = root.path().join(safe_model_dir_name(entry.id));
    fs::create_dir_all(&model_dir).expect("model dir");
    write_required_runtime_files(&entry, &model_dir);

    let status = store.verify(entry.id).expect("verify");

    assert_eq!(status.install_status, ModelInstallStatus::Ready);
    assert_eq!(status.local_path, model_dir.display().to_string());
    assert_eq!(status.approximate_size_bytes, entry.approximate_size_bytes);
    assert!(status.verified_at.is_some());
    assert!(model_dir.join("video-creater-model-manifest.json").exists());
}

#[test]
fn store_verify_partial_install_returns_failed_without_writing_manifest() {
    let root = tempdir().expect("temp model root");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    let entry = parakeet_v3_catalog_entry();
    let model_dir = root.path().join(safe_model_dir_name(entry.id));
    fs::create_dir_all(&model_dir).expect("model dir");
    write_model_file(&model_dir, &entry.required_files[0].path, b"present");

    let status = store.verify(entry.id).expect("verify");

    assert_eq!(status.install_status, ModelInstallStatus::Failed);
    assert_eq!(status.verified_at, None);
    assert!(!model_dir.join("video-creater-model-manifest.json").exists());
}

#[test]
fn store_reports_corrupt_manifest_json_as_failed_status() {
    let root = tempdir().expect("temp model root");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    let entry = parakeet_v3_catalog_entry();
    let model_dir = root.path().join(safe_model_dir_name(entry.id));
    fs::create_dir_all(&model_dir).expect("model dir");
    fs::write(
        model_dir.join("video-creater-model-manifest.json"),
        b"{not valid json",
    )
    .expect("write corrupt manifest");

    let status = store.status(entry.id).expect("status");

    assert_eq!(status.install_status, ModelInstallStatus::Failed);
    assert_eq!(status.verified_at, None);
}

#[test]
fn store_remove_deletes_model_directory() {
    let root = tempdir().expect("temp model root");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    let entry = parakeet_v3_catalog_entry();
    let model_dir = root.path().join(safe_model_dir_name(entry.id));
    fs::create_dir_all(&model_dir).expect("model dir");
    write_model_file(&model_dir, &entry.required_files[0].path, b"present");

    let status = store.remove(entry.id).expect("remove");

    assert_eq!(status.install_status, ModelInstallStatus::Missing);
    assert!(!model_dir.exists());
}

// Linux equivalents of the Core ML store lifecycle tests. The sherpa-onnx catalog pins every
// artifact's size and SHA-256, so fixture bytes exercise the fail-closed half of each lifecycle.
#[cfg(target_os = "linux")]
fn create_fake_sherpa_onnx_parakeet_tree(root: &Path) {
    for file in parakeet_v3_sherpa_onnx_catalog_entry().required_files {
        write_model_file(root, &file.path, b"fake");
    }
}

#[cfg(target_os = "linux")]
#[test]
fn sherpa_onnx_store_import_rejects_missing_onnx_file() {
    let root = tempfile::tempdir().expect("root");
    let source = tempfile::tempdir().expect("source");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    create_fake_sherpa_onnx_parakeet_tree(source.path());
    fs::remove_file(source.path().join("tokens.txt")).expect("remove tokens");

    let error = store
        .import_model("nvidia/parakeet-tdt-0.6b-v3", source.path())
        .expect_err("missing required files");

    assert!(error.to_string().contains("tokens.txt"), "{error}");
    assert!(!root
        .path()
        .join("nvidia__parakeet-tdt-0.6b-v3")
        .join(MODEL_MANIFEST_FILE_NAME)
        .exists());
}

#[cfg(target_os = "linux")]
#[test]
fn sherpa_onnx_store_import_rejects_unpinned_bytes_without_publishing_install() {
    let root = tempfile::tempdir().expect("root");
    let source = tempfile::tempdir().expect("source");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    create_fake_sherpa_onnx_parakeet_tree(source.path());
    let deterministic_import_dir = root
        .path()
        .join("nvidia__parakeet-tdt-0.6b-v3")
        .with_extension("importing");
    fs::create_dir_all(&deterministic_import_dir).expect("deterministic import dir");
    let marker = deterministic_import_dir.join("marker.txt");
    fs::write(&marker, b"stale marker").expect("marker");

    let status = store
        .import_model("nvidia/parakeet-tdt-0.6b-v3", source.path())
        .expect("import model");

    assert_eq!(status.install_status, ModelInstallStatus::Failed);
    assert_eq!(status.runtime_id, "sherpa_onnx");
    assert_eq!(status.verified_at, None);
    let model_dir = root.path().join("nvidia__parakeet-tdt-0.6b-v3");
    assert!(!model_dir.join(MODEL_MANIFEST_FILE_NAME).exists());
    assert!(!model_dir.join("sherpa-onnx/encoder.int8.onnx").exists());
    assert_ne!(
        store
            .status("nvidia/parakeet-tdt-0.6b-v3")
            .expect("status after rejected import")
            .install_status,
        ModelInstallStatus::Ready
    );
    assert_eq!(
        fs::read(&marker).expect("stale deterministic path remains untouched"),
        b"stale marker"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn sherpa_onnx_store_failed_import_keeps_existing_install_files() {
    let root = tempfile::tempdir().expect("root");
    let source = tempfile::tempdir().expect("source");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    let runtime_dir = root.path().join("nvidia__parakeet-tdt-0.6b-v3/sherpa-onnx");
    write_model_file(&runtime_dir, "tokens.txt", b"existing tokens");
    create_fake_sherpa_onnx_parakeet_tree(source.path());
    fs::write(source.path().join("encoder.int8.onnx"), b"").expect("empty required file");

    let import_status = store
        .import_model("nvidia/parakeet-tdt-0.6b-v3", source.path())
        .expect("import invalid model");

    assert_eq!(import_status.install_status, ModelInstallStatus::Failed);
    assert_eq!(
        fs::read(runtime_dir.join("tokens.txt")).expect("existing install file"),
        b"existing tokens"
    );
    assert!(!runtime_dir.join("encoder.int8.onnx").exists());
}

#[cfg(target_os = "linux")]
#[test]
fn sherpa_onnx_store_import_verifies_canonical_equivalent_destination_in_place() {
    let root = tempfile::tempdir().expect("root");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    let model_dir = root.path().join("nvidia__parakeet-tdt-0.6b-v3");
    create_fake_sherpa_onnx_parakeet_tree(&model_dir.join("sherpa-onnx"));
    let extra_file = model_dir.join("local-note.txt");
    fs::write(&extra_file, b"keep me").expect("extra file");
    let source_path = root.path().join("model-link");
    std::os::unix::fs::symlink(&model_dir, &source_path).expect("model symlink");

    let status = store
        .import_model("nvidia/parakeet-tdt-0.6b-v3", &source_path)
        .expect("import canonical equivalent path");

    // Verified in place: the unpinned bytes fail, but nothing in the install is replaced.
    assert_eq!(status.install_status, ModelInstallStatus::Failed);
    assert_eq!(
        fs::read(&extra_file).expect("extra file remains"),
        b"keep me"
    );
    assert!(model_dir.join("sherpa-onnx/encoder.int8.onnx").is_file());
    assert!(!model_dir.join(MODEL_MANIFEST_FILE_NAME).exists());
}

#[cfg(target_os = "linux")]
#[test]
fn sherpa_onnx_store_download_rejects_unpinned_local_source() {
    let root = tempfile::tempdir().expect("root");
    let source = tempfile::tempdir().expect("source");
    create_fake_sherpa_onnx_parakeet_tree(source.path());
    let entry = parakeet_v3_catalog_entry();
    let store = TranscriptionModelStore::new(root.path().to_path_buf());

    let result = store.download_from_local_source_for_test(entry.id, source.path());

    match result {
        Err(error) => assert_eq!(error.stable_code(), "model.manifest.invalid", "{error}"),
        Ok(status) => assert_ne!(status.install_status, ModelInstallStatus::Ready),
    }
    let status = store
        .status(entry.id)
        .expect("status after rejected download");
    assert_ne!(status.install_status, ModelInstallStatus::Ready);
    assert_ne!(status.install_status, ModelInstallStatus::Downloading);
    assert!(!root
        .path()
        .join(safe_model_dir_name(entry.id))
        .join(MODEL_MANIFEST_FILE_NAME)
        .exists());
}

#[cfg(target_os = "linux")]
#[test]
fn sherpa_onnx_store_verify_and_cancel_distrust_manifest_hashes_that_differ_from_catalog_pins() {
    let root = tempdir().expect("temp model root");
    let store = TranscriptionModelStore::new(root.path().to_path_buf());
    let entry = parakeet_v3_catalog_entry();
    let model_dir = root.path().join(safe_model_dir_name(entry.id));
    fs::create_dir_all(&model_dir).expect("model dir");
    write_required_runtime_files(&entry, &model_dir);
    assert!(model_dir.join("sherpa-onnx/encoder.int8.onnx").is_file());
    // The manifest records the fixture bytes' real hashes, which do not match the catalog pins.
    fs::write(
        model_dir.join(MODEL_MANIFEST_FILE_NAME),
        serde_json::to_vec(&complete_manifest(&entry)).expect("manifest json"),
    )
    .expect("write manifest");

    let cancelled = store.cancel_download(entry.id).expect("cancel");
    let status = store.status(entry.id).expect("status");
    let verified = store.verify(entry.id).expect("verify");

    assert_eq!(cancelled.install_status, ModelInstallStatus::Failed);
    assert_eq!(status.install_status, ModelInstallStatus::Failed);
    assert_eq!(verified.install_status, ModelInstallStatus::Failed);
    assert_eq!(verified.verified_at, None);
}

fn complete_manifest(entry: &TranscriptionModelCatalogEntry) -> InstalledModelManifest {
    use sha2::{Digest, Sha256};

    let installed_files = entry
        .required_files
        .iter()
        .cloned()
        .map(|mut file| {
            file.size_bytes = Some(b"present".len() as u64);
            file.sha256 = Some(format!("{:x}", Sha256::digest(b"present")));
            file
        })
        .collect();
    InstalledModelManifest {
        schema_version: 1,
        model_id: entry.id.to_string(),
        revision: entry.revision.to_string(),
        modality: Some(entry.modality),
        artifact_format: Some(entry.artifact_format),
        source: None,
        installed_files,
        installed_at: "2026-06-13T00:00:00Z".to_string(),
        verified_at: Some("2026-06-13T00:01:00Z".to_string()),
    }
}

fn sample_transcript_for_gate() -> Transcript {
    Transcript {
        id: "transcript-media-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("test".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: vec![TranscriptWord {
            text: "Hello".to_string(),
            start_seconds: 0.0,
            end_seconds: 0.5,
            confidence: Some(0.99),
            speaker: None,
        }],
    }
}

fn write_required_files(entry: &TranscriptionModelCatalogEntry, model_dir: &std::path::Path) {
    for file in &entry.required_files {
        write_model_file(model_dir, &file.path, b"present");
    }
}

/// Writes fixture bytes into the entry's runtime-local layout (`fluid-audio-coreml` for Core ML
/// bundles, `sherpa-onnx` for ONNX transducers).
fn write_required_runtime_files(
    entry: &TranscriptionModelCatalogEntry,
    model_dir: &std::path::Path,
) {
    write_required_files(entry, &required_files_dir_for_entry(entry, model_dir));
}

fn fluid_audio_installed_model(
    model_dir: &std::path::Path,
) -> video_creater_lib::transcription::runtime::InstalledModel {
    let entry = parakeet_v3_catalog_entry();
    video_creater_lib::transcription::runtime::InstalledModel {
        model_id: entry.id.to_string(),
        model_dir: model_dir.to_path_buf(),
        artifact_format: ModelArtifactFormat::from(TranscriptionModelArtifactFormat::CoreMlBundle),
        modality: ModelModality::Transcription,
        runtime_family: FLUID_AUDIO_COREML_RUNTIME_ID.to_string(),
    }
}

fn write_model_file(model_dir: &std::path::Path, relative_path: &str, contents: &[u8]) {
    let file_path = model_dir.join(relative_path);
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent).expect("model file parent");
    }
    fs::write(file_path, contents).expect("write model file");
}

#[cfg(target_os = "macos")]
fn chmod(path: &Path, mode: u32) {
    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(mode);
    fs::set_permissions(path, permissions).expect("set permissions");
}

fn create_fake_coreml_parakeet_tree(root: &Path) {
    for path in [
        "config.json",
        "parakeet_v3_vocab.json",
        "Encoder.mlmodelc/coremldata.bin",
        "Encoder.mlmodelc/metadata.json",
        "Encoder.mlmodelc/model.mil",
        "Encoder.mlmodelc/weights/weight.bin",
        "Decoder.mlmodelc/coremldata.bin",
        "Decoder.mlmodelc/metadata.json",
        "Decoder.mlmodelc/model.mil",
        "Decoder.mlmodelc/weights/weight.bin",
        "JointDecisionv3.mlmodelc/coremldata.bin",
        "JointDecisionv3.mlmodelc/metadata.json",
        "JointDecisionv3.mlmodelc/model.mil",
        "JointDecisionv3.mlmodelc/weights/weight.bin",
        "Preprocessor.mlmodelc/coremldata.bin",
        "Preprocessor.mlmodelc/metadata.json",
        "Preprocessor.mlmodelc/model.mil",
        "Preprocessor.mlmodelc/weights/weight.bin",
    ] {
        let file_path = root.join(path);
        fs::create_dir_all(file_path.parent().expect("parent")).expect("parent");
        fs::write(&file_path, b"fake").expect("write fake model file");
    }
}

#[cfg(not(target_os = "linux"))]
fn write_minimal_coreml_source(root: &Path) {
    for path in [
        "config.json",
        "parakeet_v3_vocab.json",
        "Encoder.mlmodelc/coremldata.bin",
        "Encoder.mlmodelc/metadata.json",
        "Encoder.mlmodelc/model.mil",
        "Encoder.mlmodelc/weights/weight.bin",
        "Decoder.mlmodelc/coremldata.bin",
        "Decoder.mlmodelc/metadata.json",
        "Decoder.mlmodelc/model.mil",
        "Decoder.mlmodelc/weights/weight.bin",
        "JointDecisionv3.mlmodelc/coremldata.bin",
        "JointDecisionv3.mlmodelc/metadata.json",
        "JointDecisionv3.mlmodelc/model.mil",
        "JointDecisionv3.mlmodelc/weights/weight.bin",
        "Preprocessor.mlmodelc/coremldata.bin",
        "Preprocessor.mlmodelc/metadata.json",
        "Preprocessor.mlmodelc/model.mil",
        "Preprocessor.mlmodelc/weights/weight.bin",
    ] {
        let target = root.join(path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).expect("mkdir");
        }
        fs::write(&target, format!("fixture {path}")).expect("write fixture");
    }
}

fn write_fake_coreml_inspection_report(model_dir: &Path) {
    fs::write(
        model_dir.join(COREML_INSPECTION_FILE_NAME),
        serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 1,
            "modelId": "nvidia/parakeet-tdt-0.6b-v3",
            "runtimeId": FLUID_AUDIO_COREML_RUNTIME_ID,
            "bundles": [
                "Preprocessor.mlmodelc",
                "Encoder.mlmodelc",
                "Decoder.mlmodelc",
                "JointDecisionv3.mlmodelc"
            ]
        }))
        .expect("inspection report JSON"),
    )
    .expect("write inspection report");
}

#[cfg(target_os = "macos")]
fn write_fake_fluidaudio_helper(path: &Path) {
    fs::write(
        path,
        format!(
            r#"#!/bin/sh
printf '%s\n' '{{"schemaVersion":1,"runtimeId":"{runtime_id}","modelId":"nvidia/parakeet-tdt-0.6b-v3","text":"hello world","durationSeconds":1.0,"processingSeconds":0.01,"words":[{{"text":"hello","startSeconds":0.0,"endSeconds":0.4,"confidence":0.98}},{{"text":"world","startSeconds":0.45,"endSeconds":0.9,"confidence":0.97}}]}}'
"#,
            runtime_id = FLUID_AUDIO_COREML_RUNTIME_ID
        ),
    )
    .expect("write fake helper");
    chmod(path, 0o755);
}
