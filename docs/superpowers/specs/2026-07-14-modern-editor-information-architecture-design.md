# Modern Capability-Preserving Editor Information Architecture Design

## Context

Video Creater has a broad working editor surface: project media and folders, local and indexed search, image/video/audio generation, captions and transcription, a direct canvas, a multi-track timeline, selected-source editing, Codex sessions and structured proposals, workflow activity, native render review, and several export destinations. The existing workspace exposes much of that capability simultaneously. It is powerful but visually dense, gives unrelated jobs equal weight, and makes the viewer compete with Codex, Media, render review, and inspector content.

The approved direction is a cleaner contemporary desktop editor inspired by the information hierarchy of CapCut-style editors without copying its branding, exact visual system, promotional content, or simplified timeline. Capabilities may move into clearly labeled one-click tabs, drawers, contextual toolbars, and menus. They must not disappear, silently change ownership, or become reachable only through Codex.

The design was reconciled against:

- the supplied current-editor screenshot;
- the supplied CapCut-style reference screenshot;
- `docs/parity.md` and the Palmier-first editor plans;
- the current editor visual baselines and parity audit captures;
- `EditorWorkspace`, `MediaBin`, `AgentPanel`, `PreviewPanel`, `TimelineEditor`, inspectors, captions, speech, export, and render-review components and their behavior tests;
- the project-local visuals, graphics, and video-pipeline skill contracts.

## Decision

Use a dual-contextual-rail editor shell:

- a global left rail selects source and creation destinations;
- one resizable context panel renders the active left destination;
- the viewer and timeline remain the editing center;
- a selection-aware right rail opens one inspector drawer;
- Codex is a global, optionally pinnable drawer;
- Activity is the project-wide source of workflow truth;
- Render Review and Export remain evidence-backed project workflows.

This was selected over workflow-mode workspaces, which move controls too often, and a command-hub-first shell, which makes important capabilities difficult to discover.

## Goals

- Make the viewer and timeline visually dominant.
- Give every current capability one clear primary destination.
- Keep important capabilities one rail, drawer, menu, contextual action, or shortcut away.
- Preserve the existing canonical project-action, Rust-validation, render-plan, workflow, log, artifact, and provider boundaries.
- Preserve direct manipulation, multi-track editing, multi-selection, automation, generation provenance, and render diagnostics.
- Present agent proposals as a real EDL before captions, effects, overlays, or HyperFrames.
- Keep failures visible and actionable at the affected surface.
- Maintain desktop density without nested-card or generic dashboard styling.
- Preserve and improve keyboard, focus, tooltip, and narrow-layout behavior.

## Non-Goals

- Do not clone CapCut branding, advertising, exact icons, colors, or layout measurements.
- Do not reduce the timeline to a single video track.
- Do not replace explicit editor controls with prompt-only behavior.
- Do not redesign canonical project schemas, Rust mutation ownership, provider execution, Temporal workflow contracts, render engines, or export formats.
- Do not add a feature merely because it appears in the reference screenshot.
- Do not implement the redesign in this design document.

## Capability Preservation Contract

The redesign must retain the following capability groups.

| Surface | Required preserved capabilities |
| --- | --- |
| Workspace | Home, project identity and save state, layout presets, pane visibility, resizing, maximize/restore, profile, settings, shortcuts, tour, project skills, MCP setup, responsive one-pane navigation |
| Codex | Durable named sessions, starter tasks, mentions, selected context, conversation and tool-call history, workflow links, proposal review/application, undo, runtime/transcription gating, prompt composer |
| Media | Imported and generated assets, folders, import, matte creation, organization, search, indexed/visual search, filters, analysis, drag/drop, previews, generated lifecycle and provenance |
| Generate | Image/video/audio modes, provider/model catalogs, frame/reference modes, prompt and name, model-specific options, estimates, history, retries, reruns, variations, insertion and replacement |
| Captions/transcript | Caption creation and styling, Agent Mode actions, transcription, word-level transcript, repair state, source selection, language, word/profanity controls, speakers and silence routing |
| Preview | Timeline/source tabs, playback, shared playhead, direct canvas editing, prepared/canonical frames, source metadata, missing-media and preview-failure recovery |
| Inspector | Metadata, source/timeline ranges, transforms, crop, opacity, speed, fades, blend, color, effects, automation, Lottie inputs, audio gain/fades/denoise/speakers, caption/text, generated provenance and AI Edit |
| Timeline | Multiple timelines, video/HyperFrame/overlay/caption/audio tracks, filmstrips, waveforms, track state, edit tools, snapping, zoom, selection, drag, trim, ripple, linking, copy/paste, keyframes, fades, context menus and workflow status |
| Render/export | Draft/final quality, video profiles, NLE XML, Palmier package, capability checks, progress, cancellation, retry, reports, streams, comparisons, logs, artifact paths and failure recovery |

