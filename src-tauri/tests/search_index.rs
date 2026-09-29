use serde_json::{json, Value};
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::{
    GeneratedAsset, GeneratedAssetOutput, GeneratedAssetReferences, GeneratedAssetSettings,
    GeneratedAssetStatus, GenerationModel, MediaAnalysisMoment, MediaAsset, MediaFolder, MediaKind,
    Transcript, TranscriptSegment, TranscriptWord,
};
use video_creater_lib::search::{
    load_project_search_index, query_project_search_for_project_dir, query_project_search_index,
    rebuild_project_search_index, GeneratedSearchEntry, MetadataSearchEntry, ProjectSearchIndex,
    ProjectSearchQuery, SearchScope, SpokenSearchEntry, SpokenSearchWord,
};

#[test]
fn rebuild_project_search_index_persists_status_aware_sidecar() {
    let temp = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.transcripts.push(Transcript {
        id: "transcript-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("fixture".to_string()),
        raw_artifact_path: Some("transcripts/media-1.json".to_string()),
        repairs: Vec::new(),
        segments: vec![TranscriptSegment {
            text: "founder says launch day".to_string(),
            start_seconds: 0.2,
            end_seconds: 1.4,
        }],
        words: vec![
            transcript_word("founder", 0.2, 0.5),
            transcript_word("says", 0.52, 0.7),
            transcript_word("launch", 0.8, 1.1),
            transcript_word("day", 1.12, 1.4),
        ],
    });

    let index = rebuild_project_search_index(temp.path(), &project).expect("rebuild index");

    assert_eq!(index.project_id, project.id);
    assert_eq!(index.visual_status, "notInstalled");
    assert_eq!(index.spoken_status, "ready");
    assert_eq!(index.spoken.len(), 1);
    assert!(!index.metadata.is_empty());
    assert!(temp.path().join("search/index.json").is_file());

    let loaded = load_project_search_index(temp.path()).expect("load index");
    assert_eq!(loaded.project_id, project.id);
    assert_eq!(loaded.spoken[0].media_id, "media-1");

    let results = query_project_search_index(
        &loaded,
        ProjectSearchQuery {
            query: "launch day".to_string(),
            limit: 5,
            scope: SearchScope::Both,
            media_id: None,
        },
    )
    .expect("query index");

    assert_eq!(results["visualStatus"], json!("notInstalled"));
    assert_eq!(results["spokenStatus"], json!("ready"));
    assert_eq!(results["groups"]["spoken"][0]["mediaId"], json!("media-1"));
    assert_eq!(
        results["groups"]["spoken"][0]["reason"],
        json!("matched transcript words")
    );
    assert_eq!(results["results"][0]["kind"], json!("spoken"));
}

#[test]
fn rebuild_project_search_index_writes_spoken_and_visual_sidecars() {
    let temp = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "audio-1".to_string(),
        name: Some("Voiceover".to_string()),
        relative_path: "media/voiceover.wav".to_string(),
        kind: MediaKind::Audio,
        duration_seconds: 8.0,
        width: None,
        height: None,
        fps: None,
        folder_id: None,
    });
    project.media.push(MediaAsset {
        id: "lottie-1".to_string(),
        name: Some("Logo Loop".to_string()),
        relative_path: "media/logo-loop.json".to_string(),
        kind: MediaKind::Lottie,
        duration_seconds: 2.0,
        width: Some(640),
        height: Some(360),
        fps: Some(30.0),
        folder_id: None,
    });
    project.transcripts.push(Transcript {
        id: "transcript-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("fixture".to_string()),
        raw_artifact_path: Some("transcripts/media-1.json".to_string()),
        repairs: Vec::new(),
        segments: vec![TranscriptSegment {
            text: "founder says launch day".to_string(),
            start_seconds: 0.2,
            end_seconds: 1.4,
        }],
        words: vec![
            transcript_word("founder", 0.2, 0.5),
            transcript_word("says", 0.52, 0.7),
            transcript_word("launch", 0.8, 1.1),
            transcript_word("day", 1.12, 1.4),
        ],
    });

    rebuild_project_search_index(temp.path(), &project).expect("rebuild index");

    let spoken = read_json(temp.path().join("search/spoken/media-1.json"));
    assert_eq!(spoken["mediaId"], json!("media-1"));
    assert_eq!(spoken["transcriptId"], json!("transcript-1"));
    assert_eq!(
        spoken["segments"][0]["text"],
        json!("founder says launch day")
    );
    assert_eq!(spoken["words"][2]["text"], json!("launch"));

    let visual = read_json(temp.path().join("search/visual/media-1/frames.json"));
    let media_duration = project.media[0].duration_seconds;
    assert_eq!(visual["mediaId"], json!("media-1"));
    assert_eq!(visual["status"], json!("notInstalled"));
    assert_eq!(visual["frames"][0]["timeSeconds"], json!(0.0));
    assert_eq!(
        visual["frames"][1]["timeSeconds"],
        json!(media_duration / 2.0)
    );
    assert_eq!(visual["frames"][2]["timeSeconds"], json!(media_duration));
    assert_eq!(
        visual["frames"][0]["relativeFramePath"],
        json!("search/visual/media-1/frames/frame-000000.png")
    );
    let lottie_visual = read_json(temp.path().join("search/visual/lottie-1/frames.json"));
    assert_eq!(lottie_visual["mediaId"], json!("lottie-1"));
    assert_eq!(lottie_visual["status"], json!("notInstalled"));
    assert_eq!(lottie_visual["frames"][0]["timeSeconds"], json!(0.0));
    assert_eq!(
        lottie_visual["frames"][0]["relativeFramePath"],
        json!("search/visual/lottie-1/frames/frame-000000.png")
    );
    assert!(!temp
        .path()
        .join("search/visual/audio-1/frames.json")
        .exists());
}

