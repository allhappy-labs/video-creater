# Palmier-First Editor UI Parity Design

**Date:** 2026-07-13  
**Status:** Approved  
**Decision:** Use Palmier's desktop shell, geometry, hierarchy, and density as the default visual target while preserving every Video Creater capability through contextual tabs, overflow menus, disclosures, and sheets.

## Context

The prior parity tracker correctly records a broad functional surface, but fresh screenshots of the installed Palmier application show that Video Creater still differs visibly in its project home, editor shell, media hierarchy, preview sizing, inspector structure, generation composer, captions workflow, timeline density, and export presentation.

The ten authoritative references are retained in the ignored directory `output/parity-audit-2026-07-13/reference/`:

1. `01-home.png`
2. `02-editor.png`
3. `03-export.png`
4. `04-speech-details.png`
5. `05-captions.png`
6. `06-generation.png`
7. `07-media-overflow.png`
8. `08-agent-editor.png`
9. `09-ai-edit.png`
10. `10-ai-edit-duplicate.png`

The local Palmier source in `reference/Sources/PalmierPro/` is the implementation reference for values that cannot be measured confidently from screenshots.

## Chosen Direction

The user chose the **Palmier-first shell** over a hybrid or surface-only polish.

Palmier geometry becomes the desktop default. Video Creater-only features remain available, but they no longer compete with the viewer and timeline as always-visible top-level controls. Existing alternate workspace presets remain available.

This is not a rebrand or a decorative theme pass. It is a structural parity pass covering navigation, pane allocation, contextual hierarchy, control density, modal presentation, and interaction states.

## Shared Visual Language

Use Palmier's source tokens as the baseline:

| Role | Value |
| --- | --- |
| Base | `#0a0a0a` |
| Surface | `#161616` |
| Raised | `#1e1e1e` |
| Prominent | `#2c2c2c` |
| Primary text | white at 100% |
| Secondary text | white at 80% |
| Tertiary text | white at 62% |
| Muted text | white at 34% |
| Primary border | white at 16% |
| Subtle border | white at 12% |
| Accent | warm off-white, approximately `rgb(245 239 228)` |

Use the system font stack. Palmier's working scale is 9–15px for editor chrome, 18–28px for major titles, 3–20px radii, and a 2/4/6/8/10/12/14/16/20/24/28 spacing rhythm. Avoid warm brown planes, large dashboard cards, heavy selection rings, and boxed toolbar buttons unless the reference state uses them.

## Project Home

Desktop uses an edge-to-edge split shell:

- Fixed 220px navigation sidebar.
- `New Project` and `Open Project` at the top.
- `Settings` anchored at the bottom.
- Fluid main area with a 24px inset.
- 28px light welcome title with tight tracking.
- 28px vertical rhythm between sections.
- Fixed 150×120 sample and recent-project cards.
- Project name and date/status appear in a bottom image gradient rather than a verbose body.

Existing Video Creater behavior remains:

- Bundled offline Edison sample.
- Split-project and `.palmier` folder opening.
- Explicit create/open paths and default storage location.
- Recent-project persistence.
- Missing-folder recovery, relink, remove, and detailed errors.
- Model/settings navigation.

Path entry, recovery detail, and secondary project actions move into focused dialogs, sheets, overlays, or overflow menus. Palmier's Google account row is not faked; account integration remains an explicit parity gap in `docs/parity.md`.

## Editor Shell

The desktop editor follows Palmier's compact hierarchy:

- One compact native-style top title bar.
- 5px panel gaps.
- 28px panel headers.
- 38px edit toolbar.
- Media / viewer / inspector upper columns.
- Timeline receives roughly two-thirds of default workspace height.
- Viewer is visually dominant within the upper deck.
- Agent/Codex docks on the left when visible and participates in resizing.

Palmier reference geometry remains the source of truth:

- Media default/minimum width: 500/280px.
- Inspector default/minimum width: 260/150px.
- Agent range: 240–640px.
- Viewer minimum: 400×320px.
- Timeline row: 50px.
- Track header: 100px.
- Ruler: 24px.

Responsive behavior:

- Below 1024px, retain the existing compact one-pane switcher.
- Between 1024px and 1439px, automatically collapse a secondary pane when the full pane budget would clip, without destroying stored user preferences.
- At 1440px and above, allow agent, media, viewer, and inspector together.

