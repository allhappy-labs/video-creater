# Agent Transcript Access And Core ML Import Design

## Goal

Give edit agents exact transcript text and timings on demand, and remove the current Core ML model-artifact blocker by supporting a global Parakeet Core ML bundle import and durable inspection report.

This does not replace the existing Rust-owned project mutation model. Agents can read transcript ranges and propose cuts or transcript repairs, but canonical edits still flow through validated project actions and proposal validation.

## Context

The app already stores transcripts in project state and split-project sidecars. Each `TranscriptWord` has `text`, `startSeconds`, `endSeconds`, `confidence`, and `speaker`. The initial Codex edit context currently includes only the first 80 words as `transcriptExcerpt`, which is useful for orientation but insufficient for long source media or precise video/audio range selection.

The Core ML transcription runtime is scaffolded behind `TranscriptionBackend`, but `CoreMlParakeetBackend` still stops after bundle-existence checks with `Core ML Parakeet decode loop is not producing tokens yet`. Local scanning found no installed `.mlmodelc` bundle under the app model directory. The next durable step is to let users import or copy a known-good FluidAudio/VoiceInk-compatible bundle into the app-global model store, verify it, and save an IO inspection report that the Rust decoder can target.

## References

- VoiceInk repository: https://github.com/Beingpax/VoiceInk
- FluidAudio repository: https://github.com/FluidInference/FluidAudio
- FluidAudio model catalog includes `FluidInference/parakeet-tdt-0.6b-v3-coreml`.
- FluidAudio ASR design uses Parakeet TDT token timestamps, with encoder frame timestamps convertible to seconds by `frameIndex * 0.08`.
- FluidAudio word-timing tests group SentencePiece/subword token timings into word-level spans.

## Agent Transcript Range Tool

Add a new read-only local Codex tool:

```text
video_creater.transcript_words
```

Input schema:

```json
{
  "type": "object",
  "required": ["mediaId"],
  "additionalProperties": false,
  "properties": {
    "mediaId": { "type": "string", "minLength": 1 },
    "startSeconds": { "type": "number", "minimum": 0 },
    "endSeconds": { "type": "number", "exclusiveMinimum": 0 },
    "offset": { "type": "integer", "minimum": 0 },
    "limit": { "type": "integer", "minimum": 1, "maximum": 500 }
  }
}
```

Behavior:

- Finds the transcript for `mediaId`.
- Applies optional time filtering:
  - include a word if its span overlaps `[startSeconds, endSeconds]`;
  - reject `endSeconds <= startSeconds` when both are supplied.
- Applies `offset` and `limit` after time filtering.
- Defaults `offset` to `0` and `limit` to `200`.
- Caps `limit` at `500` to keep tool output bounded.
- Returns enough metadata for agents to page deterministically.

Payload shape:

```json
{
  "mediaId": "media-1",
  "transcriptId": "transcript-media-1",
  "engine": "core_ml_parakeet",
  "rawArtifactPath": "workflow-artifacts/transcribe-1/transcribe-1-transcript.json",
  "range": { "startSeconds": 12.0, "endSeconds": 45.0 },
  "offset": 0,
  "limit": 200,
  "totalWords": 1400,
  "matchedWords": 94,
  "returnedWords": 94,
  "nextOffset": null,
  "truncated": false,
  "words": [
    {
      "wordIndex": 51,
      "text": "example",
      "startSeconds": 12.24,
      "endSeconds": 12.58,
      "confidence": 0.96,
      "speaker": null
    }
  ]
}
```

Agents should use this tool for:

- selecting exact `sourceIn` and `sourceOut` ranges;
- checking context around candidate moments;
- repairing caption/transcript words with known `transcriptId` and `wordIndex`;
- aligning overlays and captions to spoken content.

The initial prompt should continue to include a small excerpt and split-project file hints. The tool is for precise range access, not for dumping every transcript into every prompt.

## Core ML Import And Inspection

Add a global model import flow for the existing app-global model root:

```text
~/Library/Application Support/com.olhapi.video-creater/models/nvidia__parakeet-tdt-0.6b-v3
```

The import flow accepts a source directory containing:

- `Preprocessor.mlmodelc`
- `Encoder.mlmodelc`
- `Decoder.mlmodelc`
- `JointDecisionv3.mlmodelc`
- `config.json`
- `parakeet_v3_vocab.json`

Behavior:

- Reject paths that do not exist or do not contain the required files.
- Copy the model into the global app model directory atomically via a temporary directory and rename.
- Write the existing `video-creater-model-manifest.json` only after verification succeeds.
- Record `installedAt` and `verifiedAt`.
- Preserve the existing active model id behavior.
- Expose the imported model in model settings as ready once verified.

Add or extend a Tauri command:

```text
import_transcription_model(modelId, sourcePath)
```

This should return `TranscriptionModelStatus`. The UI can add a later picker around it; the first implementation can be tested through command-level Rust tests and a simple bridge function.

Keep `download_transcription_model` rejected for Core ML catalog models unless a first-party download source is explicitly configured later. This keeps offline/global-bundle behavior honest.

## Inspection Report

