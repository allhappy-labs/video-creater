use std::collections::BTreeMap;

use video_creater_lib::edit::captions::{
    build_caption_cues, CaptionGenerationOptions, CaptionSourceRange,
};
use video_creater_lib::edit::edl::{
    build_rough_cut_edl, build_rough_cut_edl_with_signals, validate_one_click_edl, EdlClip,
    EdlError, MomentSignal, RoughCutEdl,
};
use video_creater_lib::edit::preset::{
    CaptionStyle, EditJobRequest, EditPreset, EditRequestError, LanguageMode,
};
use video_creater_lib::edit::render_plan::{
    collect_template_render_layers, generate_one_click_edit_timeline,
    generate_one_click_edit_timeline_with_signals, RenderClip, RenderOutputProfile, RenderPlan,
    RenderQuality,
};
use video_creater_lib::edit::transcript::{
    apply_transcript_replacements, clean_transcript_text, group_words_into_caption_segments,
    parse_parakeet_transcript, parse_transcript_artifact, TranscriptParseError,
    TranscriptReplacement,
};
use video_creater_lib::project::action::{apply_project_action, ProjectAction};
use video_creater_lib::project::model::{
    MediaAsset, MediaKind, MediaSilenceRange, TimelineItem, TimelineItemKind, TimelineSource,
    TrackKind, Transcript, TranscriptWord, VideoProject,
};
use video_creater_lib::render_pipeline::backend::RenderBackend;
use video_creater_lib::render_pipeline::gstreamer_backend::GstreamerGesRenderBackend;

#[test]
fn edit_presets_have_expected_duration_ranges() {
    assert_eq!(EditPreset::TrailerCut.duration_range(), (30.0, 60.0));
    assert_eq!(EditPreset::HighlightReel.duration_range(), (45.0, 90.0));
    assert_eq!(EditPreset::StoryCut.duration_range(), (90.0, 180.0));
}

#[test]
fn edit_job_request_rejects_empty_prompt_and_bad_target_duration() {
    let request = EditJobRequest {
        media_id: "media-1".to_string(),
        preset: EditPreset::TrailerCut,
        prompt: "   ".to_string(),
        target_duration_seconds: Some(500.0),
        language_mode: LanguageMode::Auto,
        caption_style: CaptionStyle::Bold,
        created_at: "2026-06-12T00:00:00Z".to_string(),
    };

    assert_eq!(request.validate(), Err(EditRequestError::PromptRequired));
}

#[test]
fn language_modes_use_v1_wire_values() {
    assert_eq!(
        serde_json::to_value(LanguageMode::Auto).expect("serialize auto"),
        serde_json::json!("auto")
    );
    assert_eq!(
        serde_json::to_value(LanguageMode::Ukrainian).expect("serialize ukrainian"),
        serde_json::json!("uk")
    );
    assert_eq!(
        serde_json::to_value(LanguageMode::English).expect("serialize english"),
        serde_json::json!("en")
    );
}

#[test]
fn parses_runtime_transcript_artifact_words() {
    let json = serde_json::json!({
        "schemaVersion": 1,
        "mediaId": "media-1",
        "engine": "core_ml_parakeet",
        "modelId": "nvidia/parakeet-tdt-0.6b-v3",
        "runtimeId": "core_ml_parakeet",
        "languageMode": "en",
        "tokens": [
            {"token": "Strong", "start": 1.0, "end": 1.4, "confidence": 0.91},
            {"token": " hand", "start": 1.4, "end": 1.8, "confidence": null},
            {"token": ".", "start": 1.8, "end": 1.8, "confidence": null}
        ]
    });

    let transcript = parse_transcript_artifact(&json).expect("parse transcript artifact");

    assert_eq!(transcript.id, "transcript-media-1");
    assert_eq!(transcript.media_id, "media-1");
    assert_eq!(transcript.engine.as_deref(), Some("core_ml_parakeet"));
    assert_eq!(transcript.words.len(), 3);
    assert_eq!(transcript.words[0].text, "Strong");
    assert_eq!(transcript.words[0].confidence, Some(0.91));
    assert_eq!(transcript.words[1].text, "hand");
    assert_eq!(transcript.segments[0].text, "Strong hand.");
}

#[test]
fn rejects_runtime_transcript_artifact_missing_required_metadata() {
    let missing_runtime_id = serde_json::json!({
        "schemaVersion": 1,
        "mediaId": "media-1",
        "engine": "core_ml_parakeet",
        "modelId": "nvidia/parakeet-tdt-0.6b-v3",
        "languageMode": "en",
        "tokens": [
            {"token": "Strong", "start": 1.0, "end": 1.4, "confidence": null}
        ]
    });
    let missing_engine = serde_json::json!({
        "schemaVersion": 1,
        "mediaId": "media-1",
        "modelId": "nvidia/parakeet-tdt-0.6b-v3",
        "runtimeId": "core_ml_parakeet",
        "languageMode": "en",
        "tokens": [
            {"token": "Strong", "start": 1.0, "end": 1.4, "confidence": null}
        ]
    });

    assert!(parse_transcript_artifact(&missing_runtime_id).is_err());
    assert!(parse_transcript_artifact(&missing_engine).is_err());
}

#[test]
fn rejects_unsupported_transcript_artifact_schema_before_v1_shape_parse() {
    let json = serde_json::json!({
        "schemaVersion": 2,
        "mediaId": "media-1",
        "segments": []
    });

    assert!(matches!(
        parse_transcript_artifact(&json),
        Err(TranscriptParseError::UnsupportedSchemaVersion(2))
    ));
}

#[test]
fn rejects_runtime_transcript_artifact_blank_required_metadata() {
    let base = serde_json::json!({
        "schemaVersion": 1,
        "mediaId": "media-1",
        "engine": "core_ml_parakeet",
        "modelId": "nvidia/parakeet-tdt-0.6b-v3",
        "runtimeId": "core_ml_parakeet",
        "languageMode": "en",
        "tokens": [
            {"token": "Strong", "start": 1.0, "end": 1.4, "confidence": null}
        ]
    });

    for field_name in ["mediaId", "engine", "modelId", "runtimeId", "languageMode"] {
        let mut json = base.clone();
        json.as_object_mut()
            .expect("artifact object")
            .insert(field_name.to_string(), serde_json::json!(" \t "));

        assert!(matches!(
            parse_transcript_artifact(&json),
            Err(TranscriptParseError::BlankMetadata(field)) if field == field_name
        ));
    }
}

#[test]
fn rejects_runtime_transcript_artifact_with_no_usable_tokens() {
    let json = serde_json::json!({
        "schemaVersion": 1,
        "mediaId": "media-1",
        "engine": "core_ml_parakeet",
        "modelId": "nvidia/parakeet-tdt-0.6b-v3",
        "runtimeId": "core_ml_parakeet",
        "languageMode": "en",
        "tokens": [
            {"token": "[music]", "start": 1.0, "end": 1.4, "confidence": null},
            {"token": "<unk>noise</unk>", "start": 1.4, "end": 1.8, "confidence": null}
        ]
    });

    assert!(parse_transcript_artifact(&json).is_err());
}

