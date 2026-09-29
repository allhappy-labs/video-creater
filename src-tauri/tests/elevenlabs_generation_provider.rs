use reqwest::blocking::Client;
use serde_json::json;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::Duration;
use video_creater_lib::generation::elevenlabs::{
    build_elevenlabs_generation_submission, run_elevenlabs_generation_submission_with_client,
    ELEVENLABS_DEFAULT_VOICE_ID, ELEVENLABS_MUSIC_MODEL_ID, ELEVENLABS_PROVIDER,
    ELEVENLABS_TTS_V3_MODEL_ID,
};
use video_creater_lib::generation::GenerationTarget;
use video_creater_lib::project::model::{
    GeneratedAsset, GeneratedAssetReferences, GeneratedAssetSettings, GeneratedAssetStatus,
    GenerationModel, MediaKind,
};

fn generated_elevenlabs_asset(settings: GeneratedAssetSettings) -> GeneratedAsset {
    GeneratedAsset {
        schema_version: 1,
        id: "generated-elevenlabs-1".to_string(),
        kind: MediaKind::Generated,
        status: GeneratedAssetStatus::Queued,
        name: Some("ElevenLabs voiceover".to_string()),
        target_folder_id: None,
        placement_intent: None,
        prompt: "  Read this event promo with expressive energy.  ".to_string(),
        model: GenerationModel {
            provider: ELEVENLABS_PROVIDER.to_string(),
            id: ELEVENLABS_TTS_V3_MODEL_ID.to_string(),
        },
        references: GeneratedAssetReferences::default(),
        settings,
        outputs: Vec::new(),
        created_at: "2026-07-06T12:00:00Z".to_string(),
        parent_asset_id: None,
        retry_of_asset_id: None,
    }
}

fn generated_elevenlabs_music_asset(settings: GeneratedAssetSettings) -> GeneratedAsset {
    GeneratedAsset {
        schema_version: 1,
        id: "generated-elevenlabs-music-1".to_string(),
        kind: MediaKind::Generated,
        status: GeneratedAssetStatus::Queued,
        name: Some("ElevenLabs music bed".to_string()),
        target_folder_id: None,
        placement_intent: None,
        prompt: "  Warm synth pop intro with a bright chorus.  ".to_string(),
        model: GenerationModel {
            provider: ELEVENLABS_PROVIDER.to_string(),
            id: ELEVENLABS_MUSIC_MODEL_ID.to_string(),
        },
        references: GeneratedAssetReferences::default(),
        settings,
        outputs: Vec::new(),
        created_at: "2026-07-06T12:00:00Z".to_string(),
        parent_asset_id: None,
        retry_of_asset_id: None,
    }
}

#[test]
fn elevenlabs_tts_asset_builds_text_to_speech_request() {
    let asset = generated_elevenlabs_asset(GeneratedAssetSettings {
        duration_seconds: Some(12.0),
        voice: Some("rachel".to_string()),
        category: Some("tts".to_string()),
        ..GeneratedAssetSettings::default()
    });

    let submission =
        build_elevenlabs_generation_submission(&asset).expect("build ElevenLabs submission");

    assert_eq!(submission.method, "POST");
    assert_eq!(
        submission.url,
        format!("https://api.elevenlabs.io/v1/text-to-speech/{ELEVENLABS_DEFAULT_VOICE_ID}")
    );
    assert_eq!(submission.model, ELEVENLABS_TTS_V3_MODEL_ID);
    assert_eq!(submission.provider, ELEVENLABS_PROVIDER);
    assert!(serde_json::to_value(&submission)
        .unwrap()
        .get("authEnvVar")
        .is_none());
    assert_eq!(
        submission.input,
        json!({
            "text": "Read this event promo with expressive energy.",
            "model_id": "eleven_v3"
        })
    );
}