The existing Default, Media, and Vertical presets remain available. Palmier-first is the default composition, not the only composition.

## Preview

The preview fills its allocated pane rather than using a viewport-height cap.

- Remove the current `28vh` media cap.
- Keep the canvas black and center the prepared/canonical frame within its available area.
- Use the active timeline/source name in the tab strip.
- Separate the scrubber from the transport row.
- Provide start, frame-back, play/pause, frame-forward, and end controls.
- Keep snapshot and Fit controls at the trailing edge.
- Remove the giant central play overlay in the normal editor state.

The existing prepared-frame compositor, transforms, crop interaction, source tabs, retry behavior, diagnostics, render comparison, and exact preview/export contract remain unchanged beneath the new shell.

## Media Rail and Library

Restore the compact Media / Captions / Audio icon rail. The Media surface uses:

1. Import, Generate, overflow, flexible space, Smart Search.
2. Search, grid/view, sort, and filter controls.
3. Compact Library/count context row.
4. Direct thumbnail grid as the default presentation.
5. Attached generation drawer when generation is active.

Folders, nested drag/drop, grouped and flat modes, thumbnail sizing, semantic search, analysis, silence tools, generated lifecycle, output selection, insert/replace, history, retry, and cancellation remain available. Folders and advanced display controls move behind compact navigation and overflow rather than dominating the default view.

The overflow menu includes:

- New Folder.
- Create Matte only when backed by a real action/data path; otherwise it is tracked as a gap, not shipped as a dead button.
- Organize with Agent, using Palmier's safety contract that forbids deletion and timeline mutation without reviewed actions.
- Video Creater-only silence, analysis, and library tools.

## Generation Composer

The composer is a bottom child of the Media pane, not a viewport-fixed floating sheet.

- Preserve at least 120px of visible media area.
- Keep image/video/audio mode switching.
- Keep First/Last and Reference modes.
- Use direct click/drop reference slots; fallback media selection moves to a popover.
- Keep prompt, provider/model, resolution, duration, aspect ratio, quality, variations, cost, history, and submit behavior.
- Condense model, resolution, duration, aspect, cost, and submit into a Palmier-style footer.
- Move optional name and provider-specific extras into an advanced disclosure.
- Preserve typed provider capability validation and all lifecycle states.

## Contextual Inspector

Restore the reusable accessible contextual tab primitive removed in the current dirty tree and reconcile it with the newer inspector functionality.

Inspector destinations:

- No selection: Project / Activity.
- Selected media asset: Details / AI Edit.
- Selected visual timeline clip: Video / Adjust / AI Edit.
- Audio-only selection: focused Audio surface without redundant tab chrome.
- Text/caption selection: Content / Animate, preserving the required visual contract.

Details contains source identity, file metadata, references, generated provenance, prompt, and copy actions. AI Edit contains compact sections:

- Scope when replacement/trim/audio placement context is required.
- AI Enhance: Upscale, Edit, Rerun, and Create Video when eligible.
- AI Audio: Generate Music and Generate SFX.
- Advanced generation/variation/replacement controls beneath disclosures.

The rich transform, crop, opacity, speed, blend, effects, color, keyframe, ordering, speaker, generated-output, provider, and eligibility logic is retained and reorganized rather than rewritten.

## Captions and Speech

Captions becomes a compact workbench within the restored rail:

- Source selector.
- Local/Cloud transcription mode.
- Language.
- Maximum words.
- Profanity handling.
- Collapsible Source, Settings, Style, Animation, and Placement sections.
- A real caption preview using the production compositor or the same visual contract.
- Agent Mode actions for filler words, names/jargon, emoji, and translation; these draft reviewed agent proposals rather than mutating timing blindly.

Speech/Music tabs consolidate speaker and silence actions:

- Mark Speakers and Identify Speakers.
- Mark Silence and Remove Silence.
- Existing production analysis, local models, dead-air ripple deletion, transcript repair history, explicit word timing, per-word animation, emphasis, easing, and visual-treatment contracts remain accessible under advanced/edit-selected-cue surfaces.

## Timeline

The timeline keeps all current editing behavior while adopting Palmier density:

- 50px default rows.
- 100px track headers.
- 24px ruler.
- Flatter toolbar and subtler selected-clip treatment.
- Filmstrips fill video bodies appropriately.
- Waveforms fill most of audio clip height and keep visible silence/speech regions.
- Existing track height resizing still permits 32–200px.