Moving a capability is allowed only when the new destination is named in this design and the original workflow remains covered by tests or an explicit replacement test.

## Workspace Shell

### Regions

| Region | Default behavior |
| --- | --- |
| Top bar | Project navigation, name/save state, undo/redo, layout, render status, Export, profile |
| Global left rail | One-click source and creation destinations with icon and label |
| Context panel | Active left-rail destination, resizable from approximately 260 to 400 pixels |
| Viewer | Largest flexible upper-deck area with a protected minimum width |
| Right tool rail | Selection-aware inspector destinations |
| Inspector drawer | One active inspector, resizable from approximately 280 to 420 pixels |
| Timeline | Resizable lower deck spanning beneath the viewer and inspector |
| Codex drawer | Global drawer that replaces the context panel or pins beside it when width allows |

The default desktop geometry is approximately:

- 52-pixel top bar;
- 64-pixel global rail;
- 304-pixel context panel;
- 52-pixel collapsed inspector rail;
- 35 to 40 percent timeline height;
- functional resize separators instead of decorative card boundaries.

The upper workspace is `global rail -> context panel -> viewer -> inspector rail/drawer`. The lower workspace begins after the context-panel boundary so the timeline remains wide when the inspector is open.

### Navigation Rules

- Only one primary left destination is active.
- Selecting another destination reuses the context panel.
- Selecting the active destination collapses the panel.
- The right rail changes with the current selection while preserving category order.
- The inspector remembers the last destination per selection type.
- Codex is global and never nested inside Media or Generate.
- Codex may pin only when the layout budget preserves the minimum viewer width.
- Default, Media, and Vertical presets become saved panel configurations rather than separate UI systems.
- Render-expensive actions and running/failed status never disappear silently into overflow.

### Visual Language

- Use a dark, flat, professional editor surface with crisp separators.
- Reserve cyan for active navigation, selection, progress, and primary actions.
- Use purple for captions and agent suggestions, green for healthy audio/completion, and amber/red for warnings and failures.
- Use cards only for repeated media items, generated outputs, modals, and focused tools.
- Avoid nested cards, oversized headings, opaque full-width caption slabs, and generic SaaS-dashboard treatments.
- Use `lucide-react` icons through shadcn primitives for app chrome; keep custom rendering for editor-specific surfaces.

## Left Rail and Context Panels

### Rail Taxonomy

| Destination | Primary ownership |
| --- | --- |
| Media | Imported/generated project assets, folders, search, organization and analysis |
| Generate | Image, video and audio generation workflows |
| Templates | Overlays, title cards, caption templates, HyperFrame scenes, transitions and shader backgrounds |
| Text | Ordinary editable text overlays and reusable text treatments |
| Captions | Caption creation, cue list, style, motion, placement and reviewed agent actions |
| Transcript | Transcription, transcript browsing, search and word repair |
| Audio | Audio library, speech analysis, speakers, silence, denoise and music |
| Effects | Executable effect catalog, presets, LUTs and applied effects |

Transitions stay inside Templates because the current product models them as motion templates. Lottie files remain first-class Media assets; reusable Lottie treatments may also appear in Templates. The rail does not gain a category solely because the reference product contains it.

Home and Codex are global controls near the top. Activity carries running/failed/blocked badges. Settings/profile remains at the bottom.

### Media

Media owns:

- Import and compact overflow actions;
- search, indexed/visual search, folder-scoped search and stale-index repair;
- breadcrumbs, folders, counts, sorting, filters and view sizing;
- one project library for imported and generated output media;
- media previews, duration, AI badges and generation/workflow status;
- folder creation, rename, deletion and drag/drop moves;
- Create Matte, Organize with Agent, visual-frame analysis, explicit remote captioning and applicable silence shortcuts.