#[test]
fn groups_transcript_words_into_caption_segments() {
    let json = serde_json::json!({
        "model": "nvidia/parakeet-tdt-0.6b-v3",
        "tokens": [
            {"token": "Сильная", "start": 1.0, "end": 1.4},
            {"token": " рука", "start": 1.4, "end": 1.8},
            {"token": ".", "start": 1.8, "end": 1.8},
            {"token": " Мощная", "start": 2.4, "end": 2.8},
            {"token": " рука", "start": 2.8, "end": 3.2}
        ]
    });
    let transcript = parse_parakeet_transcript("media-1", &json).expect("parse transcript");

    let segments = group_words_into_caption_segments(&transcript.words);

    assert_eq!(segments.len(), 2);
    assert_eq!(segments[0].text, "Сильная рука.");
    assert_eq!(segments[0].start_seconds, 1.0);
    assert_eq!(segments[1].text, "Мощная рука");
}

#[test]
fn caption_stage_splits_words_into_readable_cues_with_metadata() {
    let words = vec![
        transcript_word("This", 1.0, 1.2),
        transcript_word("opening", 1.2, 1.5),
        transcript_word("line", 1.5, 1.8),
        transcript_word("needs", 1.8, 2.0),
        transcript_word("a", 2.0, 2.1),
        transcript_word("clean", 2.1, 2.4),
        transcript_word("split", 2.4, 2.8),
        transcript_word("now", 3.5, 3.9),
    ];
    let ranges = vec![CaptionSourceRange {
        media_id: "media-1".to_string(),
        source_in: 1.0,
        source_out: 4.2,
        output_start: 0.0,
    }];

    let cues = build_caption_cues(&words, &ranges, CaptionGenerationOptions::default())
        .expect("caption cues");

    assert!(cues.len() >= 2);
    assert!(cues.iter().all(|cue| cue.text.chars().count() <= 42));
    assert!(cues.iter().all(|cue| cue.duration_seconds >= 0.6));
    assert_eq!(cues[0].start_seconds, 0.0);
    assert_eq!(cues[0].style.style_preset, "boldReadableLower");
    assert!(cues[0]
        .style
        .avoid
        .contains("full-width opaque black slabs"));
}

#[test]
fn caption_stage_remaps_source_ranges_to_output_timeline_time() {
    let words = vec![
        transcript_word("first", 10.0, 10.5),
        transcript_word("range", 10.5, 11.0),
        transcript_word("second", 40.0, 40.5),
        transcript_word("range", 40.5, 41.0),
    ];
    let ranges = vec![
        CaptionSourceRange {
            media_id: "media-1".to_string(),
            source_in: 10.0,
            source_out: 12.0,
            output_start: 0.0,
        },
        CaptionSourceRange {
            media_id: "media-1".to_string(),
            source_in: 40.0,
            source_out: 42.0,
            output_start: 2.0,
        },
    ];

    let cues = build_caption_cues(&words, &ranges, CaptionGenerationOptions::default())
        .expect("caption cues");

    assert_eq!(cues.len(), 2);
    assert_eq!(cues[0].start_seconds, 0.0);
    assert_eq!(cues[1].start_seconds, 2.0);
    assert_eq!(cues[1].source_in, 40.0);
    assert_eq!(cues[1].source_out, 41.0);
}

#[test]
fn generated_edl_is_shorter_than_source_and_uses_source_ranges() {
    let words = sample_words_across_two_minutes();
    let request = sample_edit_request(EditPreset::TrailerCut);

    let edl = build_rough_cut_edl(&request, "media-1", 120.0, &words).expect("build edl");

    assert!(edl.duration_seconds() <= 60.0);
    assert!(edl.duration_seconds() < 120.0);
    assert!(edl
        .clips
        .iter()
        .all(|clip| clip.source_out > clip.source_in));
}

#[test]
fn generated_edl_prefers_prompt_relevant_moments_over_transcript_order() {
    let mut words = (0..20)
        .map(|index| {
            let start = 1.0 + index as f64 * 1.4;
            transcript_word("welcome", start, start + 0.4)
        })
        .collect::<Vec<_>>();
    words.extend([
        transcript_word("product", 42.0, 42.4),
        transcript_word("launch", 42.5, 43.0),
        transcript_word("payoff", 43.1, 43.5),
        transcript_word("demo", 43.6, 44.0),
    ]);
    let request = EditJobRequest {
        media_id: "media-1".to_string(),
        preset: EditPreset::TrailerCut,
        prompt: "Make a product launch demo trailer with the payoff".to_string(),
        target_duration_seconds: Some(45.0),
        language_mode: LanguageMode::English,
        caption_style: CaptionStyle::Bold,
        created_at: "2026-06-12T00:00:00Z".to_string(),
    };

    let edl = build_rough_cut_edl(&request, "media-1", 60.0, &words).expect("build edl");

    assert!(
        edl.clips.first().is_some_and(|clip| clip.source_in >= 41.0),
        "expected prompt-relevant launch moment to be selected first, got {edl:?}"
    );
    assert!(edl
        .clips
        .iter()
        .any(|clip| clip.reason.contains("prompt relevance")));
}

#[test]
fn generated_edl_prefers_story_beat_moments_without_literal_prompt_matches() {
    let mut words = (0..20)
        .map(|index| {
            let start = 1.0 + index as f64 * 1.4;
            transcript_word("welcome", start, start + 0.4)
        })
        .collect::<Vec<_>>();
    words.extend([
        transcript_word("problem", 42.0, 42.4),
        transcript_word("blocked", 42.5, 42.9),
        transcript_word("solution", 43.0, 43.4),
        transcript_word("proof", 43.5, 43.9),
    ]);
    let request = EditJobRequest {
        media_id: "media-1".to_string(),
        preset: EditPreset::TrailerCut,
        prompt: "Make a compelling founder story with clear stakes".to_string(),
        target_duration_seconds: Some(30.0),
        language_mode: LanguageMode::English,
        caption_style: CaptionStyle::Bold,
        created_at: "2026-06-12T00:00:00Z".to_string(),
    };

    let edl = build_rough_cut_edl(&request, "media-1", 60.0, &words).expect("build edl");

    assert!(
        edl.clips.first().is_some_and(|clip| clip.source_in >= 41.0),
        "expected story-beat problem/solution moment to be selected first, got {edl:?}"
    );
    assert!(edl
        .clips
        .iter()
        .any(|clip| clip.reason.contains("story beat")));
}

#[test]
fn generated_edl_recognizes_before_after_transformation_story_beats() {
    let mut words = (0..20)
        .map(|index| {
            let start = 1.0 + index as f64 * 1.4;
            transcript_word("welcome", start, start + 0.4)
        })
        .collect::<Vec<_>>();
    words.extend([
        transcript_word("before", 42.0, 42.4),
        transcript_word("everything", 42.5, 42.9),
        transcript_word("felt", 43.0, 43.4),
        transcript_word("slow", 43.5, 43.9),
        transcript_word("after", 44.0, 44.4),
        transcript_word("teams", 44.5, 44.9),
        transcript_word("shipped", 45.0, 45.4),
        transcript_word("faster", 45.5, 45.9),
    ]);
    let request = EditJobRequest {
        media_id: "media-1".to_string(),
        preset: EditPreset::TrailerCut,
        prompt: "Make a transformation story with clear stakes and a payoff".to_string(),
        target_duration_seconds: Some(30.0),
        language_mode: LanguageMode::English,
        caption_style: CaptionStyle::Bold,
        created_at: "2026-06-12T00:00:00Z".to_string(),
    };

    let edl = build_rough_cut_edl(&request, "media-1", 60.0, &words).expect("build edl");

    assert!(
        edl.clips.first().is_some_and(|clip| clip.source_in >= 41.0),
        "expected before/after transformation beat to be selected first, got {edl:?}"
    );
    assert!(edl
        .clips
        .iter()
        .any(|clip| clip.reason.contains("story beat: setup-to-payoff")));
}