#[test]
fn combined_search_results_are_ranked_by_score_across_groups() {
    let index = ProjectSearchIndex {
        schema_version: 1,
        project_id: "project-test".to_string(),
        project_updated_at: "2026-06-11T00:00:00Z".to_string(),
        project_content_digest: None,
        visual_status: "notInstalled".to_string(),
        visual_indexed_asset_count: 0,
        visual_indexed_media_ids: Vec::new(),
        spoken_status: "ready".to_string(),
        media_count: 1,
        transcript_count: 1,
        generated_asset_count: 1,
        timeline_item_count: 1,
        spoken: vec![SpokenSearchEntry {
            media_id: "media-1".to_string(),
            timeline_item_id: Some("item-1".to_string()),
            transcript_id: "transcript-1".to_string(),
            text: "launch".to_string(),
            start_seconds: 0.8,
            end_seconds: 1.1,
            word_count: 1,
            words: vec![SpokenSearchWord {
                text: "launch".to_string(),
                start_seconds: 0.8,
                end_seconds: 1.1,
            }],
        }],
        metadata: vec![MetadataSearchEntry {
            text: "launch hero imported clip".to_string(),
            media_id: Some("media-1".to_string()),
            timeline_item_id: None,
            track_id: None,
            start_seconds: None,
            end_seconds: None,
            reason: "matched media metadata".to_string(),
            media: Some(json!({ "id": "media-1" })),
            label: Some("Launch hero".to_string()),
        }],
        generated: vec![GeneratedSearchEntry {
            asset_id: "generated-hero".to_string(),
            media_ids: vec!["generated-hero-output".to_string()],
            text: "launch hero generated prompt".to_string(),
            generated_asset: json!({ "id": "generated-hero" }),
        }],
    };

    let results = query_project_search_index(
        &index,
        ProjectSearchQuery {
            query: "launch hero".to_string(),
            limit: 5,
            scope: SearchScope::Both,
            media_id: None,
        },
    )
    .expect("query index");

    let result_kinds = results["results"]
        .as_array()
        .expect("results array")
        .iter()
        .map(|result| result["kind"].as_str().expect("result kind"))
        .collect::<Vec<_>>();
    assert_eq!(result_kinds, vec!["generated", "metadata", "spoken"]);
}