Generated outputs do not move into a disconnected browser. Contextual shortcuts may route to Generate, Inspector, Audio, or Codex, but Media remains the project-asset owner.

### Generate

Generate is a full context destination, not a floating sheet competing with Media. It contains:

- Image, Video and Audio mode selection;
- first/last frame and reference modes;
- visible, removable, draggable reference slots;
- asset name and prompt;
- provider/model selection and model-specific controls;
- duration, aspect, resolution, voice, music, lyrics, instrumental and supported audio settings;
- readiness and credit/cost estimates;
- a sticky queue action;
- active-generation count and status;
- history with reuse, rerun, retry, variation, insertion and replacement.

Placement is a compact context chip such as `Project library`, `Insert on V1 at 03:07`, or `Replace Opening clip`, not a large default configuration section. Provider constraints and reference limits stay explicit and actionable.

### Templates and Text

Templates is searchable and categorized by overlays, captions, HyperFrame scenes, transitions, shader backgrounds, and supported motion templates. Assets support preview, direct insertion, and drag-to-timeline.

Text remains separate because adding ordinary editable copy is a different job from selecting a designed template. It exposes Add Text, recent/reusable treatments, and a route to selected-text properties. Advanced typography and timing remain in the right inspector.

### Captions and Transcript

Captions owns source, local/cloud mode, language, word limits, profanity, Build Captions, reviewed Agent Mode actions, the cue list, style, animation, and placement.

Transcript owns media/source selection, model/runtime readiness, transcription and retry, searchable word-level output, existing repair indicators, word repair, and sending selected transcript context to Codex.

The source inspector may show transcript status and a route to Transcript but does not duplicate the full transcript editor.

### Audio

Audio has Library, Speech, and Music tabs:

- Library shows imported/generated audio and waveforms.
- Speech owns speakers, identification, silence detection/removal and denoise.
- Music shows music assets and routes to supported audio generation.

Video-to-music and video-to-SFX remain contextual shortcuts from selected video; Generate owns the complete workflow.

### Effects

Effects exposes the executable Rust-backed catalog, search, presets, LUT resources, applied effects, compatibility/preparation status, explicit target, clear, and reorder actions. Detailed parameters, curves, automation, keyframes, blend, crop, color, and transform remain selection-owned inspector content.

## Viewer and Direct Canvas Editing

### Viewer

- Keep a compact Timeline/source tab strip.
- Double-clicking a timeline clip opens its source without replacing Timeline.
- Source tabs are closable and retain source playback position.
- The canvas fills its allocation rather than sitting inside a decorative card.
- Timeline and source preview keep their distinct playback contracts.

The transport remains directly below the canvas and provides start, previous frame, play/pause, next frame, end, time/duration, scrubber, fit/zoom, and applicable aspect/resolution/frame-rate/quality indicators.

Render review does not consume permanent space below the viewer. The top render-status control opens the right inspector directly to Render Review.

### Direct Manipulation

Selected visual layers show bounding outline, resize handles, rotation, crop state, safe zones for captions/overlays, and prepared/canonical-preview status. Pointer changes preview live; the canonical project action commits only when the interaction completes. Invalid bounds and unsupported operations render at the canvas target.

The floating contextual toolbar includes only frequent canvas actions: Reveal/Open Source, Fit/Fill, Crop, Transform, Replace, and More. It may adapt to captions, templates, Lottie, and generated assets but must not duplicate the inspector. Delete, linking, ripple edits, automation, detailed effects and full AI generation settings stay in their primary destinations.

## Right Rail and Inspector

### Selection Routing

| Selection | Inspector destinations |
| --- | --- |
| Video/image | Basic, Transform, Motion, Effects, AI Edit |
| Audio | Basic, Volume, Fades, Denoise, Speakers |
| Caption/text | Content, Style, Motion, Placement, Timing |
| Template/Lottie | Basic, Fields, Motion, Effects, Inputs |
| Generated media | Details, References, Prompt, AI Edit |
| Multiple items | Selection, Common Properties, Timing, Effects |
| Nothing selected | Project, Activity, Render Review |

The header identifies the target name, type, track and selection count and provides Reveal Source when applicable. Clicking the active rail destination closes the drawer.

### Information Versus Editing