#[test]
fn generated_edl_prefers_visual_action_and_audio_energy_signals() {
    let words = (0..20)
        .map(|index| {
            let start = 1.0 + index as f64 * 1.4;
            transcript_word("welcome", start, start + 0.4)
        })
        .collect::<Vec<_>>();
    let signals = vec![MomentSignal {
        source_in: 42.0,
        source_out: 45.5,
        visual_action_score: 0.95,
        audio_energy_score: 0.9,
        label: "fast product handling with music hit".to_string(),
    }];
    let request = EditJobRequest {
        media_id: "media-1".to_string(),
        preset: EditPreset::TrailerCut,
        prompt: "Make a high-energy launch cut".to_string(),
        target_duration_seconds: Some(30.0),
        language_mode: LanguageMode::English,
        caption_style: CaptionStyle::Bold,
        created_at: "2026-06-12T00:00:00Z".to_string(),
    };

    let edl = build_rough_cut_edl_with_signals(&request, "media-1", 60.0, &words, &signals)
        .expect("build edl with visual/audio signals");

    assert!(
        edl.clips.first().is_some_and(|clip| clip.source_in >= 41.0),
        "expected high-energy visual action signal to be selected first, got {edl:?}"
    );
    assert!(edl
        .clips
        .iter()
        .any(|clip| clip.reason.contains("visual action signal")));
    assert!(edl
        .clips
        .iter()
        .any(|clip| clip.reason.contains("audio energy signal")));
}

#[test]
fn generated_edl_prefers_prompt_relevant_media_analysis_signal_labels() {
    let words = (0..20)
        .map(|index| {
            let start = 1.0 + index as f64 * 1.4;
            transcript_word("welcome", start, start + 0.4)
        })
        .collect::<Vec<_>>();
    let signals = vec![
        MomentSignal {
            source_in: 8.0,
            source_out: 11.5,
            visual_action_score: 0.98,
            audio_energy_score: 0.94,
            label: "generic fast motion and loud music hit".to_string(),
        },
        MomentSignal {
            source_in: 42.0,
            source_out: 45.5,
            visual_action_score: 0.55,
            audio_energy_score: 0.52,
            label: "before after transformation payoff reveal".to_string(),
        },
    ];
    let request = EditJobRequest {
        media_id: "media-1".to_string(),
        preset: EditPreset::TrailerCut,
        prompt: "Make a transformation story with before after payoff".to_string(),
        target_duration_seconds: Some(30.0),
        language_mode: LanguageMode::English,
        caption_style: CaptionStyle::Bold,
        created_at: "2026-06-12T00:00:00Z".to_string(),
    };

    let edl = build_rough_cut_edl_with_signals(&request, "media-1", 60.0, &words, &signals)
        .expect("build edl with prompt-relevant media-analysis signals");

    assert!(
        edl.clips.first().is_some_and(|clip| clip.source_in >= 41.0),
        "expected prompt-relevant media-analysis label to beat generic energy, got {edl:?}"
    );
    assert!(edl.clips.iter().any(|clip| {
        clip.reason.contains("media-analysis label relevance")
            && clip.reason.contains("story beat: setup-to-payoff")
    }));
}

#[test]
fn generated_edl_uses_semantic_media_analysis_label_relevance_for_interview_prompts() {
    let words = (0..20)
        .map(|index| {
            let start = 1.0 + index as f64 * 1.4;
            transcript_word("welcome", start, start + 0.4)
        })
        .collect::<Vec<_>>();
    let signals = vec![
        MomentSignal {
            source_in: 8.0,
            source_out: 11.5,
            visual_action_score: 0.96,
            audio_energy_score: 0.92,
            label: "generic fast montage with music hit".to_string(),
        },
        MomentSignal {
            source_in: 42.0,
            source_out: 45.5,
            visual_action_score: 0.58,
            audio_energy_score: 0.56,
            label: "talking head speaker quote with thoughtful pause".to_string(),
        },
    ];
    let request = EditJobRequest {
        media_id: "media-1".to_string(),
        preset: EditPreset::TrailerCut,
        prompt: "Make a founder interview story with the key customer insight".to_string(),
        target_duration_seconds: Some(30.0),
        language_mode: LanguageMode::English,
        caption_style: CaptionStyle::Bold,
        created_at: "2026-06-12T00:00:00Z".to_string(),
    };

    let edl = build_rough_cut_edl_with_signals(&request, "media-1", 60.0, &words, &signals)
        .expect("build edl with semantic media-analysis relevance");

    assert!(
        edl.clips.first().is_some_and(|clip| clip.source_in >= 41.0),
        "expected interview-relevant media-analysis label to beat generic energy, got {edl:?}"
    );
    assert!(edl.clips.iter().any(|clip| clip
        .reason
        .contains("semantic media-analysis relevance: interview")));
}

#[test]
fn generated_edl_uses_semantic_media_analysis_label_relevance_for_product_detail_prompts() {
    let words = (0..20)
        .map(|index| {
            let start = 1.0 + index as f64 * 1.4;
            transcript_word("welcome", start, start + 0.4)
        })
        .collect::<Vec<_>>();
    let signals = vec![
        MomentSignal {
            source_in: 8.0,
            source_out: 11.5,
            visual_action_score: 0.96,
            audio_energy_score: 0.92,
            label: "generic fast montage with music hit".to_string(),
        },
        MomentSignal {
            source_in: 42.0,
            source_out: 45.5,
            visual_action_score: 0.58,
            audio_energy_score: 0.56,
            label: "hands closeup product feature interface detail".to_string(),
        },
    ];
    let request = EditJobRequest {
        media_id: "media-1".to_string(),
        preset: EditPreset::TrailerCut,
        prompt: "Make a product feature demo with closeup details".to_string(),
        target_duration_seconds: Some(30.0),
        language_mode: LanguageMode::English,
        caption_style: CaptionStyle::Bold,
        created_at: "2026-06-12T00:00:00Z".to_string(),
    };

    let edl = build_rough_cut_edl_with_signals(&request, "media-1", 60.0, &words, &signals)
        .expect("build edl with product-detail media-analysis relevance");

    assert!(
        edl.clips.first().is_some_and(|clip| clip.source_in >= 41.0),
        "expected product-detail media-analysis label to beat generic energy, got {edl:?}"
    );
    assert!(edl.clips.iter().any(|clip| clip
        .reason
        .contains("semantic media-analysis relevance: product-detail")));
}