#[test]
fn search_matches_media_and_generated_folder_names() {
    let temp = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media_folders.push(MediaFolder {
        id: "folder-broll".to_string(),
        name: "Daily selects".to_string(),
        parent_id: None,
    });
    project.media[0].folder_id = Some("folder-broll".to_string());
    project.generated_assets.push(GeneratedAsset {
        schema_version: 1,
        id: "generated-broll".to_string(),
        kind: MediaKind::Generated,
        status: GeneratedAssetStatus::Completed,
        name: Some("Generated alternative".to_string()),
        target_folder_id: Some("folder-broll".to_string()),
        placement_intent: None,
        prompt: "alternate product angle".to_string(),
        model: GenerationModel {
            provider: "mock".to_string(),
            id: "video".to_string(),
        },
        references: GeneratedAssetReferences {
            media_ids: Vec::new(),
            first_frame_media_id: None,
            last_frame_media_id: None,
            provider_input_urls: Vec::new(),
            ..Default::default()
        },
        settings: GeneratedAssetSettings::default(),
        outputs: vec![GeneratedAssetOutput {
            media_id: "generated-broll-output".to_string(),
            relative_path: "generated/broll.mp4".to_string(),
            source_url: None,
            width: 1280,
            height: 720,
            duration_seconds: 4.0,
            fps: 24.0,
        }],
        created_at: "2026-07-01T00:00:00Z".to_string(),
        parent_asset_id: None,
        retry_of_asset_id: None,
    });
    let index = rebuild_project_search_index(temp.path(), &project).expect("rebuild index");

    let results = query_project_search_index(
        &index,
        ProjectSearchQuery {
            query: "daily selects".to_string(),
            limit: 5,
            scope: SearchScope::Both,
            media_id: None,
        },
    )
    .expect("query index");

    assert_eq!(
        results["groups"]["metadata"][0]["mediaId"],
        json!("media-1")
    );
    assert_eq!(
        results["groups"]["metadata"][0]["media"]["folderId"],
        json!("folder-broll")
    );
    assert_eq!(
        results["groups"]["generated"][0]["assetId"],
        json!("generated-broll")
    );
    assert_eq!(
        results["groups"]["generated"][0]["mediaIds"],
        json!(["generated-broll-output"])
    );
}

#[test]
fn project_dir_search_uses_fresh_stored_index() {
    let temp = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    let mut index = rebuild_project_search_index(temp.path(), &project).expect("rebuild index");
    index.metadata.push(MetadataSearchEntry {
        text: "stored only marker".to_string(),
        media_id: Some("media-1".to_string()),
        timeline_item_id: None,
        track_id: None,
        start_seconds: None,
        end_seconds: None,
        reason: "matched media metadata".to_string(),
        media: Some(json!({ "id": "media-1" })),
        label: Some("Stored only marker".to_string()),
    });
    write_index_json(temp.path(), &index);

    let results = query_project_search_for_project_dir(
        temp.path(),
        &project,
        ProjectSearchQuery {
            query: "stored only marker".to_string(),
            limit: 5,
            scope: SearchScope::Metadata,
            media_id: None,
        },
    )
    .expect("query stored index");

    assert_eq!(results["indexStatus"]["stored"], json!(true));
    assert_eq!(results["indexStatus"]["source"], json!("stored"));
    assert_eq!(
        results["groups"]["metadata"][0]["label"],
        json!("Stored only marker")
    );
}

#[test]
fn project_dir_search_reports_ready_visual_sidecars_as_indexed_assets() {
    let temp = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "lottie-1".to_string(),
        name: Some("Logo Loop".to_string()),
        relative_path: "media/logo-loop.json".to_string(),
        kind: MediaKind::Lottie,
        duration_seconds: 2.0,
        width: Some(640),
        height: Some(360),
        fps: Some(30.0),
        folder_id: None,
    });
    rebuild_project_search_index(temp.path(), &project).expect("rebuild index");
    let sidecar_path = temp.path().join("search/visual/lottie-1/frames.json");
    let mut sidecar = read_json(&sidecar_path);
    sidecar["status"] = json!("ready");
    sidecar["visualStatus"] = json!("ready");
    sidecar["embeddingModel"] = json!({
        "id": "siglip-local",
        "embeddingDim": 768
    });
    std::fs::write(
        &sidecar_path,
        serde_json::to_string_pretty(&sidecar).expect("serialize visual sidecar"),
    )
    .expect("write ready visual sidecar");

    let results = query_project_search_for_project_dir(
        temp.path(),
        &project,
        ProjectSearchQuery {
            query: "harbor sunset".to_string(),
            limit: 5,
            scope: SearchScope::Visual,
            media_id: None,
        },
    )
    .expect("query stored index");

    assert_eq!(results["visualStatus"], json!("indexing"));
    assert_eq!(results["status"], json!("indexing"));
    assert_eq!(results["visual"]["status"], json!("indexing"));
    assert_eq!(results["visual"]["indexableAssets"], json!(2));
    assert_eq!(results["visual"]["indexedAssets"], json!(1));

    let not_ready_media_results = query_project_search_for_project_dir(
        temp.path(),
        &project,
        ProjectSearchQuery {
            query: "harbor sunset".to_string(),
            limit: 5,
            scope: SearchScope::Visual,
            media_id: Some("media-1".to_string()),
        },
    )
    .expect("query stored index for not-ready media");
    assert_eq!(
        not_ready_media_results["visual"]["indexableAssets"],
        json!(1)
    );
    assert_eq!(not_ready_media_results["visual"]["indexedAssets"], json!(0));

    let ready_lottie_results = query_project_search_for_project_dir(
        temp.path(),
        &project,
        ProjectSearchQuery {
            query: "harbor sunset".to_string(),
            limit: 5,
            scope: SearchScope::Visual,
            media_id: Some("lottie-1".to_string()),
        },
    )
    .expect("query stored index for ready Lottie media");
    assert_eq!(ready_lottie_results["visual"]["indexableAssets"], json!(1));
    assert_eq!(ready_lottie_results["visual"]["indexedAssets"], json!(1));
}