Basic/Details owns factual media path/type, source/timeline ranges, duration, resolution, frame rate, aspect, status, provenance, output and reference identities.

Editable destinations own:

- Transform: position, scale, rotation, crop, opacity, speed, fades, blend and color;
- Motion: property selection, keyframe lane, easing and motion presets;
- Effects: applied effects, parameters, resources and automation;
- Audio: gain, fades, keyframes, denoise preparation and speaker assignment;
- Caption/Text: copy, correction, style, animation, placement, emphasized words and timing;
- Template/Lottie: typed fields, theme, slots, markers, segments and state-machine inputs;
- AI Edit: supported upscale, edit, rerun, variation, replacement, video-to-music and video-to-SFX actions.

Details and AI Edit remain explicitly separate. Provider settings and generation history remain in Generate. Locked selections become visibly read-only with a concrete reason. Multi-selection exposes only compatible shared properties.

### Project, Activity and Render Review

With no selected item:

- Project shows format, duration, timeline metadata and identity.
- Activity shows workflow jobs, generation state, Temporal readiness, recent exports and diagnostics.
- Render Review shows quality, duration, streams, graphics evidence, preview/render comparison, failures, retry, logs and artifact paths.

Running and failed work badges Activity and the top render-status control. Either opens the exact affected job.

## Timeline

### Structure

The timeline uses, in order:

1. timeline tab bar;
2. editing toolbar;
3. time ruler;
4. fixed track headers and horizontally scrollable clip canvas;
5. local horizontal scrolling when needed.

The current separate selected-clip action strip moves into the main toolbar so the clip canvas gains vertical space without losing commands.

### Timeline Tabs

Preserve durable timeline switching, New Timeline, Duplicate Timeline, nested-sequence identity, and Decompose Sequence when supported. Secondary timeline actions use a labeled overflow menu.

### Toolbar

Persistent left controls are Undo, Redo, Select, Razor, Split, contextual source marks, Add Text, and Open Templates.

The selection-aware center exposes selection count, Link/Unlink, Remove, Nudge -0.25s/+0.25s, Duplicate, Ripple Delete, Grain, Vignette, Clear Effects, and Decompose Sequence when applicable. Frequent applicable controls remain visible; width-constrained actions use More Clip Actions with shortcut and disabled-reason text.

The right side owns snapping, zoom out/slider/in, an optional fit-timeline action, and display options.

### Tracks and Clips

- `V1`, `V2`: video/image;
- `H1`: HyperFrames/generated scenes;
- `O1`: overlays/templates;
- `C1`: captions;
- `A1`: audio.

Headers keep kind accent, name tooltip, applicable visibility/mute and sync/link state, direct height resize, and a menu for lock, reorder, height and other track actions. Headers remain fixed during horizontal scrolling.

Video/generated-video clips show filmstrips. Generated clips keep AI and workflow state. Audio shows waveform, gain, fades, keyframes, speakers and dead-air masks. Caption, HyperFrame, overlay and template treatments remain distinct. Selection uses cyan outline and resize handles. Locked items stay readable but visibly non-editable. Full source range, reason, lineage, validation and workflow details remain inspectable through hover and Inspector rather than permanently filling the clip.

### Interaction Contract

Preserve:

- horizontal and compatible cross-track drag;
- magnetic snapping and snap guides;
- trim and Shift-ripple-trim handles;
- Option-drag duplication;
- marquee and additive multi-selection;
- adjustable timeline ranges;
- empty-gap selection and ripple close;
- copy, cut, overwrite paste and ripple-insert paste;
- clip, gap, track and empty-canvas context menus;
- keyboard select/razor/split, source marks, deletion, duplication and playback;
- clip-local automation and fade-knee editing.

Drag previews include linked or ripple-affected items. Valid targets use cyan local feedback. Invalid targets use red local feedback and an explicit reason. Canonical actions commit only after the interaction completes.

### Ruler and Playhead

Preserve major/minor ticks, hover badge, playhead badge, edit-point guides, range boundaries, snap guides, preview-shared seeking, and persisted zoom/scroll state. The playhead must remain dominant without obscuring clip labels or handles.

## Codex and Proposal Review

### Codex Drawer

Codex contains the named-session selector; new/rename/delete controls; starter actions; chronological conversation/tool calls; compact workflow links; context chips for selected media, clips, ranges, tracks, captions and generated outputs; a sticky mention-aware composer; and runtime/transcription readiness.