#[test]
fn generated_edl_uses_semantic_media_analysis_label_relevance_for_location_prompts() {
    let words = (0..20)
        .map(|index| {
            let start = 1.0 + index as f64 * 1.4;
            transcript_word("welcome", start, start + 0.4)
        })
        .collect::<Vec<_>>();
    let signals = vec![
        MomentSignal {
            source_in: 8.0,
            source_out: 11.5,
            visual_action_score: 0.96,
            audio_energy_score: 0.92,
            label: "generic fast montage with music hit".to_string(),
        },
        MomentSignal {
            source_in: 42.0,
            source_out: 45.5,
            visual_action_score: 0.58,
            audio_energy_score: 0.56,
            label: "city route map arrival venue walkthrough".to_string(),
        },
    ];
    let request = EditJobRequest {
        media_id: "media-1".to_string(),
        preset: EditPreset::TrailerCut,
        prompt: "Make a travel route map story for the venue arrival".to_string(),
        target_duration_seconds: Some(30.0),
        language_mode: LanguageMode::English,
        caption_style: CaptionStyle::Bold,
        created_at: "2026-06-12T00:00:00Z".to_string(),
    };

    let edl = build_rough_cut_edl_with_signals(&request, "media-1", 60.0, &words, &signals)
        .expect("build edl with location media-analysis relevance");

    assert!(
        edl.clips.first().is_some_and(|clip| clip.source_in >= 41.0),
        "expected location media-analysis label to beat generic energy, got {edl:?}"
    );
    assert!(edl.clips.iter().any(|clip| clip
        .reason
        .contains("semantic media-analysis relevance: location")));
}

#[test]
fn generated_edl_uses_semantic_media_analysis_label_relevance_for_reaction_prompts() {
    let words = (0..20)
        .map(|index| {
            let start = 1.0 + index as f64 * 1.4;
            transcript_word("welcome", start, start + 0.4)
        })
        .collect::<Vec<_>>();
    let signals = vec![
        MomentSignal {
            source_in: 8.0,
            source_out: 11.5,
            visual_action_score: 0.96,
            audio_energy_score: 0.92,
            label: "generic fast montage with music hit".to_string(),
        },
        MomentSignal {
            source_in: 42.0,
            source_out: 45.5,
            visual_action_score: 0.58,
            audio_energy_score: 0.56,
            label: "audience smiles and applause after the reveal".to_string(),
        },
    ];
    let request = EditJobRequest {
        media_id: "media-1".to_string(),
        preset: EditPreset::HighlightReel,
        prompt: "Make a customer reaction highlight with the emotional payoff".to_string(),
        target_duration_seconds: Some(45.0),
        language_mode: LanguageMode::English,
        caption_style: CaptionStyle::Bold,
        created_at: "2026-06-12T00:00:00Z".to_string(),
    };

    let edl = build_rough_cut_edl_with_signals(&request, "media-1", 60.0, &words, &signals)
        .expect("build edl with reaction media-analysis relevance");

    assert!(
        edl.clips.first().is_some_and(|clip| clip.source_in >= 41.0),
        "expected reaction-relevant media-analysis label to beat generic energy, got {edl:?}"
    );
    assert!(edl.clips.iter().any(|clip| clip
        .reason
        .contains("semantic media-analysis relevance: reaction")));
}

#[test]
fn generated_edl_uses_semantic_media_analysis_label_relevance_for_process_prompts() {
    let words = (0..20)
        .map(|index| {
            let start = 1.0 + index as f64 * 1.4;
            transcript_word("welcome", start, start + 0.4)
        })
        .collect::<Vec<_>>();
    let signals = vec![
        MomentSignal {
            source_in: 8.0,
            source_out: 11.5,
            visual_action_score: 0.96,
            audio_energy_score: 0.92,
            label: "generic fast montage with music hit".to_string(),
        },
        MomentSignal {
            source_in: 42.0,
            source_out: 45.5,
            visual_action_score: 0.58,
            audio_energy_score: 0.56,
            label: "step by step workflow setup walkthrough".to_string(),
        },
    ];
    let request = EditJobRequest {
        media_id: "media-1".to_string(),
        preset: EditPreset::HighlightReel,
        prompt: "Make a tutorial workflow guide showing the process".to_string(),
        target_duration_seconds: Some(45.0),
        language_mode: LanguageMode::English,
        caption_style: CaptionStyle::Bold,
        created_at: "2026-06-12T00:00:00Z".to_string(),
    };

    let edl = build_rough_cut_edl_with_signals(&request, "media-1", 60.0, &words, &signals)
        .expect("build edl with process media-analysis relevance");

    assert!(
        edl.clips.first().is_some_and(|clip| clip.source_in >= 41.0),
        "expected process-relevant media-analysis label to beat generic energy, got {edl:?}"
    );
    assert!(edl.clips.iter().any(|clip| clip
        .reason
        .contains("semantic media-analysis relevance: process")));
}

#[test]
fn generated_edl_uses_semantic_media_analysis_label_relevance_for_pricing_prompts() {
    let words = (0..20)
        .map(|index| {
            let start = 1.0 + index as f64 * 1.4;
            transcript_word("welcome", start, start + 0.4)
        })
        .collect::<Vec<_>>();
    let signals = vec![
        MomentSignal {
            source_in: 8.0,
            source_out: 11.5,
            visual_action_score: 0.96,
            audio_energy_score: 0.92,
            label: "generic fast montage with music hit".to_string(),
        },
        MomentSignal {
            source_in: 42.0,
            source_out: 45.5,
            visual_action_score: 0.58,
            audio_energy_score: 0.56,
            label: "pricing savings budget impact and roi proof".to_string(),
        },
    ];
    let request = EditJobRequest {
        media_id: "media-1".to_string(),
        preset: EditPreset::TrailerCut,
        prompt: "Make a pricing proof cut around savings and ROI".to_string(),
        target_duration_seconds: Some(30.0),
        language_mode: LanguageMode::English,
        caption_style: CaptionStyle::Bold,
        created_at: "2026-06-12T00:00:00Z".to_string(),
    };

    let edl = build_rough_cut_edl_with_signals(&request, "media-1", 60.0, &words, &signals)
        .expect("build edl with pricing media-analysis relevance");

    assert!(
        edl.clips.first().is_some_and(|clip| clip.source_in >= 41.0),
        "expected pricing-relevant media-analysis label to beat generic energy, got {edl:?}"
    );
    assert!(edl.clips.iter().any(|clip| clip
        .reason
        .contains("semantic media-analysis relevance: pricing")));
}

#[test]
fn generated_edl_uses_semantic_media_analysis_label_relevance_for_metric_prompts() {
    let words = (0..20)
        .map(|index| {
            let start = 1.0 + index as f64 * 1.4;
            transcript_word("welcome", start, start + 0.4)
        })
        .collect::<Vec<_>>();
    let signals = vec![
        MomentSignal {
            source_in: 8.0,
            source_out: 11.5,
            visual_action_score: 0.96,
            audio_energy_score: 0.92,
            label: "generic fast montage with music hit".to_string(),
        },
        MomentSignal {
            source_in: 42.0,
            source_out: 45.5,
            visual_action_score: 0.58,
            audio_energy_score: 0.56,
            label: "metric chart graph retention line".to_string(),
        },
    ];
    let request = EditJobRequest {
        media_id: "media-1".to_string(),
        preset: EditPreset::TrailerCut,
        prompt: "Make a KPI dashboard analytics cut about growth".to_string(),
        target_duration_seconds: Some(30.0),
        language_mode: LanguageMode::English,
        caption_style: CaptionStyle::Bold,
        created_at: "2026-06-12T00:00:00Z".to_string(),
    };

    let edl = build_rough_cut_edl_with_signals(&request, "media-1", 60.0, &words, &signals)
        .expect("build edl with metric media-analysis relevance");

    assert!(
        edl.clips.first().is_some_and(|clip| clip.source_in >= 41.0),
        "expected metric-relevant media-analysis label to beat generic energy, got {edl:?}"
    );
    assert!(edl.clips.iter().any(|clip| clip
        .reason
        .contains("semantic media-analysis relevance: metric")));
}