Extend `video-creater-inspect-coreml-parakeet` so it can produce JSON:

```text
video-creater-inspect-coreml-parakeet <model-root> --json
```

Output shape:

```json
{
  "schemaVersion": 1,
  "modelId": "nvidia/parakeet-tdt-0.6b-v3",
  "runtimeId": "core_ml_parakeet",
  "bundles": [
    {
      "name": "Encoder.mlmodelc",
      "path": ".../Encoder.mlmodelc",
      "inputs": [
        { "name": "audio_signal", "featureType": "multiArray", "shape": [1, 80, 1500], "dataType": "float32", "optional": false }
      ],
      "outputs": [
        { "name": "encoder_output", "featureType": "multiArray", "shape": [1, 188, 1024], "dataType": "float32" }
      ]
    }
  ]
}
```

The import verification should save the inspection report beside the manifest, for example:

```text
video-creater-coreml-inspection.json
```

This file becomes the contract for the Rust decode loop. If an imported bundle’s IO names or shapes differ, the runtime can fail with a precise validation error before inference.

## Native Decode Loop Plan

After an imported bundle and inspection report exist, replace the current decode-loop stub with real inference in layers:

1. Load `parakeet_v3_vocab.json` into a tokenizer/vocabulary adapter.
2. Decode or resample media to the audio input shape expected by `Preprocessor.mlmodelc`.
3. Run Core ML model chain:
   - preprocessor;
   - encoder;
   - decoder;
   - joint decision.
4. Implement TDT v3 decoding:
   - token prediction;
   - duration prediction;
   - blank handling;
   - max-symbol guard per frame;
   - timestamps in encoder frame units.
5. Convert token timings to word timings:
   - strip SentencePiece boundary markers;
   - skip special tokens such as blank;
   - group subword tokens into words;
   - word start is first token start, word end is last token end.
6. Emit `TranscriptionRuntimeOutput` with non-empty tokens and `runtimeId = core_ml_parakeet`.

Frame-to-time conversion should follow the inspected/configured model contract. FluidAudio’s current Parakeet v3 path uses `frameIndex * 0.08` seconds, but this should be expressed as a constant or metadata-derived value and covered by tests.

## Error Handling

Transcript tool errors:

- media missing: return structured `ready: false`-style error payload or local tool error, matching existing local tool conventions;
- transcript missing: return a payload with `words: []`, `truncated: false`, and an `error` string;
- invalid time range: reject tool arguments.

Core ML import errors:

- missing required file: include the missing relative path;
- source equals destination: verify in place instead of recopying;
- copy failure: leave previous installed model untouched;
- inspection failure: keep copied files but report `failed` until a later verify succeeds;
- unsupported platform: import may copy files, but runtime remains `unsupported_platform` outside macOS.

Decode errors:

- missing inspection report;
- model IO mismatch;
- media decode/resample failure;
- Core ML prediction failure;
- empty token output;
- no usable word timings after token grouping.

All decode failures should fail the Temporal activity before transcript storage, so project transcript state is not corrupted by partial output.

## Tests

Rust tests:

- `video_creater.transcript_words` returns word indices and timing fields for a full transcript.
- transcript range filters by overlap and paginates deterministically.
- invalid ranges are rejected.
- missing media/transcript behavior is structured and documented.
- Core ML import rejects missing required bundles.
- Core ML import copies a fake bundle tree into the global model root and writes a manifest only after verification-compatible files exist.
- inspector JSON includes all four expected bundles with inputs and outputs on macOS; non-macOS returns an unsupported error.
- backend still reports unsupported outside macOS.
- env-gated integration test transcribes a short WAV only when both `VIDEO_CREATER_TEST_PARAKEET_MODEL_ROOT` and `VIDEO_CREATER_TEST_TRANSCRIPTION_WAV` are set.

Frontend tests:

- bridge exposes `importTranscriptionModel(modelId, sourcePath)`.
- model settings copy still says Core ML/global bundle, not Python or Transformers.

Verification:

- `rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_app_server`
- `rtk cargo test --manifest-path src-tauri/Cargo.toml --test transcription_models`
- `rtk cargo test --manifest-path src-tauri/Cargo.toml --test temporal_workflows`
- `rtk cargo check --manifest-path src-tauri/Cargo.toml --bin video-creater-temporal-worker`
- `rtk pnpm test src/App.test.tsx src/components/settings/model-settings.test.tsx`

## Non-Goals

- Do not add Python, Transformers, or a Python sidecar.
- Do not make agents mutate transcript or timeline files directly.
- Do not auto-download Core ML bundles from Hugging Face in this pass.
- Do not port all of FluidAudio. Use it as a reference for model layout, timestamps, and word timing behavior.
- Do not block transcript range access on completing the Core ML decode loop.

## Open Operational Requirement

To complete the real decode-loop integration, a compatible Parakeet v3 Core ML bundle must be available locally. The expected source is a directory equivalent to FluidAudio’s `FluidInference/parakeet-tdt-0.6b-v3-coreml` layout or a VoiceInk-installed FluidAudio model directory. Once supplied, run the inspector and wire the backend against the saved report.