The drawer replaces the context panel or pins when the layout budget allows. Session and transcript state remains project-scoped and durable.

### Proposal Stages

The review UI follows:

`Intent -> Transcript -> Moments -> EDL -> Timeline -> Layers -> Render -> Review`

1. **Intent** shows prompt, preset, target media/duration, language/transcription mode, and range or replacement target.
2. **Primary EDL** shows canonical media identity, `sourceIn`, `sourceOut`, resulting position/duration, and a reason such as hook, setup, payoff, action, quote, transition or context.
3. **Layers** shows captions, titles, overlays, diagrams, transitions and HyperFrames only after the EDL.
4. **Validation** shows canonical-ID, range, duration, overlap, track, lock, media/artifact and renderability results beside affected items.
5. **Decision** offers Revise Prompt, Reject and Apply Proposal.

The UI flags styled full-source pass-through proposals and proposals without meaningful primary cuts.

### Visual Layer Contract

Every proposed caption, title, overlay, diagram, transition or HyperFrame exposes:

- role and source beat;
- start and duration;
- exact text;
- dimensions, frame rate and alpha behavior;
- `visualTreatment`;
- `motion`;
- `safeZone`;
- `avoid`.

Validation rejects or warns about generic opaque text slabs, unsafe coverage, missing motion, transcript-language conflicts, repetitive compositions, and long static holds. Applying sends structured project actions to Rust. Codex does not mutate canonical files directly. A successful apply creates one undoable history entry and records the result in the conversation.

## Activity, Render and Export

### Activity

Activity is the complete project-wide job source for Agent Proposal, Transcription, Media Generation, Preparation, Render and Export. It represents queued, running, blocked, failed, completed and cancelled states.

Each job exposes only applicable actions: view affected media/timeline, open proposal, cancel, retry, open output, or open report/artifact/log. Codex links to conversation-created jobs but does not duplicate the complete activity center.

### Render and Export Entry Points

- Render Status opens the existing export/render sheet preselected for draft review and reflects active/completed/failed state.
- Export opens the same sheet at destination selection.

This keeps one underlying workflow while distinguishing draft review from final delivery.

The sheet groups:

- Video: WebM, H.264, HEVC and ProRes; Draft/Final quality; user-selectable resolution;
- Timeline Interchange: Premiere XML and DaVinci/FCPXML;
- Project Package: Palmier-compatible portable project.

Unavailable profiles remain visible with exact runtime, capability, policy or project-format reasons. Progress, cancellation, output path and failures stay in the sheet and Activity.

### Render Review Contract

A render is complete only after validation records:

- expected and actual duration;
- video/audio stream presence;
- caption alignment after cuts;
- overlay/HyperFrame timing;
- graphics preparation and visual-QA evidence;
- preview/render comparison;
- output, report, frame and log artifact paths.

Success opens Render Review. Failure opens the same destination focused on the actionable error with Retry Render and artifacts/logs. Top-bar and Activity status remain in sync.

## Responsive Behavior

Responsive visibility is derived from available width without mutating saved user preferences.

| Width | Behavior |
| --- | --- |
| 1600px and above | Source panel, viewer, collapsed/right inspector and timeline coexist; Codex may pin |
| 1280-1599px | Source panel/viewer coexist; inspector opens as a drawer; Codex replaces or temporarily overlays the source panel |
| 1024-1279px | Only one side drawer opens at once; viewer and timeline remain protected |
| Below 1024px | Existing Media, Timeline, Inspector and Codex one-pane switcher becomes primary navigation |

Auto-collapsed panes restore when width returns. The timeline cannot disappear because a drawer opened. Vertical layout prioritizes a portrait canvas without changing destination ownership.

## Component Boundaries

- `EditorWorkspace`: canonical UI state and callback wiring.
- `EditorShell`: top bar, global rail and layout budgeting.
- `ContextPanel`: active left destination and resizing.
- `ViewerDeck`: tabs, canvas, transport and contextual toolbar.
- `InspectorDock`: selection routing and inspector drawer.
- `TimelineDock`: timeline tabs, toolbar and editor canvas.
- `CodexDrawer`: sessions, conversation, context and proposal review.
- `ActivityPanel`: project-wide jobs and diagnostics.
- `ExportSheet`: render and delivery configuration.