#[test]
fn generated_edl_uses_semantic_media_analysis_label_relevance_for_schedule_prompts() {
    let words = (0..20)
        .map(|index| {
            let start = 1.0 + index as f64 * 1.4;
            transcript_word("welcome", start, start + 0.4)
        })
        .collect::<Vec<_>>();
    let signals = vec![
        MomentSignal {
            source_in: 8.0,
            source_out: 11.5,
            visual_action_score: 0.96,
            audio_energy_score: 0.92,
            label: "generic fast montage with music hit".to_string(),
        },
        MomentSignal {
            source_in: 42.0,
            source_out: 45.5,
            visual_action_score: 0.58,
            audio_energy_score: 0.56,
            label: "calendar milestone schedule deadline reminder".to_string(),
        },
    ];
    let request = EditJobRequest {
        media_id: "media-1".to_string(),
        preset: EditPreset::TrailerCut,
        prompt: "Make a launch announcement timing cut".to_string(),
        target_duration_seconds: Some(30.0),
        language_mode: LanguageMode::English,
        caption_style: CaptionStyle::Bold,
        created_at: "2026-06-12T00:00:00Z".to_string(),
    };

    let edl = build_rough_cut_edl_with_signals(&request, "media-1", 60.0, &words, &signals)
        .expect("build edl with schedule media-analysis relevance");

    assert!(
        edl.clips.first().is_some_and(|clip| clip.source_in >= 41.0),
        "expected schedule-relevant media-analysis label to beat generic energy, got {edl:?}"
    );
    assert!(edl.clips.iter().any(|clip| clip
        .reason
        .contains("semantic media-analysis relevance: schedule")));
}

#[test]
fn generated_edl_uses_semantic_media_analysis_label_relevance_for_comparison_prompts() {
    let words = (0..20)
        .map(|index| {
            let start = 1.0 + index as f64 * 1.4;
            transcript_word("welcome", start, start + 0.4)
        })
        .collect::<Vec<_>>();
    let signals = vec![
        MomentSignal {
            source_in: 8.0,
            source_out: 11.5,
            visual_action_score: 0.96,
            audio_energy_score: 0.92,
            label: "generic fast montage with music hit".to_string(),
        },
        MomentSignal {
            source_in: 42.0,
            source_out: 45.5,
            visual_action_score: 0.58,
            audio_energy_score: 0.56,
            label: "side by side split screen difference".to_string(),
        },
    ];
    let request = EditJobRequest {
        media_id: "media-1".to_string(),
        preset: EditPreset::TrailerCut,
        prompt: "Make a contrast comparison cut showing the delta".to_string(),
        target_duration_seconds: Some(30.0),
        language_mode: LanguageMode::English,
        caption_style: CaptionStyle::Bold,
        created_at: "2026-06-12T00:00:00Z".to_string(),
    };

    let edl = build_rough_cut_edl_with_signals(&request, "media-1", 60.0, &words, &signals)
        .expect("build edl with comparison media-analysis relevance");

    assert!(
        edl.clips.first().is_some_and(|clip| clip.source_in >= 41.0),
        "expected comparison-relevant media-analysis label to beat generic energy, got {edl:?}"
    );
    assert!(edl.clips.iter().any(|clip| clip
        .reason
        .contains("semantic media-analysis relevance: comparison")));
}

#[test]
fn generated_edl_uses_semantic_media_analysis_label_relevance_for_tracking_prompts() {
    let words = (0..20)
        .map(|index| {
            let start = 1.0 + index as f64 * 1.4;
            transcript_word("welcome", start, start + 0.4)
        })
        .collect::<Vec<_>>();
    let signals = vec![
        MomentSignal {
            source_in: 8.0,
            source_out: 11.5,
            visual_action_score: 0.96,
            audio_energy_score: 0.92,
            label: "generic fast montage with music hit".to_string(),
        },
        MomentSignal {
            source_in: 42.0,
            source_out: 45.5,
            visual_action_score: 0.58,
            audio_energy_score: 0.56,
            label: "callout tracking highlight pointer".to_string(),
        },
    ];
    let request = EditJobRequest {
        media_id: "media-1".to_string(),
        preset: EditPreset::TrailerCut,
        prompt: "Make a spotlight annotation cut for the important detail".to_string(),
        target_duration_seconds: Some(30.0),
        language_mode: LanguageMode::English,
        caption_style: CaptionStyle::Bold,
        created_at: "2026-06-12T00:00:00Z".to_string(),
    };

    let edl = build_rough_cut_edl_with_signals(&request, "media-1", 60.0, &words, &signals)
        .expect("build edl with tracking media-analysis relevance");

    assert!(
        edl.clips.first().is_some_and(|clip| clip.source_in >= 41.0),
        "expected tracking-relevant media-analysis label to beat generic energy, got {edl:?}"
    );
    assert!(edl.clips.iter().any(|clip| clip
        .reason
        .contains("semantic media-analysis relevance: tracking")));
}

#[test]
fn generated_edl_uses_semantic_media_analysis_label_relevance_for_chapter_prompts() {
    let words = (0..20)
        .map(|index| {
            let start = 1.0 + index as f64 * 1.4;
            transcript_word("welcome", start, start + 0.4)
        })
        .collect::<Vec<_>>();
    let signals = vec![
        MomentSignal {
            source_in: 8.0,
            source_out: 11.5,
            visual_action_score: 0.96,
            audio_energy_score: 0.92,
            label: "generic fast montage with music hit".to_string(),
        },
        MomentSignal {
            source_in: 42.0,
            source_out: 45.5,
            visual_action_score: 0.58,
            audio_energy_score: 0.56,
            label: "chapter reset story break section title".to_string(),
        },
    ];
    let request = EditJobRequest {
        media_id: "media-1".to_string(),
        preset: EditPreset::TrailerCut,
        prompt: "Make a narrative turning-point cut for the next act".to_string(),
        target_duration_seconds: Some(30.0),
        language_mode: LanguageMode::English,
        caption_style: CaptionStyle::Bold,
        created_at: "2026-06-12T00:00:00Z".to_string(),
    };

    let edl = build_rough_cut_edl_with_signals(&request, "media-1", 60.0, &words, &signals)
        .expect("build edl with chapter media-analysis relevance");

    assert!(
        edl.clips.first().is_some_and(|clip| clip.source_in >= 41.0),
        "expected chapter-relevant media-analysis label to beat generic energy, got {edl:?}"
    );
    assert!(edl.clips.iter().any(|clip| clip
        .reason
        .contains("semantic media-analysis relevance: chapter")));
}

