# One-Click Agent Edit Design

## Summary

Build a one-click agent-assisted editing pipeline inside the existing Tauri app. The pipeline imports a source video, transcribes it with Parakeet 3, selects the strongest moments, creates a real rough-cut edit decision list, adds captions and visual layers, and renders a shorter draft MP4. The default behavior must not be a full-length source video with captions and effects applied over it.

The implementation keeps Rust as the authority for project state, job orchestration, validation, ffmpeg execution, render planning, logging, and cancellation. Parakeet 3 runs through a narrow local Python sidecar for the MVP because the model support is mature there. The Rust code uses a trait boundary so a Rust-native or ONNX implementation can replace the Python runner later.

## Goals

- Add a one-click `Generate edit` workflow to the app.
- Support three starting presets: `Trailer Cut`, `Highlight Reel`, and `Story Cut / Long Reel`.
- Let the user adjust the preset with a freeform prompt before generation.
- Transcribe imported media with Parakeet 3 and store word-level timestamps.
- Generate a shorter rough-cut timeline from the source media.
- Add captions, titles, highlights, HyperFrames scenes, and overlay layers to the unified timeline.
- Render a playable MP4 from the generated timeline.
- Validate that one-click edits are real cuts, not styled full-source pass-through renders.

## Non-Goals

- Rust-native Parakeet inference in the first implementation.
- Cloud transcription, cloud rendering, or hosted project state.
- video-use API or file compatibility.
- Replacing the existing unified project timeline model.
- Full nonlinear editor parity in the first implementation.

## User Workflow

The primary workflow is one-click agent editing:

1. The user imports a video.
2. The user chooses a preset.
3. The user optionally edits the prompt that adjusts tone, subject emphasis, pacing, caption style, and overlay direction.
4. The user clicks `Generate edit`.
5. Rust starts an edit job and reports progress in the agent panel.
6. The app produces a generated timeline and a draft MP4.
7. The user previews the draft, adjusts timeline items if needed, regenerates, or renders a final MP4.

The preset supplies default duration and style rules. The prompt supplies intent. For example:

```text
Preset: Trailer Cut
Prompt: Make it like an action movie. Emphasize the hand, use bold captions, and add dramatic title cards.
```

## Presets

### Trailer Cut

Target duration: 30 to 60 seconds.

Use for cinematic, punchy edits with a strong hook, dramatic pacing, title cards, and high-impact captions. This preset can use aggressive cuts, flash/highlight overlays, and HyperFrames title or transition assets.

### Highlight Reel

Target duration: 45 to 90 seconds.

Use for creator-native recap edits. The edit should keep the best spoken, funny, visual, or action moments while preserving enough context to make the result understandable.

### Story Cut / Long Reel

Target duration: 90 to 180 seconds.

Use for longer narrative edits that keep context. The output can include chapter-like beat labels, less aggressive cutting, and captions optimized for watching with sound off.

This preset aligns with current short-video platform direction: Instagram Reels and YouTube Shorts both moved to support videos up to 3 minutes, while TikTok has continued investing in longer-video and long-to-short workflows such as Smart Split.

## Architecture

The pipeline is Rust-orchestrated with narrow replaceable sidecars.

### Frontend

The React TypeScript frontend adds:

- preset selector in the agent panel
- prompt textarea
- `Generate edit` button
- job status and log summary
- latest draft artifact link
- preview playback for the latest draft MP4

Visible controls and layout primitives continue to use shadcn/ui components. Editor-specific surfaces such as the timeline and media preview can use custom rendering where shadcn/ui does not provide the primitive.

### Rust Backend

Rust owns:

- project state and timeline mutation
- media import and path validation
- ffprobe metadata extraction
- audio extraction to mono 16 kHz WAV
- Parakeet sidecar supervision
- transcript parsing and storage
- preset duration rules
- moment scoring and rough-cut EDL creation
- Codex app-server context construction and proposal validation
- timeline patch creation and validation
- render-plan generation
- ffmpeg command construction and execution
- layer asset orchestration
- progress, logs, cancellation, and artifact metadata