#[test]
fn elevenlabs_tts_run_posts_json_and_writes_mp3_output() {
    let asset = generated_elevenlabs_asset(GeneratedAssetSettings {
        duration_seconds: Some(6.0),
        voice: Some("rachel".to_string()),
        category: Some("tts".to_string()),
        ..GeneratedAssetSettings::default()
    });
    let mut submission =
        build_elevenlabs_generation_submission(&asset).expect("build ElevenLabs submission");
    let (base_url, request_handle) = spawn_elevenlabs_speech_server();
    submission.url = format!("{base_url}/v1/text-to-speech/{ELEVENLABS_DEFAULT_VOICE_ID}");
    let project_dir = tempfile::tempdir().expect("temp project dir");
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("client");

    let run = run_elevenlabs_generation_submission_with_client(
        &client,
        GenerationTarget::new(
            project_dir.path(),
            &asset,
            "2026-07-06T12:01:00Z",
            Some("elevenlabs-run-tts-1"),
            None,
        ),
        &submission,
        "test-elevenlabs-token",
    )
    .expect("run ElevenLabs TTS");

    assert_eq!(run.request_id, "elevenlabs-generated-elevenlabs-1");
    assert_eq!(
        std::fs::read(&run.completion.output_path).expect("ElevenLabs TTS output"),
        b"eleven-mp3"
    );
    assert_eq!(
        run.completion.output_path,
        project_dir
            .path()
            .join("generated/generated-elevenlabs-1/elevenlabs-output.mp3")
    );
    let completion = run
        .completion
        .actions
        .iter()
        .find_map(|action| match action {
            video_creater_lib::project::action::ProjectAction::CompleteGeneratedAsset {
                outputs,
                ..
            } => outputs.first(),
            _ => None,
        })
        .expect("completion output");
    assert_eq!(
        completion.media_id,
        "generated-elevenlabs-1-elevenlabs-output"
    );
    assert_eq!(completion.source_url, None);
    assert_eq!(
        completion.relative_path,
        "generated/generated-elevenlabs-1/elevenlabs-output.mp3"
    );
    assert_eq!(completion.width, 0);
    assert_eq!(completion.height, 0);
    assert_eq!(completion.duration_seconds, 6.0);

    let requests = request_handle.join().expect("ElevenLabs speech server");
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with(
        "POST /v1/text-to-speech/21m00Tcm4TlvDq8ikWAM?output_format=mp3_44100_128 HTTP/1.1"
    ));
    assert!(requests[0]
        .to_ascii_lowercase()
        .contains("xi-api-key: test-elevenlabs-token"));
    assert!(requests[0].contains(r#""model_id":"eleven_v3""#));
    assert!(requests[0].contains(r#""text":"Read this event promo with expressive energy.""#));
}

#[test]
fn elevenlabs_music_asset_builds_compose_music_request() {
    let asset = generated_elevenlabs_music_asset(GeneratedAssetSettings {
        duration_seconds: Some(30.0),
        category: Some("music".to_string()),
        lyrics: Some(
            "[Verse]\nWarm synth pop in English\n[Chorus]\nBright chorus lift".to_string(),
        ),
        instrumental: Some(true),
        style_instructions: Some("bright, commercial, loopable".to_string()),
        ..GeneratedAssetSettings::default()
    });

    let submission =
        build_elevenlabs_generation_submission(&asset).expect("build ElevenLabs music submission");

    assert_eq!(submission.method, "POST");
    assert_eq!(submission.url, "https://api.elevenlabs.io/v1/music");
    assert_eq!(submission.model, ELEVENLABS_MUSIC_MODEL_ID);
    assert_eq!(submission.provider, ELEVENLABS_PROVIDER);
    assert!(serde_json::to_value(&submission)
        .unwrap()
        .get("authEnvVar")
        .is_none());
    assert_eq!(
        submission.input,
        json!({
            "prompt": "Warm synth pop intro with a bright chorus.\n\nStyle: bright, commercial, loopable\n\nLyrics:\n[Verse]\nWarm synth pop in English\n[Chorus]\nBright chorus lift",
            "model_id": "music_v2",
            "music_length_ms": 30000,
            "force_instrumental": true
        })
    );
}

#[test]
fn elevenlabs_music_run_posts_json_and_writes_mp3_output() {
    let asset = generated_elevenlabs_music_asset(GeneratedAssetSettings {
        duration_seconds: Some(30.0),
        category: Some("music".to_string()),
        instrumental: Some(true),
        ..GeneratedAssetSettings::default()
    });
    let mut submission =
        build_elevenlabs_generation_submission(&asset).expect("build ElevenLabs music submission");
    let (base_url, request_handle) = spawn_elevenlabs_music_server();
    submission.url = format!("{base_url}/v1/music");
    let project_dir = tempfile::tempdir().expect("temp project dir");
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("client");

    let run = run_elevenlabs_generation_submission_with_client(
        &client,
        GenerationTarget::new(
            project_dir.path(),
            &asset,
            "2026-07-06T12:01:00Z",
            Some("elevenlabs-run-music-1"),
            None,
        ),
        &submission,
        "test-elevenlabs-token",
    )
    .expect("run ElevenLabs music");

    assert_eq!(run.request_id, "elevenlabs-generated-elevenlabs-music-1");
    assert_eq!(
        std::fs::read(&run.completion.output_path).expect("ElevenLabs music output"),
        b"eleven-music-mp3"
    );
    assert_eq!(
        run.completion.output_path,
        project_dir
            .path()
            .join("generated/generated-elevenlabs-music-1/elevenlabs-music-output.mp3")
    );
    let completion = run
        .completion
        .actions
        .iter()
        .find_map(|action| match action {
            video_creater_lib::project::action::ProjectAction::CompleteGeneratedAsset {
                outputs,
                ..
            } => outputs.first(),
            _ => None,
        })
        .expect("completion output");
    assert_eq!(
        completion.media_id,
        "generated-elevenlabs-music-1-elevenlabs-music-output"
    );
    assert_eq!(completion.source_url, None);
    assert_eq!(
        completion.relative_path,
        "generated/generated-elevenlabs-music-1/elevenlabs-music-output.mp3"
    );
    assert_eq!(completion.width, 0);
    assert_eq!(completion.height, 0);
    assert_eq!(completion.duration_seconds, 30.0);

    let requests = request_handle.join().expect("ElevenLabs music server");
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("POST /v1/music?output_format=mp3_48000_192 HTTP/1.1"));
    assert!(requests[0]
        .to_ascii_lowercase()
        .contains("xi-api-key: test-elevenlabs-token"));
    assert!(requests[0].contains(r#""model_id":"music_v2""#));
    assert!(requests[0].contains(r#""music_length_ms":30000"#));
    assert!(requests[0].contains(r#""force_instrumental":true"#));
    assert!(requests[0].contains(r#""prompt":"Warm synth pop intro with a bright chorus.""#));
}

fn spawn_elevenlabs_speech_server() -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ElevenLabs speech server");
    let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept ElevenLabs speech request");
        let mut buffer = [0_u8; 8192];
        let bytes_read = stream
            .read(&mut buffer)
            .expect("read ElevenLabs speech request");
        let request = String::from_utf8_lossy(&buffer[..bytes_read]).to_string();
        let response = concat!(
            "HTTP/1.1 200 OK\r\n",
            "Content-Type: audio/mpeg\r\n",
            "Content-Length: 10\r\n",
            "\r\n",
            "eleven-mp3"
        );
        stream
            .write_all(response.as_bytes())
            .expect("write ElevenLabs speech response");
        vec![request]
    });

    (base_url, handle)
}

fn spawn_elevenlabs_music_server() -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ElevenLabs music server");
    let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept ElevenLabs music request");
        let mut buffer = [0_u8; 8192];
        let bytes_read = stream
            .read(&mut buffer)
            .expect("read ElevenLabs music request");
        let request = String::from_utf8_lossy(&buffer[..bytes_read]).to_string();
        let response = concat!(
            "HTTP/1.1 200 OK\r\n",
            "Content-Type: audio/mpeg\r\n",
            "song-id: eleven-song-1\r\n",
            "Content-Length: 16\r\n",
            "\r\n",
            "eleven-music-mp3"
        );
        stream
            .write_all(response.as_bytes())
            .expect("write ElevenLabs music response");
        vec![request]
    });

    (base_url, handle)
}