#[test]
fn generated_edl_uses_semantic_media_analysis_label_relevance_for_risk_prompts() {
    let words = (0..20)
        .map(|index| {
            let start = 1.0 + index as f64 * 1.4;
            transcript_word("welcome", start, start + 0.4)
        })
        .collect::<Vec<_>>();
    let signals = vec![
        MomentSignal {
            source_in: 8.0,
            source_out: 11.5,
            visual_action_score: 0.96,
            audio_energy_score: 0.92,
            label: "generic fast montage with music hit".to_string(),
        },
        MomentSignal {
            source_in: 42.0,
            source_out: 45.5,
            visual_action_score: 0.58,
            audio_energy_score: 0.56,
            label: "security risk compliance warning before remediation".to_string(),
        },
    ];
    let request = EditJobRequest {
        media_id: "media-1".to_string(),
        preset: EditPreset::HighlightReel,
        prompt: "Make a security risk warning highlight for compliance reviewers".to_string(),
        target_duration_seconds: Some(45.0),
        language_mode: LanguageMode::English,
        caption_style: CaptionStyle::Bold,
        created_at: "2026-06-12T00:00:00Z".to_string(),
    };

    let edl = build_rough_cut_edl_with_signals(&request, "media-1", 60.0, &words, &signals)
        .expect("build edl with risk media-analysis relevance");

    assert!(
        edl.clips.first().is_some_and(|clip| clip.source_in >= 41.0),
        "expected risk-relevant media-analysis label to beat generic energy, got {edl:?}"
    );
    assert!(edl.clips.iter().any(|clip| clip
        .reason
        .contains("semantic media-analysis relevance: risk")));
}

#[test]
fn generated_edl_uses_semantic_media_analysis_label_relevance_for_event_prompts() {
    let words = (0..20)
        .map(|index| {
            let start = 1.0 + index as f64 * 1.4;
            transcript_word("welcome", start, start + 0.4)
        })
        .collect::<Vec<_>>();
    let signals = vec![
        MomentSignal {
            source_in: 8.0,
            source_out: 11.5,
            visual_action_score: 0.96,
            audio_energy_score: 0.92,
            label: "generic fast montage with music hit".to_string(),
        },
        MomentSignal {
            source_in: 42.0,
            source_out: 45.5,
            visual_action_score: 0.58,
            audio_energy_score: 0.56,
            label: "conference keynote stage crowd booth demo".to_string(),
        },
    ];
    let request = EditJobRequest {
        media_id: "media-1".to_string(),
        preset: EditPreset::HighlightReel,
        prompt: "Make an event recap from the conference keynote and booth demos".to_string(),
        target_duration_seconds: Some(45.0),
        language_mode: LanguageMode::English,
        caption_style: CaptionStyle::Bold,
        created_at: "2026-06-12T00:00:00Z".to_string(),
    };

    let edl = build_rough_cut_edl_with_signals(&request, "media-1", 60.0, &words, &signals)
        .expect("build edl with event media-analysis relevance");

    assert!(
        edl.clips.first().is_some_and(|clip| clip.source_in >= 41.0),
        "expected event-relevant media-analysis label to beat generic energy, got {edl:?}"
    );
    assert!(edl.clips.iter().any(|clip| clip
        .reason
        .contains("semantic media-analysis relevance: event")));
}

#[test]
fn generated_edl_uses_semantic_media_analysis_label_relevance_for_brand_prompts() {
    let words = (0..20)
        .map(|index| {
            let start = 1.0 + index as f64 * 1.4;
            transcript_word("welcome", start, start + 0.4)
        })
        .collect::<Vec<_>>();
    let signals = vec![
        MomentSignal {
            source_in: 8.0,
            source_out: 11.5,
            visual_action_score: 0.96,
            audio_energy_score: 0.92,
            label: "generic fast montage with music hit".to_string(),
        },
        MomentSignal {
            source_in: 42.0,
            source_out: 45.5,
            visual_action_score: 0.58,
            audio_energy_score: 0.56,
            label: "logo reveal brand identity signage".to_string(),
        },
    ];
    let request = EditJobRequest {
        media_id: "media-1".to_string(),
        preset: EditPreset::TrailerCut,
        prompt: "Make a brand launch cut centered on the logo identity".to_string(),
        target_duration_seconds: Some(30.0),
        language_mode: LanguageMode::English,
        caption_style: CaptionStyle::Bold,
        created_at: "2026-06-12T00:00:00Z".to_string(),
    };

    let edl = build_rough_cut_edl_with_signals(&request, "media-1", 60.0, &words, &signals)
        .expect("build edl with brand media-analysis relevance");

    assert!(
        edl.clips.first().is_some_and(|clip| clip.source_in >= 41.0),
        "expected brand-relevant media-analysis label to beat generic energy, got {edl:?}"
    );
    assert!(edl.clips.iter().any(|clip| clip
        .reason
        .contains("semantic media-analysis relevance: brand")));
}

#[test]
fn generated_edl_derives_visual_action_and_audio_energy_from_timed_transcript_metadata() {
    let mut words = (0..20)
        .map(|index| {
            let start = 1.0 + index as f64 * 1.4;
            transcript_word("welcome", start, start + 0.4)
        })
        .collect::<Vec<_>>();
    words.extend([
        transcript_word("fast", 42.0, 42.11),
        transcript_word("cuts", 42.15, 42.27),
        transcript_word("tight", 42.30, 42.41),
        transcript_word("pace", 42.45, 42.56),
        transcript_word("bright", 42.60, 42.71),
        transcript_word("impact", 42.75, 42.88),
    ]);
    let request = EditJobRequest {
        media_id: "media-1".to_string(),
        preset: EditPreset::TrailerCut,
        prompt: "Make a high-energy launch cut".to_string(),
        target_duration_seconds: Some(30.0),
        language_mode: LanguageMode::English,
        caption_style: CaptionStyle::Bold,
        created_at: "2026-06-12T00:00:00Z".to_string(),
    };

    let edl = build_rough_cut_edl(&request, "media-1", 60.0, &words)
        .expect("build edl with derived visual/audio signals");

    assert!(
        edl.clips.first().is_some_and(|clip| clip.source_in >= 41.0),
        "expected derived high-cadence timed transcript metadata to win scoring, got {edl:?}"
    );
    assert!(edl
        .clips
        .iter()
        .any(|clip| clip.reason.contains("derived visual action signal")));
    assert!(edl
        .clips
        .iter()
        .any(|clip| clip.reason.contains("derived audio energy signal")));
}

#[test]
fn edl_validation_rejects_full_source_pass_through() {
    let edl = RoughCutEdl {
        media_id: "media-1".to_string(),
        source_duration_seconds: 54.0,
        source_durations_seconds: std::collections::BTreeMap::from([("media-1".to_string(), 54.0)]),
        clips: vec![EdlClip {
            media_id: "media-1".to_string(),
            source_in: 0.0,
            source_out: 54.0,
            reason: "bad full pass".to_string(),
            selection_reasons: vec!["bad full pass".to_string()],
            score: None,
        }],
    };

    assert_eq!(
        validate_one_click_edl(&EditPreset::TrailerCut, &edl),
        Err(EdlError::FullSourcePassThrough)
    );
}

#[test]
fn rough_cut_edl_becomes_timeline_items_with_source_ranges() {
    let mut project = sample_project_with_media_and_transcript();
    let request = sample_edit_request(EditPreset::TrailerCut);

    let result = generate_one_click_edit_timeline(&mut project, request).expect("generate edit");

    assert!(result.timeline_duration_seconds < 120.0);
    let video_items = project
        .timeline
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Video)
        .expect("video track")
        .items
        .clone();
    assert!(!video_items.is_empty());
    assert!(video_items
        .iter()
        .all(|item| item.properties.contains_key("sourceIn")));
    assert!(video_items
        .iter()
        .all(|item| item.properties.contains_key("sourceOut")));
}