#[test]
fn project_dir_search_returns_visual_moments_from_ready_sidecars() {
    let temp = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    rebuild_project_search_index(temp.path(), &project).expect("rebuild index");
    let sidecar_path = temp.path().join("search/visual/media-1/frames.json");
    let mut sidecar = read_json(&sidecar_path);
    sidecar["status"] = json!("ready");
    sidecar["visualStatus"] = json!("ready");
    sidecar["embeddingModel"] = json!({
        "id": "siglip-local",
        "embeddingDim": 768
    });
    sidecar["frames"] = json!([
        {
            "index": 0,
            "timeSeconds": 0.0,
            "shotStartSeconds": 0.0,
            "shotEndSeconds": 1.8,
            "relativeFramePath": "search/visual/media-1/frames/frame-000000.png",
            "description": "wide harbor at sunset with orange sky",
            "score": 0.91
        },
        {
            "index": 1,
            "timeSeconds": 2.0,
            "shotStartSeconds": 1.8,
            "shotEndSeconds": 3.5,
            "relativeFramePath": "search/visual/media-1/frames/frame-000001.png",
            "description": "close product detail on a studio table",
            "score": 0.42
        }
    ]);
    std::fs::write(
        &sidecar_path,
        serde_json::to_string_pretty(&sidecar).expect("serialize visual sidecar"),
    )
    .expect("write ready visual sidecar");

    let results = query_project_search_for_project_dir(
        temp.path(),
        &project,
        ProjectSearchQuery {
            query: "harbor sunset".to_string(),
            limit: 5,
            scope: SearchScope::Visual,
            media_id: None,
        },
    )
    .expect("query stored visual index");

    assert_eq!(results["visualStatus"], json!("ready"));
    assert_eq!(results["groups"]["visual"][0]["mediaId"], json!("media-1"));
    assert_eq!(results["groups"]["visual"][0]["startSeconds"], json!(0.0));
    assert_eq!(results["groups"]["visual"][0]["endSeconds"], json!(1.8));
    assert_eq!(
        results["groups"]["visual"][0]["framePath"],
        json!("search/visual/media-1/frames/frame-000000.png")
    );
    assert_eq!(
        results["visual"]["moments"][0]["mediaRef"],
        json!("media-1")
    );
    assert_eq!(results["moments"][0]["mediaRef"], json!("media-1"));
    assert_eq!(results["results"][0]["kind"], json!("visual"));
}