Multi-track editing, marquee selection, linked moves, Option-duplicate, snapping, ripple operations, track locks, sync locks, gaps, automation, fades, tabs, nested sources, and keyboard/context interactions are preserved.

## Agent Panel

The agent panel becomes flatter and less card-heavy. Add Palmier's starter-action pattern, adapted to Video Creater's real capabilities, so each starter populates the composer rather than running silently. Sessions, rename/delete/restore, proposal review, apply/undo, selected-media context, workflow evidence, and diagnostics remain; debug-heavy detail moves behind disclosure.

## Export Sheet

Replace the current compact export dropdown with one accessible 560×520 rounded sheet.

Destinations:

- Video: H.264, H.265, ProRes, and existing WebM profiles.
- Timeline: Premiere XMEML and DaVinci/Final Cut FCPXML.
- Palmier Project: existing package workflow.

The primary rows are Codec, File Type, Resolution, and Frame Rate. The footer summarizes duration, estimated size when honest, resolution, and output type, then provides Cancel and Export.

Runtime availability, factory/policy reasons, quality, diagnostics, Temporal behavior, progress, error, and output path remain in context or beneath an Advanced disclosure. Disabled choices retain clear reasons.

The sheet must trap focus, close with Escape/Cancel, restore focus to the Export trigger, and never turn an unavailable export path into a cosmetic choice.

## Dirty-Tree Reconciliation

The current worktree contains substantial pre-existing feature work and removes two committed parity primitives:

- `src/components/workspace/contextual-inspector-tabs.tsx` and its tests are deleted.
- `src/components/workspace/media-bin.tsx` removes the Media/Captions/Audio rail and caption integration.
- Current tests explicitly assert that inspector tabs do not exist.

Implementation must not reset or overwrite the newer work. Restore the committed primitives selectively, adapt their APIs to the new feature set, and update the contradictory tests. The relevant `docs/parity.md` entries must be reopened until fresh same-state evidence passes.

## Accessibility and Interaction

- All icon-only controls have accessible names and tooltips.
- Tabs use a single tab stop with Arrow/Home/End navigation.
- Menus, sheets, disclosures, and dialogs are keyboard operable.
- Focus returns to the invoking control when transient UI closes.
- Hover, active, selected, disabled, busy, error, and offline states remain distinguishable without color alone.
- Compact labels do not remove the richer accessible name or description.
- Narrow layouts preserve zero root/page horizontal overflow.

## Verification Contract

Every implementation slice follows test-driven development:

1. Add or change a focused test so the current UI fails for the intended Palmier contract.
2. Implement the smallest coherent visual/interaction slice.
3. Run focused tests, lint/build checks, and relevant Rust tests.
4. Capture Video Creater at the same logical viewport and state as the Palmier reference.
5. Compare reference and implementation side by side in one visual review artifact.
6. Fix visible geometry, typography, spacing, crop, border, radius, and state mismatches.
7. Update `docs/parity.md` with exact evidence and remaining gaps.
8. Commit the meaningful slice with a Conventional Commit.

Required browser viewports include 1440×960, 1280×720, 1024×768, and 390×844. Required states include home, default editor, media rail, generation drawer, captions, Details, AI Edit, export, agent starter state, and representative video/caption/audio timeline clips.

Final verification includes the full frontend suite, lint, production build, relevant Rust tests, native app launch/package smoke, keyboard interaction checks, zero-overflow gates, and inspected same-state screenshots. Passing tests without the side-by-side visual gate is not sufficient to close visual parity.

## Tracker Policy

`docs/parity.md` remains authoritative. It must distinguish:

- Implemented and visually verified.
- Implemented but awaiting same-state visual evidence.
- Missing Palmier capability.
- Video Creater-only capability preserved outside Palmier's design.
- Externally blocked release evidence.

Video Creater-only features are recorded respectfully in their relevant sections and are never removed solely because Palmier lacks them.

## Non-Goals

- Do not fake Palmier account sign-in.
- Do not add decorative dead controls for Create Matte or unavailable providers/codecs.
- Do not reduce canonical editing, render, provider, transcript, graphics, or agent safety contracts.
- Do not force the dense desktop layout onto narrow screens.
- Do not declare all-screen or pixel parity from component tests alone.