### Python Parakeet Sidecar

The MVP transcriber is a supervised Python runner.

Input:

- WAV path
- model id
- language mode
- output JSON path

Output:

- transcript JSON
- normalized text
- word timestamps
- optional segment grouping
- model metadata

Rust treats the sidecar as an implementation detail behind a `Transcriber` boundary. The sidecar cannot mutate project files directly.

### Codex App-Server

Codex is used for structured edit guidance, not direct mutation.

Rust sends bounded context:

- preset
- user prompt
- media metadata
- transcript excerpts
- detected moments
- current timeline summary when regenerating

Codex returns a structured proposal:

- source time ranges
- rough-cut rationale
- caption edits
- title or overlay suggestions
- HyperFrames scene or overlay briefs

Rust validates the proposal and converts accepted output into timeline items.

### HyperFrames Sidecar

HyperFrames remains a Node sidecar for full-frame scenes and overlay assets.

Rust sends scene or overlay render requests that include:

- dimensions
- fps
- duration
- prompt or scene inputs
- output path

The sidecar returns generated artifact metadata. Rust records artifacts in project state and composes them during render.

## Data Model

The current project model remains the base. The feature adds or expands these concepts.

### EditPreset

```rust
pub enum EditPreset {
    TrailerCut,
    HighlightReel,
    StoryCut,
}
```

Each preset defines:

- minimum target duration
- maximum target duration
- default pacing rules
- default title and caption treatment
- whether long context is favored over aggressive cuts

### EditJobRequest

```rust
pub struct EditJobRequest {
    pub media_id: String,
    pub preset: EditPreset,
    pub prompt: String,
    pub target_duration_seconds: Option<f64>,
    pub language_mode: LanguageMode,
    pub caption_style: CaptionStyle,
    pub created_at: String,
}
```

The request is stored with the job summary so a draft can be traced back to the exact preset and prompt.

### Transcript

The existing `Transcript` shape gains metadata:

- transcription engine, for example `nvidia/parakeet-tdt-0.6b-v3`
- raw transcript artifact path
- optional segment grouping metadata

Word-level timestamps stay in the canonical project JSON so rough-cut planning and caption generation do not need to reread large raw output.

### Timeline Items

Real cuts are represented by timeline clips with source ranges:

- `startSeconds`: output timeline start
- `durationSeconds`: output timeline duration
- `properties.sourceIn`: source media start
- `properties.sourceOut`: source media end

Generated captions use `TimelineSource::Text`. Generated overlays and HyperFrames outputs use `TimelineSource::Generated`.

## One-Click Edit Pipeline

1. Validate the media exists and is supported.
2. Probe the media with ffprobe.
3. Extract mono 16 kHz WAV for transcription.
4. Run Parakeet 3 through the Python transcriber.
5. Parse and store transcript words.
6. Detect candidate moments:
   - speech density
   - pauses and dead air
   - emphasized words
   - transcript segment boundaries
   - optional audio energy changes
7. Build a preset-aware rough-cut plan.
8. Send bounded context to Codex for optional structured guidance.
9. Merge deterministic scorer output and accepted Codex proposal into an EDL.
10. Validate the EDL.
11. Create timeline video clips from `sourceIn` and `sourceOut`.
12. Generate caption cues from transcript words in selected source ranges.
13. Generate title, highlight, and overlay timeline items from preset and prompt.
14. Request HyperFrames assets where needed.
15. Build a render plan.
16. Render a draft MP4.
17. Validate the rendered MP4.
18. Record job status, artifacts, logs, and draft path.

## Render Behavior

The renderer must build the primary video from the EDL before adding visual style.

Required render stages:

1. Trim source media by each clip's `sourceIn` and `sourceOut`.
2. Concatenate selected clips in output timeline order.
3. Render captions, titles, highlights, and HyperFrames assets to timed overlay assets.
4. Flatten overlay layers where practical to avoid expensive multi-layer ffmpeg graphs.
5. Composite the flattened layer stack over the primary cut.
6. Trim and normalize audio from selected source ranges.
7. Export H.264/AAC MP4.
8. Strip sensitive source metadata from the final output.

The one-click presets must reject a render plan where the only primary video item covers the entire source duration, unless the user explicitly selects a future full-length preset.

## Validation Rules

The job validates inputs, intermediate plans, and final artifacts.

Input validation:

- media id exists
- source path exists under an allowed project/media root
- preset is supported
- prompt is bounded in size

EDL validation:

- every clip has finite non-negative `sourceIn`
- every clip has `sourceOut > sourceIn`
- clip durations are positive
- clips stay inside the source duration
- generated output duration falls inside the preset range or a user-approved override
- one-click presets produce output shorter than the source
- one-click presets do not pass the whole source through as a single clip

Render validation:

- final MP4 exists
- final MP4 has video and audio streams
- final duration matches the render plan within tolerance
- dimensions and fps match render settings
- captions or caption artifacts exist when captions are enabled
- logs and artifact paths are recorded

## UI Details

The agent panel becomes the primary control surface for the MVP:

- preset segmented control
- prompt textarea
- generated edit settings summary
- `Generate edit` button
- progress indicator with current stage
- collapsible log summary
- latest draft link
- quick actions: preview, reveal file, regenerate, render final

The timeline shows the generated result:

- selected source clips on `Video`
- caption cues on `Captions`
- titles and graphic elements on `Overlays`
- HyperFrames scenes on `HyperFrames`
- normalized source audio regions on `Audio`

The preview panel plays the latest draft MP4 when one exists.

## Testing Strategy

Rust unit tests:

- preset duration defaults and overrides
- EDL validation rejects full-source pass-through
- EDL validation rejects invalid source ranges
- Parakeet runner JSON parsing
- transcript word to caption cue grouping
- render-plan generation from timeline clips and layers
- ffmpeg command construction
- job state transitions

TypeScript tests:

- preset selector updates request state
- prompt input is included in the command payload
- agent panel renders job progress and draft link
- timeline adapter displays generated clip and caption items

Integration tests:

- use a tiny fixture video
- use a deterministic transcript fixture or mock transcriber
- generate a one-click edit timeline
- render a short MP4
- assert the result is shorter than source
- assert the result has video and audio streams
- assert caption/overlay evidence exists in the render plan

Real local validation:

- run the one-click pipeline against `/Users/olhapi/Downloads/IMG_6465.MOV`
- use actual Parakeet 3
- generate at least one preset draft
- verify with ffprobe
- extract representative frame samples
- confirm the output is a true short edit, not a full-length styled pass

## Sources For Preset Direction

- Instagram Reels moved to support uploads up to 3 minutes in January 2025: https://www.theverge.com/2025/1/18/24346567/instagram-announces-reels-3-minute-video-posts
- YouTube Shorts moved to 3-minute uploads starting October 15, 2024: https://www.theverge.com/2024/10/3/24260170/youtube-shorts-increased-length-templates-tiktok
- TikTok Smart Split reflects demand for long-to-short workflows from longer videos: https://www.theverge.com/news/808749/tiktok-smart-split-ai-outline-revenue-sharing-subscriptions

## Open Implementation Decisions

- Exact local Python environment management for the Parakeet runner.
- Whether the Parakeet model is downloaded on first use or configured as an existing local model path.
- Exact caption style enum values for the first implementation.
- Whether the first rough-cut scorer uses only transcript/audio heuristics or also visual scene-change analysis.
- Exact shape of Codex structured proposal JSON.
- Whether the first preview render uses full quality or a lower-resolution proxy.