Existing focused components retain their workflow logic. Navigation components receive typed state and callbacks; they do not duplicate project mutation, provider, Temporal, proposal or render logic.

## Interaction, Error and Empty States

Every affected surface covers default, hover, focus-visible, disabled, selected, multi-selected, drag ghost, resize preview, valid/invalid target, empty, loading/preparing, queued, running, blocked, failed, completed, cancelled, and relevant missing-data states.

Errors render where users can act:

- preview failures in Viewer;
- invalid edits at timeline/canvas targets;
- provider/readiness errors in Generate or AI Edit;
- proposal problems beside affected actions;
- render/export failures in Render Review and Activity.

Toasts may supplement these states but never replace them.

## Accessibility and Keyboard Contract

- Every icon-only control has an accessible name and tooltip.
- Rail destinations and inspector categories use roving keyboard navigation.
- Tabs support arrows, Home and End.
- Drawers return focus to their triggers.
- Modal sheets trap focus; ordinary side drawers do not.
- Focus remains visible on dark surfaces.
- Disabled controls expose a reason in text or tooltip.
- Existing timeline, source-preview, clipboard, source-mark and editing shortcuts remain unchanged.
- Motion respects reduced-motion preferences.
- Status is not communicated by color alone.

## Verification Strategy

### Functional Coverage

- Add a preservation inventory test or checklist mapping every previous action to its primary destination.
- Test layout budgeting, automatic collapse, preference restoration, resize and maximize behavior.
- Test left-rail routing, active-destination collapse, focus and tooltips.
- Test right-inspector routing for every selection type and multi-selection.
- Test Codex replace/pin behavior, sessions, context, proposal stages and Rust validation results.
- Preserve focused Media, Generate, Captions, Transcript, Audio, Effects, Preview, Timeline, Inspector, Agent, Export and Render Review test coverage.
- Preserve canonical project-action, provider, workflow, export and render contracts.

### Visual QA Matrix

Capture and inspect:

- default editor;
- Media and folder interaction;
- Image, Video and Audio Generate;
- Templates and Text;
- Captions and Transcript;
- Audio speech tools;
- video, audio, caption, template and generated-source inspectors;
- Codex empty, active and proposal-review states;
- Activity queued, running, failed and completed states;
- Export configuration;
- preview and render failures;
- timeline drag, invalid drop, resize, multi-selection and automation;
- desktop, constrained desktop and narrow one-pane layouts.

### Required Gates

- Narrowest relevant Vitest suites during each implementation slice.
- Full frontend test suite after the shell converges.
- `rtk pnpm lint` and `rtk pnpm build` for TypeScript/Tailwind changes.
- Browser or Playwright inspection of desktop and narrow targets.
- Rust tests for any changed canonical action, proposal, render, export or workflow boundary.
- `rtk git diff --check` before every commit.

## Acceptance Criteria

- Every capability in the preservation contract has a named primary destination.
- Every previously exposed action is reachable through a rail destination, contextual toolbar, inspector, menu or existing shortcut.
- Viewer and timeline retain protected minimum dimensions at supported widths.
- Opening a drawer does not overlap or remove the timeline.
- No panel overlap, clipped control or unreadable text appears in the visual-QA matrix.
- Direct manipulation, track identity, source ranges, workflow state, automation and invalid targets remain inspectable.
- Generated edits show a real EDL before visual layers.
- Visual proposals expose `visualTreatment`, `motion`, `safeZone` and `avoid`.
- Rust remains the canonical validation and mutation owner.
- Render success remains evidence-backed by duration, streams, alignment, timing and artifacts/logs.
- Preview, proposal, generation, render and export failures remain actionable at their owning surface.
- Icon controls are labeled, focusable and keyboard-operable.
- Existing behavioral coverage remains green or is replaced by an equally explicit test for the intentionally moved surface.

## Mockup Gate

A generated or hand-built mockup is not approved unless it visibly demonstrates together:

- top project/render/export chrome;
- global left rail and one open context panel;
- working viewer and source tabs;
- a professional multi-track timeline;
- collapsed or open inspector rail;
- a visible Codex entry point;
- render/workflow status;
- clear selected, running and failure states.

Mockups may simplify text and sample content, but they may not omit a major region or imply that capabilities were removed.