#[test]
fn generated_timeline_can_use_visual_action_and_audio_energy_signals() {
    let mut project = sample_project_with_media_and_transcript();
    let request = EditJobRequest {
        media_id: "media-1".to_string(),
        preset: EditPreset::TrailerCut,
        prompt: "Make a high-energy launch cut".to_string(),
        target_duration_seconds: Some(30.0),
        language_mode: LanguageMode::English,
        caption_style: CaptionStyle::Bold,
        created_at: "2026-06-12T00:00:00Z".to_string(),
    };
    let signals = vec![MomentSignal {
        source_in: 42.0,
        source_out: 45.5,
        visual_action_score: 0.95,
        audio_energy_score: 0.9,
        label: "fast product handling with music hit".to_string(),
    }];

    let result = generate_one_click_edit_timeline_with_signals(&mut project, request, &signals)
        .expect("generate edit with moment signals");

    assert!(result
        .edl
        .clips
        .iter()
        .any(|clip| clip.reason.contains("visual action signal")));
    let first_video_item = project
        .timeline
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Video)
        .expect("video track")
        .items
        .first()
        .expect("first generated video item");
    assert_eq!(
        first_video_item.properties["sourceIn"],
        serde_json::json!(42.0)
    );
}

#[test]
fn generated_timeline_avoids_stored_media_silence_ranges() {
    let mut project = sample_project_with_media_and_transcript();
    project.media_silence_ranges = vec![MediaSilenceRange {
        media_id: "media-1".to_string(),
        source_in: 41.0,
        source_out: 44.0,
        confidence: 0.91,
        label: "speech-free dead air".to_string(),
    }];
    let request = EditJobRequest {
        media_id: "media-1".to_string(),
        preset: EditPreset::TrailerCut,
        prompt: "Make a high-energy launch cut".to_string(),
        target_duration_seconds: Some(30.0),
        language_mode: LanguageMode::English,
        caption_style: CaptionStyle::Bold,
        created_at: "2026-06-12T00:00:00Z".to_string(),
    };
    let signals = vec![MomentSignal {
        source_in: 42.0,
        source_out: 45.5,
        visual_action_score: 0.95,
        audio_energy_score: 0.9,
        label: "fast product handling with music hit".to_string(),
    }];

    let result = generate_one_click_edit_timeline_with_signals(&mut project, request, &signals)
        .expect("generate edit with stored dead-air analysis");

    assert!(
        result
            .edl
            .clips
            .iter()
            .any(|clip| clip.reason.contains("stored dead air")),
        "expected EDL to record stored dead-air avoidance, got {:?}",
        result.edl
    );
    for clip in &result.edl.clips {
        assert!(
            clip.source_out <= 41.0 || clip.source_in >= 44.0,
            "EDL clip should not overlap stored dead air: {clip:?}"
        );
    }
    for item in project
        .timeline
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Video)
        .expect("video track")
        .items
        .iter()
    {
        let source_in = item.properties["sourceIn"]
            .as_f64()
            .expect("generated item sourceIn");
        let source_out = item.properties["sourceOut"]
            .as_f64()
            .expect("generated item sourceOut");
        assert!(
            source_out <= 41.0 || source_in >= 44.0,
            "generated timeline item should not overlap stored dead air: {item:?}"
        );
    }
}

#[test]
fn generated_timeline_uses_stored_media_analysis_moment_signals() {
    let project = sample_project_with_media_and_transcript();
    let mut project_json = serde_json::to_value(project).expect("serialize project");
    project_json["mediaAnalysis"] = serde_json::json!([
        {
            "mediaId": "media-1",
            "sourceIn": 42.0,
            "sourceOut": 45.5,
            "visualActionScore": 0.94,
            "audioEnergyScore": 0.88,
            "label": "real media analysis: fast product handling with music hit"
        },
        {
            "mediaId": "unrelated-media",
            "sourceIn": 5.0,
            "sourceOut": 8.0,
            "visualActionScore": 1.0,
            "audioEnergyScore": 1.0,
            "label": "unrelated media should not affect this edit"
        }
    ]);
    let mut project: VideoProject =
        serde_json::from_value(project_json).expect("deserialize project with media analysis");
    let request = EditJobRequest {
        media_id: "media-1".to_string(),
        preset: EditPreset::TrailerCut,
        prompt: "Make a high-energy launch cut".to_string(),
        target_duration_seconds: Some(30.0),
        language_mode: LanguageMode::English,
        caption_style: CaptionStyle::Bold,
        created_at: "2026-06-12T00:00:00Z".to_string(),
    };

    let result = generate_one_click_edit_timeline(&mut project, request)
        .expect("generate edit from stored media analysis");

    assert!(
        result
            .edl
            .clips
            .first()
            .is_some_and(|clip| clip.source_in >= 41.0),
        "expected stored real media-analysis signal to drive the first clip, got {:?}",
        result.edl
    );
    assert!(result.edl.clips.iter().any(|clip| {
        clip.reason.contains("visual action signal")
            && clip.reason.contains("audio energy signal")
            && clip.reason.contains("real media analysis")
    }));
}

#[test]
fn generated_timeline_uses_deterministic_caption_stage_metadata() {
    let mut project = sample_project_with_dense_caption_words();
    let request = sample_edit_request(EditPreset::TrailerCut);

    let result = generate_one_click_edit_timeline(&mut project, request).expect("generate edit");

    let caption_items = project
        .timeline
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Caption)
        .expect("caption track")
        .items
        .clone();

    assert!(caption_items.len() > result.clip_count);
    assert!(caption_items
        .iter()
        .all(|item| item.properties.contains_key("visualTreatment")));
    assert!(caption_items
        .iter()
        .all(|item| item.properties.contains_key("motion")));
    assert!(caption_items
        .iter()
        .all(|item| item.properties.contains_key("safeZone")));
    assert!(caption_items
        .iter()
        .all(|item| item.properties.contains_key("avoid")));
    assert!(caption_items
        .iter()
        .all(|item| item.properties["textEdited"] == serde_json::json!(false)));
}

#[test]
fn rerunning_generated_timeline_preserves_user_edited_caption_text() {
    let mut project = sample_project_with_dense_caption_words();
    let request = sample_edit_request(EditPreset::TrailerCut);

    generate_one_click_edit_timeline(&mut project, request.clone()).expect("generate edit");
    let first_caption_id = caption_track_items(&project)
        .first()
        .expect("first caption")
        .id
        .clone();

    apply_project_action(
        &mut project,
        ProjectAction::EditCaptionText {
            item_id: first_caption_id.clone(),
            text: "Corrected agency-ready caption".to_string(),
        },
    )
    .expect("edit caption");

    generate_one_click_edit_timeline(&mut project, request).expect("rerun generate edit");
    let first_caption = caption_track_items(&project)
        .iter()
        .find(|item| item.id == first_caption_id)
        .expect("preserved first caption");

    assert_eq!(
        caption_text(first_caption),
        "Corrected agency-ready caption"
    );
    assert_eq!(
        first_caption.properties["textEdited"],
        serde_json::json!(true)
    );
}