#[test]
fn rebuilt_visual_sidecars_seed_searchable_frame_descriptions_from_media_metadata() {
    let temp = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media[0].name = Some("Harbor sunset b-roll".to_string());
    rebuild_project_search_index(temp.path(), &project).expect("rebuild index");
    let sidecar_path = temp.path().join("search/visual/media-1/frames.json");
    let mut sidecar = read_json(&sidecar_path);
    sidecar["status"] = json!("ready");
    sidecar["visualStatus"] = json!("ready");
    sidecar["embeddingModel"] = json!({
        "id": "metadata-seeded-visual-fixture",
        "embeddingDim": 0
    });
    std::fs::write(
        &sidecar_path,
        serde_json::to_string_pretty(&sidecar).expect("serialize visual sidecar"),
    )
    .expect("write ready visual sidecar");

    let results = query_project_search_for_project_dir(
        temp.path(),
        &project,
        ProjectSearchQuery {
            query: "harbor sunset".to_string(),
            limit: 5,
            scope: SearchScope::Visual,
            media_id: None,
        },
    )
    .expect("query stored visual index");

    assert_eq!(results["visualStatus"], json!("ready"));
    assert_eq!(results["groups"]["visual"][0]["mediaId"], json!("media-1"));
    assert_eq!(
        results["groups"]["visual"][0]["reason"],
        json!("matched visual frame description")
    );
    assert_eq!(
        results["visual"]["moments"][0]["name"],
        json!("Harbor sunset b-roll")
    );
}

#[test]
fn rebuild_preserves_ready_visual_analysis_sidecars() {
    let temp = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    rebuild_project_search_index(temp.path(), &project).expect("initial rebuild");
    let sidecar_path = temp.path().join("search/visual/media-1/frames.json");
    let mut sidecar = read_json(&sidecar_path);
    sidecar["status"] = json!("ready");
    sidecar["visualStatus"] = json!("ready");
    sidecar["analyzerVersion"] = json!("frame-analysis-v1");
    sidecar["frames"][0]["description"] = json!("real extracted frame analysis");
    std::fs::write(
        &sidecar_path,
        serde_json::to_string_pretty(&sidecar).expect("serialize sidecar"),
    )
    .expect("write ready sidecar");

    rebuild_project_search_index(temp.path(), &project).expect("second rebuild");

    let preserved = read_json(sidecar_path);
    assert_eq!(preserved["visualStatus"], json!("ready"));
    assert_eq!(preserved["analyzerVersion"], json!("frame-analysis-v1"));
    assert_eq!(
        preserved["frames"][0]["description"],
        json!("real extracted frame analysis")
    );
}

#[test]
fn project_dir_visual_search_returns_a_provider_captioned_frame() {
    let temp = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    rebuild_project_search_index(temp.path(), &project).expect("initial rebuild");
    let sidecar_path = temp.path().join("search/visual/media-1/frames.json");
    let mut sidecar = read_json(&sidecar_path);
    sidecar["status"] = json!("ready");
    sidecar["visualStatus"] = json!("ready");
    sidecar["captionProvider"] = json!("fal.ai");
    sidecar["captionModel"] = json!("fal-ai/florence-2-large/caption");
    sidecar["frames"][0]["caption"] = json!("a red sports car in a bright studio");
    std::fs::write(
        &sidecar_path,
        serde_json::to_string_pretty(&sidecar).expect("serialize sidecar"),
    )
    .expect("write captioned sidecar");

    let results = query_project_search_for_project_dir(
        temp.path(),
        &project,
        ProjectSearchQuery {
            query: "sports car".to_string(),
            limit: 5,
            scope: SearchScope::Visual,
            media_id: None,
        },
    )
    .expect("query captioned visual index");

    assert_eq!(results["visualStatus"], json!("ready"));
    assert_eq!(results["groups"]["visual"][0]["mediaId"], json!("media-1"));
    assert_eq!(
        results["groups"]["visual"][0]["reason"],
        json!("matched visual frame description")
    );
}

#[test]
fn rebuild_preserves_frame_cache_pointers_without_claiming_an_embedding_model() {
    let temp = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    rebuild_project_search_index(temp.path(), &project).expect("initial rebuild");
    let sidecar_path = temp.path().join("search/visual/media-1/frames.json");
    let mut sidecar = read_json(&sidecar_path);
    sidecar["frameCache"] = json!({
        "status": "ready",
        "cacheKey": "immutable-frame-cache",
        "sourceFingerprint": "fixture-fingerprint",
        "analyzerVersion": "frame-extraction-v1",
        "samplingPolicy": "opening-middle-closing-640png-v1"
    });
    sidecar["frames"][0]["relativeFramePath"] =
        json!("search/visual/immutable-frame-cache/frames/frame-000000.png");
    std::fs::write(
        &sidecar_path,
        serde_json::to_string_pretty(&sidecar).expect("serialize sidecar"),
    )
    .expect("write cached sidecar");

    rebuild_project_search_index(temp.path(), &project).expect("second rebuild");

    let preserved = read_json(sidecar_path);
    assert_eq!(preserved["visualStatus"], json!("notInstalled"));
    assert_eq!(preserved["embeddingModel"], Value::Null);
    assert_eq!(
        preserved["frameCache"]["cacheKey"],
        json!("immutable-frame-cache")
    );
    assert_eq!(
        preserved["frames"][0]["relativeFramePath"],
        json!("search/visual/immutable-frame-cache/frames/frame-000000.png")
    );
}