#[test]
fn render_plan_builds_gstreamer_ges_metadata_shape() {
    let plan = sample_render_plan();
    let command = GstreamerGesRenderBackend::new()
        .build_command(&plan, &[])
        .expect("GStreamer/GES command metadata");

    assert_eq!(command.program, "gstreamer-ges");
    assert!(command
        .args
        .contains(&"--input=media/input.mp4".to_string()));
    assert!(command
        .args
        .contains(&"--output=renders/draft.webm".to_string()));
    assert!(command
        .args
        .contains(&"--clip-source=media/input.mp4".to_string()));
    assert!(command.args.contains(&"--clip-start=0.000".to_string()));
    assert!(command.args.contains(&"--clip-start=3.000".to_string()));
    assert!(command.args.contains(&"--clip=1.000..4.000".to_string()));
}

#[test]
fn render_plan_collects_template_overlay_layers_from_timeline() {
    let mut project = sample_project_with_media_and_transcript();
    let overlay_track = project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.kind == TrackKind::Overlay)
        .expect("overlay track");
    let mut properties = std::collections::BTreeMap::new();
    properties.insert(
        "templateId".to_string(),
        serde_json::json!("kinetic-lower-third-v1"),
    );
    properties.insert(
        "templateFields".to_string(),
        serde_json::json!({
            "headline": "Olha API",
            "subline": "Founder",
        }),
    );
    properties.insert(
        "visualTreatment".to_string(),
        serde_json::json!("compact lower-third block with translucent backing"),
    );
    properties.insert("motion".to_string(), serde_json::json!("slide-and-fade in"));
    properties.insert(
        "safeZone".to_string(),
        serde_json::json!("keep essential text inside 10% margins"),
    );
    properties.insert(
        "avoid".to_string(),
        serde_json::json!("full-width opaque black slabs"),
    );
    properties.insert(
        "previewVariant".to_string(),
        serde_json::json!("lower-third"),
    );
    overlay_track.items.push(TimelineItem {
        id: "template-item-1".to_string(),
        kind: TimelineItemKind::Overlay,
        start_seconds: 1.2,
        duration_seconds: 2.4,
        source: TimelineSource::Generated {
            artifact_id: "template:kinetic-lower-third-v1:template-item-1".to_string(),
        },
        label: "Kinetic Lower Third".to_string(),
        properties,
    });

    let layers = collect_template_render_layers(&project).expect("template layers");

    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].template_id, "kinetic-lower-third-v1");
    assert_eq!(layers[0].timeline_start_seconds, 1.2);
    assert_eq!(layers[0].duration_seconds, 2.4);
    assert_eq!(layers[0].fields["headline"], "Olha API");
    assert_eq!(layers[0].preview_variant, "lower-third");
}

#[test]
fn transcript_cleanup_removes_hallucination_markers_and_normalizes_spacing() {
    let cleaned = clean_transcript_text("  Привіт   [music] <noise>тест</noise>  світе  ! ");

    assert_eq!(cleaned, "Привіт світе!");
}

#[test]
fn transcript_replacements_are_longest_first_and_case_insensitive() {
    let replacements = vec![
        TranscriptReplacement {
            originals: vec!["відео юз".to_string(), "video use".to_string()],
            replacement: "video-use".to_string(),
        },
        TranscriptReplacement {
            originals: vec!["кодекс".to_string()],
            replacement: "Codex".to_string(),
        },
    ];

    let replaced =
        apply_transcript_replacements("Кодекс запускає відео юз для монтажу", &replacements);

    assert_eq!(replaced, "Codex запускає video-use для монтажу");
}

fn sample_edit_request(preset: EditPreset) -> EditJobRequest {
    EditJobRequest {
        media_id: "media-1".to_string(),
        preset,
        prompt: "Make an action edit with captions".to_string(),
        target_duration_seconds: None,
        language_mode: LanguageMode::English,
        caption_style: CaptionStyle::Bold,
        created_at: "2026-06-12T00:00:00Z".to_string(),
    }
}

fn sample_words_across_two_minutes() -> Vec<TranscriptWord> {
    (0..36)
        .map(|index| {
            let start = index as f64 * 3.0;
            TranscriptWord {
                text: if index % 5 == 0 {
                    "мощно".to_string()
                } else {
                    format!("word-{index}")
                },
                start_seconds: start,
                end_seconds: start + 0.8,
                confidence: Some(0.9),
                speaker: None,
            }
        })
        .collect()
}

fn transcript_word(text: &str, start_seconds: f64, end_seconds: f64) -> TranscriptWord {
    TranscriptWord {
        text: text.to_string(),
        start_seconds,
        end_seconds,
        confidence: Some(0.95),
        speaker: None,
    }
}

fn caption_track_items(project: &VideoProject) -> &[TimelineItem] {
    project
        .timeline
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Caption)
        .expect("caption track")
        .items
        .as_slice()
}

fn caption_text(item: &TimelineItem) -> &str {
    let TimelineSource::Text { text } = &item.source else {
        panic!("caption item should use text source");
    };
    text
}

fn sample_project_with_media_and_transcript() -> VideoProject {
    let mut project = VideoProject::new_empty(
        "project-1".to_string(),
        "One Click Test".to_string(),
        "2026-06-12T00:00:00Z".to_string(),
    );
    project.media.push(MediaAsset {
        id: "media-1".to_string(),
        name: None,
        relative_path: "media/input.mp4".to_string(),
        kind: MediaKind::Video,
        duration_seconds: 120.0,
        width: Some(1080),
        height: Some(1920),
        fps: Some(30.0),
        folder_id: None,
    });
    project.transcripts.push(Transcript {
        id: "transcript-media-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("nvidia/parakeet-tdt-0.6b-v3".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: sample_words_across_two_minutes(),
    });

    project
}

fn sample_project_with_dense_caption_words() -> VideoProject {
    let mut project = VideoProject::new_empty(
        "project-1".to_string(),
        "Dense Caption Test".to_string(),
        "2026-06-13T00:00:00Z".to_string(),
    );
    project.media.push(MediaAsset {
        id: "media-1".to_string(),
        name: None,
        relative_path: "media/input.mp4".to_string(),
        kind: MediaKind::Video,
        duration_seconds: 120.0,
        width: Some(1080),
        height: Some(1920),
        fps: Some(30.0),
        folder_id: None,
    });

    let words = (0..18)
        .map(|index| {
            let start = 5.0 + index as f64 * 0.35;
            transcript_word(&format!("caption-{index}"), start, start + 0.22)
        })
        .collect();

    project.transcripts.push(Transcript {
        id: "transcript-media-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("nvidia/parakeet-tdt-0.6b-v3".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words,
    });

    project
}

fn sample_render_plan() -> RenderPlan {
    RenderPlan {
        input_path: "media/input.mp4".to_string(),
        output_path: "renders/draft.webm".to_string(),
        width: 1080,
        height: 1920,
        fps: 30.0,
        quality: RenderQuality::Draft,
        output_profile: RenderOutputProfile::default(),
        encode_tier: video_creater_lib::edit::render_plan::ExportEncodeTier::Standard,
        clips: vec![
            RenderClip {
                source_path: None,
                timeline_start_seconds: None,
                properties: BTreeMap::new(),
                timeline_track_index: 0,
                source_in: 1.0,
                source_out: 4.0,
            },
            RenderClip {
                source_path: None,
                timeline_start_seconds: None,
                properties: BTreeMap::new(),
                timeline_track_index: 0,
                source_in: 10.0,
                source_out: 14.5,
            },
        ],
        audio_clips: Vec::new(),
        transitions: Vec::new(),
        audio_transitions: Vec::new(),
    }
}