#[test]
fn rebuilt_visual_sidecars_seed_frame_descriptions_from_media_analysis_moments() {
    let temp = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media[0].name = Some("Generic clip".to_string());
    project.media[0].duration_seconds = 9.0;
    project.media_analysis.push(MediaAnalysisMoment {
        media_id: "media-1".to_string(),
        source_in: 2.5,
        source_out: 5.0,
        visual_action_score: 0.91,
        audio_energy_score: 0.42,
        label: "chef flames onions in close-up".to_string(),
    });

    rebuild_project_search_index(temp.path(), &project).expect("rebuild index");
    let sidecar_path = temp.path().join("search/visual/media-1/frames.json");
    let sidecar = read_json(&sidecar_path);

    assert_eq!(
        sidecar["frames"][1]["description"],
        json!("chef flames onions in close-up")
    );
    assert_eq!(sidecar["frames"][1]["shotStartSeconds"], json!(2.5));
    assert_eq!(sidecar["frames"][1]["shotEndSeconds"], json!(5.0));
}

#[test]
fn project_dir_search_falls_back_to_memory_when_index_is_missing() {
    let temp = tempfile::tempdir().expect("project dir");
    let project = sample_project();

    let results = query_project_search_for_project_dir(
        temp.path(),
        &project,
        ProjectSearchQuery {
            query: "media".to_string(),
            limit: 5,
            scope: SearchScope::Metadata,
            media_id: None,
        },
    )
    .expect("query memory index");

    assert_eq!(results["indexStatus"]["stored"], json!(false));
    assert_eq!(results["indexStatus"]["source"], json!("memory"));
    assert_eq!(results["indexStatus"]["reason"], json!("missing"));
}

#[test]
fn project_dir_search_ignores_stale_stored_index() {
    let temp = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    let mut index = rebuild_project_search_index(temp.path(), &project).expect("rebuild index");
    index.metadata.push(MetadataSearchEntry {
        text: "stale only marker".to_string(),
        media_id: Some("media-1".to_string()),
        timeline_item_id: None,
        track_id: None,
        start_seconds: None,
        end_seconds: None,
        reason: "matched media metadata".to_string(),
        media: Some(json!({ "id": "media-1" })),
        label: Some("Stale only marker".to_string()),
    });
    write_index_json(temp.path(), &index);
    project.updated_at = "2026-07-02T00:00:00Z".to_string();

    let results = query_project_search_for_project_dir(
        temp.path(),
        &project,
        ProjectSearchQuery {
            query: "stale only marker".to_string(),
            limit: 5,
            scope: SearchScope::Metadata,
            media_id: None,
        },
    )
    .expect("query memory index");

    assert_eq!(results["indexStatus"]["stored"], json!(false));
    assert_eq!(results["indexStatus"]["source"], json!("memory"));
    assert_eq!(results["indexStatus"]["reason"], json!("stale"));
    assert_eq!(
        results["indexStatus"]["storedProjectUpdatedAt"],
        json!(index.project_updated_at)
    );
    assert_eq!(results["groups"]["metadata"], json!([]));
}

fn read_json(path: impl AsRef<std::path::Path>) -> Value {
    let contents = std::fs::read_to_string(path).expect("read JSON sidecar");
    serde_json::from_str(&contents).expect("parse JSON sidecar")
}

fn write_index_json(project_dir: &std::path::Path, index: &ProjectSearchIndex) {
    let path = project_dir.join("search/index.json");
    std::fs::write(
        path,
        serde_json::to_string_pretty(index).expect("serialize index"),
    )
    .expect("write search index");
}

fn transcript_word(text: &str, start_seconds: f64, end_seconds: f64) -> TranscriptWord {
    TranscriptWord {
        text: text.to_string(),
        start_seconds,
        end_seconds,
        confidence: Some(0.99),
        speaker: None,
    }
}
