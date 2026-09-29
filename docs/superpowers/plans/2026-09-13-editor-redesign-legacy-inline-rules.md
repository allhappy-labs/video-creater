# Editor Redesign — Legacy Inline Rules Inventory

Reference for plans 03–07. It captures domain rules that live **inline** in the pre-cut `src/components/workspace/*` components (handler bodies, hooks, JSX conditions, local constants): validation ranges, clamps, defaults, guards, action construction, user-facing copy and backend call sequencing. Pure top-level helpers already extracted to `src/lib` are not repeated; they are named only where a sequence depends on them.

- **Line numbers** refer to HEAD `9854bad8` (the workspace files are identical to `2c544965`). After the hard cut, read them with `rtk git show 9854bad8:<path>`.
- **Plan column:** P03 Timeline, P04 Preview & Properties, P05 Left tabs (media, generation, audio/speech, text, captions, effects), P06 AI tab / Codex, P07 Export, tasks, menus, undo/redo.
- **Copy** in double quotes is verbatim user-facing text. Keep it unless the redesign spec replaces it.
- Styling and layout are out of scope.
- **Local logic with no `src/lib` copy** in the preview files: `useTimelineMediaSynchronization`, `renderCaptionTextWithEmphasis`, `sourceViewerTreatment`, `PreviewTransport` and `CanvasSelectionOverlay`. Port them or reimplement them from the rows below. Fullscreen, audio fade math and preview-preparation triggers are driven from `editor-workspace.tsx`, not the preview files.

## Summary

| Section | Rules |
|---|---|
| Timeline editing | 180 |
| Clip properties / visual | 99 |
| Audio | 22 |
| Captions / transcript | 57 |
| Templates / text | 32 |
| Preview / canvas | 97 |
| Media / import / folders / search | 81 |
| Generation | 119 |
| Speech | 26 |
| Export / render | 65 |
| Jobs / activity | 40 |
| Agent / Codex | 98 |
| Undo/redo & persistence | 14 |
| Native menu / shortcuts | 38 |
| **Total** | **968** |

## Timeline editing

| Rule | Source | Plan |
|---|---|---|
| Compact timeline selection toolbar is used when media query "(max-width: 1023px)" matches (live-updated on change) | `src/components/workspace/editor-workspace.tsx:388-407` | P03 |
| Initial selected item: without `initialProject` use `initialSelectedTimelineItem(project)`; with `initialProject` select item id "caption-1" if present, else none; selectedItemIds seeded with it | `src/components/workspace/editor-workspace.tsx:906-927` | P03 |
| Timeline entries fallback when `project.timelines` empty: single entry `{id: activeTimelineId ?? "main", name: "Timeline 1"}`; active id = `project.activeTimelineId ?? first ?? "main"` | `src/components/workspace/editor-workspace.tsx:1460-1466` | P03 |
| Per-timeline view state `{playheadSeconds, zoomPercent, scrollLeft}` persisted to localStorage key `video-creater.timeline-view-state:${projectDir}`; invalid JSON/non-object → `{}` | `src/components/workspace/editor-workspace.tsx:1495-1508` | P03 |
| On active timeline change, playhead = pending switch playhead ?? stored view playhead ?? 0, clamped to timeline duration; pending cleared | `src/components/workspace/editor-workspace.tsx:1510-1524` | P03 |
| `updateActiveTimelineViewState` keeps stored playhead (else current playhead), merges zoom/scroll; no state update if all three values unchanged | `src/components/workspace/editor-workspace.tsx:1526-1545` | P03 |
| Active selection ids = deduped `selectedItemIds` if they include `selectedItemId`, else `[selectedItemId]`, else `[]` | `src/components/workspace/editor-workspace.tsx:1546-1553` | P03 |
| Link enabled when >= 2 selected items; Unlink enabled when any selected item has non-empty string `linkGroupId` | `src/components/workspace/editor-workspace.tsx:1554-1562` | P03 |
| Remove / ripple-delete / duplicate / nudge enabled only when >= 1 item selected and none are on a locked track | `src/components/workspace/editor-workspace.tsx:1563-1570` | P03 |
| Decompose enabled when removable AND exactly 1 selected AND its `source.type === "timeline"` | `src/components/workspace/editor-workspace.tsx:1580-1583` | P03 |
| Track header actions: `setTrackLocked{trackId,locked}`, `setTrackEnabled{trackId,enabled}`, `reorderTrack{trackId,targetTrackId,placement:"before"\|"after"}`, `setTrackSyncLocked{trackId,syncLocked}` | `src/components/workspace/editor-workspace.tsx:2189-2224` | P03 |
| Duplicate single item: no-op if not found or track locked; id `duplicateTimelineItemId`; start = start+duration (toFixed 3); label "`<label>` copy"; deep-cloned source/properties; `addItems` on same track; selects duplicate on success | `src/components/workspace/editor-workspace.tsx:2861-2890` | P03 |
| Duplicate-at-positions: abort entirely if any move's item/track missing, target locked, or `!itemAllowedOnTrack(kind, track.kind)`; ids `${id}-copy`, then `${id}-copy-2`, `-3`...; one `addItems` per target track via `applyProjectActions`; selects clones (first = primary) | `src/components/workspace/editor-workspace.tsx:2892-2972` | P03 |
| Duplicate link groups: if > 1 member of a group is duplicated, clones get new shared `linkGroupId` "link-copy-<uuid>" (fallback Date.now base36); single-member → `linkGroupId` deleted | `src/components/workspace/editor-workspace.tsx:2924-2948` | P03 |
| Duplicate selection: 1 item → single duplicate; multiple → all shifted by (selectionEnd - selectionStart) on own tracks (3dp); only runs if every item's track found | `src/components/workspace/editor-workspace.tsx:2974-3001` | P03 |
| Copy: captures sorted by track index, then start, then id; entry `trackOffset` = trackIndex - min trackIndex of first capture; `startOffset` = start - min start (3dp); clipboard `durationSeconds` = max(startOffset + duration); deep clones | `src/components/workspace/editor-workspace.tsx:3003-3044` | P03 |
| Cut single: no-op if track missing or locked; copy then `removeItems`. Cut selection requires removable selection; copy selection then remove selection | `src/components/workspace/editor-workspace.tsx:3046-3059` | P03 |
| Paste/insert materialize: anchor = first entry's original track (must still exist); target track = anchorIndex + trackOffset; abort all if target missing, locked, or kind not allowed | `src/components/workspace/editor-workspace.tsx:3061-3088` | P03 |
| Paste/insert link groups: > 1 members → new `linkGroupId` "link-paste-<uuid>"; else removed; id via `duplicateTimelineItemId`; label "`<label>` copy" (paste) or "`<label>` insert" (insert) | `src/components/workspace/editor-workspace.tsx:3089-3116` | P03 |
| Paste: `addItems` per track, start = playhead+startOffset (3dp); single action via `applyProjectAction` else `applyProjectActions`; selects clones | `src/components/workspace/editor-workspace.tsx:3119-3136` | P03 |
| Insert: `insertItems{targetTrackId, insertSeconds: start.toFixed(3), items}` with item start = startOffset; selects clones | `src/components/workspace/editor-workspace.tsx:3138-3156` | P03 |
| Ripple delete single item: no-op if locked; `rippleDeleteRanges` [start, start+duration (3dp)] on `rippleTrackIdsForItem(project,item,trackId)` (linked tracks); clears `selectedItemId` | `src/components/workspace/editor-workspace.tsx:3158-3183` | P03 |
| Ripple trim action: throws "Selected ripple-trim clip was not found."; `rippleTrimItem{itemId, edge, deltaSeconds, propagateLinked: true, syncLockedTrackIds: syncLocked tracks not among linked ripple tracks}` | `src/components/workspace/editor-workspace.tsx:3185-3202` | P03 |
| Remove current dead air: `currentTimelineSilenceRippleRange(project, playhead)`; no-op if null; `rippleDeleteRanges` with that one range | `src/components/workspace/editor-workspace.tsx:3216-3220` | P03 |
| Switch timeline: no-op if already active; saves current view state (zoom default 100, scrollLeft default 0); stops playback; clears selection, range, replacement target, clipboard; `setActiveTimeline`; on success playhead = target stored playhead ?? 0 clamped to new duration | `src/components/workspace/editor-workspace.tsx:3515-3540` | P03 |
| Create timeline: id `timeline-${count+1}` (collision → `-1`, `-2`...); name "Copy of `<source name ?? "Timeline">`" when duplicating else "Timeline `<count+1>`"; pending playhead 0; same state resets as switch; `createTimeline{timelineId,name,duplicateActive,sourceTimelineId?}` | `src/components/workspace/editor-workspace.tsx:3542-3570` | P03 |
| Rename timeline → `renameTimeline{timelineId,name}` (no trim/validation here) | `src/components/workspace/editor-workspace.tsx:3572-3574` | P03 |
| Delete timeline requires `window.confirm("Delete ${name ?? "this timeline"}? This cannot be undone.")`; then saves view state, stops playback, clears selection/range/replacement/clipboard, `deleteTimeline{timelineId}` | `src/components/workspace/editor-workspace.tsx:3576-3594` | P03 |
| Decompose: requires canDecompose; clears selection first; `decomposeTimelineItem{itemId}` | `src/components/workspace/editor-workspace.tsx:3596-3603` | P03 |
| Inspector split single clip: `splitItems` with `newItemId` `${itemId}-split-${round(splitSeconds*1000)}` | `src/components/workspace/editor-workspace.tsx:3822-3834` | P03 |
| Split selection: only unlocked items where start < split < end (strict); no-op if none; right-half ids `${id}-split-${ms}`; link groups with >= 2 selected members get right halves re-linked to "link-split-<uuid>" via extra `updateItemProperties` | `src/components/workspace/editor-workspace.tsx:3836-3888` | P03 |
| Trim selection to playhead: skip locked or playhead not strictly inside; start edge → `createLeftTrimPatchFromDrag`; end edge → `createRightTrimPatchFromDrag` ?? `createResizePatchFromDrag(duration = playhead-start, 3dp)`; batched as `trimItems` + `resizeItems` | `src/components/workspace/editor-workspace.tsx:3890-3942` | P03 |
| Automation lane keyframes (opacity/volumeDb) read from `properties.keyframes[property]`, keeping only finite numeric `atSeconds`/`value` (+ string easing) | `src/components/workspace/editor-workspace.tsx:3944-3971` | P03 |
| Move automation keyframe: drop points within 0.0005s of `fromSeconds`, push new, sort by time, `setItemKeyframes`; delete drops points within 0.0005s | `src/components/workspace/editor-workspace.tsx:3973-3994` | P03 |
| Timeline fade handles: `audio_clip` → `updateItemProperties set {fadeInSeconds, fadeOutSeconds}`; other kinds → `updateVisualClipFades` | `src/components/workspace/editor-workspace.tsx:3996-4022` | P03 |
| Source clip reorder → `reorderItems{reorder: update}` | `src/components/workspace/editor-workspace.tsx:4024-4029` | P03 |
| Link selected: requires >= 2; `linkItems{itemIds, linkGroupId: "link-<uuid>"}` (fallback `${Date.now()}-${Math.random()}`); unlink → `unlinkItems{itemIds}` | `src/components/workspace/editor-workspace.tsx:4049-4062` | P03 |
| Selecting expands to all items sharing non-empty `linkGroupId`; additive: if all linked already selected remove them, else union; non-additive: linked set; primary = last clicked if still selected else last of set; clears range + inspector override; returns [] if no items match | `src/components/workspace/editor-workspace.tsx:4064-4101` | P03 |
| On select (not playing and not `preservePlayhead`): playhead jumps to first selected item's midpoint (start + duration/2) clamped; applied in `flushSync` | `src/components/workspace/editor-workspace.tsx:4103-4114` | P03 |
| Remove single item: `removeItems`; on success clears selectedItemId/replacement target if they equal it | `src/components/workspace/editor-workspace.tsx:4161-4173` | P03 |
| Remove selection: requires removable; `removeItems{itemIds}`; clears selection; clears replacement target if removed | `src/components/workspace/editor-workspace.tsx:4175-4186` | P03 |
| Nudge selection: `moveItems` on same track, start = max(0, start+delta) (3dp) | `src/components/workspace/editor-workspace.tsx:4188-4205` | P03 |
| Multi ripple delete: one range per item on its own track only (no linked-track expansion), unlocked tracks; clears selection on success | `src/components/workspace/editor-workspace.tsx:4207-4227` | P03 |
| Ripple delete dispatcher: 1 item → linked ripple (`rippleTrackIdsForItem`); multiple → per-own-track ranges | `src/components/workspace/editor-workspace.tsx:4229-4237` | P03 |
| Ripple delete gap: no-op if gap <= 0; shift items with start >= gap end on the gap track AND every `syncLocked` track by -gap (3dp) via `moveItems` (locked tracks not excluded) | `src/components/workspace/editor-workspace.tsx:4239-4258` | P03 |
| TimelineEditor is remounted per timeline (`key={activeTimelineId}`); keyboard shortcuts enabled only when workspace `isActive`; transport shortcuts only when `isActive && viewerMode === "timeline"`; `nativeMenuRequest` forwarded; `framesPerSecond` = `project.renderSettings.fps` | `src/components/workspace/editor-workspace.tsx:8269-8276` | P03 |
| Timeline seek from editor clamps via `clampTimelinePlayhead(seconds, timeline.durationSeconds)` | `src/components/workspace/editor-workspace.tsx:8278-8282` | P03 |
| `selectedItemIds` passed to timeline: `[]` when no primary item; if primary item not in multi-selection list, list becomes `[selectedItemId]` | `src/components/workspace/editor-workspace.tsx:8283-8290` | P03 |
| Selection toolbar rendered only when `selectedTimelineItems.length > 0`; capability flags canLink/canUnlink/canRemove/canRippleDelete/canDuplicate/canNudge/canApplyEffects(batch visual)/canDecompose | `src/components/workspace/editor-workspace.tsx:8292-8303` | P03 |
| Selection toolbar disabled reasons: link "Select at least two clips to link."; unlink "The selected clips are not linked."; remove "Unlock the selected tracks to remove these clips."; ripple-delete "Unlock the selected tracks to ripple delete these clips."; duplicate "Unlock the selected tracks to duplicate these clips."; nudge "Unlock the selected tracks to nudge these clips."; effects "Select only unlocked visual clips to edit effects." | `src/components/workspace/editor-workspace.tsx:8304-8311` | P03 |
| Decompose disabled reason: if exactly 1 selected item with `source.type === "timeline"` -> "Unlock the selected track to decompose this sequence." else "Select one nested timeline sequence to decompose." | `src/components/workspace/editor-workspace.tsx:8312-8316` | P03 |
| Batch effect "grain" = `[{effectInstanceId:"batch:stylize.grain:1", effectType:"stylize.grain", enabled:true, params:{amount:0.18, size:1.5}}]`; otherwise (vignette) `[{effectInstanceId:"batch:stylize.vignette:1", effectType:"stylize.vignette", enabled:true, params:{amount:-0.25, midpoint:0.5, roundness:0, feather:0.5}}]`; applied via `applyBatchVisualEffects` (replaces list) | `src/components/workspace/editor-workspace.tsx:8324-8345` | P03 |
| Batch "clear effects" = `applyBatchVisualEffects([])` | `src/components/workspace/editor-workspace.tsx:8346` | P03 |
| Per-timeline view state: `initialZoomPercent` default 100, `initialScrollLeft` default 0, keyed by `activeTimelineId`; changes via `updateActiveTimelineViewState` | `src/components/workspace/editor-workspace.tsx:8356-8358` | P03 |
| `canUndo` = `projectHistory.past.length > 0`; `canRedo` = `projectHistory.future.length > 0`; `canPasteItems` = clipboard non-null | `src/components/workspace/editor-workspace.tsx:8359-8361` | P03 |
| Ripple-insert preview from clipboard: requires first entry; `trackId` = first entry trackId, `durationSeconds` = clipboard duration, label = single item label or "`${n} selected clips`" | `src/components/workspace/editor-workspace.tsx:8362-8373` | P03 |
| onSelectRange: clears project inspector override, sets range; a non-null range clears the selected item | `src/components/workspace/editor-workspace.tsx:8376-8382` | P03 |
| onMoveItems -> `applyProjectAction({type:"moveItems", moves})`; onPlanRippleTrim -> `planProjectRippleTrim(project, rippleTrimAction(request))` (dry-run); onRippleTrim -> `applyProjectAction(rippleTrimAction(request))` | `src/components/workspace/editor-workspace.tsx:8439-8444` | P03 |
| Timeline wiring: split item/selected, trim-to-playhead, automation keyframe change/delete, item fades, remove/ripple delete item & selection, ripple delete gap, duplicate item/selection/at positions, copy/cut item & selection, paste (overwrite) vs insert (ripple) at startSeconds, patch -> `applyTimelinePatch`, undo/redo -> `undoProjectEdit`/`redoProjectEdit`, track enable/reorder/sync-lock | `src/components/workspace/editor-workspace.tsx:8389-8447` | P03 |
| Template/shader-background drops on timeline -> `insertTemplate(templateId, startSeconds)` / `insertShaderBackgroundTemplate(templateId, startSeconds)` | `src/components/workspace/editor-workspace.tsx:8448-8453` | P03 |
| "Timeline template drawer" toggled by `onOpenTemplateLibrary`; inserts template/shader background at given start | `src/components/workspace/editor-workspace.tsx:8436,8455-8473` | P03 |
| Timeline error banner: "Timeline edit rejected: {timelinePatchError}" (role alert, aria-live off) | `src/components/workspace/editor-workspace.tsx:8474-8482` | P03 |
| TimelineTabBar: create -> `createTimeline(false)`; duplicate -> `createTimeline(true, timelineId)`; rename/delete/activate wired | `src/components/workspace/editor-workspace.tsx:8260-8268` | P03 |
| Timeline/preview height resize handle shown only in multi-pane layout with no maximized pane; dispatches `resizeTimeline` ratio | `src/components/workspace/editor-workspace.tsx:8251-8258` | P03 |
| Filmstrip request skipped when projectDir empty or starts with "browser://"; deduped per itemId by `JSON.stringify(request)` (mediaId, sourceIn, sourceOut, speed, zoomBucket, heightBucket, clipPixelWidth); stale responses ignored; empty frame list ignored; frames without preview URL dropped; on error the key is deleted so it can retry | `src/components/workspace/editor-workspace.tsx:6968-7001` | P03 |
| Remove current dead air at playhead available when `currentTimelineSilenceRippleRange(project, playhead)` truthy; -> `removeCurrentDeadAir()` | `src/components/workspace/editor-workspace.tsx:6719-6721,6739` | P03 |
| onQueueItemUpscale from timeline: select item, then `queueMediaUpscale(mediaId)` (no source-clip context) | `src/components/workspace/editor-workspace.tsx:8418-8421` | P03 |
| Note: file has no multi-timeline selector, no dialogs/validation copy, no silence-removal action; dead-air is display-only (waveform masks). Lib constants referenced (not local): snap step `timelineSnapSeconds`=0.25s, zoom 50..200%, basePixelsPerSecond 80, track height 44..200 default 48 | `src/components/workspace/timeline-editor.tsx:86-113` | P03 |
| Track item vertical inset 4px (item top = row top + 4, height = max(1, rowHeight - 8)); ruler height 24px; header column width `clamp(152px, 15vw, 220px)` | `src/components/workspace/timeline-editor.tsx:122-124` | P03 |
| Overview canvas width = max(1, viewportWidth - 140 (zoom controls) - 8 (gap) - 16 (padding)) | `src/components/workspace/timeline-editor.tsx:125-127,1816-1822` | P03 |
| Minimum canvas width 720px; canvasWidth = max(renderedDuration * pixelsPerSecond, 720); viewport width fallback 720 when unmeasured, floored, min 1 | `src/components/workspace/timeline-editor.tsx:176,1647,1801-1804` | P03 |
| Zoom step 25%; zoom in/out buttons clamp via `clampZoomPercent(zoom ± 25)`; disabled when zoom <= min / >= max; range input min/max lib values, step 25; every zoom change emits onViewStateChange({zoomPercent, scrollLeft}) | `src/components/workspace/timeline-editor.tsx:177,1627-1628,3682-3691,5139-5178` | P03 |
| pixelsPerSecond = basePixelsPerSecond * zoomPercent/100; initial zoom = clampZoomPercent(initialZoomPercent ?? 100); initial scrollLeft = max(0, initialScrollLeft) | `src/components/workspace/timeline-editor.tsx:1336-1337,1432,1438-1440,1629` | P03 |
| Ruler/canvas/overview scroll sync: ignore echo scroll within 0.5px of a programmatically assigned value; scroll state only updates if delta > 0.5px; ruler scroll pushes to canvas, canvas to ruler, overview to both | `src/components/workspace/timeline-editor.tsx:1711-1750` | P03 |
| Overview pan by fraction: no-op if window duration <= 0; newStart clamped to [0, max(0, renderedDuration - windowDuration)]; window change resolved via `resolveTimelineOverviewViewState` then sets zoom and scroll | `src/components/workspace/timeline-editor.tsx:3693-3732` | P03 |
| Minimum move drag threshold 3px (either axis) before a move counts; marquee movement < 3px (hypot) is a click that selects the empty gap under pointer | `src/components/workspace/timeline-editor.tsx:178,3438-3448,3161-3171` | P03 |
| Resize control mode: clip pixel width < 48px → "compact-dock", else "edge-handles"; compact dock width 32px, centered on clip and clamped within [0, max(32, timelineWidth) - 32], alignment "start"/"center"/"end" | `src/components/workspace/timeline-editor.tsx:179-180,183-214,378-382` | P03 |
| Resize handles shown only if track not locked AND (clip selected OR mode "edge-handles"); resize handles tabbable only when selected; mode frozen at interaction start for the active item | `src/components/workspace/timeline-editor.tsx:4678-4685,4925` | P03 |
| Tracks reserve compact resize strip when any selected, unlocked, rendered item on the track is in compact-dock mode | `src/components/workspace/timeline-editor.tsx:1847-1864,4735-4737` | P03 |
| Standard track names (non-ellipsis treatment): "Video","HyperFrames","Overlays","Captions","Audio"; other names truncate with ellipsis | `src/components/workspace/timeline-editor.tsx:182-188,4227-4229` | P03 |
| Track enable toggle: audio tracks label "audible"/"muted", title "Mute track"/"Unmute track" (Volume2/VolumeX); non-audio "visible"/"hidden", title "Hide track"/"Show track" (Eye/EyeOff); click → onToggleTrackEnabled(id, !enabled); disabled without handler | `src/components/workspace/timeline-editor.tsx:539-580` | P03 |
| Track sync-lock toggle (Magnet): aria "{name} track sync locked/unlocked", aria-pressed = syncLocked===true; title "Exclude track from independent ripple timing" when locked, "Keep track synchronized during ripple edits" when unlocked; click → onToggleTrackSyncLocked(id, syncLocked !== true) | `src/components/workspace/timeline-editor.tsx:581-591` | P03 |
| Track reorder: zones split audio vs non-audio; up moves before previous track in same zone, down moves after next in same zone; buttons disabled at zone boundaries or without onReorderTrack; titles "Move track up"/"Move track down" | `src/components/workspace/timeline-editor.tsx:596-629,4230-4241,4311-4325` | P03 |
| Track height handle (separator, aria min/max/now): pointer drag previews clampTrackDisplayHeight(start + dy), commits on pointerup, cancel discards preview; keyboard ArrowUp/ArrowDown step 2px (Shift 10px), ArrowDown increases; only rendered when onSetTrackDisplayHeight given; title "Resize track height" | `src/components/workspace/timeline-editor.tsx:631-704,4328-4351` | P03 |
| Locked track: drag-over dropEffect "none" (else "copy"); pointerdown on locked clip only selects (no move/razor); resize handles hidden; locked tracks excluded as move targets | `src/components/workspace/timeline-editor.tsx:4513-4519,4947-4950,4683,1938,2611` | P03 |
| Hidden/muted state passed to clip shell as `enabled={trackEnabled(track)}` | `src/components/workspace/timeline-editor.tsx:4659,4727` | P03 |
| Playhead clamped via clampPlayheadSecondsForTimeline(controlled ?? internal, duration); seek sets internal only when uncontrolled then calls onSeekPlayhead | `src/components/workspace/timeline-editor.tsx:1534-1544` | P03 |
| Canvas pointer → seconds = clamp(round3((clientX - left)/pps), duration) | `src/components/workspace/timeline-editor.tsx:2853-2865` | P03 |
| Canvas click seeks unless suppressed-once flag set or target is button/input/select/textarea/contenteditable | `src/components/workspace/timeline-editor.tsx:524-528,3304-3315` | P03 |
| Canvas pointerdown: first try marquee (select tool, no Shift, non-interactive target); else scrub: Shift+pointerdown starts range drag anchor (clears range, suppresses next click); plain pointerdown starts playhead scrub and seeks | `src/components/workspace/timeline-editor.tsx:3127-3144,3317-3337,4373-4377` | P03 |
| Mouse scrub fallback ignored when marquee active, interactive target, or Shift | `src/components/workspace/timeline-editor.tsx:3407-3436` | P03 |
| Pointer move priority: update hover → marquee → range edge drag → range anchor drag → playhead scrub | `src/components/workspace/timeline-editor.tsx:3353-3384` | P03 |
| Pointer up: finish marquee if active, else clear range drag anchors, snap and scrub; pointer leave/cancel clears hover and marquee too | `src/components/workspace/timeline-editor.tsx:3390-3405` | P03 |
| Ruler: Shift+pointerdown starts range drag (suppresses next ruler click); ruler click seeks unless suppressed or Shift held | `src/components/workspace/timeline-editor.tsx:3339-3351,4110-4116` | P03 |
| Range selection min length 0.25s (timelineSnapSeconds): shorter → null; both edges clamped to duration | `src/components/workspace/timeline-editor.tsx:2867-2885` | P03 |
| Range edge drag: dragged edge constrained to keep >= 0.25s from opposite edge; snap targets filtered likewise; snaps to edit points (excluding none) + playhead via resolveTimelineSnap, else grid snap; shows snap guide; clamped to duration | `src/components/workspace/timeline-editor.tsx:2887-2935` | P03 |
| Range edge drag start requires existing range; suppresses next canvas & ruler click; clears snap; hover set to dragged edge; handles titled "Adjust range start"/"Adjust range end" on both ruler and canvas | `src/components/workspace/timeline-editor.tsx:2982-3012,4134-4155,4456-4477` | P03 |
| Range is controlled when selectedRange prop !== undefined; setting non-null range clears pending I/O marks; always calls onSelectRange | `src/components/workspace/timeline-editor.tsx:1752-1753,2937-2945` | P03 |
| I/O range marks (when source mark not possible): I sets start=playhead, O sets end=playhead, other edge from current range, else pending mark, else playhead; if resulting range < 0.25s store pending partial mark and clear range | `src/components/workspace/timeline-editor.tsx:2947-2980` | P03 |
| Range toolbar group shows "{start}-{end}" timecodes, "Add" button (aria "Add range to Codex", disabled without onAddRangeToChat) and "Clear" button (aria "Clear timeline range") | `src/components/workspace/timeline-editor.tsx:1754-1758,4027-4059` | P03 |
| Snap during move/resize: targets = edit points (0, duration, every other item start/end, excluding dragged items) + playhead via timelineSnapTargets; probes = start & end of every group item (move), end (resizeRight), start (resizeLeft); sticky snap retained; if guide found use pointer delta + snap delta else grid snap `snapTimelineSeconds` | `src/components/workspace/timeline-editor.tsx:2765-2818,3466-3471` | P03 |
| Visible snap guide = explicit range-drag guide ?? move/resize evaluation guideSeconds | `src/components/workspace/timeline-editor.tsx:2825-2829` | P03 |
| Move evaluation: raw delta clamped so earliest group item start >= 0; proposed lead start rounded to 3 decimals; rejected with reason code "track_incompatible", message "Destination unavailable" when any non-pinned item's translated row doesn't exist; else evaluateTimelineMove with per-item target track ids | `src/components/workspace/timeline-editor.tsx:3456-3507` | P03 |
| Move target track resolution: cursor row from original row center + pointer dy; items linked to lead (same linkGroupId) or incompatible with lead's track kind are pinned to their own track; track delta stepped toward 0 until all movable items land on existing, unlocked, kind-compatible tracks; accepted only if delta unchanged and candidate track exists & unlocked | `src/components/workspace/timeline-editor.tsx:2581-2661` | P03 |
| Move commit on pointerup: only if passed 3px threshold and evaluation not "rejected"; Alt held at pointerdown → onDuplicateItemsAtPositions(patches); >1 patches → onMoveItems; else onTimelinePatch(createMovePatchFromDrag(move)) | `src/components/workspace/timeline-editor.tsx:3603-3636,4991` | P03 |
| Clip pointerdown (select tool): select via selectTimelineItem (Cmd/Ctrl additive); abort if item not in resulting selection or any group item on locked track; cancels previous preview/snap, dismisses interaction error, starts move with all selected items as group | `src/components/workspace/timeline-editor.tsx:4945-5005` | P03 |
| Clip keyboard click (event.detail === 0) selects (Cmd/Ctrl additive); double-click opens source media via onOpenSource(itemId, mediaId) if source media exists | `src/components/workspace/timeline-editor.tsx:4928-4944` | P03 |
| Lost pointer capture / pointercancel cancel interaction (no commit) | `src/components/workspace/timeline-editor.tsx:5017-5018,5076-5077` | P03 |
| Selection fallback logic: with onSelectItems use returned ids, else additive toggles membership, non-additive on already-selected keeps selection, otherwise single; with only onSelectItem → single; always clears selected gap | `src/components/workspace/timeline-editor.tsx:3067-3097` | P03 |
| Resize (non-ripple) evaluation: resizeLeft proposed start snapped, duration = originalEnd - start; resizeRight end = max(start + 0.1, snapped end) (min clip 0.1s); evaluateTimelineResize with edge "left"/"right"; commit onTimelinePatch(evaluation.patch) if patch | `src/components/workspace/timeline-editor.tsx:3538-3581,3630-3633` | P03 |
| Shift held at resize-handle pointerdown → ripple trim: request {itemId, edge "left"/"right", deltaSeconds round3}; if delta != 0 call onPlanRippleTrim, thrown error message becomes preview error; commit onRippleTrim(request) only when error null | `src/components/workspace/timeline-editor.tsx:3513-3536,3624-3629,5039,5050` | P03 |
| Ripple trim preview: planned resizes/shifts rendered on clips (opacity-70 ring); error banner "Ripple trim blocked: {error}"; live message "Ripple resizing {label}" or error | `src/components/workspace/timeline-editor.tsx:4594-4598,4600-4601,3885-3892` | P03 |
| Resize handle keyboard (ArrowLeft/Right, no Alt/Ctrl/Meta): Shift → ripple trim by ±0.25s (onPlanRippleTrim then onRippleTrim, errors swallowed fail-closed); else evaluateTimelineResize by ±0.25s, clamped/rejected sets feedback "Resize limited to the available range"/"Invalid resize"; applies patch if any | `src/components/workspace/timeline-editor.tsx:3734-3808` | P03 |
| Move feedback copy: clamped "Move limited to the available range", rejected "Invalid destination"; resize: "Resize limited to the available range", "Invalid resize"; evaluation reason.message overrides | `src/components/workspace/timeline-editor.tsx:2676-2720,1948-1965` | P03 |
| Track row target label "{rowTitle} accepts/clamps/rejects moving/resizing {itemLabel}"; accepted resize doesn't highlight row; reason chip shown on row | `src/components/workspace/timeline-editor.tsx:2721-2755,4522-4531` | P03 |
| Live region message: clamped/rejected → reason ?? "Timeline edit unavailable"; else "Moving {label}"/"Resizing {label}"; idle → visible interaction error displayMessage | `src/components/workspace/timeline-editor.tsx:3870-3894` | P03 |
| Interaction error (prop) placement: target track from parsed error, else item's current track, else last interaction target row; shown inline on row, else in bottom status strip; dismissed when a new interaction starts; reset when error clears | `src/components/workspace/timeline-editor.tsx:1491-1521,3591-3601,4532-4545,5088-5105` | P03 |
| Razor tool: button disabled without onSplitItem; title "Razor timeline tool (C)"; click on unlocked clip splits at snapped pointer seconds (round3), ignored if <= start, >= end, or clip duration <= 0.1s; selects the clip first | `src/components/workspace/timeline-editor.tsx:3014-3037,3942-3954,4951-4963` | P03 |
| Split seconds: multi-select → playhead; single → playhead if strictly inside clip else clip midpoint (round3); splittable items = selected, unlocked, split point strictly inside | `src/components/workspace/timeline-editor.tsx:1560-1586` | P03 |
| canSplitSelected = handler (onSplitSelectedItems if >1 selected else onSplitItem) && selectedItem && splittable items > 0; multi → onSplitSelectedItems(seconds) else onSplitItem(id, seconds); button "Split selected clip (S)" | `src/components/workspace/timeline-editor.tsx:1587-1591,3955-3977` | P03 |
| canRemoveSelected / canRippleDeleteSelected / canCutSelected require handler + selectedItem + primary item's track unlocked; canCopySelected requires onCopyItem + selectedItem (locked ok) | `src/components/workspace/timeline-editor.tsx:1592-1611` | P03 |
| canDuplicateSelected requires (onDuplicateSelectedItems ?? onDuplicateItem) and every selected item exists on an unlocked track | `src/components/workspace/timeline-editor.tsx:1600-1606` | P03 |
| canSetSourceMark requires onTimelinePatch, selected source.type "media", unlocked track, playhead strictly inside clip; source mark buttons shown only then: "Set source in point" ("Trim selected clip in point to the playhead (I)"), "Set source out point" ("... out point ... (O)"); patch via createTrimPatchFromSourceMark | `src/components/workspace/timeline-editor.tsx:1612-1617,3810-3823,3978-4000` | P03 |
| canNudgeSelected requires (multi: onMoveItems ?? onNudgeSelectedItems; single: onTimelinePatch) + selectedItem on unlocked track | `src/components/workspace/timeline-editor.tsx:1618-1625` | P03 |
| Shift+ArrowLeft/Right nudge by ±0.25s: delta clamped so group min start >= 0 and (single item only) end <= timeline duration; no-op if delta 0 or evaluation rejected (feedback "Invalid destination"); >1 patches → onMoveItems else onNudgeSelectedItems(delta); single → onTimelinePatch(createMovePatchFromDrag) | `src/components/workspace/timeline-editor.tsx:2254-2309` | P03 |
| Shift+ArrowUp/Down moves selection to nearest unlocked kind-compatible track above/below; linked (same linkGroupId) or incompatible companions stay; others shift by same row delta (missing → "__unavailable_timeline_track__"); evaluateTimelineMove at same start; rejected → feedback; commit onMoveItems or onTimelinePatch | `src/components/workspace/timeline-editor.tsx:1916-1946,2182-2252` | P03 |
| Marquee selects items whose box intersects rect (inclusive) using rowTop+4..rowBottom-4; non-empty result → onSelectItems({itemIds, additive: Cmd/Ctrl at start}); starts only in select tool and without Shift | `src/components/workspace/timeline-editor.tsx:3107-3178` | P03 |
| Empty-gap selection: click-without-drag on empty canvas selects gap via timelineGapAtSeconds(track, x/pps); gap overlay title "Selected empty gap. Press Delete to ripple-close it."; Delete/Backspace on it → onRippleDeleteGap(gap) then clear | `src/components/workspace/timeline-editor.tsx:3165-3170,4158-4184` | P03 |
| Ruler/canvas edit points shown only when > 0, < duration and within render window | `src/components/workspace/timeline-editor.tsx:1881-1886` | P03 |
| Drop on track row: reads "application/x-video-creater-background-template" (shader background) and "application/x-video-creater-template" or "text/plain" (template); ignore if neither; locked target → dropEffect "none"; start = round3(snap(max(0, Number(getData("application/x-video-creater-start-seconds")) if finite else clientX/pps))); shader → onShaderBackgroundDrop(id, start, trackId) (priority), else onTemplateDrop(id, start, trackId). Quirk: empty start-seconds string → Number("")=0 so start 0 | `src/components/workspace/timeline-editor.tsx:3638-3680,4513-4520` | P03 |
| Ripple insert preview (hover/focus "Insert (Ripple)"): requires preview duration > 0 and track exists; items on that track starting >= insert point shifted by duration (round3); rendered duration = max(duration, insert end); label "Insert: {label}"; aria "Ripple insert preview: {label} at {tc}, shifts later clips by {tc}"; shifted clips title suffix " (ripple preview)" | `src/components/workspace/timeline-editor.tsx:1767-1800,4572-4593,4654-4658,4733,5265-5275` | P03 |
| Context menu open on clip: selects clip (non-additive), kind "clip", seconds = playhead; on canvas (non-interactive target): seconds = pointer seconds, kind "range" if inside selected range (inclusive) else "canvas"; both clear ripple insert preview | `src/components/workspace/timeline-editor.tsx:3050-3065,3180-3202` | P03 |
| Context menu labels: "Timeline clip actions" / "Timeline range actions" / "Timeline actions" | `src/components/workspace/timeline-editor.tsx:3831-3836` | P03 |
| Context menu items & enable: clip-only "Copy" (item && onCopyItem), "Cut" (item && onCutItem && unlocked); range-only "Add Range to Chat" (range && handler), "Save Range as Media" (range && onSaveRangeAsMedia), "Clear Range" (always); always "Paste (Overwrite)" (canPasteItems && onPasteItems), "Insert (Ripple)" (canPasteItems && onInsertItems); clip-only "Duplicate" (onDuplicateItem && unlocked), "Queue Upscale" (kind != audio_clip && source media && handler), "Set as first frame"/"Set as reference" (rendered only for image_clip with source media && onCreateVideoFromItem), "Delete" (onRemoveItem && unlocked), "Ripple Delete" (onRippleDeleteItem && unlocked) | `src/components/workspace/timeline-editor.tsx:3837-3869,5195-5343` | P03 |
| Context menu actions: paste/insert use menu seconds (playhead for clip, pointer for canvas); copy/cut/duplicate/delete/rippleDelete act on single itemId (not multi-selection); queueUpscale re-checks media && not audio → onQueueItemUpscale(id, mediaId); createVideo re-checks image_clip → onCreateVideoFromItem(id, mediaId, "firstFrame"\|"reference") | `src/components/workspace/timeline-editor.tsx:3204-3302` | P03 |
| Text overlay button "Add text overlay" → onAddTextOverlay(playhead), disabled without handler; "Open template assets" → onOpenTemplateLibrary | `src/components/workspace/timeline-editor.tsx:4001-4020` | P05 |
| Option/Alt-drag duplicate preview: clip title suffix " (Option-drag duplicate preview)", ghost "{label} copy" translated by lead delta and row delta (only if target accepted) | `src/components/workspace/timeline-editor.tsx:4699-4701,4868-4881` | P03 |
| Move preview geometry: dragged (non-duplicate) items rendered at start + lead delta on resolved target track only after threshold and not rejected; resize preview uses evaluation patch ("trimItem" start+duration, "resizeItem" duration) | `src/components/workspace/timeline-editor.tsx:2830-2851,4603-4643` | P03 |
| Virtualization: only items intersecting viewport render, but selected/dragged/resizing items always persist; filmstrip requests only for truly visible items | `src/components/workspace/timeline-editor.tsx:1831-1876` | P03 |
| Timeline canvas aria-description "{duration tc} duration, {round(zoom)}% zoom"; footer "Project · {renderedDuration.toFixed(2)}s"; header range "00:00 - {formatTimestamp(renderedDuration)}" | `src/components/workspace/timeline-editor.tsx:1806,4367,5113-5118` | P03 |
| Escape (no modifiers): tool → select, cancel interaction/preview, close context menu, clear ripple insert preview, clear pending I/O marks and range, stop pointer interaction | `src/components/workspace/timeline-editor.tsx:2083-2101` | P03 |
| Undo/Redo toolbar buttons "Undo timeline edit"/"Redo timeline edit" disabled by !canUndo/!canRedo | `src/components/workspace/timeline-editor.tsx:3909-3928` | P07 |
| Nothing in the source clip inspector commits on blur or Enter. Every draft commits only through its own Apply button. Exceptions: speaker rename commits on blur; caption group selects and caption word emphasis, timing and animation edits apply on every change. | `src/components/workspace/source-clip-inspector.tsx:1660-2371` | P04 |
| Edit controls render only when `item && !readOnly` | `src/components/workspace/source-clip-inspector.tsx:1660` | P03 |
| Trim inputs: "Clip start" min 0 step 0.1; "Clip duration" min 0.1 step 0.1; "Source in"/"Source out" min 0 step 0.1 | `src/components/workspace/source-clip-inspector.tsx:1663-1710` | P03 |
| `optionalNumber`: blank/whitespace -> null; a non-finite value -> null; otherwise Number(value). Used by all optional fields. | `src/components/workspace/source-clip-inspector.tsx:304-311` | P03 |
| Source range: valid if both in/out are empty, or both are set with in >= 0 and out > in. A partial range (only one set) blocks Apply. | `src/components/workspace/source-clip-inspector.tsx:772-788` | P03 |
| Source span = Number((out-in).toFixed(3)); must match the draft clip duration within tolerance 0.01 s (skipped if span is null or duration is non-finite) | `src/components/workspace/source-clip-inspector.tsx:355,789-799` | P03 |
| Span mismatch warning (amber): "Source span ${formatSeconds(span)} must match clip duration ${formatSeconds(duration)}." | `src/components/workspace/source-clip-inspector.tsx:800-805,1712-1716` | P03 |
| "Apply clip trim" enabled only if: no trim draft conflict, item and onApply exist, start finite >= 0, duration finite > 0, no partial range, range valid, span matches | `src/components/workspace/source-clip-inspector.tsx:815-825` | P03 |
| Apply trim calls onApply(item.id, {startSeconds, durationSeconds}) and adds sourceIn/sourceOut only when both are present | `src/components/workspace/source-clip-inspector.tsx:1717-1741` | P03 |
| "Split at" defaults to item.start + duration/2 (or "0" with no item). Input min = item.start, max = start+duration, step 0.1. | `src/components/workspace/source-clip-inspector.tsx:313-315,2303-2312` | P03 |
| "Split clip" enabled if no split conflict, onSplit exists, value finite and strictly start < splitAt < start+duration. Calls onSplit(item.id, absolute timeline seconds). | `src/components/workspace/source-clip-inspector.tsx:969-976,2314-2327` | P03 |
| "Sequence" block shows only when reorderContext.itemIds.length > 1. "Earlier" (aria "Move clip earlier") needs selectedIndex > 0; "Later" (aria "Move clip later") needs selectedIndex < length-1. Calls onReorder(sourceClipReorderUpdate(ctx, "earlier"\|"later")) (lib). | `src/components/workspace/source-clip-inspector.tsx:977-984,2329-2367` | P03 |
| Selection label: "`N` clip selected" when N===1 else "`N` clips selected" | `src/components/workspace/timeline-selection-toolbar.tsx:86` | P03 |
| Selection action order + labels: "Link selected" (canLink), "Unlink selected" (canUnlink), "Remove selected" (canRemove, destructive), "Ripple delete selected" (canRippleDelete, destructive), "Duplicate selected" (canDuplicate), "Nudge -0.25 seconds" / "Nudge +0.25 seconds" (canNudge; onNudge(-0.25 / 0.25)), "Apply grain" / "Apply vignette" / "Clear effects" (all gated by canApplyEffects; onApplyEffect("grain"\|"vignette"), onClearEffects), "Decompose sequence" (canDecompose) | `src/components/workspace/timeline-selection-toolbar.tsx:87-155` | P03 |
| Nudge delta type is restricted to exactly -0.25 \| 0.25 seconds | `src/components/workspace/timeline-selection-toolbar.tsx:47` | P03 |
| Disabled-reason lookup keyed by action group ("link","unlink","remove","ripple-delete","duplicate","nudge","effects","decompose"); shown only when action disabled | `src/components/workspace/timeline-selection-toolbar.tsx:20-28,190,234` | P03 |
| Each action button disabled = !enabled | `src/components/workspace/timeline-selection-toolbar.tsx:200,247` | P03 |
| Compact mode: "Actions" button (aria-label "More clip actions") toggles a menu "Selected clip actions"; Escape closes; running an item closes menu; disabled reason rendered as description text under item (aria-describedby) | `src/components/workspace/timeline-selection-toolbar.tsx:157-221` | P03 |
| Full mode: icon buttons with aria-label = label, tooltip title = disabled reason ?? label; separators before indices 3 (remove group), 5 (nudge), 7 (effects), 10 (decompose) | `src/components/workspace/timeline-selection-toolbar.tsx:232-254` | P03 |
| Resize control mode default: pixelWidth < 48 -> "compact-dock", else "edge-handles" (override via resizeControlMode) | `src/components/workspace/timeline-item.tsx:232-235` | P03 |
| Edge handles rendered only when showResizeHandles and not compact dock; left handle before body, right after; each is 20px wide (w-5) absolute at item edge, z-30 | `src/components/workspace/timeline-item.tsx:141,298-312,442-456` | P03 |
| Compact dock: 32px x 16px strip (w-8 h-4) at bottom, left offset = compactResizeDockOffsetPx (default 0), alignment default "center"; contains left+right 16px handles | `src/components/workspace/timeline-item.tsx:207-208,405-441` | P03 |
| reserveCompactResizeStrip: clip body hit area shrinks to top..bottom-16px (bottom-4) so resize strip does not overlap body | `src/components/workspace/timeline-item.tsx:318-326` | P03 |
| Resize handle a11y: aria-label "Resize `label` left\|right edge", aria-valuemin 0, aria-valuenow = edge seconds (left=start, right=start+duration), valuetext "Left\|Right edge at X.XX seconds; duration Y.YY seconds", title "Resize left\|right edge" | `src/components/workspace/timeline-item.tsx:135-158` | P03 |
| Resize handles tabbable (tabIndex 0) when resizeTabbable (default = selected) OR item focused OR focus is inside shell; internal focus flag set on focus capture only if !resizeTabbable && !focused, cleared when focus leaves shell | `src/components/workspace/timeline-item.tsx:204,260-261,288-296` | P03 |
| Resize handles forward pointerdown/move/up/cancel/lostpointercapture/keydown with edge arg | `src/components/workspace/timeline-item.tsx:162-167` | P03 |
| Resize rule highlight: amber when selected; otherwise visible only on hover/focus-within | `src/components/workspace/timeline-item.tsx:172-176` | P03 |
| Live resize bubble shown when interactionEdge && interactionPlacement: aria-label "Left\|Right edge `timecode`, duration `timecode`" (formatInteractionTimecode), text "Left\|Right `timecode` · D.DDDs" (duration toFixed(3)); positioned at the dragged edge | `src/components/workspace/timeline-item.tsx:241-245,386-404` | P03 |
| Displayed duration/end during interaction uses interactionPlacement.durationSeconds ?? item.durationSeconds; end = timelineStartSeconds + duration | `src/components/workspace/timeline-item.tsx:236-237` | P03 |
| Density (timelineItemDensity(pixelWidth) from lib): title shown unless "accent"; timing shown for "timing"/"rich"; visuals (waveform/filmstrip) and status only for "rich" | `src/components/workspace/timeline-item.tsx:231,238-240,351,364` | P03 |
| Timing label format "`formatTimecode(start)`-`formatTimecode(end)`" | `src/components/workspace/timeline-item.tsx:379` | P03 |
| Rich-media content label boxed (max width 100%-2rem) only when showRich && richMedia | `src/components/workspace/timeline-item.tsx:355-360` | P03 |
| When not rich and compactStatusLabel set, body aria-label = "`label`, `compactStatusLabel`"; title = "`title ?? label` — `compactStatusTitle`" | `src/components/workspace/timeline-item.tsx:254-259` | P03 |
| Disabled item (enabled=false) rendered at opacity 45% + grayscale; invalid -> red border/ring; razor tool -> crosshair cursor; dragging -> z-20, opacity 75% | `src/components/workspace/timeline-item.tsx:249-253,270` | P03 |
| liveGeometry disables left/width transitions (otherwise 100ms ease-out) | `src/components/workspace/timeline-item.tsx:266-268` | P03 |
| Shell data attributes: data-selected, data-focused, data-dragging, data-generated, data-invalid, data-drag-mode ("move"\|"duplicate"), data-resize-layout, hit regions "clip-body" / "resize-control" / "resize-dock" | `src/components/workspace/timeline-item.tsx:272-286,317,159,408` | P03 |
| Kind color mapping: caption=slate/sky accent, overlay=cyan/amber accent, hyperframe_scene=violet, audio_clip=emerald, default (video etc.)=blue/cyan accent | `src/components/workspace/timeline-item.tsx:86-108` | P03 |
| TimelineLiveStatus: polite atomic live region; re-announces only when message changes from last value (null -> "") | `src/components/workspace/timeline-item.tsx:461-476` | P03 |

## Clip properties / visual

| Rule | Source | Plan |
|---|---|---|
| Inspector selection kind: caption if `kind === "caption"`; template if `isTemplateTimelineItem`; text overlay if `kind "overlay"` + `source.type "text"` and not template; otherwise source clip | `src/components/workspace/editor-workspace.tsx:1636-1655` | P04 |
| Batch visual effects enabled only when every selected item is a visual source item with `source.type !== "timeline"` and none locked; applies `updateItemEffects{itemIds, effects}` | `src/components/workspace/editor-workspace.tsx:1571-1579`, `:3708-3715` | P04 |
| After adding an effect, focus moves to inspector input with aria-label "Enable `<displayName>`" once it renders | `src/components/workspace/editor-workspace.tsx:1095-1114` | P04 |
| Source clip trim → `trimItems` with `startSeconds`, `durationSeconds`, optional `sourceIn`/`sourceOut` only when defined | `src/components/workspace/editor-workspace.tsx:3377-3390` | P04 |
| Lottie inputs → `updateItemProperties set {lottieInputs}` | `src/components/workspace/editor-workspace.tsx:3392-3397` | P04 |
| Opacity → `updateVisualClipOpacity`; opacity keyframes → `setItemKeyframes property "opacity"`; motion keyframes → `setItemKeyframes` with given property | `src/components/workspace/editor-workspace.tsx:3507-3513`, `:3605-3620` | P04 |
| Keyframe CRUD actions: `upsertItemKeyframe`, `moveItemKeyframe{fromSeconds,toSeconds}`, `deleteItemKeyframe{atSeconds}`; effect params: `upsertEffectParameterKeyframe`, `moveEffectParameterKeyframe`, `deleteEffectParameterKeyframe` (with effectInstanceId, parameterKey) | `src/components/workspace/editor-workspace.tsx:3622-3698` | P04 |
| Color grade → `updateItemColorGrade{itemIds:[id], reset: false, grade}`; effects → `updateItemEffects{itemIds:[id]}`; transform → `updateVisualClipTransform` | `src/components/workspace/editor-workspace.tsx:3700-3719` | P04 |
| Rotation: if item has `keyframes.rotationDegrees` array → `upsertItemKeyframe` at clamp(playhead - start, 0, duration); else `updateItemProperties` set `rotationDegrees`, value 0 removes the property | `src/components/workspace/editor-workspace.tsx:3721-3761` | P04 |
| Crop: if any of cropTop/cropRight/cropBottom/cropLeft keyframed → upsert all 4 keyframes (missing side = 0) at local playhead time; else `updateVisualClipCrop` | `src/components/workspace/editor-workspace.tsx:3763-3785` | P04 |
| Visual fades → `updateVisualClipFades{fadeInSeconds, fadeOutSeconds}` | `src/components/workspace/editor-workspace.tsx:3787-3793` | P04 |
| Speed change: current speed = `properties.speed` if number > 0 else 1; new duration = duration * current / new (toFixed 3); actions `updateVisualClipSpeed` + `resizeItems` together | `src/components/workspace/editor-workspace.tsx:3795-3809` | P04 |
| Blend mode "over" removes `blendMode` property; other modes set it | `src/components/workspace/editor-workspace.tsx:3811-3820` | P04 |
| Inspector selection kind: >1 items "multiple"; template item or `lottie_clip` "template"; caption/text overlay item, or (no timeline item && source destination "captions" && transcript) "caption"; generated asset "generated"; `audio_clip` item or audio media "audio"; any item/media "visual"; else "none" | `src/components/workspace/editor-workspace.tsx:6201-6216` | P04 |
| Project inspector override forces selection kind "none" | `src/components/workspace/editor-workspace.tsx:6217-6219` | P04 |
| Active inspector destination = override ?? remembered-per-selection-kind, if registered for that kind; else `defaultInspectorDestinationForSelection(kind)` | `src/components/workspace/editor-workspace.tsx:6220-6226` | P04 |
| Inspector drawer open = layout inspector visible OR transient open; source inspector view "ai-edit" when destination "ai-edit" else "details" | `src/components/workspace/editor-workspace.tsx:6227-6230` | P04 |
| Inspector owner: generated -> source inspector; visual/audio -> timeline source inspector if source clip item selected else media source inspector; caption -> selected item editor; template -> timeline source inspector for `lottie_clip` else item editor; none -> ProjectTimelineInspector; multiple -> null (summary) | `src/components/workspace/editor-workspace.tsx:6235-6253` | P04 |
| Inspector label: override -> project name; >1 items "Selected items"; else item label ?? media display name ?? project name | `src/components/workspace/editor-workspace.tsx:6260-6265` | P04 |
| Locked reason (not when override or no locked tracks): multi "Unlock {track names joined ", "} to edit {n} selected items."; single "Unlock {track} to edit {label}." | `src/components/workspace/editor-workspace.tsx:6266-6271` | P04 |
| Multiple-selection summary: "Timing" = "Shared timeline ranges"; "Effects" = "Compatible visual effects" if all selected items are visual else "No common editable effect" | `src/components/workspace/editor-workspace.tsx:6272-6307` | P04 |
| Registered destinations filter: transcript-only caption builder -> only content/style/timing; media-only visual -> basic + ai-edit; media-only audio -> basic | `src/components/workspace/editor-workspace.tsx:6308-6327` | P04 |
| Panel per destination: multiple -> summary for all; none+"activity" -> ActivityPanel; none+"render-review" -> render status + RenderReportPanel; else owner | `src/components/workspace/editor-workspace.tsx:6346-6382` | P04 |
| Selection identity: "project-override:{dest}:{projectId}" / "caption-builder:{mediaId\|none}:{transcriptId\|none}" / "timeline:{sorted ids joined ,}" / "media:{id}" / "project:{id}" | `src/components/workspace/editor-workspace.tsx:6383-6391` | P04 |
| Clip inspector (timeline clip) wires trim, lottie inputs, audio fade-out/fades/volume/denoise/retry denoise/volume keyframes, opacity & opacity keyframes, motion keyframes, generic + effect-parameter keyframe upsert/move/delete, color grade, effects, transform, crop, visual fades, speed, blend mode, split, reorder; `playheadSeconds` = timeline playhead | `src/components/workspace/editor-workspace.tsx:5722-5795` | P04 |
| Timeline-variant clip inspector = same as above plus `variant="timeline"` and `hideGeneratedDetails` | `src/components/workspace/editor-workspace.tsx:5839-5919` | P04 |
| Inspector view change remembers destination per current selection kind and opens transient inspector | `src/components/workspace/editor-workspace.tsx:5732-5738` | P04 |
| InspectorDock: selectionCount 0 under override else `max(items.length, media ? 1 : 0)`; selecting a destination while override active updates override; remember per kind; opens inspector + compact view "inspector" + focus pane | `src/components/workspace/editor-workspace.tsx:8494-8530` | P04 |
| InspectorDock toggle closes transient; toggles layout pane only if layout inspector visible; onRevealSource omitted for "none"/"multiple"; resizable only multi-pane; drawer motion class when presentation "drawer"/"single-pane" | `src/components/workspace/editor-workspace.tsx:8506-8543` | P04 |
| revealInspectorSource: timeline item -> reveal viewer source; else media -> `revealSourceMedia(media.id)` | `src/components/workspace/editor-workspace.tsx:6541-6549` | P04 |
| Effects panel target only when exactly 1 selected visual item; disabled reasons: >1 "Select one visual clip at a time to preserve clip-specific effects."; none "Select one visual clip to apply effects."; locked track "Unlock the selected track to apply effects." | `src/components/workspace/editor-workspace.tsx:6796-6813` | P05 |
| Effects preparation status: active canonical prep for current project: "pending" -> "preparing", else "failed" (with `preparationError` message); else prepared preview -> "ready"; else "idle" | `src/components/workspace/editor-workspace.tsx:6814-6824,6885-6887` | P05 |
| Catalog effect apply: requires target; descriptor must exist and have no `resourceKey`; no-op if effectType already applied; appends `{effectInstanceId: nextCatalogEffectInstanceId(id, existing), effectType: id, enabled: true, params: descriptor param defaults}` | `src/components/workspace/editor-workspace.tsx:6889-6909` | P05 |
| Inspect effect only for descriptors WITH `resourceKey`: clear override, route kind to "effects", open/focus inspector, focus input aria-label "Enable {displayName}", set pending focus effect id | `src/components/workspace/editor-workspace.tsx:6910-6937` | P05 |
| Clear effects -> `applyVisualClipEffects(itemId, [])` | `src/components/workspace/editor-workspace.tsx:6938-6940` | P05 |
| Choose visual clip guidance: "Choose a video, image, generated, or HyperFrame clip on the timeline." then compact view timeline and focus "Editor timeline" region | `src/components/workspace/editor-workspace.tsx:6941-6951,8549-8556` | P05 |
| Effects panel retry preparation increments `canonicalPreviewRetryToken` | `src/components/workspace/editor-workspace.tsx:6952-6954` | P04 |
| Automation lane property: audio_clip → "volumeDb" ("Volume"), else "opacity" ("Opacity"); overlay hidden when no points or duration <= 0; volume normalized (value + 60)/84 clamped 0..1, opacity clamped 0..1 | `src/components/workspace/timeline-editor.tsx:760-804` | P03 |
| Automation overlay rendered for audio clips, clips with visualClipOpacity, or any opacity keyframes | `src/components/workspace/timeline-editor.tsx:4824-4828` | P03 |
| Keyframe handles only when clip selected and both onChangeItemAutomationKeyframe & onDeleteItemAutomationKeyframe exist; pointer drag: atSeconds clamped [0, duration] by dx/width*duration, value clamped to automationValueBounds by -dy/height*(max-min) (height fallback 36); commit rounds atSeconds & value to 3 decimals, preserves easing, keyed by original atSeconds | `src/components/workspace/timeline-editor.tsx:814-908,4882-4903` | P03 |
| Keyframe keyboard: Delete/Backspace → onDelete(property, atSeconds); ArrowLeft/Right time step 0.1s (Shift 0.5s) clamped [0,duration]; ArrowUp/Down value step bounds.step (Shift ×10) clamped to bounds | `src/components/workspace/timeline-editor.tsx:910-938` | P03 |
| Fade handles shown when selected && onApplyItemFades && kind in audio_clip/video_clip/image_clip/lottie_clip/generated_clip; stored fadeIn/fadeOut clamped [0, duration]; fadeIn <= duration - fadeOut and vice versa; drag fade-out moves opposite to pointer; commit rounds to 3 decimals; keyboard step 0.1s (Shift 0.5s), ArrowRight grows fade-in / shrinks fade-out | `src/components/workspace/timeline-editor.tsx:946-1047,4904-4921` | P03 |
| Visual opacity overlay darkness = 1 - opacity (2 decimals); aria "Opacity for {label}, {percent}" | `src/components/workspace/timeline-editor.tsx:1175-1192` | P03 |
| Source range mismatch warning chip: "Source range duration mismatch for {label}: source span {s}, clip duration {s}" | `src/components/workspace/timeline-editor.tsx:1153-1173` | P03 |
| Video filmstrip: speed = max(0.01, speed ?? 1); source duration = sourceOut-sourceIn or duration*speed; zoomBucket = max(25, round(zoom/25)*25); cache key mediaId:in:out:speed(3dp):zoomBucket:round(trackHeight):round(clipWidth); cached frames preferred, else <video> preview url (trimmed), else 6 deterministic placeholder frames; image/video error falls back | `src/components/workspace/timeline-editor.tsx:1194-1307` | P03 |
| Filmstrip request per visible video_clip with source media: {mediaId, sourceIn (default 0), sourceOut (default in + duration*speed), speed, zoomBucket, heightBucket = round(trackHeight - 8), clipPixelWidth} | `src/components/workspace/timeline-editor.tsx:1888-1914` | P03 |
| Generated clips (generatedItemIds or isGeneratedTimelineItem) show "AI" badge; workflow status chip "Workflow {status}"; colors: completed green, failed/blocked/cancelled red, running/progress amber, else slate | `src/components/workspace/timeline-editor.tsx:706-718,4673-4677,4746-4753,4835-4855` | P05 |
| Metadata reason under clip label: for audio clips parts starting "fade in " / "fade out " stripped; clip title "{label}: {range}, {reason}" | `src/components/workspace/timeline-editor.tsx:4660-4672,4696-4698` | P03 |
| Clip test ids: `timeline-item-template-{templateId}` or `timeline-item-shader-background-{shaderBackgroundTemplateId}` | `src/components/workspace/timeline-editor.tsx:4771-4780` | P03 |
| 16 independent draft groups (trim, effects, audio fades, volume, denoise, volume kf, opacity, opacity kf, color grade, transform, crop, visual fades, speed, blend, motion kf, split), each keyed on item.id via useInspectorDraftLifecycle. Each Apply is disabled while its own group hasExternalConflict. | `src/components/workspace/source-clip-inspector.tsx:568-728` | P04 |
| A single InspectorDraftConflict banner shows if any group conflicts. Reload calls reloadCanonical only on conflicting groups. | `src/components/workspace/source-clip-inspector.tsx:730-732,1645-1652` | P04 |
| Canonical draft defaults: fadeIn/Out "0"; volumeDb ""; denoise enabled = hasEffect("audio.denoise") \|\| preparation status present; denoise strength = round(amount*100); volume start/end "0"; opacity ""; opacity start/end "1"; exposure "0"; contrast "1"; saturation "1"; centerX/Y "0.5"; width/height "1"; flips false; crops "0"; visual fades "0"; speed "1"; blend "over"; motion property "positionX" with start/end from motionKeyframeDefault | `src/components/workspace/source-clip-inspector.tsx:317-353` | P04 |
| All visual property blocks are gated by isVisualOpacityItem(item) (lib) | `src/components/workspace/source-clip-inspector.tsx:1850,1893,1981,2002,2118,2180,2209,2256` | P04 |
| Blend select (aria "Visual blend mode"), values/labels: over "Normal", darken "Darken", multiply "Multiply", colorBurn "Color Burn", lighten "Lighten", screen "Screen", colorDodge "Color Dodge", overlay "Overlay", softLight "Soft Light", hardLight "Hard Light", difference "Difference", exclusion "Exclusion", hue "Hue", saturation "Saturation", color "Color", luminosity "Luminosity", add "Add" | `src/components/workspace/source-clip-inspector.tsx:1854-1877` | P04 |
| "Apply blend" calls onApplyVisualBlendMode(item.id, mode). Only guards are conflict and callback presence. | `src/components/workspace/source-clip-inspector.tsx:956-957,1879-1890` | P04 |
| Opacity input min 0 max 1 step 0.05. Blank is valid; otherwise 0..1. "Apply opacity" sends parsedOpacity ?? 1. | `src/components/workspace/source-clip-inspector.tsx:813-814,844,1893-1923` | P04 |
| Legacy "Motion keyframes" form renders only when !onUpsertKeyframe. Property options: positionX "Position X", positionY "Position Y", scale "Scale", rotationDegrees "Rotation". Changing property resets start and end to motionKeyframeDefault(item, property). | `src/components/workspace/source-clip-inspector.tsx:1925-1951` | P04 |
| Motion kf start/end min/max/step come from motionKeyframeBounds(property) (lib). Both are required and must be within bounds. "Apply motion keyframes" sends [{atSeconds:0,value:start},{atSeconds:item.durationSeconds,value:end}]. | `src/components/workspace/source-clip-inspector.tsx:958-968,1952-1962` | P04 |
| Legacy "Opacity keyframes" (only when !onUpsertKeyframe): "Start opacity"/"End opacity" 0..1 step 0.05, default "1", both required. "Apply opacity keyframes" sends 2 keyframes at 0 and item.durationSeconds. | `src/components/workspace/source-clip-inspector.tsx:845-854,1965-1980` | P04 |
| Color grade: "Exposure" -3..3, "Contrast" 0.5..1.5, "Saturation" 0..2, all step 0.05, all required. "Apply color grade" calls onApplyColorGrade(item.id,{exposure,contrast,saturation}). | `src/components/workspace/source-clip-inspector.tsx:855-867,1981-2001` | P04 |
| Effects header "Effects" with subtitle "Executable catalog · unknown project effects are preserved". Empty catalog shows "Effect catalog unavailable." and disables Apply. | `src/components/workspace/source-clip-inspector.tsx:2009-2014,2072` | P04 |
| Effect drafts initialize from sourceEffectDrafts(item, catalog) (lib). Checkbox aria "Enable ${displayName}"; it auto-focuses when focusedEffectId === descriptor.id. | `src/components/workspace/source-clip-inspector.tsx:503-505,2039-2049` | P04 |
| Param inputs render only when the effect is enabled. Number input min=param.min, max=param.max, step 0.01; label gets " (unit)" when a unit exists. A resourceKey adds a text input labelled with the key. | `src/components/workspace/source-clip-inspector.tsx:2051-2064` | P04 |
| Extra UI params: color.curves adds curveMidpoint "Master midpoint" 0..1. color.hueCurves adds targetHue "Target hue" 0..360 "°", hueShift "Hue shift" -30..30 "°", satScale "Saturation" 0..2, lumShift "Luminance" -0.5..0.5. | `src/components/workspace/source-clip-inspector.tsx:2024-2033` | P04 |
| Effect validation: disabled drafts are skipped. Enabled drafts need every descriptor param finite within [min,max], a non-empty trimmed resource when resourceKey is set, curves midpoint 0..1, and the 4 hueCurves ranges. | `src/components/workspace/source-clip-inspector.tsx:869-889` | P04 |
| Apply effects: effects whose effectInstanceId matches any draft (enabled or not) are replaced. Effects without an instance id or not in the draft set are kept first. A disabled draft removes that effect. | `src/components/workspace/source-clip-inspector.tsx:2074-2077` | P04 |
| Enabled drafts build {effectInstanceId, effectType: descriptor.id, enabled:true, params: Number(each param), plus resourceKey: trimmed resource}. curves params are REPLACED by {masterCurve:[[0,0],[0.5,mid],[1,1]]}; hueCurves by {targets:[{targetHue,hueShift,satScale,lumShift}]}. Calls onApplyEffects(id, [...untouched, ...known]). | `src/components/workspace/source-clip-inspector.tsx:2078-2092` | P04 |
| Effect automation lanes: only for effects that are enabled, have a string effectInstanceId and a known descriptor. One lane per descriptor param; key `${effectInstanceId}${param.key}`, label `${displayName} · ${param.label}`, min/max = param, step = max(0.01,(max-min)/100), default = current numeric param else param.defaultValue. Curve extra params get no lane. | `src/components/workspace/source-clip-inspector.tsx:890-913` | P04 |
| Effect lane editor (itemLabel `${label} effects`) needs targets > 0 and all 3 effect-param callbacks; otherwise shows "Apply an enabled numeric effect to automate its parameters." | `src/components/workspace/source-clip-inspector.tsx:2093-2115` | P04 |
| Lane playhead is clip-local: clamp((playheadSeconds ?? item.startSeconds) - item.startSeconds, 0, duration) | `src/components/workspace/source-clip-inspector.tsx:2097,2566-2572` | P04 |
| "Video transform" inputs "Center X", "Center Y", "Width", "Height": min 0 max 1 step 0.01. Center must be 0..1 inclusive; width/height > 0 and <= 1; all required. | `src/components/workspace/source-clip-inspector.tsx:914-928,2127-2147` | P04 |
| Checkboxes "Flip horizontal"/"Flip vertical". "Apply transform" calls onApplyTransform(id,{centerX,centerY,width,height,flipHorizontal,flipVertical}). | `src/components/workspace/source-clip-inspector.tsx:2149-2177` | P04 |
| Speed (aria "Visual speed") min 0.1 max 8 step 0.1, required, valid 0.1..8 inclusive. Visual items only (no audio speed). "Apply speed" calls onApplyVisualSpeed(id, speed). | `src/components/workspace/source-clip-inspector.tsx:949-955,2180-2208` | P04 |
| "Visual fades": aria "Visual fade in"/"Visual fade out", min 0 step 0.1, both required and >= 0. in+out must be <= item.durationSeconds (saved duration, not the draft). "Apply visual fades" calls onApplyVisualFades(id,in,out). | `src/components/workspace/source-clip-inspector.tsx:940-948,2209-2255` | P04 |
| Crop inputs "Crop top/right/bottom/left": min 0 max 0.99 step 0.01, all required. Each 0 <= v < 1, left+right < 1, top+bottom < 1. "Apply crop" calls onApplyCrop(id,{cropTop,cropRight,cropBottom,cropLeft}). | `src/components/workspace/source-clip-inspector.tsx:929-939,2256-2299` | P04 |
| Unified KeyframeLaneEditor for visual items or audio_clip, when onUpsertKeyframe+onMoveKeyframe+onDeleteKeyframe all exist. Properties come from keyframePropertyConfigsForItem, keyframes from inspectorKeyframesByProperty (lib). It hides the legacy start/end keyframe forms. | `src/components/workspace/source-clip-inspector.tsx:2557-2586` | P04 |
| Lane editor renders nothing if there are no properties. Selected property defaults to the first and resets to the first if it disappears. Changing property clears the selection. | `src/components/workspace/keyframe-lane-editor.tsx:66,85-89,102,237-240` | P04 |
| Keyframe identity is atSeconds. If the selected time no longer exists the selection clears; otherwise time/value/easing inputs resync (easing defaults to "linear"). | `src/components/workspace/keyframe-lane-editor.tsx:91-100,120-125` | P04 |
| Easing options: linear "Linear", hold "Hold", easeIn "Ease in", easeOut "Ease out", easeInOut "Ease in/out" | `src/components/workspace/keyframe-lane-editor.tsx:35-44` | P04 |
| Times and values are rounded to 3 decimals (Number(toFixed(3))). Local playhead = rounded(clamp(playhead, 0, duration)). | `src/components/workspace/keyframe-lane-editor.tsx:46-52,105` | P04 |
| "Add at ${playhead.toFixed(2)}s" upserts {atSeconds: localPlayhead, value: rounded(clamp(suggestedValue(config,keyframes,playhead),min,max)), easing:"linear"}, then selects it | `src/components/workspace/keyframe-lane-editor.tsx:127-141,249-252` | P04 |
| Validation: time 0..duration inclusive; value min..max. Messages (role alert): "Time must be between 0 and ${duration.toFixed(2)} seconds." / "${label} must be between ${min} and ${max}." | `src/components/workspace/keyframe-lane-editor.tsx:107-118,382-386` | P04 |
| Marker drag: left button only and not disabled; uses pointer capture; lane width >= 1. Preview = rounded(clamp(from + dx/width*duration, 0, duration)). On pointerup, onMove(prop, from, preview) only if changed, then select the new time. pointercancel discards. | `src/components/workspace/keyframe-lane-editor.tsx:143-183` | P04 |
| Marker keys: ArrowLeft/Right move time by ±0.1 s (Shift ±1 s), clamped and rounded; calls onMove only if changed. ArrowUp/Down upsert value ±config.step, clamped and rounded, keeping time/easing. Delete/Backspace calls onDelete and clears selection. | `src/components/workspace/keyframe-lane-editor.tsx:185-225` | P04 |
| Marker aria "${label} keyframe at ${t.toFixed(2)} seconds"; title "Drag to change time. Arrow keys change time or value; Delete removes."; lane aria "${label} keyframe lane for ${itemLabel}" | `src/components/workspace/keyframe-lane-editor.tsx:258,274-276` | P04 |
| Selected editor: Time min 0 max duration step 0.1; Value min/max/step from config. "Move" is disabled if disabled, time invalid, or time unchanged; it calls onMove(prop, sel.at, rounded(time)). | `src/components/workspace/keyframe-lane-editor.tsx:295-352` | P04 |
| "Apply" is disabled if either validation message exists (a bad time also blocks it). It calls onUpsert({atSeconds: selected.atSeconds, value: parsedValue unrounded, easing}); Apply never moves time. Delete button aria "Delete selected keyframe" calls onDelete and clears selection. | `src/components/workspace/keyframe-lane-editor.tsx:353-380` | P04 |
| No-selection copy: "Add a point at the playhead or select a diamond to edit time, value, and easing." | `src/components/workspace/keyframe-lane-editor.tsx:389-391` | P04 |
| Effect categories = unique effect.category sorted; filter chips "All" + categories (aria-pressed) | `src/components/workspace/effect-catalog-panel.tsx:42-45,129-147` | P05 |
| Effect search: trimmed, locale-lowercased substring match over id, displayName, category, resourceKey | `src/components/workspace/effect-catalog-panel.tsx:46-53` | P05 |
| targetUnavailable = !targetLabel \|\| disabledReason present; applyUnavailable = targetUnavailable \|\| preparationStatus "preparing" \|\| "failed" | `src/components/workspace/effect-catalog-panel.tsx:54-58` | P05 |
| Header: "Target: `label`" or "No effect target selected"; status "Preparation: Idle\|Preparing\|Ready\|Failed" | `src/components/workspace/effect-catalog-panel.tsx:20-25,67-76` | P05 |
| "Choose a visual clip" button shown when no target and onChooseVisualClip provided | `src/components/workspace/effect-catalog-panel.tsx:77-87` | P05 |
| disabledReason shown as alert when status != "failed"; when "failed" alert shows disabledReason (if any) + preparationError or "Preview preparation failed. Try again." + "Retry preparation" button (if onRetryPreparation) | `src/components/workspace/effect-catalog-panel.tsx:88-110` | P05 |
| Effect with resourceKey is inspector-owned: button "Configure in inspector" (aria "Configure `name` in inspector"), disabled only if targetUnavailable, calls onInspectEffect(id) | `src/components/workspace/effect-catalog-panel.tsx:152,178-196` | P05 |
| Non-resource effect: button "Apply" / "Applied" (aria "Apply `name`" / "`name` applied", aria-pressed=applied), disabled if applyUnavailable \|\| already applied, calls onApply(id) | `src/components/workspace/effect-catalog-panel.tsx:151,178-196` | P05 |
| Effect card meta: "Color processing" when effect.colorEffect; "Resource: `resourceKey`" when present | `src/components/workspace/effect-catalog-panel.tsx:164-171` | P05 |
| Empty filter copy "No effects match this filter" | `src/components/workspace/effect-catalog-panel.tsx:201-205` | P05 |
| "Clear effects" disabled when targetUnavailable \|\| appliedEffectIds.length === 0 | `src/components/workspace/effect-catalog-panel.tsx:208-218` | P05 |

## Audio

| Rule | Source | Plan |
|---|---|---|
| Audio actions: `updateAudioFadeOut{fadeOutSeconds}`, `updateAudioFades{fadeInSeconds,fadeOutSeconds}`, `updateAudioVolume{volumeDb: number\|null}`, volume keyframes `setItemKeyframes property "volumeDb"` | `src/components/workspace/editor-workspace.tsx:3399-3426`, `:3503-3505` | P04 |
| Denoise only for `kind === "audio_clip"`; rebuilds effects list dropping existing "audio.denoise" and malformed entries (non-string effectType, non-boolean enabled, non-object params); missing instance ids → `legacyEffectInstanceId(type, occurrence)` | `src/components/workspace/editor-workspace.tsx:3428-3459` | P04 |
| Denoise enabled → append `{effectInstanceId: legacyEffectInstanceId("audio.denoise",0), effectType "audio.denoise", enabled true, params {amount}}` and set `audioDenoisePreparation {status "queued", progress 0, retryable true, algorithm "adaptive-noise-gate-v1"}`; disabled → remove `audioDenoisePreparation`; both via one `applyProjectActions` | `src/components/workspace/editor-workspace.tsx:3460-3483` | P04 |
| Retry denoise re-sets `audioDenoisePreparation` to queued/progress 0/retryable true/"adaptive-noise-gate-v1" | `src/components/workspace/editor-workspace.tsx:3485-3501` | P04 |
| Speech analysis status per clip = `speechAnalysisStatuses[itemId] ?? "idle"` | `src/components/workspace/editor-workspace.tsx:5751,5870` | P04 |
| Audio fade overlays width = fade/duration*100 (2dp); aria "Fade in for {label}, {n.toFixed(2)} seconds" / "Fade out ..." | `src/components/workspace/timeline-editor.tsx:720-758` | P03 |
| Dead-air display: speechMask entries valid only if finite startRatio < endRatio (clamped 0..1, optional speakerId, color); dead-air = gaps between sorted masks incl. leading from 0 and trailing to 1; no masks → whole clip [0,1] is dead air; aria "Dead air region {n} for {label}"; speech regions "{speakerId} speech region for {label}" or "Speech region {n} for {label}" with 2px top border in speaker color | `src/components/workspace/timeline-editor.tsx:1049-1125` | P03 |
| Audio blocks render only for item.kind === "audio_clip" | `src/components/workspace/source-clip-inspector.tsx:1742,2424,2500` | P04 |
| Audio "Fade in"/"Fade out": min 0 step 0.1. Blank counts as 0; each set value must be >= 0. (in??0)+(out??0) must be <= the DRAFT clip duration. | `src/components/workspace/source-clip-inspector.tsx:806-809,1751-1774` | P04 |
| "Apply fades" needs onApplyAudioFades or onApplyAudioFadeOut. Prefers onApplyAudioFades(id,in??0,out??0); otherwise onApplyAudioFadeOut(id,out??0). | `src/components/workspace/source-clip-inspector.tsx:826-830,1775-1796` | P04 |
| "Volume dB" min -60 max 24 step 0.5. Blank is valid and "Apply volume" then sends null (clear); otherwise -60..24. | `src/components/workspace/source-clip-inspector.tsx:810-812,831-833,1805-1832` | P04 |
| Legacy "Volume keyframes" (only when !onUpsertKeyframe): "Start dB"/"End dB" -60..24 step 0.5, default "0", both required. "Apply volume keyframes" sends [{0,start},{durationSeconds,end}]. | `src/components/workspace/source-clip-inspector.tsx:834-843,1834-1847` | P04 |
| Denoise section needs onApplyAudioDenoise. Checkbox aria "Enable audio denoise" with label "Denoise". Strength range 0..100 step 1, shown as "${n}%", disabled when denoise is off. | `src/components/workspace/source-clip-inspector.tsx:2424-2466` | P04 |
| Denoise status (only when enabled, role status): failed -> "Failed"; completed\|ready -> "Ready"; preparing\|progress -> "Preparing"; else "Queued" | `src/components/workspace/source-clip-inspector.tsx:2443-2453` | P04 |
| "Apply denoise" is disabled only on draft conflict. Calls onApplyAudioDenoise(id, enabled, Number(strength)/100). | `src/components/workspace/source-clip-inspector.tsx:2467-2483` | P04 |
| "Retry denoise" shows when status === "failed" and onRetryAudioDenoise exists; calls onRetryAudioDenoise(id) | `src/components/workspace/source-clip-inspector.tsx:2484-2494` | P04 |
| Copy: "Preview and export share one cached prepared-audio artifact." | `src/components/workspace/source-clip-inspector.tsx:2495-2497` | P04 |
| Timeline audio layers render a hidden `<audio preload=metadata>` per `frame.audioLayers` entry that has a preview URL and has not failed. Volume comes from `layer.gain`, applied as `el.volume = clamp(gain, 0, 1)` on each clock sync. No fade logic is inline; fades must already be baked into `gain` by lib. | `src/components/workspace/timeline-preview-compositor.tsx:114-145,176-178,337-343` | P04 |
| Clock sync (video and audio): set `currentTime = sourceTimeSeconds` when not playing, when currentTime is not finite, or when drift > 0.1 s. While playing with drift <= 0.1 s, no seek. Runs on isPlaying, sourceTimeSeconds or volume change, and on loadedmetadata. | `src/components/workspace/timeline-preview-compositor.tsx:169-179,206-211` | P04 |
| Transport sync: if not desired-playing, pause if needed. If already playing, or a `play()` for the current generation is pending, skip. Otherwise call `play()`; after it settles (errors are swallowed), pause if desired-playing became false; clear the pending entry on finally. | `src/components/workspace/timeline-preview-compositor.tsx:181-204` | P04 |
| Playback identity is `itemId + " " + sourceUrl`. When it changes, the generation increments and the pending play is dropped. The transport effect reruns on isPlaying, itemId or sourceUrl. Unmount sets desired to false and pauses. | `src/components/workspace/timeline-preview-compositor.tsx:157-167,212-225` | P04 |
| Timeline video layers are always `muted`; audio comes only from audio layers. The source-viewer video is also `muted`; source audio uses `<audio>`. | `src/components/workspace/timeline-preview-compositor.tsx:101`, `src/components/workspace/preview-panel.tsx:287` | P04 |

## Captions / transcript

| Rule | Source | Plan |
|---|---|---|
| Caption generation defaults: mode "local", language "auto", maxWords null, censorProfanity false; speaker marking true; silence marking true | `src/components/workspace/editor-workspace.tsx:944-949` | P05 |
| Caption group = all caption items with same `captionGroupId`; no group id → just the selected caption | `src/components/workspace/editor-workspace.tsx:1659-1672` | P04 |
| Selected transcript: by caption's `transcriptId` if set, else transcript whose mediaId = selected timeline item media ?? selectedMediaId | `src/components/workspace/editor-workspace.tsx:1676-1682` | P05 |
| Caption source options = media of kind video or audio (label `mediaDisplayName`); source id = transcript media ?? selectedMediaId ?? first option | `src/components/workspace/editor-workspace.tsx:1683-1687` | P05 |
| Speech target item = selected source clip ?? first timeline item whose source media = caption source | `src/components/workspace/editor-workspace.tsx:1688-1693` | P05 |
| Caption build range (requires transcript): selected source clip → `captionBuildRangeForSourceItem`; else selected timeline range; else first word start → last word end; no words → null | `src/components/workspace/editor-workspace.tsx:1694-1708` | P05 |
| Edit caption text: use `captionRepairActionForItem` when available, else timeline patch `editCaptionText{itemId,text}` | `src/components/workspace/editor-workspace.tsx:2558-2574` | P04 |
| Caption group placement/style/motion → `updateItemProperties` per item: `captionPlacement`, `captionStyleProperties(preset)`, `motionPresetId` | `src/components/workspace/editor-workspace.tsx:2576-2603` | P04 |
| Word emphasis: non-empty sets `emphasizedWordIndices`; empty removes it. Word timings → `captionWordTimings`; animations → `captionWordAnimations` | `src/components/workspace/editor-workspace.tsx:2605-2632` | P04 |
| Word animation preset: stagger clamped [0, 0.5] (non-finite → 0); words = text split on `/\S+/` (text source or label); only emphasized indices that exist | `src/components/workspace/editor-workspace.tsx:2634-2650` | P04 |
| Per word: start/end from timing with same wordIndex (default 0 / item duration); offset = 0 for "groupPulse" else min(order*stagger, (end-start)*0.25); enterEnd = enterStart + 20% span, holdEnd = +80%, exitEnd = end | `src/components/workspace/editor-workspace.tsx:2651-2663` | P04 |
| Preset styling: scale "karaokeFade" 1 / "groupPulse" 1.08 / other 1.14; color karaokeFade "#ffffff" else "#ffcf5a"; opacity karaokeFade 0.82 else 1; easing "sequentialPop" → "outBack" else "outQuad"; stores `captionWordAnimationPreset` and `captionWordStaggerSeconds` | `src/components/workspace/editor-workspace.tsx:2664-2673` | P04 |
| Add built captions: no-op if empty or no unlocked caption track; `addItems` on first unlocked caption track; selects all built items (first primary), clears range | `src/components/workspace/editor-workspace.tsx:2676-2694` | P05 |
| Transcript word repairs: no-op if empty; `editTranscriptWords` with shared `createdAt` and `repairId` "word-repair-`<transcriptId>`-`<wordIndex>`-`<Date.now base36>`" | `src/components/workspace/editor-workspace.tsx:2696-2712` | P05 |
| Selected item editor returns null unless template/caption/text overlay item or transcript; region label priority "Template" > "Caption" > "Text overlay" > "Caption builder" | `src/components/workspace/editor-workspace.tsx:5935-5954` | P04 |
| Caption item editor wires text, build (transcript + range), group placement/style/motion, word emphasis/timings/animations/animation preset; transcript-only builder has `item={null}` and only build | `src/components/workspace/editor-workspace.tsx:5957-5986` | P04 |
| Build captions: if no transcript or no build range -> queue transcription instead; else `buildCaptionItems({transcript, range, wordsPerCue: captionMaxWords ?? 4, stylePreset: "boldReadableLower", groupId: "caption-group-" + Date.now().toString(36)})` -> `addBuiltCaptionItems` | `src/components/workspace/editor-workspace.tsx:6595-6609` | P05 |
| Captions workbench copy: "Captions will look like this"; "Use the advanced editor to apply caption group styles."; "Word timing and motion remain editable after captions are built."; "Captions use the timeline safe zone and selected source range."; advanced editor = selected item editor; source change -> `selectMediaSource` | `src/components/workspace/editor-workspace.tsx:6561-6594` | P05 |
| Transcript panel seek clamps to `[0, timeline.durationSeconds]`; "request transcription" only offered when media selected and no transcript; repairs via `applyTranscriptWordRepairs` | `src/components/workspace/editor-workspace.tsx:6863-6878` | P05 |
| Caption text draft is keyed on item.id with a conflict banner. "Apply text correction" is disabled when text.trim() is empty or on conflict. Sends onApply(item.id, text) untrimmed. | `src/components/workspace/caption-inspector.tsx:60-71,135,172-180` | P04 |
| Status label: "User edited" if properties.textEdited === true, else "Generated text". Reading warning from getCaptionReadingWarning (lib), shown in amber. | `src/components/workspace/caption-inspector.tsx:73-77,170,182` | P04 |
| Timing readout "Start ${t}" / "End ${start+duration}"; empty-state copy "Select a caption cue to edit its text." | `src/components/workspace/caption-inspector.tsx:146-151,400` | P04 |
| Group scope = captionGroupItems if non-empty, else [item]. Placement, style and motion selects apply IMMEDIATELY on change to all group ids. Hint: "${n} cue${n===1?"":"s"} receive this placement./style./motion." | `src/components/workspace/caption-inspector.tsx:83,196-207,223-234,250-259` | P04 |
| Placement: "lower" unless the property is "center"/"upper". Options "Lower"/"Center"/"Upper". | `src/components/workspace/caption-inspector.tsx:84-86,203-205` | P04 |
| Style: "boldReadableLower" unless "kineticFocus"/"centeredMinimal". Options "Bold lower"/"Kinetic focus"/"Centered minimal". | `src/components/workspace/caption-inspector.tsx:87-90,230-232` | P04 |
| Motion: "snap-pop-v1" unless "pulse-emphasis-v2"/"soft-depth-card-v2". Options "Snap pop"/"Pulse emphasis"/"Soft depth". | `src/components/workspace/caption-inspector.tsx:91-94,255-257` | P04 |
| Word tokens come from the DRAFT text (captionWordTokens). Stored emphasizedWordIndices are filtered to integers 0..words.length-1. | `src/components/workspace/caption-inspector.tsx:95,120-130` | P04 |
| Word emphasis toggle buttons (aria-pressed) add the index (sorted ascending) or remove it, then apply immediately via onApplyWordEmphasis(id,next). Copy "Choose the words that receive the caption accent in preview and export." | `src/components/workspace/caption-inspector.tsx:262-290` | P04 |
| Emphasis timing (needs onApplyWordTimings and emphasized > 0): "In" min 0, max max(0,end-0.01), step 0.01; "Out" min min(duration,start+0.01), max duration, step 0.01. Non-finite input is ignored; no clamp. Each change sends the full timings array with the value rounded toFixed(3). | `src/components/workspace/caption-inspector.tsx:291-354` | P04 |
| Word motion preset: "Sequential pop" (sequentialPop, default)/"Group pulse"/"Karaoke fade". Stagger default "0.06", min 0 max 0.5 step 0.01. "Apply" calls onApplyWordAnimationPreset(id, preset, Number(stagger)) with no validation. | `src/components/workspace/caption-inspector.tsx:63-64,355-368` | P04 |
| Per-word animation defaults when not stored: enterStart = timing.start; enterEnd = start + span*0.2; holdEnd = start + span*0.8; exitEnd = timing.end; emphasisScale 1.12; emphasisColor "#ffcf5a"; emphasisOpacity 1; easing "outQuad" unless "linear"/"outBack" | `src/components/workspace/caption-inspector.tsx:100-119` | P04 |
| Per-word editor: "Enter"/"Hold"/"Exit"/"End" edit enterStart/enterEnd/holdEnd/exitEnd, min 0 max duration step 0.01; "Scale" 0.5..2 step 0.01; "Color"; "Opacity" 0..1 step 0.05; "Easing" Linear/Out quad/Out back. Every change immediately sends the full array via onApplyWordAnimations, with Number() unvalidated. | `src/components/workspace/caption-inspector.tsx:369-397` | P04 |
| "Build captions" block shows when buildTranscript && buildRange. Copy "${n} timed transcript words. Existing caption cues are unchanged." Range shown as "start–end". | `src/components/workspace/caption-inspector.tsx:402-418` | P05 |
| Builder: "Words per cue" default 4, min 1 max 12 (Number(), no clamp); Style default "boldReadableLower" | `src/components/workspace/caption-inspector.tsx:61-62,428-458` | P05 |
| "Build timed captions" enabled if onBuild, transcript and range exist, words.length > 0 and range.end > range.start. Calls onBuild(buildCaptionItems({transcript, range, wordsPerCue, stylePreset, groupId:`caption-group-${Date.now().toString(36)}`})). | `src/components/workspace/caption-inspector.tsx:79-82,460-480` | P05 |
| Overlays with empty `text` are not rendered. Caption placement: "upper" = top 12%, "center" = top 42%, default "lower" = bottom 12%, with a 16% horizontal inset. | `src/components/workspace/timeline-preview-compositor.tsx:869-879` | P04 |
| Caption style presets: "kineticFocus" (amber, text-2xl black weight), "centeredMinimal" (slate, rounded-xl, text-lg), default (black/55, text-xl bold). The wrapper gets motion style with opacity forced to 1, and layer opacity is applied to the inner span. | `src/components/workspace/timeline-preview-compositor.tsx:880-899` | P04 |
| Word emphasis (local helper `renderCaptionTextWithEmphasis`): split on `/(\s+)/`; word index counts non-whitespace tokens only. Emphasized indices are `activeEmphasizedWordIndices ?? emphasizedWordIndices`. Emphasized or styled words get an amber glow; a per-word style applies color, opacity and `scale(scale)`. | `src/components/workspace/timeline-preview-compositor.tsx:893-897,1060-1086` | P04 |
| Caption source select disabled when no sources; placeholder option "No speech source" | `src/components/workspace/captions-workbench.tsx:141-154` | P05 |
| Transcription mode radio: "local" ("Local") \| "cloud" ("Cloud") | `src/components/workspace/captions-workbench.tsx:15,156-173` | P05 |
| Caption language options: "auto" Auto, "en" English, "de" German, "fr" French, "es" Spanish | `src/components/workspace/captions-workbench.tsx:185-196` | P05 |
| Max words options: "None" (null) or 1..6; value parsed via Number | `src/components/workspace/captions-workbench.tsx:198-215` | P05 |
| "Censor profanity" boolean checkbox | `src/components/workspace/captions-workbench.tsx:216-225` | P05 |
| Disclosures: "Style" (fallback "Use the selected caption style."), "Animation" (fallback "Use clip animation settings."), "Placement" open by default containing placement panel + 16:9 preview frame, "Advanced" only when advancedEditor provided | `src/components/workspace/captions-workbench.tsx:229-246` | P05 |
| "Build captions" disabled when !sourceId | `src/components/workspace/captions-workbench.tsx:253-255` | P05 |
| "Agent Mode" menu drafts agent prompt (does not send) and closes: "Remove filler words" -> "Remove filler words from the selected caption source. Preserve meaning. Do not change caption timing without a reviewed project action."; "Fix names & jargon" -> "Correct names and domain jargon in the selected caption source. Preserve word timing unless a reviewed project action explicitly changes it."; "Add emoji" -> "Suggest restrained emoji additions for the selected captions. Return a reviewed proposal and preserve timing."; "Translate" -> "Translate the selected captions to the chosen language. Preserve source timing and return reviewed project actions." | `src/components/workspace/captions-workbench.tsx:43-68,275-292` | P05 |
| Transcript drafts reset to canonicalDrafts(transcript) whenever transcript changes | `src/components/workspace/transcript-panel.tsx:25-31` | P05 |
| Word change detected when trimmed text != word.text or Number(start) != startSeconds or Number(end) != endSeconds; repair input {transcriptId, wordIndex, text: trimmed, startSeconds, endSeconds} | `src/components/workspace/transcript-panel.tsx:33-56` | P05 |
| Changed word invalid if text empty, start/end non-finite, start < 0, or end <= start | `src/components/workspace/transcript-panel.tsx:58-73` | P05 |
| Invalid copy: "Word `N`: End must be later than start and text cannot be empty." (N = index+1; end input aria-invalid) | `src/components/workspace/transcript-panel.tsx:161,172-176` | P05 |
| Header repair count "`n` repair\|repairs"; meta "`engine.trim() \|\| "Transcript"` · `n` words" | `src/components/workspace/transcript-panel.tsx:94-104` | P05 |
| Word row: seek button shows formatSecondsInput(start), aria "Seek to word N at X seconds", seeks to word.startSeconds (original, not draft) | `src/components/workspace/transcript-panel.tsx:120-129` | P05 |
| Row badges "Modified" (changed draft) and "Repaired" (any transcript.repairs matches via repairMatchesWord) | `src/components/workspace/transcript-panel.tsx:109-135` | P05 |
| Start/End inputs type number, min 0, step 0.01 | `src/components/workspace/transcript-panel.tsx:145-170` | P05 |
| "Apply `n` change\|changes" disabled when no changes or any invalid; "Discard changes" disabled when no changes and resets drafts | `src/components/workspace/transcript-panel.tsx:182-199` | P05 |
| Live region "`n` transcript changes pending" | `src/components/workspace/transcript-panel.tsx:201-203` | P05 |
| No transcript: "No transcript for selected media." + "Transcribe selected media" disabled when no onRequestTranscription | `src/components/workspace/transcript-panel.tsx:205-217` | P05 |

## Templates / text

| Rule | Source | Plan |
|---|---|---|
| Text overlay inspector update → `updateTextOverlayItems` with start, duration, text, visualTreatment, motion, safeZone, avoid | `src/components/workspace/editor-workspace.tsx:2714-2730` | P04 |
| Inline text edit: only non-template text overlays; trimmed text must be non-empty; missing props default to "clean editable text overlay with high-contrast type and transparent backing", "quick fade in, hold, and soft fade out", "keep text inside 10% title-safe margins", "opaque slabs, default-font template look, and covering faces or key action" | `src/components/workspace/editor-workspace.tsx:2732-2771` | P04 |
| Insert motion template: id "template-`<templateId>`-`<Date.now base36>`"; start = given ?? template `placement.defaultStartSeconds` ?? 0; requires first `overlay` track else error "Add an Overlay track before inserting this template."; selects item on success | `src/components/workspace/editor-workspace.tsx:2773-2797` | P05 |
| Insert shader background: id "shader-background-`<templateId>`-`<base36>`"; requires `hyperframe_scene` track else "Add a HyperFrames track before inserting this background."; selects on success | `src/components/workspace/editor-workspace.tsx:2799-2826` | P05 |
| Add text overlay: silently no-op without overlay track; id "text-overlay-`<base36>`", duration 3s, text/label "New text overlay", default treatment/motion/safeZone/avoid strings (same as inline defaults) | `src/components/workspace/editor-workspace.tsx:2828-2859` | P05 |
| Template inspector update: `updateTemplateItems` (start, duration, fields) then, only if it succeeded, `updateTemplateOverride` applied on the returned project (two history entries) | `src/components/workspace/editor-workspace.tsx:3222-3245` | P04 |
| Template field edit: value trimmed, merged into `templateFieldsForItem`; timing edit keeps unspecified start/duration; both `updateTemplateItems` | `src/components/workspace/editor-workspace.tsx:3247-3304` | P04 |
| Template style keys accentColor/backgroundColor/textColor (trimmed) and metadata keys visualTreatment/motion/safeZone/avoid (trimmed) → `updateTemplateOverride{templateId, name, fields, style, ...metadata}`; no-op if template id not in catalog | `src/components/workspace/editor-workspace.tsx:3306-3375` | P04 |
| Codex suggestion template name constant "Kinetic Lower Third" | `src/components/workspace/editor-workspace.tsx:603` | P06 |
| Text treatments = motion template catalog excluding `category === "captions"` | `src/components/workspace/editor-workspace.tsx:6825-6827` | P05 |
| Templates panel inserts template / shader background at chosen start (library given playhead) | `src/components/workspace/editor-workspace.tsx:6832-6850` | P05 |
| Text panel: add text at playhead; `selectedTextLabel` only when text overlay selected; inspect -> focus inspector pane; insert treatment at playhead | `src/components/workspace/editor-workspace.tsx:6851-6861` | P05 |
| Template insert error alert shown only when source destination is "templates" or "text" | `src/components/workspace/editor-workspace.tsx:8066-8074` | P05 |
| Template suggestion accept inserts "kinetic-lower-third-v1" at playhead; suggestion name only shown when `latestEditRequest` exists | `src/components/workspace/editor-workspace.tsx:7958-7960,8007-8009` | P06 |
| Template inspector: templateId only when isTemplateTimelineItem(item); template = getMotionTemplate(id). Without a template shows "Select a motion template to edit its fields." | `src/components/workspace/template-inspector.tsx:33-35,308-310` | P04 |
| Field drafts = template.defaultTextFields overlaid by getTemplateFields(item) (item wins) | `src/components/workspace/template-inspector.tsx:36-43` | P04 |
| No conflict lifecycle: all drafts reset whenever item, mergedDefaults or template change, so unsaved edits are lost on any item update | `src/components/workspace/template-inspector.tsx:60-71` | P04 |
| "Start" min 0 step 0.1; "Duration" min 0.1 step 0.1. Multiline field definitions use a textarea (rows 3), others a text input. | `src/components/workspace/template-inspector.tsx:113-172` | P04 |
| "Apply template" enabled if: start finite >= 0; duration finite > 0; "Accent color"/"Background color"/"Text color" and "Visual treatment"/"Motion"/"Safe zone"/"Avoid" all non-empty after trim (colors are free text, not validated); every required field non-empty after trim | `src/components/workspace/template-inspector.tsx:73-89` | P04 |
| "Reset guidance" restores template.visualTreatment/motion/safeZone/avoid (fields and colors untouched) | `src/components/workspace/template-inspector.tsx:91-100,215-226` | P04 |
| Apply: fields are limited to template.fieldDefinitions names, trimmed (extras dropped). Calls onApply(id,{fields, startSeconds, durationSeconds, override:{templateId, name, fields, style:{accentColor,backgroundColor,textColor}, visualTreatment, motion, safeZone, avoid}}), all strings trimmed. | `src/components/workspace/template-inspector.tsx:269-305` | P04 |
| Text overlay: one draft lifecycle over start/duration/text/visualTreatment/motion/safeZone/avoid (defaults via stringPropertyOrFallback, lib) with a conflict banner | `src/components/workspace/text-overlay-inspector.tsx:32-67,90` | P04 |
| Overlay "Clip start"/"Duration" are text inputs with inputMode decimal (no min/step) | `src/components/workspace/text-overlay-inspector.tsx:109-128` | P04 |
| "Apply overlay details" enabled if item exists, start finite >= 0, duration finite > 0, text/visualTreatment/motion/safeZone/avoid all non-empty after trim, and no conflict. Sends values UNTRIMMED. | `src/components/workspace/text-overlay-inspector.tsx:69-81,219-236` | P04 |
| Overlay status "User edited" (textEdited === true) or "Manual overlay". Empty copy "Select a text overlay to edit its copy." Readout "Start"/"End". | `src/components/workspace/text-overlay-inspector.tsx:82-83,101-106,217,240` | P04 |
| Lottie inputs section for kind "lottie_clip" with onApplyLottieInputs. Copy "Lottie inputs" / "Typed theme, slot, segment, and state controls". Drafts are NOT loaded from the item (empty; slot color "#ffffff"; input type "boolean"; value "true"). | `src/components/workspace/source-clip-inspector.tsx:482-492,2372-2387` | P04 |
| Lottie segment start/end frame inputs min 0 step 0.001; input types "Boolean"/"Numeric"/"String"; "Events (comma separated)" | `src/components/workspace/source-clip-inspector.tsx:2391-2400` | P04 |
| Lottie build: slots [] always. themeId if trimmed. Marker (trimmed) takes precedence; otherwise segment only if end > start, as {startFrameMicros, endFrameMicros} = round(frame*1_000_000). | `src/components/workspace/source-clip-inspector.tsx:2402-2405` | P04 |
| Lottie color slot (when slot id is trimmed non-empty): hex -> rgb micros round(parseInt(hex,16)*1_000_000/255), alpha 1_000_000, type "color" | `src/components/workspace/source-clip-inspector.tsx:2406-2409` | P04 |
| Lottie state machine (when id is trimmed): one input if name is trimmed. numeric -> valueMicros round(v*1e6); boolean -> value === "true"; string -> raw value. Events = split(",") trimmed, empties removed. "Apply Lottie inputs" is never disabled. | `src/components/workspace/source-clip-inspector.tsx:2410-2420` | P04 |
| Template overlays render `MotionTemplatePreview` only when `findTimelineItem(timeline, itemId)` finds the item. Other overlays render `TimelinePreviewOverlayLayer`. | `src/components/workspace/timeline-preview-compositor.tsx:727-735` | P04 |
| Text overlays are fixed at left 8%, top 12%, max-width 46%. Motion style uses opacity 1 and layer opacity goes on the inner span. There is no inline text editing in these files. | `src/components/workspace/timeline-preview-compositor.tsx:903-915` | P04 |

## Preview / canvas

| Rule | Source | Plan |
|---|---|---|
| Initial viewer media = initial item's source media ?? first media; initial playhead = initial item midpoint clamped, else 0; viewer mode default "timeline" | `src/components/workspace/editor-workspace.tsx:912-914`, `:983-996` | P04 |
| Playhead re-clamped whenever timeline duration changes | `src/components/workspace/editor-workspace.tsx:1360-1364` | P04 |
| Canonical preview: if `!projectNeedsCanonicalPreview` clear prepared+status; if no projectDir → failed "Canonical preview preparation requires a saved project folder."; else pending then `prepareProjectPreview`; failure message = error message; rerun on project/projectDir/retry token | `src/components/workspace/editor-workspace.tsx:1366-1404` | P04 |
| Leaving "timeline" viewer mode stops playback | `src/components/workspace/editor-workspace.tsx:1406-1410` | P04 |
| Playback clock: rAF loop adds elapsed ms/1000 (min 0) to playhead, clamped; stops when reaching duration; `visibilitychange` resets timestamp (no jump after tab hidden) | `src/components/workspace/editor-workspace.tsx:1412-1451` | P04 |
| Source viewer: active source in "source" mode uses its range context; otherwise falls back to activeViewerSourceId ?? selectedMediaId | `src/components/workspace/editor-workspace.tsx:1828-1846` | P04 |
| Activate source: clears inspector override, appends tab if not open, stores range label context, sets active + selected media, mode "source" | `src/components/workspace/editor-workspace.tsx:4031-4043` | P04 |
| Select media source clears timeline item selection and range first; select tab no-op if media not in project | `src/components/workspace/editor-workspace.tsx:4117-4132` | P04 |
| Close source tab: removes context; if active, fallback = tab before closed index ?? first ?? null; mode "source" if fallback else "timeline" | `src/components/workspace/editor-workspace.tsx:4134-4154` | P04 |
| Reveal source media = select media source + open "media" source destination | `src/components/workspace/editor-workspace.tsx:4156-4159` | P05 |
| Viewer context disabled reason: locked track -> "Unlock {track} to edit {label ?? "this selection"}."; else if item needs canonical preparation and not prepared: failed -> message ?? "Canonical preview preparation failed.", else "Canonical preview is still preparing." | `src/components/workspace/editor-workspace.tsx:6480-6494` | P04 |
| Prepared/preparation only count when `sourceProject === project` (identity match) | `src/components/workspace/editor-workspace.tsx:6483-6487` | P04 |
| canReplaceFromViewer requires selected source clip and context kind in ["visual","lottie","generated"] | `src/components/workspace/editor-workspace.tsx:6495-6499` | P04 |
| Canvas control focus finds button aria-label "Crop {label} from top" / "Move {label} in preview canvas"; if absent opens inspector | `src/components/workspace/editor-workspace.tsx:6508-6521` | P04 |
| Reveal viewer source: source media exists -> `revealSourceMedia`; else open source destination "templates" for template kind, else "media" | `src/components/workspace/editor-workspace.tsx:6531-6539` | P04 |
| Viewer replace: requires canReplace; sets replacement target item id; opens "generate" destination (composer open) | `src/components/workspace/editor-workspace.tsx:6551-6555,6523-6529` | P04 |
| Context toolbar only when context kind and item; safe zone visible for "caption"/"text"; Fit/Fill = transform `{centerX:0.5, centerY:0.5, width:1, height:1}`; onReplace only if canReplace; edit caption text for caption, edit text for text; "more" opens inspector | `src/components/workspace/editor-workspace.tsx:8162-8197` | P04 |
| Preview preparation prop: visual-QA failure -> `{status:"failed", message:"The deterministic visual-QA preview decoder failed."}` (scenario "modern-editor-preview-failed"); matching preparation; matching prepared -> `{status:"ready"}`; `!projectNeedsCanonicalPreview` -> null; else `{status:"pending"}` | `src/components/workspace/editor-workspace.tsx:8092-8119` | P04 |
| Canonical frame sequences only from matching prepared preview (itemId, preparedMediaId, start, duration, fps, frame URLs with null URLs dropped); prepared timeline/timelines/media/generatedAssets passed only when matching | `src/components/workspace/editor-workspace.tsx:8120-8149` | P04 |
| Retry canonical preview increments retry token | `src/components/workspace/editor-workspace.tsx:8150-8152` | P04 |
| Switching viewer mode away from "timeline" stops timeline playback | `src/components/workspace/editor-workspace.tsx:8198-8203` | P04 |
| Toggle playback: if playing -> pause; else if playhead >= timeline duration reset to 0, then play | `src/components/workspace/editor-workspace.tsx:8225-8237` | P04 |
| Preview seek clamps with `clampTimelinePlayhead`; preview item click -> `selectTimelineItems({itemId, additive:false}, true)`; interaction enabled only when `isActive` | `src/components/workspace/editor-workspace.tsx:8091,8206-8208,8220-8224` | P04 |
| Open source from preview/timeline: select item, `activateViewerSource(mediaId, sourceRangeLabelForItem(item) or null)` | `src/components/workspace/editor-workspace.tsx:8212-8219,8391-8398` | P04 |
| Preview canvas transform/rotate/crop -> `applyVisualClipTransform` / `applyVisualClipRotation` / `applyVisualClipCrop` | `src/components/workspace/editor-workspace.tsx:8209-8211` | P04 |
| Viewer mode: `activeViewerMode` is the `viewerMode` prop only when there is at least 1 source tab AND a `selectedSource` exists. Otherwise it is forced to "timeline". Default `viewerMode` is "timeline". | `src/components/workspace/preview-panel.tsx:450-451,413` | P04 |
| Source tabs are the `sourceTabs` prop, or `[selectedSource]` if that prop is missing, or `[]`. The active source id is `activeSourceId ?? selectedSource.id ?? null`. | `src/components/workspace/preview-panel.tsx:448-449` | P04 |
| Viewer tab index: Timeline tab = 0, source tabs = 1..N. Tab count = 1 + number of source tabs. In source mode the index is `max(0, findIndex+1)`. The "Previous viewer tab" button is disabled when index is 0. "Next viewer tab" is disabled when index is count-1. | `src/components/workspace/preview-panel.tsx:471-479,857-880` | P04 |
| `selectViewerTabAt(index)`: if index <= 0, call `onSelectViewerMode("timeline")`. Otherwise look up source `tabs[index-1]`; if it is missing, do nothing; else call `onSelectSourceTab(id)` then `onSelectViewerMode("source")`. | `src/components/workspace/preview-panel.tsx:489-502` | P04 |
| Viewer tab keys: ArrowLeft = previous (wraps around), ArrowRight = next (wraps), Home = 0, End = last. The handler calls preventDefault and stopPropagation, selects the tab, then focuses the tab with role="tab" at the new index. | `src/components/workspace/preview-panel.tsx:504-517` | P04 |
| Clicking a source tab calls `onSelectSourceTab(id)` then `onSelectViewerMode("source")`. Clicking the Timeline tab calls `onSelectViewerMode("timeline")`. Roving tabIndex: only the active tab has tabIndex 0. | `src/components/workspace/preview-panel.tsx:882-930` | P04 |
| The close button (`"Close ${label} viewer tab"`) calls stopPropagation. If the tab being closed is active, it sets a pending-focus flag. It then calls `onCloseSourceTab(id)`. After the active tab id or the tab list changes, the flagged close moves focus to the new active tab element. | `src/components/workspace/preview-panel.tsx:931-946,519-523` | P04 |
| Tab id for a missing source id is `sourceViewerTabId("unknown")`. The Timeline tab id is "preview-viewer-tab-timeline" and the panel id is "preview-viewer-panel". | `src/components/workspace/preview-panel.tsx:91-92,484-486` | P04 |
| When the selected source's id, previewUrl or durationLabel changes: clear the failed-source flag, reset position to 0, set duration from `parsePreviewDurationLabel(durationLabel)`, and set playing to false. | `src/components/workspace/preview-panel.tsx:525-532` | P04 |
| Leaving source mode pauses the source media element and sets `previewPlaying` to false. | `src/components/workspace/preview-panel.tsx:534-539` | P04 |
| Source playback can toggle only when: mode is "source", the source has a previewUrl, the preview has not failed, and kind is video, generated or audio. Stepping uses the same condition. | `src/components/workspace/preview-panel.tsx:454-461` | P04 |
| Timeline transport is usable only when: mode is "timeline", a timeline exists, and `durationSeconds` is finite and > 0. Seek additionally requires `onSeekTimelinePlayback`; toggle requires `onToggleTimelinePlayback`. | `src/components/workspace/preview-panel.tsx:462-470` | P04 |
| Timeline step size is 0.25 s per step. Source step size is `sourcePreviewStepSeconds(source)` from lib. | `src/components/workspace/preview-panel.tsx:684-685` | P04 |
| Timeline step calls `onSeekTimelinePlayback(current + 0.25*dir)` without clamping. The timeline step button is enabled by `canSeekTimelinePlayback`. | `src/components/workspace/preview-panel.tsx:826-832,672-675` | P04 |
| Timeline clock: duration = `max(0, timeline.durationSeconds ?? 0)`; current = `clamp(playheadSeconds ?? 0, 0, duration)`. Mode "timeline" uses `timelinePlaying`; mode "source" uses local `previewPlaying`. | `src/components/workspace/preview-panel.tsx:657-671` | P04 |
| Source seek is allowed only in toggle-able source mode with a duration > 0. | `src/components/workspace/preview-panel.tsx:676-679` | P04 |
| `stepSourcePreview`: guard on can-step, source and element present. If step <= 0, do nothing. Duration is `mediaElementDurationSeconds`, or Infinity when <= 0. New time = `clamp(currentTime + step*dir, 0, duration)`, then sync position state. | `src/components/workspace/preview-panel.tsx:605-623` | P04 |
| `seekSourcePreview`: guard on can-toggle, source and element present. Do nothing if duration <= 0 or the target is not finite. Otherwise set `currentTime = clamp(target, 0, duration)` and sync. | `src/components/workspace/preview-panel.tsx:625-638` | P04 |
| `syncSourcePreviewPosition`: current = `max(0, currentTime)` if finite, else 0. Duration comes from `mediaElementDurationSeconds(el, source)`. Runs on durationchange, loadedmetadata and timeupdate. | `src/components/workspace/preview-panel.tsx:595-603,254-260,292-298` | P04 |
| `toggleSourcePlayback`: if playing, call `pause()` and set false. Otherwise call `play()`; on resolve set true, on reject set false. Media events play, pause and ended also update the playing flag. | `src/components/workspace/preview-panel.tsx:640-655,975-977` | P04 |
| Scrubber: range with min 0, max = duration (0 when there is no live position), step 0.001. Value is clamped to [0, max]. Disabled when seek is not allowed or max <= 0. onChange seeks only if the value is finite. aria-valuetext is `formatPreviewCurrentTime`. | `src/components/workspace/preview-panel.tsx:151-183` | P04 |
| Live position exists only for (source mode with a selected source) or (timeline mode with a timeline). Otherwise the label is "00:00:00" and progress is 0. Progress = `clamp(cur/dur*100, 0, 100)`, rounded to 2 decimals. | `src/components/workspace/preview-panel.tsx:140-150` | P04 |
| Time label reads `"{current} / {transport.durationLabel}"`. Secondary label and badges come from `previewTransportData`. | `src/components/workspace/preview-panel.tsx:187,207-208` | P04 |
| Transport buttons: "Jump to start" seeks to 0 and "Jump to end" seeks to the scrubber max; both are disabled when seek is not allowed. Step and play are disabled when not allowed, and their onClick is left undefined in that case. | `src/components/workspace/preview-panel.tsx:190-204` | P04 |
| Transport titles: timeline mode shows "Step timeline backward"/"Step timeline forward"; source mode shows "Step source backward (Left Arrow)"/"Step source forward (Right Arrow)". Play shows "Pause preview (Space)"/"Play preview (Space)". aria-labels: "Preview transport pause"/"Preview transport play", "Preview jump to start", "Preview step backward", "Preview step forward", "Preview jump to end". | `src/components/workspace/preview-panel.tsx:190-203` | P04 |
| Source viewer: if the preview failed, show an alert "Preview failed" with a "Retry preview" button. Retry clears the failed id and sets playing to false. A media or img error sets the failed id to the source id and playing to false. | `src/components/workspace/preview-panel.tsx:230-242,971-981` | P04 |
| Source kinds: audio shows an `<audio preload=metadata>` (only when previewUrl exists and has not failed) plus a fake waveform of 28 bars with height `18+((i*13)%42)` px. Video/generated shows a muted, playsInline `<video preload=metadata>`. Image shows an `<img>`, or a placeholder "Image source frame" without a URL. Lottie shows a placeholder only. The fallback is a placeholder "Video source frame". | `src/components/workspace/preview-panel.tsx:244-362` | P04 |
| The source viewer overlay shows `source.label` plus `sourceRangeLabel` when present. The region label is `"Source viewer ${label}"`. | `src/components/workspace/preview-panel.tsx:379-393` | P04 |
| Viewport choice: source mode with a selected source shows the source viewer. Otherwise, if a timeline exists, show the compositor. Otherwise show a compositor on a synthetic timeline: one track `id "selected-preview-track"`, name "Selected", kind video, unlocked, enabled, holding only the selected item; duration = selected item start + duration, or 0. | `src/components/workspace/preview-panel.tsx:965-1047` | P04 |
| Compositor playhead is `playheadSeconds ?? selectedItem.startSeconds ?? 0` (the unclamped value, not the clamped `timelineCurrentSeconds`). The synthetic-timeline fallback gets no `canonicalFrameCoverageItemIds` and no retry handler. | `src/components/workspace/preview-panel.tsx:991-994,1032-1045` | P04 |
| Canonical prepared frame: build a prepared-timeline frame at the clamped timeline time. For each prepared layer, find a sequence where `preparedMediaId === layer.mediaId`, `start <= t < start+duration`, `fps > 0` and `frameUrls` is non-empty. Frame index = `clamp(floor((t-start)*fps), 0, len-1)`. | `src/components/workspace/preview-panel.tsx:686-715` | P04 |
| Canonical coverage = item ids of the prepared frame layers. If any covered id starts with "flatten-", also mark as covered every direct-frame layer that has `canonicalPreparationRequired` and is not itself in the prepared frame. | `src/components/workspace/preview-panel.tsx:725-737` | P04 |
| Canonical frames are drawn only in timeline mode, as absolute `<img>` z-10 over the compositor. Full inset when the layer has no centerX/centerY, otherwise `previewMotionStyle`. A frame whose URL failed is skipped. An img error sets `failedCanonicalFrameUrl`. | `src/components/workspace/preview-panel.tsx:1048-1070` | P04 |
| Frame preload window: in timeline mode only, preload from the current index -2 to +12 in each active sequence using `new Image()` with `decoding="async"` and `decode()`. A URL is not loaded twice while it is pending or loaded. The loaded LRU is capped at 480 URLs, evicting the oldest first. Errors remove the URL from pending. The cache is cleared when `canonicalFrameSequences` changes. | `src/components/workspace/preview-panel.tsx:747-802` | P04 |
| Canonical frame failure: the failed URL matches an active frame layer, OR (DEV and `visualQaScenarioId === "modern-editor-preview-failed"`). The failed URL is cleared automatically once no active layer uses it. | `src/components/workspace/preview-panel.tsx:738-745,804-811` | P04 |
| "Canonical ready" badge shows in timeline mode when there is at least 1 canonical frame layer and no failure. | `src/components/workspace/preview-panel.tsx:1071-1075` | P04 |
| Canonical failure alert copy: "Preview failed" / "Canonical preview frame failed to load." / details "Repair details" → "Rebuild the prepared frame, or open its source to inspect the clip." Buttons: "Retry preview" (clears the failed URL, then calls `onRetryCanonicalPreview`) and "Open source" (only when the failed layer is known; calls `onOpenSource(itemId, mediaId)`). | `src/components/workspace/preview-panel.tsx:1090-1125,821-824` | P04 |
| Context toolbar slot shows only when: timeline mode, a toolbar exists, no canonical failure, and `resolveContextToolbarPlacement(placement, canvasState)` returns a placement. Default placement is "bottom". | `src/components/workspace/preview-panel.tsx:1076-1089,480-483,418` | P04 |
| Compositor frame is built from `buildTimelinePreviewFrame(timeline, timelines, media, generatedAssets, playheadSeconds)`. Failed layer ids reset whenever the active layer key changes; the key is the `itemId:relativePath` of visual and audio layers joined. | `src/components/workspace/timeline-preview-compositor.tsx:274-290` | P04 |
| Visible media layers skip: layers needing canonical preparation whenever any `canonicalPreparation` object exists (any status); layers with no preview URL; and failed layers. | `src/components/workspace/timeline-preview-compositor.tsx:298-307` | P04 |
| Editable canvas layers: items on unlocked tracks with kind video_clip, image_clip, lottie_clip or generated_clip. | `src/components/workspace/timeline-preview-compositor.tsx:308-325` | P04 |
| Awaiting canonical = prep exists with status != "ready" AND the layer needs prep. Missing canonical = status "ready" AND the layer needs prep AND its item id is not in the coverage set. | `src/components/workspace/timeline-preview-compositor.tsx:327-336` | P04 |
| Unresolved media item: on an enabled track (`enabled !== false`), source type media, `start <= playhead < start+duration`, and its mediaId is not in `media`. | `src/components/workspace/timeline-preview-compositor.tsx:345-356` | P04 |
| Problem layer priority for "Open source": failed visual, then failed audio, then missing canonical, then unavailable visual, then unavailable audio, then unresolved media item. | `src/components/workspace/timeline-preview-compositor.tsx:357-365` | P04 |
| Issue list order: `frame.issues`, then canonical message, then missing, then unavailable visual, then unavailable audio, then failed visual, then failed audio. Canonical message when pending: "Preparing canonical Lottie, LUT, or richer-blend preview frames."; otherwise `message ??` "Canonical preview preparation failed; affected layers are hidden to avoid an inaccurate approximation." | `src/components/workspace/timeline-preview-compositor.tsx:366-387` | P04 |
| Issue copy: "Prepared frame missing for timeline item ${itemId}. Rebuild the canonical preview." / "Timeline item ${itemId} has no local preview URL." / "Timeline audio item ${itemId} has no local preview URL." / "Timeline item ${itemId} failed to load." / "Timeline audio item ${itemId} failed to load." | `src/components/workspace/timeline-preview-compositor.tsx:376-386` | P04 |
| Issue title priority: any failed layer gives "Preview failed"; else awaiting and pending gives "Preparing preview"; else missing gives "Prepared frame missing"; else "Preview unavailable". | `src/components/workspace/timeline-preview-compositor.tsx:388-395` | P04 |
| Retry is visible when there are failed layers or missing canonical layers. Issue state: no issues = "clear", retry visible = "retry", otherwise "notice". | `src/components/workspace/timeline-preview-compositor.tsx:396-399` | P04 |
| Issue alert: shows title, first issue, and "Repair details" listing all issues. Buttons appear only when a problem layer exists. "Retry preview" (if retry visible) calls `onRetryCanonicalPreview` when layers are missing, else clears failed layer ids. "Open source" calls `onOpenSource(itemId, mediaId)`. | `src/components/workspace/timeline-preview-compositor.tsx:785-837` | P04 |
| Canvas lane occupancy is reported through `useLayoutEffect`. Top is occupied if there are any issues, a template variant occupies top, or any text overlay or upper caption exists. Bottom is occupied if a lower caption exists or a template variant occupies bottom. Template variant comes from `getMotionTemplate(templateId).preview.cssVariant` via `motionTemplatePreviewLaneOccupancy`. | `src/components/workspace/timeline-preview-compositor.tsx:400-436` | P04 |
| Empty state appears when there are no visible media layers and no overlay layers. Copy: missing canonical gives "Canonical preview unavailable"; awaiting+pending gives "Preparing canonical preview"; awaiting+other status gives "Canonical preview unavailable"; otherwise "No timeline media at playhead". | `src/components/workspace/timeline-preview-compositor.tsx:438-439,769-784` | P04 |
| Screen-reader live text: "Preview skipped {n} failed timeline layer{s}." (the "s" is dropped when n === 1). | `src/components/workspace/timeline-preview-compositor.tsx:854-859` | P04 |
| Interactive (hit-testable) layers: with prep status "ready", all frame layers that either don't need prep or are covered. Otherwise, the visible media layers. The selected layer is the interactive layer matching `selectedItemId`. | `src/components/workspace/timeline-preview-compositor.tsx:440-447` | P04 |
| Media layer render: image shows `<img>`; lottie shows a placeholder div; otherwise a muted, playsInline `<video preload=metadata>` that syncs on loadedmetadata. A load error adds the item id to the failed set. | `src/components/workspace/timeline-preview-compositor.tsx:69-111,753-755` | P04 |
| Viewport size is read from `getBoundingClientRect`, with width and height each at least 1. | `src/components/workspace/timeline-preview-compositor.tsx:449-455` | P04 |
| Canvas click-select: left button only, and only when no interaction is in progress. Pointer position is normalized as `(client-left)/max(1,width)`. Hit-test with `topmostTimelinePreviewLayerAtPoint` over interactive layers that include transient transforms. On a hit, call `onSelectItem(id)` then start a "move" interaction. | `src/components/workspace/timeline-preview-compositor.tsx:624-639` | P04 |
| Starting move/resize requires: left button, an editable layer, and `onTransformCommit`. Then preventDefault, stopPropagation, pointer capture on the viewport, and store the viewport size. Initial transform comes from `canonicalCanvasTransform(layer)`. | `src/components/workspace/timeline-preview-compositor.tsx:468-491` | P04 |
| Starting rotate requires: left button, an editable layer, `onRotationCommit`, and viewport bounds. Center = `bounds.left + (centerX??0.5)*width + positionX` (Y is the same using top/height/positionY). Initial angle = `atan2(dy, dx)`. | `src/components/workspace/timeline-preview-compositor.tsx:493-518` | P04 |
| Starting crop requires: left button, an editable layer, and `onCropCommit`. Initial crop comes from `canonicalCanvasCrop(layer)`. Mode is the edge: cropTop, cropRight, cropBottom or cropLeft. | `src/components/workspace/timeline-preview-compositor.tsx:520-543` | P04 |
| Pointer move: pointer id must match and the layer must still exist in the frame. Rotate: delta in degrees, then `normalizeRotationDegrees`; Shift snaps to `round(x/15)*15`. Crop uses `croppedCanvasEdges`. Move: center += delta/viewport, then `clamp(0,1)`, then `roundedCanvasValue`. Resize uses `resizedCanvasTransform`. | `src/components/workspace/timeline-preview-compositor.tsx:545-606` | P04 |
| Movement threshold: `moved` becomes true (and stays true) once the pointer travels >= 2 px on either axis from the start. | `src/components/workspace/timeline-preview-compositor.tsx:566-569,577-580,601-604` | P04 |
| Pointer up: pointer id must match; release capture. Commit only if moved: rotate calls `onRotationCommit(id, deg)`, crop calls `onCropCommit(id, crop)`, otherwise `onTransformCommit(id, transform)`. Then clear the interaction. pointercancel clears it without committing. | `src/components/workspace/timeline-preview-compositor.tsx:608-622,745` | P04 |
| During an interaction the layer renders a transient override: `rotationDegrees` for rotate, otherwise the `current` crop or transform merged into the layer. | `src/components/workspace/timeline-preview-compositor.tsx:457-466` | P04 |
| Keyboard move (Move button): arrow keys only, requires `onTransformCommit`. Step 0.01 in normalized units; Left/Up negative. center = `round(clamp(c+step, 0, 1))`. Commits immediately. | `src/components/workspace/timeline-preview-compositor.tsx:641-660` | P04 |
| Keyboard resize (corner): builds a synthetic interaction with pointerId -1, viewport 100x100 and moved true. Calls `resizedCanvasTransform(synthetic, horizontal*100, vertical*100, layer)`, i.e. 1 px of a 100-px viewport per keypress, and commits. | `src/components/workspace/timeline-preview-compositor.tsx:661-676` | P04 |
| Keyboard rotate: ArrowLeft/ArrowRight only, requires `onRotationCommit`. Step 1°, or 15° with Shift; Left is negative. Result goes through `normalizeRotationDegrees`, then commits. | `src/components/workspace/timeline-preview-compositor.tsx:679-690` | P04 |
| Keyboard crop: arrow keys, requires `onCropCommit`. Direction: cropLeft = +horizontal, cropRight = -horizontal, cropTop = +vertical, cropBottom = -vertical. If direction is 0, return without preventDefault. Step 0.01, or 0.05 with Shift. Clamp to [0, 1 - opposite - `minimumVisibleCropFraction`] (lib value is 0.05), then round and commit. | `src/components/workspace/timeline-preview-compositor.tsx:692-725` | P04 |
| Selection overlay shows when the selected layer is editable and at least one of the transform, rotation or crop commit callbacks exists. Box style: left/top = `center??0.5`, size = `width/height??1`, then `translate(-50%,-50%) translate(posX,posY) scale(sx,sy) rotate(deg)`. | `src/components/workspace/timeline-preview-compositor.tsx:839-853,955-962` | P04 |
| Overlay controls (aria): "Canvas transform controls for ${label}", "Move ${label} in preview canvas", "Rotate ${label} in preview canvas" (title "Drag to rotate. Hold Shift to snap to 15 degrees."), "Crop ${label} from top/right/bottom/left" (handles placed at crop insets), "Resize ${label} from top left/top right/bottom left/bottom right". A dashed crop rectangle is drawn at the crop insets. | `src/components/workspace/timeline-preview-compositor.tsx:945-1056` | P04 |
| Global preview shortcuts are ignored when: interaction is disabled, the event is defaultPrevented, any of Alt/Ctrl/Meta/Shift is held, or the target or the active element is editable (`isEditableKeyboardTarget`). | `src/components/workspace/preview-panel.tsx:541-554` | P04 |
| Space / "Spacebar": if neither source nor timeline toggle is allowed, do nothing. Otherwise preventDefault; timeline mode calls `onToggleTimelinePlayback`, else `toggleSourcePlayback`. | `src/components/workspace/preview-panel.tsx:556-568` | P04 |
| ArrowLeft/ArrowRight step the source preview by -1/+1 only when source stepping is allowed, with preventDefault. The global Arrow shortcut does not step the timeline. | `src/components/workspace/preview-panel.tsx:570-577` | P04 |

## Media / import / folders / search

| Rule | Source | Plan |
|---|---|---|
| Native Tauri webview drag-drop: "over" → drop active true; "leave" → false; "drop" → false and `importMediaPaths(paths)`; failure to load listener silently ignored (browser uses media-bin DOM drop) | `src/components/workspace/editor-workspace.tsx:1217-1246` | P05 |
| Import status default "idle", importError null, source destination default "media", source panel open true | `src/components/workspace/editor-workspace.tsx:934-941` | P05 |
| Assign folder → `assignMediaFolder{mediaId, folderId\|null}`; delete → `deleteMediaFolder{folderId}` | `src/components/workspace/editor-workspace.tsx:4260-4266`, `:4297-4302` | P05 |
| Create folder: trimmed name must be non-empty; id `mediaFolderIdForName(trimmed, project.mediaFolders ?? [])`; `parentId: null` (root only) | `src/components/workspace/editor-workspace.tsx:4268-4282` | P05 |
| Rename folder: trimmed name must be non-empty → `renameMediaFolder` | `src/components/workspace/editor-workspace.tsx:4284-4295` | P05 |
| Import from picker: `openMediaFilePaths()` then `importMediaPaths`; picker error -> importStatus "failed" + message | `src/components/workspace/editor-workspace.tsx:4876-4884` | P05 |
| importMediaPaths: empty list no-op; status "importing"; selects first imported media (keeps current otherwise); importError = `formatSkippedMediaImports(skipped)` | `src/components/workspace/editor-workspace.tsx:4886-4907` | P05 |
| createMatte requires schemaVersion >= 2 and projectDir; throws "Save this project before creating a matte."; selects new media, destination "media", opens source panel | `src/components/workspace/editor-workspace.tsx:4909-4925` | P05 |
| `onCreateMatte` only provided when schemaVersion >= 2 && projectDir non-empty | `src/components/workspace/editor-workspace.tsx:6774-6776` | P05 |
| Search media / rebuild search index / extract visual frames / caption visual frames callbacks only provided when projectDir non-empty (`searchProjectMedia`, `rebuildProjectSearchIndex`, `extractVisualFrameCacheInSplitProjectFolder`, `captionVisualFrameCacheInSplitProjectFolder`) | `src/components/workspace/editor-workspace.tsx:6745-6768` | P05 |
| Media/generate/audio source destinations share one MediaBin; active destination "generate"/"audio" pass through else "media" | `src/components/workspace/editor-workspace.tsx:6557-6560,6828-6831` | P05 |
| Insert selected media: try generated output insertion first; else `mediaTimelineAction(project, mediaId)` -> `applyProjectAction`; on success select inserted item + media | `src/components/workspace/editor-workspace.tsx:4848-4874` | P05 |
| Media-only inspector (media selected, no timeline item): no video-audio/replacement-variation hooks; has insert source media / insert generated output; replacement label = replacement target label ?? null | `src/components/workspace/editor-workspace.tsx:5799-5833` | P05 |
| MediaBin import paths wired (drag-drop into bin); external drop overlay flag `externalMediaDropActive` | `src/components/workspace/editor-workspace.tsx:6732-6734` | P05 |
| mediaId = timelineItemSourceMediaId(item) if an item exists, else selectedMediaId | `src/components/workspace/source-clip-inspector.tsx:425` | P05 |
| Provenance card: a preview URL that errored is suppressed per mediaId; image uses img; video/generated uses muted video with preload metadata; otherwise an icon | `src/components/workspace/source-clip-inspector.tsx:1094-1138` | P05 |
| Card summary joined with " - ": kind, dimensions, aspect, duration (only if > 0), fps, quality label. Without an asset, shows the path. | `src/components/workspace/source-clip-inspector.tsx:1097-1108` | P05 |
| Card is a reveal button (aria "Reveal ${revealLabel} ${mediaId}") only when onRevealSource exists; calls onRevealSource(mediaId) | `src/components/workspace/source-clip-inspector.tsx:1170-1182` | P05 |
| Imported source card ("Imported source") shows only with item + mediaAsset + no generatedAsset | `src/components/workspace/source-clip-inspector.tsx:1198-1213,1635-1637` | P05 |
| Imported details: "Insert on timeline" (aria "Insert ${mediaId} on timeline") calls onInsertSourceMedia, included only when there is no item. Rows Name/Path/Type/Duration/Resolution/"Frame Rate". | `src/components/workspace/source-clip-inspector.tsx:1379-1412,1638` | P05 |
| Readouts: generated clip shows "Generated timeline source", "Clip ${label}", "Timeline a-b"; library generated shows "Generated source"; library imported shows "Library media" | `src/components/workspace/source-clip-inspector.tsx:1563-1634` | P05 |
| Timeline metadata: "Source" = sourceRangeLabel ?? mediaId ?? "unlinked"; "Reason" = properties.reason ?? "Media ${mediaId ?? "unlinked"}" | `src/components/workspace/source-clip-inspector.tsx:1348-1377` | P05 |
| No item and no media: "Select a source clip to inspect media, source range, and generation provenance." | `src/components/workspace/source-clip-inspector.tsx:2944-2946` | P05 |
| Region labels: variant timeline -> "Timeline source clip editor" with heading "Timeline" and no tabs; source variant -> "Source Inspector" | `src/components/workspace/source-clip-inspector.tsx:422-423,2951-2964` | P05 |
| No inline accepted extensions/file filters: "Import" button just calls `onImport` (native dialog lives upstream); DOM drop uses `externalFilePathsFromDataTransfer` (src/lib/media-import) then `onImportPaths(paths)` | `src/components/workspace/media-bin.tsx:3754-3760,3820` | P05 |
| External file drag highlight only when `dataTransfer.types` includes "Files"; dragover sets dropEffect "copy"; dragleave clears only when relatedTarget leaves the section; drop with 0 paths is ignored (no preventDefault) | `src/components/workspace/media-bin.tsx:3740-3760` | P05 |
| Drop overlay copy "Drop media to import" shown when `externalDropActive \|\| domFileDropActive` (role=status) | `src/components/workspace/media-bin.tsx:3762-3769` | P05 |
| Import button disabled while `importStatus === "importing"`; label "Importing" vs "Import"; aria/title "Import media" | `src/components/workspace/media-bin.tsx:3813-3825` | P05 |
| Empty library: "No media imported" + "Import media" button disabled when `importing \|\| !onImport` | `src/components/workspace/media-bin.tsx:4018-4031` | P05 |
| `importError` string rendered verbatim as amber notice above grid | `src/components/workspace/media-bin.tsx:4013-4017` | P05 |
| Media exists but none visible after filtering: "No media matches this search" | `src/components/workspace/media-bin.tsx:4032-4035` | P05 |
| Internal media drag payload: `effectAllowed = "copyMove"`, sets MIME "application/x-video-creater-media-id" = asset.id AND "text/plain" = asset.id (local constants, not in src/lib) | `src/components/workspace/media-bin.tsx:173,1110-1114` | P05 |
| Dropped media id read from custom MIME, fallback "text/plain"; accepted only if id exists in project `media` | `src/components/workspace/media-bin.tsx:1116-1122` | P05 |
| Folder drop (move): requires `onAssignMediaFolder` and a valid dropped media id; dragover sets dropEffect "move" and highlights target; drop calls `onAssignMediaFolder(mediaId, folderId)`; root target id sentinel "__library-root__" maps to folderId `null` | `src/components/workspace/media-bin.tsx:174,1186-1219` | P05 |
| Folder cards accept drops (move into folder); "Back to library" button accepts drops (move to root/null); dragleave clears highlight | `src/components/workspace/media-bin.tsx:3478-3483,3687-3702` | P05 |
| Create folder: name trimmed; empty -> no-op; calls `onCreateMediaFolder(trimmed)` then clears input; Enter key in "New folder name" input submits; "Create folder" button disabled when `!newFolderName.trim()` | `src/components/workspace/media-bin.tsx:855-863,3026-3046` | P05 |
| Rename folder: draft (or current name) trimmed; empty -> no-op; `onRenameMediaFolder(folder.id, trimmed)`; "Rename <label>" button disabled when trimmed draft empty; no uniqueness/length validation or error copy inline | `src/components/workspace/media-bin.tsx:954-961,3071-3083` | P05 |
| Delete folder: "Delete <label>" calls `onDeleteMediaFolder(folder.id)` immediately, no confirm dialog | `src/components/workspace/media-bin.tsx:3084-3095` | P05 |
| Folder manager section hidden unless at least one of create/rename/delete callbacks provided; rows indented `depth * 0.75rem`; opened via "New Folder" menu item (toggles) | `src/components/workspace/media-bin.tsx:804-805,3013-3016,3892-3905,4012` | P05 |
| Active folder auto-reset to null if its id no longer exists in folder tree; same for generation target folder | `src/components/workspace/media-bin.tsx:807-811,846-853` | P05 |
| Folder navigation: clicking folder card sets active folder; "Up to <parent label>" when parent exists; "Back to library" clears active folder; folder cards shown only in "folder" view (children of active folder or root tree) | `src/components/workspace/media-bin.tsx:3480,3514-3516,3669-3710` | P05 |
| Folder card count is recursive (folder + all descendants); aria "Folder <label> count <n> item\|items" | `src/components/workspace/media-bin.tsx:3451-3498` | P05 |
| Count label: no query -> "<n> item\|items" (whole library or active folder recursive); with query -> `Showing <visibleScoped> of <activeFolderCount> for "<query.trim()>"` | `src/components/workspace/media-bin.tsx:3436,3517-3546` | P05 |
| Search: NO debounce and NO min query length; indexed search fires on every change when `onSearchProjectMedia` present and normalized query non-empty; request `{query: searchQuery.trim(), limit: 20, scope}`; stale responses dropped via `cancelled` flag | `src/components/workspace/media-bin.tsx:813-844` | P05 |
| Empty normalized query or no search callback clears indexed result and error | `src/components/workspace/media-bin.tsx:814-818` | P05 |
| Indexed search failure: stores error message; UI shows only "Search index unavailable" (amber status) | `src/components/workspace/media-bin.tsx:834-838,3662-3666` | P05 |
| Visible media = (local match [only if query empty or scope uses local media] OR id in indexed results) AND type filter AND AI-only filter (`kind === "generated"`) | `src/components/workspace/media-bin.tsx:749-762` | P05 |
| Sort modes: "name" = localeCompare displayName sensitivity "base"; "duration" = descending durationSeconds; "type" = kind localeCompare; "dateAdded" = original project media order | `src/components/workspace/media-bin.tsx:763-776` | P05 |
| Defaults: view "folder", sort "dateAdded", type "all", AI-only false, thumbnail 80, search scope "both" | `src/components/workspace/media-bin.tsx:659-668` | P05 |
| "View & Filter" badge "<n> active" counts non-defaults among view, thumb size, sort, type, AI-only, scope | `src/components/workspace/media-bin.tsx:3169-3194` | P05 |
| View options "Folders"/"Flat"/"Grouped"; switching to non-folder view clears active folder | `src/components/workspace/media-bin.tsx:3197-3213` | P05 |
| Thumbnail sizes: 80 "Small", 110 "Medium", 150 "Large", 200 "Extra Large"; grid `repeat(auto-fill, minmax(min(<size>px,100%),1fr))` | `src/components/workspace/media-bin.tsx:3165-3167,3214-3231` | P05 |
| Sort labels "Date Added"/"Name"/"Duration"/"Type"; type filter "All Types"/"Video"/"Audio"/"Image" (no lottie/generated option) | `src/components/workspace/media-bin.tsx:3232-3263` | P05 |
| Search scope select ("Indexed search scope") only when `onSearchProjectMedia`: both="All", visual="Visual", spoken="Transcript", metadata="Metadata", generated="Generated" | `src/components/workspace/media-bin.tsx:3264-3282` | P05 |
| "AI Generated" toggle button (aria-pressed) toggles AI-only filter | `src/components/workspace/media-bin.tsx:3283-3294` | P05 |
| "Smart search" button only resets scope to "both" | `src/components/workspace/media-bin.tsx:3970-3980` | P05 |
| Search input type=search, aria "Search project media", placeholder "Search" | `src/components/workspace/media-bin.tsx:3989-3996` | P05 |
| Grid rendering: flat view or no folders -> single grid; grouped view -> "Library" (no folder) + each folder section with count, empty -> "No media matches the active filters"; folder view without active folder -> all visible media flat; with active folder -> its assets + nested child groups; empty -> "No media matches this folder" (query) / "Folder is empty" | `src/components/workspace/media-bin.tsx:3335-3433` | P05 |
| Nested folder group rendered only if it or descendants have visible assets | `src/components/workspace/media-bin.tsx:3300-3308` | P05 |
| Indexed status labels + "Rebuild search index" button shown when query non-empty and search callback present; rebuild button only if `onRebuildProjectSearchIndex` and `indexedSearchNeedsRebuild(result)`; disabled while rebuilding, label "Rebuilding index" | `src/components/workspace/media-bin.tsx:3547-3597` | P05 |
| Rebuild status copy: "Search index rebuilt" / "Search index rebuild failed" | `src/components/workspace/media-bin.tsx:874,885` | P05 |
| "Analyze" menu available only when selected media exists, `onExtractVisualFrames` provided and kind is video/image/generated | `src/components/workspace/media-bin.tsx:892-900,3604` | P05 |
| Extract frames button "Extract visual frames" / busy "Extracting visual frames" (disabled while busy); help "Frames prepare local analysis; caption search needs an explicit model run." | `src/components/workspace/media-bin.tsx:3614-3629` | P05 |
| Extract status: cacheHit -> "Visual frames already cached (<frameCount>)", else "Visual frames extracted (<frameCount>)"; error -> error.message or "Visual frame extraction failed" | `src/components/workspace/media-bin.tsx:915-923` | P05 |
| Caption frames button only if `onCaptionVisualFrames`: "Caption frames with fal.ai" / busy "Captioning visual frames"; notice "Uploads sampled frames to fal.ai and uses your configured key." | `src/components/workspace/media-bin.tsx:3630-3648` | P05 |
| Caption status: "Captioned <captionedFrameCount> visual frames with <provider>"; error -> message or "Visual captioning failed" | `src/components/workspace/media-bin.tsx:942-948` | P05 |
| Media tile: click -> `onSelectMedia(id)`; draggable; aria "Select media <filename>", aria-pressed selected; "AI" badge for kind generated; duration badge; generation status strip for generated outputs; kind icon hidden for audio | `src/components/workspace/media-bin.tsx:3104-3163` | P05 |
| Thumbnail: preview URL + video/generated -> muted `<video preload="metadata">`; image -> `<img>`; on load error media id added to failed set (placeholder used thereafter); audio placeholder = 18 deterministic waveform bars seeded from id char codes + round(duration*100), height `round((0.2+abs(sin((seed+i*13)*0.36)*cos((seed+i*5)*0.24))*0.78)*100)%` | `src/components/workspace/media-bin.tsx:309-467` | P05 |
| Status strip color: completed=cyan, queued/running=amber, else destructive; aria "<label> <status>", title "Generation <status>" | `src/components/workspace/media-bin.tsx:481-506` | P05 |
| More media actions menu: positioned `left = min(max(8, trigger.left), max(8, innerWidth-192-8))`, `top = trigger.bottom + 4`; focuses first enabled menuitem on open; Escape closes and refocuses trigger; outside pointerdown closes; ArrowDown/ArrowUp wrap, Home/End jump | `src/components/workspace/media-bin.tsx:682-711,3847-3890` | P05 |
| Menu item "Create Matte" disabled when `!onCreateMatte`; clears matte error and opens sheet | `src/components/workspace/media-bin.tsx:3906-3919` | P05 |
| Captions tab fallback copy "Select media with a transcript to edit captions." | `src/components/workspace/media-bin.tsx:4398-4405` | P05 |
| Panel tabs (non-external nav): "Media"/"Captions"/"Audio"; selecting audio sets generation mode "audio", audio view "library", closes composer; selecting captions closes composer; external nav derives tab from activeDestination (audio->audio, else media) | `src/components/workspace/media-bin.tsx:602-607,713-725,3770-3798` | P05 |
| Audio panel views "Library"/"Speech"/"Music"; switching closes composer; Library shows only `kind === "audio"` media else "No audio in the project library" | `src/components/workspace/media-bin.tsx:4412-4454` | P05 |
| Music view: "Generate music" sets mode audio, opens composer, focuses prompt when destination becomes "generate", calls `onOpenGenerateAudio`; lists media that are outputs of completed audio generations with `settings.category === "music"`, else "No generated music yet" | `src/components/workspace/media-bin.tsx:588-601,636-640,4458-4482` | P05 |
| Matte sheet: default hex "#000000", aspect "Project"; hex validated via `normalizedHex`; color picker writes uppercase value | `src/components/workspace/matte-sheet.tsx:40-45,137-144` | P05 |
| Invalid hex alert copy "Enter a six-digit hex color, such as #112233."; submit disabled when `busy \|\| !validHex`; submit guard `validHex && !busy` -> `onCreate({hex: validHex, aspectRatio})` | `src/components/workspace/matte-sheet.tsx:126-129,156-160,219-225` | P05 |
| Matte sheet copy: title "Create Matte", subtitle "Add a solid image to this project.", labels "Matte color"/"Matte aspect", buttons "Cancel" and "Create Matte" / busy "Creating…"; preview shows "<aspect>" and "<w> × <h>" from `mattePreviewSize(aspect, timelineWidth, timelineHeight)` (defaults 1920x1080) | `src/components/workspace/matte-sheet.tsx:46-49,106-225`, `src/components/workspace/media-bin.tsx:535-536` | P05 |
| Matte preview box width `min(100, w/max(w,h)*82)%` | `src/components/workspace/matte-sheet.tsx:188` | P05 |
| Matte sheet cannot close while busy (Escape, backdrop mousedown, close/Cancel disabled, all inputs disabled); focus hex input on open via rAF; return focus to "More media actions" trigger on close; Tab/Shift+Tab focus trap | `src/components/workspace/matte-sheet.tsx:51-90,95,111-119`, `src/components/workspace/media-bin.tsx:4494-4501` | P05 |
| createMatte guard: no-op if no `onCreateMatte` or already busy; passes `folderId` = active folder id when a folder is open; closes sheet on success; error -> message or "The matte could not be created." | `src/components/workspace/media-bin.tsx:3715-3732` | P05 |

## Generation

| Rule | Source | Plan |
|---|---|---|
| Readiness probes (generation models, provider credentials, Temporal backend) tracked as `{refreshId: configurationRefreshId, status}` starting "checking"; only updated when `appPreferences` prop is provided; re-run on `configurationRefreshId` change | `src/components/workspace/editor-workspace.tsx:1064-1076` | P05 |
| Provider credentials: array → statuses + "ready"; non-array → "failed"; rejection → statuses `undefined` + "failed" | `src/components/workspace/editor-workspace.tsx:1888-1930` | P05 |
| Generation catalog uses `appPreferences ?? loadAppSettingsPreferences()`; null catalog from payload → "failed" | `src/components/workspace/editor-workspace.tsx:1944-1985` | P05 |
| Temporal worker report accepted only if `isTemporalWorkerEnvironmentReport`; rejection clears report | `src/components/workspace/editor-workspace.tsx:1987-2031` | P05 |
| Generated asset poller created per non-empty projectDir (loads split project, commits via setProject); disposed on change | `src/components/workspace/editor-workspace.tsx:1162-1178` | P05 |
| Generated timeline provenance: `modelLabel` = "`<provider>`/`<model id>`", prompt, assetId; workflow status from job whose id === generated asset id (workflowType/taskQueue from workflow ?? startRequest ?? null) | `src/components/workspace/editor-workspace.tsx:1584-1635` | P05 |
| Replacement target = selected source clip ?? stored `replacementTargetItemId`; selecting a non-source item clears it; `canReplaceGeneratedOutputMedia` via `timelineItemAcceptsGeneratedOutputMedia` | `src/components/workspace/editor-workspace.tsx:1709-1720`, `:1847-1856` | P05 |
| Generate job: `buildTemporalJobSummary("generate_media", projectId, assetId, createdAt)`; no startRequest when projectDir empty; `mockMode` = `generationExecutionModeForModel(model) === "mock"`; jobId = assetId | `src/components/workspace/editor-workspace.tsx:2299-2332` | P05 |
| Workflow start dispatch: `mockMode === true` → mock path; `loadAppSettingsPreferences().generationExecutionBackend === "temporal"` → Temporal; else in-process; duplicate starts of same job id ignored via `startingTemporalWorkflowIds` | `src/components/workspace/editor-workspace.tsx:2415-2431` | P05 |
| Mock start requires startRequest, schemaVersion >= 2 and projectDir, else returns base project unchanged; runId `mockTemporalRunId(job.id)`; sets asset status "running" | `src/components/workspace/editor-workspace.tsx:2334-2373` | P05 |
| Temporal start success requires `status === "started"` and `runId`; else error = `result.message`; poller started only for `generate_media` with `mockMode === false` | `src/components/workspace/editor-workspace.tsx:2375-2413` | P05 |
| Variation/rerun: id `generatedVariationId`; placement = replacement intent if `replacementItemId` else asset's own; copies name/folder/model/references/settings; asset `status "queued"`, `outputs []`, `parentAssetId = parent ?? asset.id`, `retryOfAssetId = asset.id` | `src/components/workspace/editor-workspace.tsx:4304-4357` | P05 |
| Rerun requires asset with non-blank prompt else "Generated asset `<assetId>` cannot be rerun without a prompt." | `src/components/workspace/editor-workspace.tsx:4359-4368` | P05 |
| Retry download errors: "Generated asset `<id>` was not found." / "Generated asset `<id>` has no output media `<outputMediaId>`." / "Generated asset `<id>` has no retriable output download." / "Generated asset `<id>` output `<mediaId>` has no provider retry URL. Rerun generation to recreate the file." / "Generated asset `<id>` output `<mediaId>` can only be retried from a split project folder." | `src/components/workspace/editor-workspace.tsx:4370-4410` | P05 |
| Retriable output pick: requested output ?? first output whose media is missing from project ?? first output with non-blank `sourceUrl` | `src/components/workspace/editor-workspace.tsx:4388-4393` | P05 |
| Retry download: output lacking `sourceUrl` -> "Generated asset {assetId} output {mediaId} has no provider retry URL. Rerun generation to recreate the file." | `src/components/workspace/editor-workspace.tsx:4399-4404` | P05 |
| Retry download requires schemaVersion >= 2 and projectDir: "Generated asset {assetId} output {mediaId} can only be retried from a split project folder." | `src/components/workspace/editor-workspace.tsx:4405-4410` | P05 |
| Variation set: asset must exist; `validVariationDrafts(drafts).length < 2` -> no-op | `src/components/workspace/editor-workspace.tsx:4425-4438` | P05 |
| Variation set asset per draft: id `generatedVariationSetId(asset.id, index)`, name/prompt from draft, inherit targetFolderId/model/references/settings, placementIntent `generatedAssetPlacementIntent(asset)`, status "queued", outputs [], `parentAssetId = asset.parentAssetId ?? asset.id`, `retryOfAssetId = asset.id` | `src/components/workspace/editor-workspace.tsx:4441-4478` | P05 |
| Variation set: all recordJob+recordGeneratedAsset pairs applied in ONE batch; workflows then started sequentially threading returned project | `src/components/workspace/editor-workspace.tsx:4480-4490` | P05 |
| New generation asset: status "queued", outputs [], parentAssetId null, retryOfAssetId null; optional timeline placeholder action appended | `src/components/workspace/editor-workspace.tsx:4493-4536` | P05 |
| Provider upload confirm only when setting `requireProviderUploadConfirmation` and references need upload: confirm "This generation references local project media. Continue and allow provider upload preparation for the referenced media?"; cancel aborts | `src/components/workspace/editor-workspace.tsx:4556-4568` | P05 |
| Needs upload if mediaIds non-empty, or firstFrameMediaId, or lastFrameMediaId, or (placementIntent "timeline" && provider "fal.ai" && model id "sonilo/v1.1/video-to-music" or "mirelo-ai/sfx-v1.5/video-to-audio" && numeric videoSourceStartSeconds & videoSourceEndSeconds) | `src/components/workspace/editor-workspace.tsx:4570-4582` | P05 |
| Mock completion requires projectDir; replacement item only when `options.replaceSelectedClip` and replacement target exists | `src/components/workspace/editor-workspace.tsx:4588-4595` | P05 |
| Mock completion always records history of pre-completion project; if no replacement and first output exists and placement intent "timeline" -> insert output on timeline and select item + output media | `src/components/workspace/editor-workspace.tsx:4605-4635` | P05 |
| Fail/cancel generation requires projectDir; missing job -> "Generated media workflow job is missing for {assetId}." | `src/components/workspace/editor-workspace.tsx:4641-4650` | P05 |
| Real (`kind "generate_media"` && `startRequest.input.mockMode === false`) cancel: in-process cancel, then provider cancel only if `providerRequest.cancelUrl` and provider "fal.ai" or "replicate"; provider failure -> "Generation was cancelled locally, but the provider cancellation request failed: {message}" | `src/components/workspace/editor-workspace.tsx:4654-4680` | P05 |
| Non-real jobs fail via `buildTemporalGenerateMediaFailureActions({job, assetId, runId: job.workflow?.runId ?? null, updatedAt})` | `src/components/workspace/editor-workspace.tsx:4681-4687` | P05 |
| Referenced generation guard: media exists, not audio, trimmed prompt non-empty; default placementIntent "library" | `src/components/workspace/editor-workspace.tsx:4693-4703` | P05 |
| Video reference: source span only when context sourceIn/sourceOut are finite numbers and out > in (rounded); duration = span duration ?? context.durationSeconds (>0) ?? media.durationSeconds (>0) ?? 4 | `src/components/workspace/editor-workspace.tsx:4705-4725` | P05 |
| Video reference request: kind "generated", name null, folder null, placement = context ? `generatedReplacementPlacementIntent(context.itemId)` : placementIntent; model fal.ai `falWanVideoToVideoModelId`; references `{mediaIds:[id], sourceVideoMediaRef:id, firstFrameMediaId:null, lastFrameMediaId:null}`; width ?? 1280, height ?? 720, fps ?? 24, aspectRatio `aspectRatioLabel(w,h)` or "16:9"; adds videoSourceStart/EndSeconds when span | `src/components/workspace/editor-workspace.tsx:4727-4762` | P05 |
| Image reference request: model fal.ai "fal-ai/wan-25-preview/text-to-video"; references `{mediaIds:[id], firstFrameMediaId:id, lastFrameMediaId:null}`; settings 1280x720, 4 s, 24 fps, "16:9" | `src/components/workspace/editor-workspace.tsx:4765-4787` | P05 |
| Upscale: media exists and not audio -> `upscaleGenerationRequest(media, context)` | `src/components/workspace/editor-workspace.tsx:4790-4797` | P05 |
| Video-to-audio: media must be kind "video" -> `videoAudioGenerationRequest(media, kind, context)` | `src/components/workspace/editor-workspace.tsx:4799-4810` | P05 |
| Replace clip with generated output: requires replacement target and `canReplaceGeneratedOutputMedia(mediaId)`; selects media; action `{type:"replaceTimelineItemWithGeneratedOutput", replacement:{itemId, mediaId}}` | `src/components/workspace/editor-workspace.tsx:4812-4828` | P05 |
| Insert generated output: `generatedOutputTimelineActions`; 1 action -> applyProjectAction, else applyProjectActions; select item + media on success | `src/components/workspace/editor-workspace.tsx:4830-4846` | P05 |
| Clip inspector: `referencedGenerationPlacementIntent="timeline"`; replacement variation passes `{replacementItemId: itemId}`; "use in composer" sets composerSeed `{type:"asset", assetId, placementIntent}` | `src/components/workspace/editor-workspace.tsx:5777-5794` | P05 |
| Mock complete/fail callbacks only when projectDir non-empty | `src/components/workspace/editor-workspace.tsx:6769-6770` | P05 |
| Enabled model count: catalog null -> `appPreferences.enabledGenerationModelIds.length`, else sum of catalog model list lengths; only when appPreferences loaded | `src/components/workspace/editor-workspace.tsx:6678-6688` | P05 |
| Execution backend = appPreferences ?? `loadAppSettingsPreferences()`; `temporalBackendReady` only when worker report non-null | `src/components/workspace/editor-workspace.tsx:6690-6696` | P05 |
| Configuration readiness models/providers/temporal = status only if its `refreshId === configurationRefreshId`, else "checking" | `src/components/workspace/editor-workspace.tsx:6697-6714` | P05 |
| Composer open while destination "media" switches to "generate" and opens panel; "Generate audio" opens generate + composer | `src/components/workspace/editor-workspace.tsx:6629-6647` | P05 |
| External generation close (destination "generate" only): close panel; compact view "timeline" if compact view was media else null; focus "Generate" button inside "Editor destinations" | `src/components/workspace/editor-workspace.tsx:6648-6661` | P05 |
| Composer seed cleared only when handled asset/media id matches current seed | `src/components/workspace/editor-workspace.tsx:6779-6792` | P05 |
| Timeline "create video from item": select item & media; composerSeed `{type:"media", mediaId, placementIntent:"replace:{itemId}", mode}`; compact view "media" | `src/components/workspace/editor-workspace.tsx:8422-8434` | P05 |
| Silence section count shown = `timelineSilenceRippleRanges(project).length` | `src/components/workspace/editor-workspace.tsx:6718` | P05 |
| "Queue Upscale" (non-audio clip with source media) → onQueueItemUpscale(itemId, mediaId); "Set as first frame"/"Set as reference" (image_clip with source media) → onCreateVideoFromItem(itemId, mediaId, "firstFrame"/"reference") | `src/components/workspace/timeline-editor.tsx:3276-3294,5291-5323` | P05 |
| Tabs "Details"/"AI Edit" show in the source variant when (generatedAsset \|\| canShowImportedAiEdit) && !externalNavigation. aria "Generated source inspector"/"Source inspector". Tab change sets local view and calls onViewChange. | `src/components/workspace/source-clip-inspector.tsx:2972-2988` | P05 |
| View resets to "details" and referenced prompt clears when item.id or mediaId changes; activeView prop overrides local view | `src/components/workspace/source-clip-inspector.tsx:557-559,738-741` | P05 |
| Variation prompt re-initializes to generatedAsset.prompt when asset id or prompt changes. "Restore original prompt" shows when the draft differs. | `src/components/workspace/source-clip-inspector.tsx:734-736,1007-1008,2864-2874` | P05 |
| Busy when status queued\|running; shows reason "Generation in progress" | `src/components/workspace/source-clip-inspector.tsx:985-988,2856-2858` | P05 |
| "Queue variation" enabled with asset + onQueueVariation, not busy, and trimmed prompt non-empty. Calls onQueueVariation(asset.id, prompt.trim()). | `src/components/workspace/source-clip-inspector.tsx:989-993,2883-2898` | P05 |
| "Queue and replace selected clip" enabled with asset + item + onQueueReplacementVariation, not busy, prompt non-empty. Calls onQueueReplacementVariation(asset.id, prompt.trim(), item.id). Target shown under "Replacement target" = replacementTargetLabel.trim() \|\| item.label. | `src/components/workspace/source-clip-inspector.tsx:994-999,1325-1346,2899-2927` | P05 |
| AI-edit "Rerun generation" shows if onQueueVariation exists, disabled when busy. Calls onQueueVariation(asset.id, asset.prompt) with the original prompt. | `src/components/workspace/source-clip-inspector.tsx:2786-2802` | P05 |
| "Rerun and replace selected clip" needs asset, item, truthy replacementTargetLabel, onQueueReplacementVariation, not busy, non-empty asset.prompt. Sends the original prompt with item.id. | `src/components/workspace/source-clip-inspector.tsx:1000-1006,2803-2823` | P05 |
| "Use in composer" (aria "Use generated source in composer") needs asset + callback, not busy. Calls onUseGeneratedAssetInComposer(asset.id, generatedComposerPlacementForItem(item)). | `src/components/workspace/source-clip-inspector.tsx:1040-1041,2769-2785` | P05 |
| Generated "Queue upscale" needs outputMediaId + onQueueUpscale, not busy, and no generatedUpscaleLimitReasonForAsset. Calls onQueueUpscale(outputMediaId) with no context. With a limit reason: disabled button plus the reason text. | `src/components/workspace/source-clip-inspector.tsx:1034-1039,2824-2853` | P05 |
| Details-tab icon "Rerun generation" (aria "Rerun generation for ${id}") needs onRerunGeneratedAsset, NO onQueueVariation, non-empty prompt, status not queued/running | `src/components/workspace/source-clip-inspector.tsx:1042-1048,2642-2654` | P05 |
| "Retry download" (aria "Retry download for ${id}") shows when output exists but its media asset is missing. Calls onRetryGeneratedAssetDownload(asset.id, outputMediaId). | `src/components/workspace/source-clip-inspector.tsx:1049-1053,2655-2672` | P05 |
| Replace (title "Replace selected clip", aria "Replace ${targetLabel} with ${outputMediaId}") needs NO item, outputMediaId, replacementTargetLabel, callback, and canReplaceGeneratedOutputMedia(outputMediaId) (default always true) | `src/components/workspace/source-clip-inspector.tsx:415,1054-1059,2603-2620` | P05 |
| Generated "Insert on timeline" icon (aria "Insert ${outputMediaId} on timeline") shows whenever onInsertGeneratedOutput and outputMediaId exist | `src/components/workspace/source-clip-inspector.tsx:2621-2633` | P05 |
| Output swap list ("Outputs") needs item + onReplaceGeneratedOutput + outputs.length > 1. Current output is marked "Current". Others get "Swap" (aria "Swap ${label} to ${mediaId}") only if canReplaceGeneratedOutputMedia(candidate); calls onReplaceGeneratedOutput(candidate.mediaId). | `src/components/workspace/source-clip-inspector.tsx:1060-1063,1251-1323` | P05 |
| Output mediaId = output.mediaId ?? item property generatedOutputMediaId ?? mediaId. Path fallback: output.relativePath -> output asset path -> media path -> ids -> "unlinked". | `src/components/workspace/source-clip-inspector.tsx:433-442` | P05 |
| References: unique union of references.mediaIds + referenceImage/Video/AudioMediaRefs, plus "First frame"/"Last frame"/"Output" tiles | `src/components/workspace/source-clip-inspector.tsx:443-452,1215-1249` | P05 |
| Generated rows: Model "${provider}/${id}", Status, Aspect (?? "unknown"), Path, Type (?? "generated"). Duration/Resolution/Frame Rate fall back output -> media asset -> settings. | `src/components/workspace/source-clip-inspector.tsx:2684-2716` | P05 |
| "Copy generation prompt" calls navigator.clipboard?.writeText(prompt) (fire and forget). hideGeneratedDetails hides both generated views. | `src/components/workspace/source-clip-inspector.tsx:291-293,2587,2734-2743` | P05 |
| Empty AI edit: "AI edit actions are unavailable for this project state." when there is no variation callback, no replacement (with item), no upscale and no composer action | `src/components/workspace/source-clip-inspector.tsx:2930-2937` | P05 |
| Imported upscale needs mediaId + asset, no generatedAsset, kind not audio/generated, onQueueUpscale, and no importedUpscaleLimitReasonForMedia. With a limit reason: disabled "Queue upscale" plus the reason. | `src/components/workspace/source-clip-inspector.tsx:1009-1017,1448-1481` | P05 |
| Imported video audio needs item, kind "video", no generatedAsset, onQueueVideoAudio, and a non-null context from sourceClipGenerationContextForItem. "Queue music from clip" sends kind "music"; "Queue SFX from clip" sends "sfx". | `src/components/workspace/source-clip-inspector.tsx:1018-1024,1066,1426-1447` | P05 |
| Referenced generation shows with mediaId + asset, no generatedAsset, kind not audio/generated, and onQueueReferencedGeneration. Textarea "Referenced generation prompt". Enabled when prompt trimmed non-empty and no importedVideoEditLimitReasonForMedia(asset, ctx); the reason text shows. | `src/components/workspace/source-clip-inspector.tsx:1025-1031,1067-1074,1482-1497` | P05 |
| Referenced button label: "Queue video edit" for video, else "Queue referenced shot". Section title "Imported AI edit". canShowImportedAiEdit = referenced \|\| upscale \|\| videoAudio. | `src/components/workspace/source-clip-inspector.tsx:1032-1033,1423,1535-1537` | P05 |
| Composer defaults: mode "video", duration "5s", aspect "16:9", resolution "1280x720", image count "1", quality "high", generateAudio true, instrumental false, voice "", placement "library", reference pane "first-last", model values `defaultGenerationModelValues` | `src/components/workspace/media-bin.tsx:584,608-629,641-643` | P05 |
| "Generate" toolbar button disabled when `!onGenerateMedia`; toggles composer (drawer overlay in non-external nav; full view when external destination "generate") | `src/components/workspace/media-bin.tsx:3826-3838,3800-3803,4003-4010` | P05 |
| Opening composer seeds: first frame = selected generated asset's firstFrame (frame-ref-valid) else selected frame-reference media; last frame from asset; references = asset's visual reference ids (deduped) else [selected visual media]; source video = asset's sourceVideoMediaRef/referenceVideoMediaRefs/mediaIds first kind "video" else selected video; also seeds settings from selected generated asset; target folder = active folder; placement "library"; history closed | `src/components/workspace/media-bin.tsx:1038-1108,1221-1237` | P05 |
| Seeding from asset: model used only if valid for mode else default model (model value only stored if valid); duration = matching option or `boundedGenerationDurationValue`; aspect only if `isGenerationAspectRatio`; numImages default 1, quality default "high", generateAudio default true, voice default "" | `src/components/workspace/media-bin.tsx:1064-1108` | P05 |
| Close composer: resets target folder null, placement "library", history closed, source video ""; external audio destination resets audio view to "library"; external generate/audio calls `onExternalGenerationClose(destination)` | `src/components/workspace/media-bin.tsx:1239-1254` | P05 |
| "Use prompt" (history draft): copies prompt; name -> "<name.trim()> variation" if asset named; copies duration/aspect/resolution/count/quality/audio/instrumental/voice/lyrics/style; video mode copies first/last frame, visual refs, source video; image mode copies refs only (clears frames+source); audio clears frames/refs but keeps source video | `src/components/workspace/media-bin.tsx:1462-1532,1856-1865` | P05 |
| "Use in composer" on generated asset card (only when `onGenerateMedia`): history draft + open composer, target folder kept only if still exists, placement = given ?? asset.placementIntent ?? "library" | `src/components/workspace/media-bin.tsx:1534-1547,4113-4125` | P05 |
| Media seed into composer: mode "reference" requires visual media, else frame-reference media (returns false/no-op if missing); forces mode "video", clears name/prompt; reference -> refs=[id], pane "reference"; firstFrame -> first frame=id, source video=id if kind video, pane "first-last"; placement default "library" | `src/components/workspace/media-bin.tsx:1549-1583` | P05 |
| Seed effects: `composerSeedAssetId` missing asset still calls `onComposerSeedHandled`; `composerSeedMediaId` always calls `onComposerMediaSeedHandled` after attempt; defaults placement "library", media mode "firstFrame" | `src/components/workspace/media-bin.tsx:1585-1623` | P05 |
| Selecting mode "image" with no references seeds refs with selected frame-reference media | `src/components/workspace/media-bin.tsx:1680-1688` | P05 |
| Add reference: only if id in `generationReferenceMediaForModel(mode, model, media)`; exclusive-frame-mode models clear first/last frames and switch to "reference" pane; dedup append | `src/components/workspace/media-bin.tsx:1625-1649` | P05 |
| Switch reference pane on exclusive-frame-mode models: to "reference" clears first/last frame; to "first-last" clears references | `src/components/workspace/media-bin.tsx:1663-1678` | P05 |
| Reference drops: frame slots accept only frame-reference media; reference list accepts visual media; source video slot accepts source-video media; dragover dropEffect "copy" | `src/components/workspace/media-bin.tsx:1124-1184,1955-1956,2038-2039,2127-2128` | P05 |
| Reference tag autocomplete: shown when prompt has trailing tag query; filters tags whose name (minus first char) case-insensitively startsWith query; Enter with visible tags inserts first tag (preventDefault) | `src/components/workspace/media-bin.tsx:726-739,2202-2235,2747-2756` | P05 |
| Reference list shows "References not sent" when model doesn't support reference media but refs selected; empty copy "Drop style, subject, or composition references"; select placeholder "Add reference"; remove aria "Remove generation reference media <filename>" | `src/components/workspace/media-bin.tsx:2106-2200` | P05 |
| Frame slots: "First frame" empty "Drop an opening frame", "Last frame" empty "Drop a closing frame"; select option "None"; filled label strips trailing " frame" | `src/components/workspace/media-bin.tsx:1931-2021,2696-2713` | P05 |
| Source video slot (audio mode only when model requires source video): "Source Video", empty "Select video", remove aria "Remove generation source video" | `src/components/workspace/media-bin.tsx:2023-2104,2725-2729` | P05 |
| Video reference pane UI: pane toggles "First/Last" (if frame refs supported or frames already selected) and "Reference" (if reference media supported); forced pane when only one supported | `src/components/workspace/media-bin.tsx:2484-2500,2657-2719` | P05 |
| Image mode always shows reference list | `src/components/workspace/media-bin.tsx:2720-2724` | P05 |
| Configuration blocker precedence: models checking -> "Checking generation configuration…"; models failed -> "Generation model configuration could not be checked." (Settings aiModels/generationModels, "Configure generation models in Settings"); no enabled models -> "Media generation needs at least one enabled generation model."; providers checking -> checking copy; providers failed -> "<Provider> provider configuration could not be checked." ("Configure <Provider> in Settings", integrations/provider); provider not configured -> source "missing": "<Name> <mode> generation needs a\|an <Name> provider key." (an if name starts with vowel) else "<Name> <mode> generation cannot access its provider key."; temporal backend checking -> checking copy; temporal failed -> "Temporal configuration could not be checked."; temporal not ready -> "Temporal generation is selected, but the Temporal backend is not configured." (advanced/execution, "Configure Temporal in Settings") | `src/components/workspace/media-bin.tsx:2247-2334` | P05 |
| Enabled-models check: `enabledGenerationModelCount > 0` if provided, else catalog undefined or any mode list non-empty; provider status matched by trimmed lowercase provider id | `src/components/workspace/media-bin.tsx:2247-2256` | P05 |
| Action blocker renders `ConfigurationNotice` only if `onOpenSettings`; otherwise plain status text | `src/components/workspace/media-bin.tsx:2619-2633` | P05 |
| Queue readiness label precedence: "Checking configuration" / "Configuration required" / prompt readiness message / "Select video" / "Select first frame" / reference limit message / "Ready" | `src/components/workspace/media-bin.tsx:2528-2538` | P05 |
| "Queue generation" enabled iff no blocker, no prompt readiness message, timeline placement ok, required source video present (selected or valid timeline source range), required first frame present, no reference limit message | `src/components/workspace/media-bin.tsx:2457-2527,2992-2998` | P05 |
| Prompt readiness message suppressed when model requires source video and one is available | `src/components/workspace/media-bin.tsx:2501-2508` | P05 |
| Timeline placement allowed if (placement != "timeline" and no valid timeline source range) or a timeline target exists for `timelineTargetMode(mode)` | `src/components/workspace/media-bin.tsx:2457-2464` | P05 |
| Placement note (shown when placement != "library"): "replace:*" -> "Will replace <replacementTargetLabel ?? "selected timeline clip">."; timeline with target -> "Will append to <trackName> at <formatTimelinePlacementTime(start)>."; no target -> "No unlocked <kind> track available." | `src/components/workspace/media-bin.tsx:2441-2456,2649-2656` | P05 |
| Field visibility: voice if model has voices; duration if mode != image and (options or bounds); aspect/size/quality/count only mode != audio; size if resolution choices; quality only image mode with choices; count only image with max images > 1 (options 1..max); instrumental/audio toggle/lyrics/style per model capability | `src/components/workspace/media-bin.tsx:2393-2436,2827-2970` | P05 |
| Bounded duration renders number input min=bounds.minSeconds max=bounds.maxSeconds step=1; otherwise select of options; invalid stored duration falls back to first option; invalid aspect falls back to first choice | `src/components/workspace/media-bin.tsx:2347-2364,2844-2873` | P05 |
| Cost estimate: `selectedGenerationCost(mode, model, bounded? boundedDuration : duration, resolution, imageCount, generateAudio, prompt, quality)` rendered via `formatGenerationCreditEstimate`; balance chip = `formatCredits(mockGenerationCreditBalance)` | `src/components/workspace/media-bin.tsx:2415-2437,2552-2559,2972-2977` | P05 |
| Active queue chip "1 active generation" / "<n> active generations" when count > 0 | `src/components/workspace/media-bin.tsx:2428-2432,2560-2567` | P05 |
| History panel toggle shows 4 most recent generated assets (createdAt desc); empty "No generation history yet" | `src/components/workspace/media-bin.tsx:2427,2634-2648` | P05 |
| Mode selector order image, video, audio | `src/components/workspace/media-bin.tsx:2596` | P05 |
| Inputs copy: "Name (optional)" placeholder "Name generated asset"; "Prompt" placeholder from `generationPromptPlaceholder`; "Lyrics" placeholder "[Verse]"; "Style" placeholder "Tone, delivery, instrumentation"; "Instrumental"; "Audio" | `src/components/workspace/media-bin.tsx:2730-2796,2947-2970` | P05 |
| "Agent Mode" button (title "Draft this music task in Codex") only when `generationAgentModePrompt(mode, model)` non-empty and `onDraftGenerationAgentPrompt`; calls it with that prompt | `src/components/workspace/media-bin.tsx:1449-1460,2978-2991` | P06 |
| Generated asset cards listed when `generatedAssetNeedsHistoryCard` and (query empty or scope uses local generated) and matches search; sorted createdAt desc | `src/components/workspace/media-bin.tsx:777-789` | P05 |
| Card badges: "Timeline target" (placement "timeline"), "Replacement target" (starts "replace:"), "Destination <folder label>"; shows status, prompt when named, lineage, workflow label (job id === asset id), pending output label or outputs list, reference label | `src/components/workspace/media-bin.tsx:4046-4389` | P05 |
| Output "Replace selected clip" shown when `onReplaceGeneratedOutput && replacementTargetLabel && canReplaceGeneratedOutputMedia(mediaId)`; aria "Replace <label> with <mediaId>" | `src/components/workspace/media-bin.tsx:4222-4236` | P05 |
| Output "Insert on timeline" shown whenever `onInsertGeneratedOutput`; calls with output mediaId (no inline target validation) | `src/components/workspace/media-bin.tsx:4237-4249` | P05 |
| Output "Retry download" when callback and `generatedOutputHasProviderSourceUrl(output)` -> `onRetryGeneratedAssetDownload(asset.id, output.mediaId)`; asset-level retry when any output has provider URL -> `(asset.id)` | `src/components/workspace/media-bin.tsx:4088-4090,4177-4179,4250-4306` | P05 |
| "Rerun generation" when callback, prompt non-empty (trimmed), `generatedAssetHasRerunnableModel`, and status not queued/running | `src/components/workspace/media-bin.tsx:4078-4087,4281-4293` | P05 |
| "Retry generation" (failed status, mock-worker completable, `onQueueGeneratedVariation`) -> `onQueueGeneratedVariation(asset.id, asset.prompt)` | `src/components/workspace/media-bin.tsx:4074-4077,4314-4328` | P05 |
| "Complete mock" when callback, `canCompleteWithMockWorker`, status queued/running; "Complete mock & replace" additionally needs `replacementTargetLabel` -> `{replaceSelectedClip: true}` | `src/components/workspace/media-bin.tsx:4058-4061,4091-4092,4329-4384` | P05 |
| "Cancel generation" when `onFailMockGeneration`, workflow job kind "generate_media" with `startRequest.input.mockMode === false`, status queued/running -> `onFailMockGeneration(id)`; "Fail mock" only when not cancelable, mock-completable, queued/running | `src/components/workspace/media-bin.tsx:4062-4073,4342-4367` | P05 |
| AI media metrics: outputs = sum of asset.outputs.length; "`n` active" generation jobs; timeline count = assets with placementIntent "timeline"; selects = recentGeneratedAssets | `src/components/workspace/project-timeline-inspector.tsx:231-239,332-337` | P05 |
| Generated asset card: status badge; "duration unknown" or "`d.d`s" (toFixed(1)); "Destination: `folder path`" when folder label exists; "Workflow `status`" when job with same id as asset exists | `src/components/workspace/project-timeline-inspector.tsx:340-398` | P05 |
| Generated outputs list capped at first 2; "Inspect" (aria "Inspect generated output `mediaId`") only when onSelectGeneratedOutput | `src/components/workspace/project-timeline-inspector.tsx:399-438` | P05 |
| Empty copy "No generated media yet" | `src/components/workspace/project-timeline-inspector.tsx:446-454` | P05 |

## Speech

| Rule | Source | Plan |
|---|---|---|
| Speaker registry loaded via `getProjectSpeakerRegistry` only when schemaVersion >= 2 and projectDir non-empty (else []); non-array speakers → []; backend-unavailable errors silent, others `console.warn` | `src/components/workspace/editor-workspace.tsx:1195-1215` | P05 |
| Per-media speech analysis status map values "analyzing" \| "failed" | `src/components/workspace/editor-workspace.tsx:955-957` | P05 |
| Remove detected silence: `timelineSilenceRippleRanges(project)`; no-op if empty; single `rippleDeleteRanges` | `src/components/workspace/editor-workspace.tsx:3204-3214` | P05 |
| Transcription: requires selectedMediaId; missing media -> "Cannot transcribe missing media {id}."; `languageMode: "auto"` | `src/components/workspace/editor-workspace.tsx:5555-5573` | P05 |
| Transcription start request falls back to `buildFallbackTranscribeMediaStartRequest` only on `isBackendUnavailableError`; record failure -> "Transcription workflow could not be recorded." | `src/components/workspace/editor-workspace.tsx:5583-5601` | P05 |
| Speech refresh reloads split project, records history of previous project, sets project | `src/components/workspace/editor-workspace.tsx:5609-5614` | P05 |
| Analyze speech requires item, source media id, media asset, projectDir; PCM path = `properties.audioDenoisePreparation.artifact` (string) else `media.relativePath` | `src/components/workspace/editor-workspace.tsx:5616-5631` | P05 |
| Analyze statuses: "analyzing" -> entry deleted on success -> "failed" on error plus action `updateItemProperties` set `speechAnalysis: {status:"failed", quality:"production"}` applied against base project | `src/components/workspace/editor-workspace.tsx:5627-5646` | P05 |
| Rename speaker requires non-empty trimmed name and projectDir (sends untrimmed name) | `src/components/workspace/editor-workspace.tsx:5649-5659` | P05 |
| Recolor speaker requires projectDir | `src/components/workspace/editor-workspace.tsx:5661-5671` | P05 |
| Assign speaker requires `properties.speechAnalysis.fingerprint` string: else "Analyze speech before assigning a speaker." | `src/components/workspace/editor-workspace.tsx:5678-5683` | P05 |
| Assign range = clip ∩ selected timeline range (or whole clip); empty -> "The selected range does not overlap this clip."; sourceIn default 0, sourceOut default sourceIn + duration; source rate = `max(0, out-in) / max(duration, 0.001)`; source seconds = `sourceIn + (t - item.start) * rate` | `src/components/workspace/editor-workspace.tsx:5684-5702` | P05 |
| Speech workbench: analysis status for `speechTargetItem` else "idle"; identify speakers analyzes target item; remove silence -> `removeDetectedSilence()`; speaker/silence marking toggles | `src/components/workspace/editor-workspace.tsx:6613-6628,6738` | P05 |
| Speaker section for audio_clip with onAnalyzeSpeech. Title "Speakers", subtitle "On-device Silero and WeSpeaker analysis". | `src/components/workspace/source-clip-inspector.tsx:2500-2511` | P05 |
| Analyze button is disabled while "analyzing". Label: "Analyzing…" while analyzing; "Retry analysis" if status "failed" or item.properties.speechAnalysis.status === "failed"; else "Analyze speech". Calls onAnalyzeSpeech(item.id). | `src/components/workspace/source-clip-inspector.tsx:2512-2527` | P05 |
| "No identified speakers yet." when the list is empty | `src/components/workspace/source-clip-inspector.tsx:2529-2530` | P05 |
| Speaker color input (aria "Color for ${name}") calls onRecolorSpeaker on every change. Name input is uncontrolled (defaultValue) and commits onBlur via onRenameSpeaker(id, value) with no trim or empty guard. "Assign" calls onAssignSpeaker(item.id, speaker.id). | `src/components/workspace/source-clip-inspector.tsx:2533-2551` | P05 |
| "Remove All Silence" menu item shown if `onRemoveDetectedSilence`; disabled when `detectedSilenceSectionCount <= 0` | `src/components/workspace/media-bin.tsx:3936-3950` | P05 |
| "Remove Current Silence" shown if `onRemoveCurrentSilence`; disabled when `!currentSilenceSectionAvailable`; separator shown if either exists | `src/components/workspace/media-bin.tsx:3933-3965` | P05 |
| Audio panel "Speech" view renders injected `speechPanel` | `src/components/workspace/media-bin.tsx:4455-4456` | P05 |
| No silence thresholds / denoise amounts in this UI; only toggles + actions | `src/components/workspace/speech-workbench.tsx:5-14` | P05 |
| "Mark Speakers" checkbox -> onMarkSpeakers(checked); help "Analyze speech and mark identified speakers on the timeline." | `src/components/workspace/speech-workbench.tsx:69-85` | P05 |
| Speaker button disabled while analysisStatus "analyzing"; label "Identifying…" (analyzing) / "Retry Speakers" (failed) / "Identify Speakers" (idle) | `src/components/workspace/speech-workbench.tsx:86-100` | P05 |
| "Mark Silence" checkbox -> onMarkSilence(checked); help "Mark silent timeline ranges before choosing which sections to remove." | `src/components/workspace/speech-workbench.tsx:110-126` | P05 |
| "Remove Silence" disabled when silenceSectionCount === 0 (default 0); count "`n` section\|sections" | `src/components/workspace/speech-workbench.tsx:53,127-141` | P05 |
| Help tooltip button aria "About `label`", opens on hover/focus, closes on leave/blur | `src/components/workspace/speech-workbench.tsx:20-48` | P05 |

## Export / render

| Rule | Source | Plan |
|---|---|---|
| Export status values idle/exporting/exported/failed/cancelled; default "idle"; render quality default `defaultRenderQualityProfile`; export sheet entry intent default "destination" | `src/components/workspace/editor-workspace.tsx:607`, `:1005-1015`, `:1032-1040` | P07 |
| Visual QA render-failed scenario message "The deterministic visual-QA render stopped before completion. Review its failure details and retry when ready." and inspector override "render-review" | `src/components/workspace/editor-workspace.tsx:972-974`, `:1035-1039` | P07 |
| Crash recovery: latest failed render job (once per job id) → status "failed", message "The previous render stopped before completion. Review its failure details and retry when ready.", path null, open "render-review"; loads report if projectDir; report load failure ignored | `src/components/workspace/editor-workspace.tsx:1125-1160` | P07 |
| Export availability state starts `{status "checking", profiles: fallbackExportProfileAvailability}`; non-empty array report → "ready"; empty/rejection → "unavailable" with fallback | `src/components/workspace/editor-workspace.tsx:1049-1056`, `:1858-1886` | P07 |
| Media/NLE/package export allowed only when schemaVersion >= 2 and projectDir non-empty | `src/components/workspace/editor-workspace.tsx:1745-1748` | P07 |
| Video profiles order: webm, mp4H264, mp4H265, proResMov; labels "WebM", "H.264", "H.265 (HEVC)", "ProRes"; codec vp8/h264/h265/prores; fileType "`<CONTAINER>` (.`<ext>`)"; availability = profile.available && split project; reason `exportProfileDisabledReason(profile, canExport, executionUsesTemporal())` | `src/components/workspace/editor-workspace.tsx:1749-1781` | P07 |
| Quality availability draft/final ANDed with split project; otherwise both reasons "Save as a split project to enable media export." | `src/components/workspace/editor-workspace.tsx:1782-1792` | P07 |
| Timeline exports: "Premiere XML" fileType "XMEML (.xml)" target "Adobe Premiere Pro"; "DaVinci XML" "FCPXML (.fcpxml)" version "1.11" target "DaVinci Resolve / Final Cut Pro"; unavailable reason "Save as a split project to enable timeline export." | `src/components/workspace/editor-workspace.tsx:1794-1816` | P07 |
| "Palmier Project" fileType "Palmier Project (.palmier)", destination "palmier-project"; unavailable "Save as a split project to enable project packaging." | `src/components/workspace/editor-workspace.tsx:1817-1826` | P07 |
| recordRenderReport: webm profile sets selected quality "draftWebm"/"finalWebm" by quality | `src/components/workspace/editor-workspace.tsx:5073-5096` | P07 |
| Preview/render comparison guard: report has `previewComparisonRequest`, projectDir non-empty, not already running; request sent with `status: "pending"`; stale runId ignored | `src/components/workspace/editor-workspace.tsx:5098-5110` | P07 |
| Comparison messages: "Preview/render review passed" (status "passed") / "Preview/render review found differences; reveal the retained evidence to inspect them" / "Preview/render review failed: {message}" | `src/components/workspace/editor-workspace.tsx:5117-5129` | P07 |
| In-process export: new runId; local optimistic `recordJob {id, kind:"render_draft", status:"running", updatedAt}` via `applyProjectActionLocally`; message "{renderLabel} rendering" (`inProcessExportLabel`) | `src/components/workspace/editor-workspace.tsx:5141-5170` | P07 |
| In-process success: "{renderLabel} rendered"; opens inspector "render-review"; completion notification "{message}: {outputPath}" only if `renderCompletionNotifications` setting | `src/components/workspace/editor-workspace.tsx:5172-5188` | P07 |
| In-process failure: if projectDir, reload project + render report in parallel (each failure -> null); status "failed", message = error; opens "render-review" | `src/components/workspace/editor-workspace.tsx:5189-5222` | P07 |
| Default draft render = webm, quality "draft", `draftExportDimensions(renderSettings.width, height)` | `src/components/workspace/editor-workspace.tsx:5225-5231` | P07 |
| Cancel render only when exportStatus "exporting"; bumps runId; "Render cancelled"; backend cancel only with projectDir + jobId + attemptId | `src/components/workspace/editor-workspace.tsx:5233-5265` | P07 |
| Save range as media requires projectDir: "Save Range as Media requires a saved split project folder." | `src/components/workspace/editor-workspace.tsx:5267-5273` | P07 |
| Save range: selected quality "draftWebm"; "Timeline range rendering"; importStatus "importing"; render webm draft at draft dimensions with rangeStart/EndSeconds; non project-relative output -> "Rendered range output path is not project-relative."; success "Timeline range saved as media"; skipped -> "{n} file(s) skipped during import." | `src/components/workspace/editor-workspace.tsx:5275-5331` | P07 |
| NLE label "Premiere XML" for "premiereXmeml" else "DaVinci XML"; guard -> "{label} export requires a saved split project folder."; "{label} export running"; "{label} exported" | `src/components/workspace/editor-workspace.tsx:5334-5363` | P07 |
| Media profile export: ignores "palmierProject"; default input quality "final", project render dimensions, output path `mediaExportOutputPath(projectId, profile, extension, jobId)`; silently no-op unless available && qualityAvailability[quality] && canExportMediaProfiles | `src/components/workspace/editor-workspace.tsx:5365-5390` | P07 |
| Codec label "H.264" (mp4H264) / "H.265 (HEVC)" (mp4H265) / "ProRes"; quality "Draft"/"Final"; message "{codec} {quality} queued at {w}×{h}"; record failure "{profile.label} workflow could not be recorded"; after start status "idle" | `src/components/workspace/editor-workspace.tsx:5394-5425` | P07 |
| Palmier package guard: "Palmier Project export requires a saved split project folder."; extension "palmier" | `src/components/workspace/editor-workspace.tsx:5428-5439` | P07 |
| Palmier non-Temporal: "Palmier Project package export running" -> "Palmier Project package exported"; Temporal: "Palmier Project package workflow queued" / "Palmier Project package workflow could not be recorded" | `src/components/workspace/editor-workspace.tsx:5442-5491` | P07 |
| Export sheet routing: "webm" -> in-process; "premiereXmeml"/"davinciFcpxml" -> NLE; "palmierProject" -> package; other -> `startMediaProfileExport` with sheet quality/width/height | `src/components/workspace/editor-workspace.tsx:5494-5521` | P07 |
| Codec export backend: Temporal -> `exportMediaProfile` with explicit input; else desktop process only if available && quality available && canExportMediaProfiles | `src/components/workspace/editor-workspace.tsx:5524-5548` | P07 |
| Temporal mode = `generationExecutionBackend === "temporal"` | `src/components/workspace/editor-workspace.tsx:5550-5552` | P07 |
| Render review status: exporting "running", failed "failed", cancelled "cancelled", report present "completed", else "empty" | `src/components/workspace/editor-workspace.tsx:5991-6000` | P07 |
| Render review panel: message role alert when failed; retry = default webm draft; cancel; run preview review; artifact/log evidence message "{Artifact\|Log}: {path}" | `src/components/workspace/editor-workspace.tsx:6352-6380,6183-6186` | P07 |
| Header "Render status" button opens export sheet with intent "draft-review", shows render review status | `src/components/workspace/editor-workspace.tsx:7037-7056` | P07 |
| "Export" button opens export sheet intent "destination"; "More export options" toggles menu | `src/components/workspace/editor-workspace.tsx:7117-7145` | P07 |
| Export menu copy: "Export timeline" / "Split projects enable NLE XML, project package, and media exports."; items "Premiere XML" ("XMEML interchange from canonical timeline"), "DaVinci XML" ("FCPXML interchange for Resolve"), "Palmier Project" ("Self-contained project package with media") | `src/components/workspace/editor-workspace.tsx:7161-7220` | P07 |
| Premiere/DaVinci disabled when `!canExportNleXml \|\| exporting`; Palmier disabled when `!canExportMediaProfiles \|\| exporting` | `src/components/workspace/editor-workspace.tsx:7174,7192,7210` | P07 |
| Menu codec items (mp4H264, mp4H265, proResMov) disabled when exporting \|\| !available \|\| !canExportMediaProfiles; subtitle `exportProfileDisabledReason(profile, canExport, temporal)` + runtime detail; lock icon if unavailable; click -> quality "final" at project dimensions | `src/components/workspace/editor-workspace.tsx:7221-7269` | P07 |
| Warning when !canExportNleXml: "Save as a schema-v2 split project to enable Premiere, DaVinci, and media exports." | `src/components/workspace/editor-workspace.tsx:7270-7274` | P07 |
| Export status in menu: role alert when failed; spinner while exporting; path shown when present | `src/components/workspace/editor-workspace.tsx:7275-7292` | P07 |
| ExportSheet: timeline name = active timeline name ?? "Timeline 1"; busy = exporting; error only when failed; statusMessage otherwise; `missingMediaCount={0}` hardcoded; onCancel only when activeRenderJobId; return focus to Render status button for "draft-review" else Export button | `src/components/workspace/editor-workspace.tsx:8557-8582` | P07 |
| Render completion notification rendered as sr-only status | `src/components/workspace/editor-workspace.tsx:7460-7468` | P07 |
| Preview render draft / cancel wired to default webm draft / cancelRender; timeline save range -> `saveTimelineRangeAsMedia` | `src/components/workspace/editor-workspace.tsx:8238-8239,8388` | P07 |
| "Save Range as Media" context item (range menu only) → onSaveRangeAsMedia(selectedTimelineRange); disabled without range or handler | `src/components/workspace/timeline-editor.tsx:3235-3240,3846-3848,5230-5240` | P07 |
| Destinations "Video" (video), "Timeline" (timeline), "Palmier Project" (palmier-project); radio disabled if no profile has that destination; default destination "video" | `src/components/workspace/export-sheet.tsx:56-63,109,321-334` | P07 |
| Defaults: quality "draft", resolutionOverridden false, resolution = `defaultResolutionForQuality("draft", {w,h}, false, "match-timeline")`; capabilityStatus default "ready", busy false, missingMediaCount 0 | `src/components/workspace/export-sheet.tsx:92-98,113-122` | P07 |
| Entry intent "draft-review" on open: force destination video, quality draft, un-override resolution → match-timeline default, select `firstDraftVideoProfile(profiles)` for video if any | `src/components/workspace/export-sheet.tsx:123-147` | P07 |
| Selected profile per destination remembered; falls back to `firstProfile(profiles, destination)` | `src/components/workspace/export-sheet.tsx:149-155,214-219` | P07 |
| Codec select lists one profile per unique codec within destination; option disabled if !available, or capability-checked profile while capabilityStatus !== "ready", or no quality available | `src/components/workspace/export-sheet.tsx:160-163,339-365` | P07 |
| Changing codec: keep current quality if available, else "final" if available, else "draft" if available, else unchanged; then recompute resolution via defaultResolutionForQuality(nextQuality, dims, overridden, current) | `src/components/workspace/export-sheet.tsx:221-240` | P07 |
| Quality options "Draft"/"Final", each disabled if capability blocked or profile quality unavailable; change recomputes resolution (respecting override) | `src/components/workspace/export-sheet.tsx:242-252,367-391` | P07 |
| Manual resolution choice sets resolutionOverridden = true; options from lib `resolutionOptions`; "Frame Rate" row read-only "{fps} fps" | `src/components/workspace/export-sheet.tsx:392-409` | P07 |
| capabilitySelectionBlocked = isCapabilityCheckedVideoProfile(selected video profile) && capabilityStatus !== "ready"; qualityAvailable = !blocked && availability[quality] | `src/components/workspace/export-sheet.tsx:164-169` | P07 |
| canSubmit: video → profile.available && qualityAvailable; other destinations → selectedProfile.available | `src/components/workspace/export-sheet.tsx:172-175` | P07 |
| Unavailable quality messages (only when capabilityStatus "ready"): "{profile label} Draft: {reason}" / "{profile label} Final: {reason}" for unavailable qualities with a reason; list aria "Unavailable export profiles" | `src/components/workspace/export-sheet.tsx:176-188,477-483` | P07 |
| Capability copy (video only): checking → "Checking native export capabilities…"; unavailable → "Native export capabilities are unavailable in this app session." | `src/components/workspace/export-sheet.tsx:467-476` | P07 |
| Timeline (NLE XML) format select: all destination profiles, disabled if !available; Compatibility = id "premiereXmeml" → "Adobe Premiere Pro", else target ?? "DaVinci Resolve / Final Cut Pro"; for "davinciFcpxml" also Version (default "1.11") and Target rows | `src/components/workspace/export-sheet.tsx:413-453` | P07 |
| Palmier project: "Collect project media into a portable package"; Timeline name row; Media status "All media available" or "{n} missing media file(s)" | `src/components/workspace/export-sheet.tsx:455-465` | P07 |
| Error shown as role=alert; statusMessage shown only when no error; progress clamped to [0,1], bar shown when progress !== null (aria "Export progress") | `src/components/workspace/export-sheet.tsx:189-190,484-503` | P07 |
| Footer: timecode via formatDurationTimecode(duration,fps), formatEstimatedSize(bytes), "{w}×{h}" from dimensionsForResolution, fileType ?? "—" | `src/components/workspace/export-sheet.tsx:506-512` | P07 |
| Cancel button: busy && onCancel → "Cancel render" calls onCancel; else "Cancel" closes; disabled when busy && !onCancel | `src/components/workspace/export-sheet.tsx:513-527` | P07 |
| Export button: "Exporting" when busy else "Export"; disabled when !canSubmit \|\| busy | `src/components/workspace/export-sheet.tsx:528-535` | P07 |
| No in-process vs Temporal choice in sheet; selection payload only {destination, profileId, quality, resolution, width, height, resolutionOverridden} | `src/components/workspace/export-sheet.tsx:286-297` | P07 |
| Title "Export project"; closed sheet renders nothing | `src/components/workspace/export-sheet.tsx:212,314-316` | P07 |
| Project is "Split files" when schemaVersion >= 2 and projectDir non-blank; else "Embedded project" (inspector) / "Embedded" (context); Path row only for split | `src/components/workspace/project-timeline-inspector.tsx:164,189,219,260,293` | P07 |
| No resolution/fps/duration/name validation in this file (display-only via lib labels); Render row "`resolution` @ `frameRate`"; context Format "`res` / `fps` / `aspect`" | `src/components/workspace/project-timeline-inspector.tsx:167-169,299,313-316` | P07 |
| Latest export rows Format/Output/Job ("not recorded" when no jobId); empty "No export artifact"; Recent exports section only when non-empty | `src/components/workspace/project-timeline-inspector.tsx:465-537` | P07 |
| Latest render: Status, Output, Duration, "Video stream"/"Audio stream" "Present"\|"Missing", Log, Artifacts list or "none", preview comparison, "Check `label`" rows; empty "No render report" | `src/components/workspace/project-timeline-inspector.tsx:747-777` | P07 |
| Preview comparison: "`matched`/`total` matched" (frame.passed); failed frames list time, diff ratio, preview/rendered/diff frame paths | `src/components/workspace/project-timeline-inspector.tsx:98-122` | P07 |

## Jobs / activity

| Rule | Source | Plan |
|---|---|---|
| `recordQueuedWorkflowJob`: `recordJob` action; split project → `applyProjectActionsToSplitProjectFolder`, else `applyProjectActionToProject`; records history; merges via `mergeCodexProjectMetadata`; errors → timelinePatchError, no save-status change | `src/components/workspace/editor-workspace.tsx:2269-2297` | P07 |
| In-flight workflow start ids kept in ref + state Set (for UI "starting" indicators), always cleared in `finally` | `src/components/workspace/editor-workspace.tsx:1119-1122`, `:2383-2412` | P07 |
| Cancellable jobs: active render job id; plus (projectDir non-empty) jobs with status running/progress, kind "generate_media", `workflow.runId` starting "in-process/", `mockMode === false`, and a generated asset with same id | `src/components/workspace/editor-workspace.tsx:6002-6021` | P07 |
| Retryable jobs: status "failed" AND (generated asset with non-blank prompt OR kind includes "render" with no startRequest) | `src/components/workspace/editor-workspace.tsx:6022-6036` | P07 |
| Open activity target: first output media -> select + open "media"; else generated asset -> composer seed asset, destination "generate", composer open | `src/components/workspace/editor-workspace.tsx:6038-6057` | P07 |
| Cancel activity job: must be cancellable; active render -> cancelRender; job must be running/progress; real in-process generate: in-process cancel then provider cancel (fal.ai/replicate with cancelUrl); other jobs no-op; errors not caught | `src/components/workspace/editor-workspace.tsx:6070-6100` | P07 |
| Retry activity job: must be retryable & failed; generated asset -> `rerunGeneratedAsset`; render kind -> default webm draft | `src/components/workspace/editor-workspace.tsx:6102-6114` | P07 |
| Show output: requires outputPath; matching generated output -> select media; report id -> open render record; else message "Output: {path}" and open export sheet "destination" | `src/components/workspace/editor-workspace.tsx:6116-6136` | P07 |
| Open render record requires reportId or logPath; opens "render-review"; no reload if current report has same jobId | `src/components/workspace/editor-workspace.tsx:6138-6146` | P07 |
| Render record without projectDir: status failed/exported by job status; message evidence "{label}: {path}" or "Render report: {jobId} (full report unavailable until the project is saved)" | `src/components/workspace/editor-workspace.tsx:6147-6156` | P07 |
| Render record loaded: status by `summary.status`; message evidence, or failed -> `errors[0].message` ?? "Render {jobId} failed", else "Render report: {jobId}"; path evidence ?? summary.outputPath | `src/components/workspace/editor-workspace.tsx:6157-6172` | P07 |
| Open log: message "Log: {logPath}" then open record with evidence `{label:"Log", path}` | `src/components/workspace/editor-workspace.tsx:6175-6181` | P07 |
| Activity rail badge: failed count -> "{n} failed" (priority) else running (queued/running/progress) -> "{n} running" else none | `src/components/workspace/editor-workspace.tsx:6958-6966,7832` | P07 |
| openProjectInspectorDestination (project/activity/render-review): set override, remember for "none", open transient inspector, compact view inspector, focus inspector | `src/components/workspace/editor-workspace.tsx:5709-5720` | P07 |
| Project inspector hides workflow queue (`showWorkflowQueue={false}`); start workflow + select generated output wired; ActivityPanel start workflow wired | `src/components/workspace/editor-workspace.tsx:5921-5933,6329-6345` | P07 |
| Activity header "Activity" / "Project jobs and persisted evidence" / "{n} total"; empty state "No project activity" | `src/components/workspace/activity-panel.tsx:94-101,245` | P07 |
| Records ordered via lib `orderRecentProjectJobs(jobs, records.length)` (no truncation) and remapped by job id | `src/components/workspace/activity-panel.tsx:83-91` | P07 |
| Temporal worker preflight shown if report: "Temporal worker" status "Ready" when ready && featureEnabled else "Setup needed"; list unavailable tools with installHint ?? "Available in source-development builds only" | `src/components/workspace/activity-panel.tsx:102-131` | P07 |
| Job card: title formatJobKindTitleCase, updatedAt, "Target: {targetLabel}", status badge (completed green, failed/cancelled destructive, blocked amber, progress/running sky, queued muted) | `src/components/workspace/activity-panel.tsx:31-46,152-171` | P07 |
| Action buttons (aria "{Action} {jobId}", visible text = aria minus last word): "Open target" if targetLabel; "Open proposal" if proposalAvailable; "Cancel" if cancellableJobIds has id; "Retry" if retryableJobIds has id; "Open output" if outputPath; "Open report" if reportId; "Open log" if logPath | `src/components/workspace/activity-panel.tsx:48-66,172-221` | P07 |
| Start workflow button shown when job.startRequest && status queued or blocked; startBlocked = report present && (!ready \|\| !featureEnabled); disabled when !onStartWorkflow \|\| startBlocked \|\| pending; label "Temporal setup needed" > "Starting…" > "Start workflow" | `src/components/workspace/activity-panel.tsx:138-144,222-238` | P07 |
| Render report absent: status = prop ?? (report ? "completed" : "empty"); title running "Render running", queued "Render queued", cancelled "Render cancelled", failed "Render failed", else "No render report yet" | `src/components/workspace/render-report-panel.tsx:47-63` | P07 |
| Render report absent body copy: failed "The previous render stopped before completion. Review its failure details and retry when ready."; cancelled "The render was cancelled. Start another draft when ready."; running "The draft render is in progress."; else "Render a draft to review duration, streams, artifacts, and logs." (queued uses default copy) | `src/components/workspace/render-report-panel.tsx:64-72` | P07 |
| Absent-report actions: running → "Cancel render" (disabled without onCancelRender); otherwise "Retry render" if failed/cancelled else "Render draft" (disabled without onRetryRender) | `src/components/workspace/render-report-panel.tsx:74-96` | P07 |
| Report summary: Job id, Status, Output (outputPath ?? "No output path"), Selected quality = deliveryQualityLabel ?? renderQualityProfileLabel(selectedQuality), Command quality = deliveryQuality ?? label(commandQualityProfile(command)) ?? "No command quality", title "--quality={profile}" | `src/components/workspace/render-report-panel.tsx:28-34,101-140` | P07 |
| Streams (if report.streams): Video/Audio stream "Present"/"Missing" | `src/components/workspace/render-report-panel.tsx:142-153` | P07 |
| Failure details (errors > 0): message, "{code} · {path}", fix, details sorted by key "k=v"; "Retry render" shown when panelStatus "failed" or summary.status "failed" | `src/components/workspace/render-report-panel.tsx:155-196` | P07 |
| Graphics rows: layerId, renderer, qualityProfile ?? "none", visualQaStatus ?? "not-run"; "Frame evidence {backed}/{sampled} artifact-backed" when sampledFrames > 0; sampled frames + qaMetricEntries "metric=value" | `src/components/workspace/render-report-panel.tsx:198-258` | P07 |
| Preview comparison: status, "{passed}/{total} matched", per-frame time + formatMismatchRatio (destructive styling when !passed; pass flag from report, no local threshold), preview/rendered/diff frame paths | `src/components/workspace/render-report-panel.tsx:260-305` | P07 |
| Preview review button (only if previewComparisonRequest): "Review running" when running, "Retry review" if comparison exists, else "Run review"; disabled when !onRunPreviewReview \|\| running; "Reveal evidence" if an artifact ends with "/preview-comparison.json" | `src/components/workspace/render-report-panel.tsx:104-106,307-331` | P07 |
| Artifacts: only first 4 shown as "Open artifact" buttons; "Open log" shown if report.stderr (passes stderr to onOpenLog) | `src/components/workspace/render-report-panel.tsx:333-361` | P07 |
| Job summary "`active` active / `total` total"; context "`n` active workflows"; jobs ordered by orderRecentProjectJobs | `src/components/workspace/project-timeline-inspector.tsx:170,229-230` | P07 |
| Job status colors: completed=green; failed/cancelled=destructive; blocked=amber; progress/running=sky; queued=muted | `src/components/workspace/project-timeline-inspector.tsx:70-85` | P07 |
| Temporal worker preflight: "Ready" vs "Setup needed"; missing tools (available=false) listed with installHint ?? "Available in source-development builds only"; else "Required tools available" | `src/components/workspace/project-timeline-inspector.tsx:550-617` | P07 |
| temporalStartBlocked = report present && (!ready \|\| !featureEnabled); pending = startingWorkflowIds.has(job.id) | `src/components/workspace/project-timeline-inspector.tsx:622-626` | P07 |
| canStartWorkflow = onStartWorkflow && job.startRequest && canShowStartWorkflowAction(status) && requestStatus != "mismatch" && !blocked && !pending | `src/components/workspace/project-timeline-inspector.tsx:627-634` | P07 |
| Start request group shown when startRequest and (action visible or any of profile/format/outputPath/video validation/audio validation); rows "Profile","Format","Output","Validation","Audio"; badge = requestStatus | `src/components/workspace/project-timeline-inspector.tsx:635-697` | P07 |
| Start button rendered when onStartWorkflow && startRequest && action visible && status != "mismatch"; label "Temporal setup needed" (blocked) / "Starting..." (pending) / "Start workflow"; aria "Start workflow `id`"; click re-checks canStartWorkflow | `src/components/workspace/project-timeline-inspector.tsx:698-721` | P07 |
| Workflow queue section hidden when showWorkflowQueue=false; empty "No workflow jobs" | `src/components/workspace/project-timeline-inspector.tsx:538,728-736` | P07 |
| No-selection status "No selected timeline item" with always-disabled "Select a clip" button | `src/components/workspace/project-timeline-inspector.tsx:263-280` | P04 |

## Agent / Codex

| Rule | Source | Plan |
|---|---|---|
| Codex status values "idle" \| "running" \| "ready" \| "applying"; default "idle" ("running" in codex-active visual QA) | `src/components/workspace/editor-workspace.tsx:605`, `:882-885` | P06 |
| Undo Codex edit available only when last agent history entry exists and `projectSnapshotsEqual(project, entry.after)` | `src/components/workspace/editor-workspace.tsx:1453-1456` | P06 |
| Proposal review timeline context: active timeline id/name (fallback "Timeline") and tracks id/name/kind/locked; `transcriptReady` = transcript for proposal media exists OR `hasExistingTranscript`; prompt default "", target duration default null | `src/components/workspace/editor-workspace.tsx:1467-1494` | P06 |
| Agent timeline clip context priority: source clip ?? text overlay ?? caption ?? template; range context from selected timeline range | `src/components/workspace/editor-workspace.tsx:1725-1742` | P06 |
| Chat history scope key = "`<projectDir.trim()>`::`<project.id>`" or just project.id without projectDir | `src/components/workspace/editor-workspace.tsx:1743-1744` | P06 |
| App-server conversation history loaded only for schemaVersion >= 2 + projectDir (else []); keeps current entries when both loaded and current are empty; on error clears non-empty entries; non-backend-unavailable errors warn | `src/components/workspace/editor-workspace.tsx:2033-2066` | P06 |
| Agent session manifest loaded only for split project (else null); `updateAgentSession` no-op without split project; failures warn "Failed to update project agent session" (backend-unavailable silent) | `src/components/workspace/editor-workspace.tsx:2068-2091` | P06 |
| Undo Codex edit when project changed since: error "Undo Codex edit is unavailable after another project edit." | `src/components/workspace/editor-workspace.tsx:2503-2512` | P06 |
| Copy agent setup writes `agentSetupSnippet(client, projectDir)` to clipboard | `src/components/workspace/editor-workspace.tsx:2554-2556` | P06 |
| Project skills panel: skills "video-creater-video-pipeline", "video-creater-graphics", "video-creater-visuals" with `.agents/skills/<name>/SKILL.md` paths and focus copy; "Copy skill paths" copies "`<name>`: `<path>`" lines | `src/components/workspace/editor-workspace.tsx:516-570` | P06 |
| Draft agent prompt: trimmed empty -> no-op; seed id = previous id + 1 (start 1); opens Codex | `src/components/workspace/editor-workspace.tsx:4544-4554` | P06 |
| Codex edit uses epoch ref; any stale epoch after each await aborts silently | `src/components/workspace/editor-workspace.tsx:4927-4999` | P06 |
| Codex edit: pre-analysis only for schemaVersion >= 2 && projectDir; analysis failure only `console.warn("Failed to analyze media before edit generation")` | `src/components/workspace/editor-workspace.tsx:4936-4954` | P06 |
| Codex errors: "Codex edit workflow job could not be queued."; "Codex did not return a structured edit proposal."; statuses running -> ready / idle | `src/components/workspace/editor-workspace.tsx:4985-5015` | P06 |
| Cancel Codex edit bumps epoch, status idle; cancel error shown only if no newer epoch | `src/components/workspace/editor-workspace.tsx:5018-5032` | P06 |
| Apply proposal: blocked when `!reviewModel.canApply` -> status "ready", "Codex proposal review is blocked by invalid EDL or visual-layer metadata." | `src/components/workspace/editor-workspace.tsx:5034-5042` | P06 |
| Apply proposal: status "applying"; materialize error -> ready + message; rejected -> "Codex proposal actions were rejected."; success pushes agent history `{before, after}` snapshots, clears proposal, status idle | `src/components/workspace/editor-workspace.tsx:5044-5071` | P06 |
| Reject and Revise proposal both: clear proposal + validation issues, status "idle", error null | `src/components/workspace/editor-workspace.tsx:7994-8005` | P06 |
| Open proposal from activity: set proposal, issues null, status "ready", error null, open Codex, focus "codex" pane | `src/components/workspace/editor-workspace.tsx:6059-6068` | P06 |
| updateCodexDisclosure: clears transient compact view; single-pane + opening -> compact view "codex"; else "openCodexRail"/"toggleCodexRail" | `src/components/workspace/editor-workspace.tsx:6453-6467` | P06 |
| Agent sessions: create `{id:"session-"+Date.now(), title:"New chat", threadId:null}`; rename via prompt "Rename Codex session" (default current title ?? "Chat"), trimmed non-empty; restore last deleted session; delete/select wired with timestamps | `src/components/workspace/editor-workspace.tsx:7915-7932` | P06 |
| AgentPanel remounts per chat history scope; mediaId fallback "media-1" | `src/components/workspace/editor-workspace.tsx:7907-7909` | P06 |
| Transcription notice when no transcript && model not ready && onOpenSettings: "Edit generation needs an installed transcription model." / "Configure transcription models in Settings" (target aiModels/transcription) | `src/components/workspace/editor-workspace.tsx:7888-7897` | P06 |
| Agent selected-clip actions wired: insert media, trim, split, remove, reorder, audio fade-out/volume, opacity, text overlay text, caption text, template field/timing/style/metadata, track lock/enabled, variation/replacement variation/variation set, generation (placement "timeline"), upscale, open source/timeline reference | `src/components/workspace/editor-workspace.tsx:7962-7992` | P06 |
| Agent open model settings: with onOpenSettings requires focused HTMLElement as origin (else no-op); else passes `onOpenModelSettings` | `src/components/workspace/editor-workspace.tsx:8010-8028` | P06 |
| Add timeline range to chat: set range, clear item selection, open Codex | `src/components/workspace/editor-workspace.tsx:8383-8387` | P06 |
| Compact "Codex" view button closes source panel and opens Codex | `src/components/workspace/editor-workspace.tsx:7809-7824` | P06 |
| MCP panel copy: "Connect your agent"; runtime status "ready"/"setup needed"/"unchecked"; endpoint/Web UI "Not checked" fallback; tools "{name} ready\|missing"; folder fallback "project manifest"; files "video-creater.project.json, timeline.json, media/index.json, templates"; copy setup targets codex/claudeCode/claudeDesktop/cursor | `src/components/workspace/editor-workspace.tsx:7590-7743` | P06 |
| Range → chat: toolbar "Add" (aria "Add range to Codex") and context "Add Range to Chat" call onAddRangeToChat(selectedTimelineRange) | `src/components/workspace/timeline-editor.tsx:3228-3233,4038-4047,5219-5229` | P06 |
| "Organize with Agent" menu item disabled when `!onOrganizeWithAgent`; sends `organizeMediaPrompt` (src/lib/generation/provider-rules) | `src/components/workspace/media-bin.tsx:3920-3932` | P06 |
| Starter action chips (in order) "Generate an AI video", "Generate B-roll", "Create a letterbox opening", "Add captions to my timeline", "Create a voiceover", "Generate music and sync to my timeline", "Organize my media into structured folders"; clicking sets prompt = label, clears dismissed mention, mention index 0 | `src/components/workspace/agent-panel.tsx:202-210,2323-2342` | P06 |
| Starter actions shown only when no app-server entries AND no local transcript entries AND no latest request shown; header copy "Ask anything, or start with:" | `src/components/workspace/agent-panel.tsx:2179-2182,2320` | P06 |
| Tool-call row: toolName defaults "tool", status defaults "queued"; label via `codexToolDisplayName`; aria-label "Codex tool call {toolName} {status}"; details toggle "Show/Hide {toolName} tool call details" with Detail / Target(meta) / Status rows | `src/components/workspace/agent-panel.tsx:212-299` | P06 |
| Component defaults: mediaId "media-1", transcriptionModelReady true, runtimeReady true, codexStatus "idle", canUndoAgentEdit false, preset fixed "trailer_cut", language fixed "en"; initial prompt = `getEditPresetOption("trailer_cut").defaultPrompt` | `src/components/workspace/agent-panel.tsx:301-371` | P06 |
| Transcript entries loaded from `loadStoredTranscriptEntries(scopedCodexChatTranscriptStorageKey(chatHistoryScope))` and saved on every change | `src/components/workspace/agent-panel.tsx:370-381` | P06 |
| draftPromptSeed (keyed by id+prompt) replaces prompt, clears dismissed mention, resets mention index | `src/components/workspace/agent-panel.tsx:383-390` | P06 |
| New chat ("Start new Codex chat"): clears prompt to "", clears transcript + storage, clears local request key, resets mention state, then calls onCreateAgentSession | `src/components/workspace/agent-panel.tsx:396-403,2255-2257` | P06 |
| Session select shown only if agentSessions.length > 0 (value = activeAgentSessionId ?? first session id), else label "New chat" | `src/components/workspace/agent-panel.tsx:2224-2235` | P06 |
| "Rename active Codex session" / "Delete active Codex session" disabled when no activeAgentSessionId or no handler | `src/components/workspace/agent-panel.tsx:2243-2272` | P06 |
| "Restore last deleted Codex session" disabled when deletedAgentSessionCount === 0 or no handler | `src/components/workspace/agent-panel.tsx:2278-2281` | P06 |
| History indicator label: app-server entries > 0 → "Codex app-server history saved"; else transcript > 0 → "Codex chat history saved locally"; else "No Codex chat history saved" (disabled when both empty) | `src/components/workspace/agent-panel.tsx:2290-2304` | P06 |
| Canned intro copy "I can see the project media, timeline, templates, and render settings." | `src/components/workspace/agent-panel.tsx:2354` | P06 |
| Default context rows (only when transcript empty and no latest request): list_models "3 model families ready"; get_timeline = proposal clips > 0 ? "{n} proposal clips" : "timeline ready"; project_context = clip → "clip {itemId}", range → "range {start}-{end}" (toFixed(2)+"s"), else "context @{mediaId}" | `src/components/workspace/agent-panel.tsx:1941-1944,2168-2208` | P06 |
| App-server conversation shows only last 3 entries; aria "Codex app-server turn {turnStatus ?? "recorded"}"; title "App-server turn"; status badge turnStatus ?? "saved"; meta "{threadId}[ / {turnId}][ / proposal]" | `src/components/workspace/agent-panel.tsx:2368-2390` | P06 |
| Latest request bubble shown when latestRequest exists and its key (`mediaId prompt`) differs from local generate key; shows "Target @{mediaId}" | `src/components/workspace/agent-panel.tsx:1736-1738,2175-2178,2419-2431` | P06 |
| Latest request footer: "Last request: {preset label}, {language label ?? languageMode}, with prompt saved for generation." | `src/components/workspace/agent-panel.tsx:2512-2521` | P06 |
| Status phase copy: error → "Codex hit an error: {displayed error}"; running → "Codex is generating a timeline edit from this prompt."; ready → "Codex proposal ready: {n} clip(s), {m} layer(s)." where layers = captions+overlays+hyperframes+gpuVisuals; applying → "Codex is applying validated project actions to the timeline."; idle → "Codex is ready to generate a timeline edit." | `src/components/workspace/agent-panel.tsx:1484-1508` | P06 |
| Error text rewrite: if codexError matches `/hyperframes?\[\d+\]\|hyperframe/i` → "HyperFrame layer is not renderable yet. Use supported kinds: title_card, diagram, transition, immersive_scene, or template_overlay."; shown in role=alert box | `src/components/workspace/agent-panel.tsx:1472-1482,2522-2526` | P06 |
| "Undo Codex edit" button rendered only if onUndoAgentEdit provided; disabled when !canUndoAgentEdit | `src/components/workspace/agent-panel.tsx:2527-2541` | P06 |
| Proposal review (CodexProposalReview) rendered only when codexStatus is "ready" or "applying" AND proposal AND proposalTimeline; status prop "applying" if applying else "ready"; revise/reject/apply fallback to no-op | `src/components/workspace/agent-panel.tsx:2542-2557` | P06 |
| No auto-apply in panel: proposal only applied via onApplyProposal from review component | `src/components/workspace/agent-panel.tsx:2555` | P06 |
| Suggested template card shown only when latestRequest AND suggestedTemplateName: "Suggested template" / "Add a motion template beat to the overlay track for this draft." / button "Add {suggestedTemplateName}" | `src/components/workspace/agent-panel.tsx:2558-2576` | P06 |
| Composer: label "Prompt", chip "@{effectiveMediaId}", placeholder "Ask Codex, or type @ to reference media", rows 4; hint "Mention @{mediaId} in the prompt to anchor this edit." | `src/components/workspace/agent-panel.tsx:2584-2605,2648-2650` | P06 |
| Typing clears dismissed mention key | `src/components/workspace/agent-panel.tsx:2597-2600` | P06 |
| Mention resolution: first `@([A-Za-z0-9][A-Za-z0-9_-]*)` whose id is a known mention target wins; effectiveMediaId = mention ?? mediaId; mentionOverridesTarget only when resolved id !== mediaId | `src/components/workspace/agent-panel.tsx:841-855,1905-1906` | P06 |
| Active mention query = trailing `(^\|[\s(])@([A-Za-z0-9_-]*)$`; suggestions filter mediaId/label/kind/description/searchTerms (lowercase substring), max 5; hidden if dismissed key "{startIndex}:{query}" matches | `src/components/workspace/agent-panel.tsx:857-882,1886-1894` | P06 |
| Mention keys (only when suggestions exist): ArrowDown/ArrowUp cycle (wrap), Escape dismisses current query, Enter or Tab inserts active suggestion (all preventDefault); index reset to 0 when query key or suggestion count changes | `src/components/workspace/agent-panel.tsx:1435-1466,1933-1935` | P06 |
| insertMention: if active query → replace from startIndex with "@{id} "; else append " @{id} " to trimEnd'd prompt; reset index/dismissed | `src/components/workspace/agent-panel.tsx:1421-1433` | P06 |
| Mention option DOM id "codex-mention-option-{id with non [A-Za-z0-9_-] → _}"; listbox "Codex mention suggestions" with aria-activedescendant | `src/components/workspace/agent-panel.tsx:1417-1419,2606-2647` | P06 |
| Range context chip "Timeline range {start}-{end}" (toFixed(2)s) shown when selectedTimelineRangeContext | `src/components/workspace/agent-panel.tsx:2651-2666` | P06 |
| "Resolved mention" chip shown when mention overrides target: "@{id} - {label}" + description | `src/components/workspace/agent-panel.tsx:2667-2680` | P06 |
| "Queue referenced shot from mention" shown when mention overrides, kind !== "audio", onQueueSelectedGeneration; no-op if prompt empty; transcript "Queued a referenced shot from {label}.", tool detail "referenced shot" | `src/components/workspace/agent-panel.tsx:1907-1912,2681-2702` | P06 |
| Mention insertion: shown when mention overrides + target.canInsertOnTimeline + onInsertSelectedMedia; becomes primary action if prompt is insertion prompt, else inline "Insert mention on timeline" button | `src/components/workspace/agent-panel.tsx:1913-1930,2703-2715` | P06 |
| Insertion-prompt detection: requires verb `\b(add\|drop\|insert\|place\|put)\b` AND context `\b(after\|before\|beginning\|clip\|current shot\|cut\|edit\|end\|sequence\|shot\|start\|timeline)\b` | `src/components/workspace/agent-panel.tsx:884-897` | P06 |
| Mention insert transcript: user = prompt \|\| "Insert @{id} on the timeline."; codex "Inserted {label} on the timeline."; meta "@{id}"; detail "timeline insert"; tool "project_action" | `src/components/workspace/agent-panel.tsx:598-607` | P06 |
| "Cancel generation" button shown only when codexStatus === "running" and onCancelGenerate | `src/components/workspace/agent-panel.tsx:2718-2722` | P06 |
| canGenerateEdit = hasExistingTranscript \|\| (transcriptionModelReady && runtimeReady) | `src/components/workspace/agent-panel.tsx:1861` | P06 |
| Primary button shown if canGenerateEdit or any prompt quick-action applies; disabled when prompt.trim() empty or running; label "Generating..." while running | `src/components/workspace/agent-panel.tsx:2723-2746` | P06 |
| Primary label precedence: "Insert mention on timeline" > "Set selected caption text" > "Set selected overlay text" > "Set selected template field" > "Set selected template timing" > "Set selected template style" > "Set selected template metadata" > "{Lock\|Unlock\|Mute\|Unmute\|Show\|Hide} selected track" > "Set selected clip opacity" > "Set selected clip fade" > "Set selected clip volume" > "Delete selected clip" > "Split selected clip" > "Trim selected clip" > "Move selected clip {earlier\|later}" > "Queue selected clip variation" > "Queue variation set" > "Generate edit" (NOTE: differs from handler dispatch order) | `src/components/workspace/agent-panel.tsx:2746-2783` | P06 |
| Blocked state (no action possible): message transcriptionModelReady ? "Transcription runtime unavailable." : "Local transcription model required."; "Open settings" (aria "Open model settings") → onOpenModelSettings; disabled "Generate edit" with title "Check the local transcription runtime before generating edits from source media." / "Install or select a local transcription model before generating edits from source media." | `src/components/workspace/agent-panel.tsx:1864-1869,2785-2809` | P06 |
| All selected-clip quick actions (except track state) require !selectedTimelineTrackLocked (trackLocked default false) | `src/components/workspace/agent-panel.tsx:2028-2118` | P06 |
| Split prompt: requires `\bsplit\b` and `at N [s\|sec\|secs\|second\|seconds]`; enabled only if start < N < end of selected clip; transcript "Split {label} at {N}s." detail "split clip at {N}s" | `src/components/workspace/agent-panel.tsx:1218-1239,2028-2035,826-839` | P06 |
| Trim prompt: requires `\btrim\b` + `(selected clip\|clip)`; range "from A to B" or "to A-B" (-, en dash, em dash); values rounded via roundTimelineSeconds | `src/components/workspace/agent-panel.tsx:1241-1280` | P06 |
| Trim enable: start >= clipStart, end <= clipEnd, duration > 0; if clip has sourceIn/sourceOut then newSourceIn = sourceIn + (start - clipStart) >= 0 and newSourceOut = sourceOut - (clipEnd - end) > newSourceIn | `src/components/workspace/agent-panel.tsx:2003-2050` | P06 |
| Trim update = {startSeconds, durationSeconds=end-start (rounded), sourceIn/sourceOut only if source range}; transcript "Trimmed {label} to {A}s-{B}s." detail "trim clip to {A}s-{B}s" | `src/components/workspace/agent-panel.tsx:795-824` | P06 |
| Delete prompt: `\b(delete\|remove)\b` + `(selected clip\|this clip\|clip)`; transcript user fallback "Delete {label}.", codex "Deleted {label}.", detail "delete clip" | `src/components/workspace/agent-panel.tsx:1365-1371,2051-2056,782-793` | P06 |
| Volume prompt: `(volume\|gain)` + clip phrase; "volume ... to/at N dB" or "N dB ... volume"; allowed only for kind "audio_clip" and -60 <= dB <= 24; label "{N}dB"; detail "set volume to {N}dB" | `src/components/workspace/agent-panel.tsx:1282-1309,2057-2064,767-780` | P06 |
| Fade prompt: `fade\s*out` + clip phrase; "fade out ... to/at N [s]"; only "audio_clip" and N >= 0 (no max); label "{N}s"; detail "set fade out to {N}s" | `src/components/workspace/agent-panel.tsx:1311-1337,2065-2071,752-765` | P06 |
| Opacity prompt: `opacity` + clip phrase; "opacity ... to/at N" or "N opacity"; allowed when isVisualOpacityClip and 0 <= N <= 1; detail "set opacity to {N}" | `src/components/workspace/agent-panel.tsx:1339-1363,2072-2079,737-750` | P06 |
| Overlay text prompt: needs `(text\|copy)` + `(… overlay)`; value from `to "…"`/`to '…'`/`` to `…` `` else `to (.+)$`, trailing .!? stripped; allowed when kind "overlay" with text defined; detail "set overlay text"; message `Set {label} text to "{text}".` | `src/components/workspace/agent-panel.tsx:961-981,2080-2086,609-621` | P06 |
| Caption text prompt: same parsing with `(… caption)`; allowed when kind "caption" with text defined; detail "set caption text" | `src/components/workspace/agent-panel.tsx:983-1003,2087-2093,623-635` | P06 |
| Template field prompt: requires templateFields non-empty + "template" word; value after "to"; field matched by longest alias (field name or templateFieldLabels, normalized to lowercase alnum-space) as whole-word in prefix before "to"; message `Set {label} {fieldName} to "{value}".`, detail "set template field" | `src/components/workspace/agent-panel.tsx:1005-1064,2094-2100,637-650` | P06 |
| Template timing prompt: requires templateId, "template", and verb `(start\|starts\|begin\|begins\|duration\|length\|timing\|move\|place\|put)`; start from "start/begin [at\|to] N", "timing at/to N", or "move/place/put … at/to N"; duration from "duration/length [at\|to\|of] N" or "for N"; valid start >= 0, duration > 0; at least one | `src/components/workspace/agent-panel.tsx:1066-1130,2101-2106` | P06 |
| Template timing transcript: "Set {label} timing to start {s}s, duration {d}s." detail "set template timing to …" | `src/components/workspace/agent-panel.tsx:652-673` | P06 |
| Template style prompt: requires templateId + "template"; key from "accent color"→accentColor, "background color"→backgroundColor, "text color"→textColor (first match); value after "to"; detail "set template style {key}" | `src/components/workspace/agent-panel.tsx:1132-1172,675-687` | P06 |
| Template metadata prompt: key "visual treatment"→visualTreatment, "motion"→motion, "safe zone"→safeZone, "avoid"→avoid (first match); detail "set template metadata {key}" | `src/components/workspace/agent-panel.tsx:1174-1216,689-701` | P06 |
| Track state prompt: requires `(selected track\|current track\|this track\|track)`; precedence unlock→locked=false "Unlock"; lock→locked=true "Lock"; (audio_clip only) unmute→enabled=true "Unmute", mute→enabled=false "Mute"; show/enable/unhide→enabled=true "Show"; hide/disable→enabled=false "Hide"; toolLabel "{verb} track" lowercase | `src/components/workspace/agent-panel.tsx:899-959` | P06 |
| Track state allowed when trackId present and matching handler (locked→onSetSelectedTimelineTrackLocked else Enabled); NOT gated by track lock; transcript "{Label} {trackName ?? "selected track"}." meta "track {id}" | `src/components/workspace/agent-panel.tsx:2119-2125,703-720` | P06 |
| Reorder prompt: `(move\|reorder)` + `(selected clip\|clip)` + "earlier"/"later"; earlier allowed if selectedIndex > 0, later if selectedIndex < itemIds.length-1; update via lib `timelineClipReorderUpdate`; transcript "Moved {label} {dir}." detail "move clip {dir}" | `src/components/workspace/agent-panel.tsx:1400-1415,2126-2145,722-735` | P06 |
| Timeline generated clip variation prompt: phrase `(selected generated clip\|generated clip\|ai clip\|selected clip\|this clip)` + verb `(tweak\|change\|adjust\|vary\|variation\|make\|warmer\|softer\|brighter\|darker\|cinematic)`; requires clip generatedAssetId + onQueueSelectedVariation; transcript "Queued a variation for {label}." meta "[@sourceMediaId ]clip {itemId}" detail "variation" | `src/components/workspace/agent-panel.tsx:1385-1398,2162-2167,1817-1842,1721-1734` | P06 |
| Source variation set prompt: verb `(create\|generate\|make\|queue)` + `(variation\|variations)` + `(four\|4\|multiple\|several\|set)`; requires selectedMediaContext.generatedAssetId + handler; drafts = lib `defaultVariationDrafts(prompt)`; transcript "Queued {n} variations for {label}." followup "Directions: {names}." detail "variation set" | `src/components/workspace/agent-panel.tsx:1373-1383,1922-1927,1795-1815,1711-1719` | P06 |
| Generation tool name by kind: image/image_clip→generate_image; video/video_clip→generate_video; audio/audio_clip→generate_audio; "generated" by label ext png/jpg/jpeg/webp/gif/avif→image, wav/mp3/m4a/aac/flac/ogg→audio, mp4/mov/m4v/webm→video; else generate_media | `src/components/workspace/agent-panel.tsx:1623-1652` | P06 |
| Queue transcript template: user entry, then tool list_models "3 model families ready" (loaded), then tool {toolName default "generate_media"} with detail (queued), then codex message, optional codex followup; ids "queue-user-{i}", "queue-tool-models-{i+1}", "queue-tool-{i+2}", "queue-codex-{i+3}", "queue-codex-followup-{i+4}" | `src/components/workspace/agent-panel.tsx:1654-1709` | P06 |
| Generate edit transcript: user (meta "@{mediaId}"), tools list_models "3 model families ready", get_timeline "timeline context", video_creater.inspect_timeline "active timeline window", project_context "project context" (all loaded), codex "Sent an EDL-first {preset label} request." | `src/components/workspace/agent-panel.tsx:1740-1793` | P06 |
| Workflow activity (in Codex rail) shown when ordered jobs non-empty (lib `orderRecentProjectJobs` default limit); summary "1 active workflow" / "{n} active workflows" counting blocked/progress/queued/running | `src/components/workspace/agent-panel.tsx:1931,1936-1940,2438-2452` | P06 |
| Workflow job labels from startRequest.input: model "{provider}/{id}" or whichever non-empty; prompt trimmed if non-empty; refs "Refs First {id} / Last {id} / Reference(s) {ids}"; settings "Settings {WxH rounded} / {dur int or toFixed(1)}s / {fps int or toFixed(2)} fps / {aspectRatio}" (each part only if finite > 0 / non-empty) | `src/components/workspace/agent-panel.tsx:1525-1621` | P06 |
| Job status badge color: completed green; failed/cancelled destructive; blocked amber; progress/running sky; queued muted | `src/components/workspace/agent-panel.tsx:1844-1859` | P06 |
| Dead code: computed but not rendered — selectedReferenceLabel "refs {ids}", selectedClipRangeLabel, "duration {s}", "source {in}-{out}", track media state audible/muted/visible/hidden, "1 ref"/"{n} refs", copyTextToClipboard; props onQueueSelectedReplacementVariation/onQueueSelectedUpscale/onOpenSelected*Reference unused | `src/components/workspace/agent-panel.tsx:392-394,1870-1882,1945-1974,2146-2161` | P06 |

## Undo/redo & persistence

| Rule | Source | Plan |
|---|---|---|
| Default project dir "/tmp/video-creater-editor-project"; project = `initialProject ?? createBundledSampleProject()` | `src/components/workspace/editor-workspace.tsx:463`, `:886-888` | P07 |
| Save status values "saved" \| "saving" \| "unsaved" \| "failed"; default "saved" | `src/components/workspace/editor-workspace.tsx:606`, `:953` | P07 |
| New `initialProject` reference replaces project, clears prepared preview and canonical preparation (history not cleared) | `src/components/workspace/editor-workspace.tsx:1180-1189` | P07 |
| History: every recorded edit appends `projectHistorySnapshot(previous)` to past and clears future; NO size limit and NO coalescing | `src/components/workspace/editor-workspace.tsx:2113-2118` | P07 |
| Split-project path condition everywhere: `schemaVersion >= 2 && projectDir.trim().length > 0` | `src/components/workspace/editor-workspace.tsx:2122`, `:2150`, `:2232` | P07 |
| Single action: split → "saving" then "saved"; in-memory → "unsaved"; error falls back to `applyProjectActionLocally` only if schema < 2 or backend unavailable and result changed ("unsaved"); otherwise "failed" + error message | `src/components/workspace/editor-workspace.tsx:2143-2187` | P07 |
| Batch actions: split → one backend call, one history entry; in-memory → sequential per-action applies without history then one history entry; error fallback `applyProjectActionsLocally` only when backend unavailable | `src/components/workspace/editor-workspace.tsx:2226-2267` | P07 |
| Undo/redo persist restored snapshot with `saveSplitProjectToFolder{expectedRevision: project.contentRevision ?? 0}` on split projects; in-memory returns snapshot as-is | `src/components/workspace/editor-workspace.tsx:2466-2479` | P07 |
| Undo no-op if past empty; redo no-op if future empty; errors → timelinePatchError, history unchanged | `src/components/workspace/editor-workspace.tsx:2481-2552` | P07 |
| Undo Codex edit also pops last past entry and pushes current snapshot to future | `src/components/workspace/editor-workspace.tsx:2521-2526` | P06 |
| Layout (`saveWorkspaceLayoutState`) and responsive rail state (`saveResponsiveRailState`) persisted to localStorage on every change; localStorage access errors → null storage | `src/components/workspace/editor-workspace.tsx:809-818`, `:1248-1254` | P07 |
| Header title appends " - Edited" when save status "unsaved"; "Saving" (status) / "Save failed" (alert) for other non-saved states | `src/components/workspace/editor-workspace.tsx:7014-7031` | P07 |
| Mock generation completion and speech refresh record project history before replacing project | `src/components/workspace/editor-workspace.tsx:4605,5611` | P07 |
| Editor tour skip persists `saveEditorTourDismissed()`; tour menu item hidden once dismissed | `src/components/workspace/editor-workspace.tsx:7384,7580-7588` | P07 |

## Native menu / shortcuts

| Rule | Source | Plan |
|---|---|---|
| Native menu requests handled only when `isActive`: openTour, openShortcuts, openMcp, openSkills open panels; importMedia → `importMediaFromPicker()`; exportProject → opens export menu | `src/components/workspace/editor-workspace.tsx:1256-1278` | P07 |
| toggleMedia: source panel open, transient closed, `togglePane "media"`; toggleInspector: transient closed, `togglePane "inspector"`; toggleCodex → `updateCodexDisclosure("toggle")`; toggleMaximize; layoutDefault/layoutMedia/layoutVertical → `setPreset` "default"/"media"/"vertical" | `src/components/workspace/editor-workspace.tsx:1279-1302` | P07 |
| sendFeedback opens "mailto:feedback@video-creater.local" in "_self" | `src/components/workspace/editor-workspace.tsx:1303-1305` | P07 |
| Native menu state: view "editor"; canImport = projectDir non-empty; canExport = any track has items; canMaximize true; canSelectForward/canDelete/canRippleDelete = any selected item on unlocked track; canSplit = that AND (exactly 1 selected OR playhead strictly inside an unlocked selected item); canTrimStart/End = playhead strictly inside unlocked selected item; plus media/inspector/codex visibility, maximized, layoutPreset | `src/components/workspace/editor-workspace.tsx:1311-1358` | P07 |
| Header menus: open focuses first enabled `button[role="menuitem"]` next frame; Escape closes and refocuses trigger; ArrowDown/ArrowUp wrap; Home/End first/last | `src/components/workspace/editor-workspace.tsx:755-797`, `:1079-1093` | P07 |
| Header menu portal anchoring: top = trigger bottom + 8px, right = max(8, viewport - trigger right) | `src/components/workspace/editor-workspace.tsx:469-476` | P07 |
| Pane resize separator: ArrowLeft/ArrowRight change width by 16px (direction inverted for left-side handle); aria min/max from `workspacePaneWidthBounds` | `src/components/workspace/editor-workspace.tsx:660-681` | P07 |
| Timeline deck resize: ArrowUp/ArrowDown ±0.02 ratio (Shift ±0.08); Home → max, End → min; drag ratio delta = dy / upper-deck height (fallback window height, then 800) | `src/components/workspace/editor-workspace.tsx:692-751` | P03 |
| Editor tour: steps "Import media", "Generate media", "Edit timeline", "Ask Codex", "Render and export" with body copy; numbered "01".."05"; "Skip tour" button; dismissal stored as "true" in "video-creater.editorTour.dismissed.v1" | `src/components/workspace/editor-workspace.tsx:493-514`, `:572-601`, `:608`, `:820-826` | P07 |
| Shortcut sheet Timeline: V "Select tool"; C "Razor tool"; S "Split selected clip"; {mod} K "Split at playhead"; Shift Drag "Select timeline range"; Drag Edge "Adjust timeline range"; I "Mark range start"; O "Mark range end"; Delete "Remove selected clip"; Shift Del "Ripple delete selected clip"; {mod} D "Duplicate selected clip"; {mod} C "Copy selected clip"; {mod} X "Cut selected clip"; {mod} V "Paste copied clip with overwrite"; Shift {mod} V "Insert copied clip and ripple later clips"; Esc "Deselect and reset tool" ({mod} = host primaryModifier) | `src/components/workspace/editor-workspace.tsx:7469-7543` | P07 |
| Shortcut sheet Source trim: I "Set source in point"; O "Set source out point"; Enter "Insert selected source"; Preview: Space "Play or pause source preview"; Left "Step preview backward"; Right "Step preview forward" | `src/components/workspace/editor-workspace.tsx:7544-7577` | P07 |
| Profile menu panels (Keyboard shortcuts, Editor tour, Project skills, MCP instructions) are mutually exclusive toggles; each closes the menu | `src/components/workspace/editor-workspace.tsx:7361-7454` | P07 |
| "Model settings" opens settings `{category:"aiModels", item:"transcription"}` anchored to clicked item, else `onOpenModelSettings`; subtitles "Transcription and agent setup", "Timeline, preview, and trim controls", "Import, generate, edit, and export", "Required Video Creater agent context", "Connect agents and copy setup" | `src/components/workspace/editor-workspace.tsx:7333-7454` | P07 |
| Nav rail settings uses `document.activeElement` as origin; else `onOpenModelSettings`; Home -> `onOpenProjectHome` | `src/components/workspace/editor-workspace.tsx:7833,7857-7871` | P07 |
| Nav rail source select: same destination toggles panel (closing sets compact view "timeline" if compact was media); new destination opens panel, compact "media", composer open iff "generate" | `src/components/workspace/editor-workspace.tsx:7835-7855` | P07 |
| Layout toolbar hidden in single-pane; presets "default"/"media"/"vertical" ("Default layout"/"Media layout"/"Vertical layout"); media toggle forces `sourcePanelOpen` true + transient false; inspector toggle clears transient; maximize toggle ("Restore editor layout"/"Maximize focused panel") | `src/components/workspace/editor-workspace.tsx:7057-7115` | P07 |
| Compact view buttons ("Media library", "Timeline preview", "Inspector", "Codex") shown only in single-pane; Media opens source panel, others close it | `src/components/workspace/editor-workspace.tsx:7744-7825` | P07 |
| Layout: codexPinned always true; single-pane shows requested source panel; else also needs budget `contextPanelVisible`; wide mode mounts panel whenever open; inspector always visible in effective layout; maximize ignored in single-pane | `src/components/workspace/editor-workspace.tsx:6392-6419` | P07 |
| Grid columns: media `minmax(10rem, {mediaWidth}px)` or "0px"; inspector `52 + (open ? resizePreview ?? width : 0)` px; vertical preset order media, inspector, `minmax(12rem, 1fr)`; else media, `minmax(12rem, 1fr)`, inspector | `src/components/workspace/editor-workspace.tsx:6422-6436` | P07 |
| Pane visibility: single-pane per compact view; maximized -> only that pane; timeline always; codex iff rail open; media iff source visible | `src/components/workspace/editor-workspace.tsx:6437-6452` | P07 |
| Upper deck hidden when multi-pane and codex maximized; pane resize handles (Codex/media) only multi-pane; focus/pointer on a pane dispatches `focusPane` | `src/components/workspace/editor-workspace.tsx:7881-7906,8032-8060,8081-8082,8489-8490` | P07 |
| Header menus (export, profile) portal-rendered with `anchoredHeaderMenuStyle` and closed via `handleWorkspaceMenuKeyDown` (local helpers at :469, :761) | `src/components/workspace/editor-workspace.tsx:7146-7160,7318-7331` | P07 |
| Keyboard handler on document keydown; ignored when keyboardShortcutsEnabled false, event.defaultPrevented, or editable focus (event target or activeElement) | `src/components/workspace/timeline-editor.tsx:2069-2081` | P03 |
| V (no mods) → select tool; C (no mods) → razor tool; Esc → reset (see above) | `src/components/workspace/timeline-editor.tsx:2118-2140` | P03 |
| Cmd/Ctrl+Shift+V → onInsertItems(playhead) (requires canPasteItems && onInsertItems); Cmd/Ctrl+V → onPasteItems(playhead) (requires canPasteItems) | `src/components/workspace/timeline-editor.tsx:2103-2116,2481-2494` | P03 |
| PageUp/PageDown (transport shortcuts enabled, no mods) → seek previous/next edit point if any; Home → 0, End → duration | `src/components/workspace/timeline-editor.tsx:2142-2180` | P03 |
| ArrowLeft/Right (transport enabled, no Alt/Ctrl/Meta, evaluated after Shift variants) → seek playhead ±0.25s (round3) | `src/components/workspace/timeline-editor.tsx:2371-2386` | P03 |
| A → select forward on selected item's track (items with start >= selected start); Shift+A → forward on all tracks; requires selectedItem && onSelectItems; non-additive; clears gap | `src/components/workspace/timeline-editor.tsx:2311-2333` | P03 |
| [ / ] → trim start/end to playhead: multi-select → onTrimSelectedItemsToPlayhead("start"\|"end", playhead) if any splittable; single requires unlocked track, playhead inside, onTimelinePatch: [ → createLeftTrimPatchFromDrag, ] → createRightTrimPatchFromDrag ?? createResizePatchFromDrag(duration = round3(playhead - start)) | `src/components/workspace/timeline-editor.tsx:2335-2369` | P03 |
| Delete/Backspace: selected gap → onRippleDeleteGap; Shift → ripple delete (multi onRippleDeleteSelectedItems, else single if allowed); plain → multi onRemoveSelectedItems else onRemoveItem if allowed | `src/components/workspace/timeline-editor.tsx:2388-2423` | P03 |
| Cmd/Ctrl+D duplicate (onDuplicateSelectedItems preferred else onDuplicateItem); Cmd/Ctrl+C copy (multi onCopySelectedItems else onCopyItem); Cmd/Ctrl+X cut (multi onCutSelectedItems else onCutItem) | `src/components/workspace/timeline-editor.tsx:2425-2479` | P03 |
| S (no Ctrl/Meta) or Cmd/Ctrl+K (no Alt) → split at activeSplitSeconds | `src/components/workspace/timeline-editor.tsx:2496-2519` | P03 |
| I / O (no mods): if canSetSourceMark → trim clip source in/out to playhead; else set timeline range in/out mark | `src/components/workspace/timeline-editor.tsx:2521-2535` | P03 |
| Native menu request commands executed unless activeElement editable: "selectForwardTrack"/"selectForwardAll", "trimStart"/"trimEnd", "delete", "rippleDelete" (gap first), "split" — same guards as keyboard | `src/components/workspace/timeline-editor.tsx:1967-2067` | P07 |
| Toolbar titles advertise shortcuts: "Select timeline tool (V)", "Razor timeline tool (C)", "Split selected clip (S)", I/O source mark titles | `src/components/workspace/timeline-editor.tsx:3934,3946,3958,3984,3994` | P07 |
| Export sheet: Escape closes (onOpenChange(false)); Tab/Shift+Tab focus trap cycling first/last focusable; focuses first focusable on open; returns focus to returnFocusRef on close/unmount | `src/components/workspace/export-sheet.tsx:65-72,192-210,254-280` | P07 |
| Export sheet backdrop mousedown closes only when target is backdrop and !busy | `src/components/workspace/export-sheet.tsx:282-284` | P07 |
| Codex composer mention keys ArrowDown/ArrowUp/Escape/Enter/Tab (only while suggestions visible) | `src/components/workspace/agent-panel.tsx:1435-1466` | P06 |

## Backend call sequences

Ordered steps for multi-step handlers, grouped by source file.

### editor-workspace.tsx (lines 1-4400)

**applyTimelinePatch** (`src/components/workspace/editor-workspace.tsx:2120`, P03)
1. Clear timelinePatchError.
2. If split project → `applyProjectAction(projectActionFromTimelinePatch(patch))` and return.
3. Else `applyTimelinePatchToProject({project, patch})` → `recordProjectHistory(project)`, `setProject`.
4. On error → `applyTimelinePatchLocally(project, patch)`; if changed record history + setProject; else set timelinePatchError to error message. (Save status untouched on this path.)

**applyProjectAction** (`src/components/workspace/editor-workspace.tsx:2143`, P07)
1. Clear timelinePatchError.
2. Split project: status "saving" → `applyProjectActionToSplitProjectFolder({projectDir, action})` → optional `recordProjectHistory(base)` → `setProject(result.project)` → "saved"; return project.
3. Else `applyProjectActionToProject({project: base, action})` → optional history → setProject → "unsaved".
4. Catch: if schema < 2 OR backend unavailable → `applyProjectActionLocally`; if changed → history, setProject, "unsaved", return.
5. Otherwise "failed" + timelinePatchError(message); return null.

**applyProjectActions** (`src/components/workspace/editor-workspace.tsx:2226`, P07)
1. Clear error. Split: "saving" → `applyProjectActionsToSplitProjectFolder({projectDir, actions})` → history → setProject → "saved".
2. Non-split: loop `applyProjectAction(action, next, recordHistory=false)`; any null → return null; then one `recordProjectHistory(base)`, "unsaved".
3. Catch: backend unavailable → `applyProjectActionsLocally`; if changed history/setProject/"unsaved". Else "failed" + error; null.

**recordQueuedWorkflowJob** (`src/components/workspace/editor-workspace.tsx:2269`, P07)
1. Build `{type:"recordJob", job}`; clear error.
2. Split → `applyProjectActionsToSplitProjectFolder` ; else `applyProjectActionToProject`.
3. `recordProjectHistory(base)`; `setProject(cur => mergeCodexProjectMetadata(cur, result))`. Error → timelinePatchError, null.

**buildGenerateMediaJob** (`src/components/workspace/editor-workspace.tsx:2299`, P05)
1. `buildTemporalJobSummary("generate_media", project.id, assetId, createdAt)`.
2. If projectDir empty → return job without startRequest.
3. `generationExecutionModeForModel(brief.model)` → input `{projectId, projectDir, assetId, jobId: assetId, mockMode, ...brief}`.
4. `buildTemporalGenerateMediaStartRequest(input)`; on backend-unavailable → `buildFallbackGenerateMediaStartRequest(input)`; other errors rethrow.
5. Return `{...job, startRequest}`.

**startMockGenerateMediaWorkflow** (`src/components/workspace/editor-workspace.tsx:2334`, P05)
1. Guard: no startRequest / schema < 2 / no projectDir → return base.
2. `buildTemporalStartResultAction({job, runId: mockTemporalRunId(job.id), updatedAt: now})` (backend-unavailable → `buildFallbackTemporalStartResultAction`).
3. `applyProjectActions([startResultAction, {updateGeneratedAssetStatus, assetId: job.id, status:"running"}], base)`. Error → timelinePatchError, null.

**startQueuedTemporalWorkflow** (`src/components/workspace/editor-workspace.tsx:2375`, P05)
1. Guard: no startRequest or id already starting → return base.
2. Add id to starting set (ref + state); clear error.
3. `startTemporalWorkflow({job})`.
4. If `status === "started"` && runId → `buildTemporalStartResultAction({job, runId, updatedAt})` → `applyProjectActions([action], base)`; if `generate_media` and `mockMode === false` → `generatedAssetPollerRef.start(job.id)`; return project.
5. Else timelinePatchError(`result.message`), null. Catch → error message. Finally remove id from starting set.

**startGenerateMediaWorkflow** (`src/components/workspace/editor-workspace.tsx:2415`, P05)
1. `mockMode === true` → startMockGenerateMediaWorkflow.
2. `loadAppSettingsPreferences().generationExecutionBackend === "temporal"` → startQueuedTemporalWorkflow.
3. Guard no startRequest / already starting → base. Add to starting set; clear error.
4. `runGenerateMediaInProcess({startRequest, updatedAt})` (promise), then immediately `generatedAssetPollerRef.start(job.id)`; await.
5. Falsy result (bridge lacks native command) → remove id from ref and fall back to `startQueuedTemporalWorkflow(job, base)`.
6. Success → `recordProjectHistory(base)`; `setProject(cur => mergeCodexProjectMetadata(cur, completed))`.
7. Catch → try `loadSplitProjectFromFolder({projectDir})` and setProject (native runner persisted failed job); ignore reload error; timelinePatchError(message); null. Finally clear starting id.

**queueGeneratedClipVariation** (`src/components/workspace/editor-workspace.tsx:4304`, P05)
1. Find generated asset (no-op if missing); id `generatedVariationId(asset.id)`; placement = `generatedReplacementPlacementIntent(replacementItemId)` or `generatedAssetPlacementIntent(asset)`.
2. `buildGenerateMediaJob(variationId, createdAt, {name, targetFolderId, placementIntent, prompt, model, references, settings})`.
3. `applyProjectActions([recordJob, recordGeneratedAsset{status "queued", outputs [], parentAssetId, retryOfAssetId}])`.
4. If success → `startGenerateMediaWorkflow(job, nextProject)`.

**rerunGeneratedAsset** (`src/components/workspace/editor-workspace.tsx:4359`, P05)
1. Missing asset or blank prompt → error "Generated asset `<id>` cannot be rerun without a prompt.".
2. Else `queueGeneratedClipVariation(assetId, asset.prompt)` (no replacement).

**retryGeneratedAssetDownload** (`src/components/workspace/editor-workspace.tsx:4370`, P05)
1. Validate asset, requested output, retriable output, non-blank `sourceUrl`, split project (errors verbatim in Generation table).
2. Clear error → `retryGeneratedAssetOutputDownloadInSplitProjectFolder({projectDir, assetId, outputMediaId})` → `setProject(result.project)` (no history). Error → timelinePatchError.

**undoProjectEdit** (`src/components/workspace/editor-workspace.tsx:2481`, P07)
1. `past.at(-1)` or return. Clear error.
2. `persistRestoredProject(previous, project.contentRevision ?? 0)` → split: `saveSplitProjectToFolder({projectDir, project, expectedRevision})` returns saved project; else snapshot.
3. setProject(restored); history = {past: past.slice(0,-1), future: [snapshot(current), ...future]}. Error → timelinePatchError.

**redoProjectEdit** (`src/components/workspace/editor-workspace.tsx:2532`, P07)
1. `future[0]` or return; clear error.
2. `persistRestoredProject(next, contentRevision ?? 0)` → setProject; history = {past: [...past, snapshot(current)], future: future.slice(1)}. Error → timelinePatchError.

**undoAgentProjectEdit** (`src/components/workspace/editor-workspace.tsx:2503`, P06)
1. Last agent entry or return; if `!projectSnapshotsEqual(project, entry.after)` → codexError "Undo Codex edit is unavailable after another project edit.".
2. Clear patch error + codexError; `persistRestoredProject(entry.before, contentRevision ?? 0)`.
3. setProject; pop agent history; pop last past, push current snapshot to future. Error → timelinePatchError.

**Canonical preview effect** (`src/components/workspace/editor-workspace.tsx:1366`, P04)
1. `projectNeedsCanonicalPreview(project)` false → clear prepared + preparation.
2. No projectDir → preparation failed "Canonical preview preparation requires a saved project folder.".
3. Else clear prepared, set pending → `prepareProjectPreview({projectDir, project})` → set `{sourceProject, result}`, clear preparation; error → failed with message. Cancelled on re-run (project, projectDir, retry token).

**Failed render restore effect** (`src/components/workspace/editor-workspace.tsx:1125`, P07)
1. `latestFailedRenderJob(project)`; none → reset restored ref. Same id as last restored → return.
2. Set export "failed", message, path null, `openProjectInspectorDestination("render-review")`.
3. If projectDir → `loadRenderPipelineReportFromSplitProjectFolder({projectDir, jobId})` → setRenderReport; errors ignored.

**Configuration probes on mount/refresh** (`src/components/workspace/editor-workspace.tsx:1858-2031`, P05/P07)
1. `getExportProfileAvailabilityReport()` (mount only) → ready/unavailable.
2. `listProviderCredentialStatuses()` → statuses/readiness.
3. `listVisualEffectCatalog()` (mount only) → catalog; non-backend errors warn "Failed to load visual effect catalog".
4. `listGenerationModelCatalog()` → `generationModelCatalogFromPayload(payload, preferences)` → catalog/readiness.
5. `getTemporalWorkerEnvironmentReport()` → `isTemporalWorkerEnvironmentReport` → report/readiness.
6. `loadShaderBackgroundTemplates({projectDir})` → replace catalog only if non-empty and not the default catalog object; error → default catalog.

**Project-scoped loads** (`src/components/workspace/editor-workspace.tsx:1195`, `:2033`, `:2068`, P05/P06)
1. Guard schemaVersion >= 2 and projectDir.
2. `getProjectSpeakerRegistry({projectDir})` → speakers.
3. `loadAppServerConversationHistoryFromSplitProjectFolder({projectDir})` → entries (warn "Failed to load Codex app-server conversation history").
4. `loadAgentSessionsFromSplitProjectFolder({projectDir})` → manifest (warn "Failed to load project agent sessions; local chat history remains available").
5. `updateAgentSession(action)` → `applyAgentSessionActionToSplitProjectFolder({projectDir, projectId, action})` → manifest.

**Native drag-drop listener** (`src/components/workspace/editor-workspace.tsx:1217`, P05)
1. Dynamic `import("@tauri-apps/api/webview")` → `getCurrentWebview().onDragDropEvent`.
2. over/leave toggle drop highlight; drop → `importMediaPaths(paths)` (defined at :4886, outside this range).
3. Unlisten on unmount/projectDir change; import failure ignored.

**switchActiveTimeline** (`src/components/workspace/editor-workspace.tsx:3515`, P03)
1. Same id → return. Stash target playhead (stored ?? 0) in pending ref; save current view state.
2. Stop playback; clear selection, range, replacement target, clipboard.
3. `applyProjectAction({type:"setActiveTimeline", timelineId})` → on success playhead = clamp(target, result.timeline.durationSeconds).

**applyTemplateUpdate** (`src/components/workspace/editor-workspace.tsx:3222`, P04)
1. `applyProjectAction(updateTemplateItems{itemId,start,duration,templateFields})`; null → stop.
2. `applyProjectAction(updateTemplateOverride{override}, nextProject)`.

**insertTemplate / insertShaderBackgroundTemplate** (`src/components/workspace/editor-workspace.tsx:2773`, `:2799`, P05)
1. Clear templateInsertError; resolve template (`getMotionTemplate` / `getShaderBackgroundTemplate(id, templates)`); build item via `createTemplateOverlayItem` / `createShaderBackgroundTemplateItem`.
2. Find overlay / `hyperframe_scene` track; missing → set error copy, return.
3. `applyProjectAction(addItems)` → select item on success.

**applyAudioDenoise** (`src/components/workspace/editor-workspace.tsx:3428`, P04)
1. Find `audio_clip` item or return; sanitize existing effects (drop "audio.denoise").
2. If enabled append denoise effect `{amount}`.
3. `applyProjectActions([updateItemEffects{itemIds:[id], effects}, updateItemProperties{set audioDenoisePreparation queued \| remove it}])`.

**applyVisualClipSpeed** (`src/components/workspace/editor-workspace.tsx:3795`, P04)
1. Compute new duration = duration * currentSpeed / speed (3dp).
2. `applyProjectActions([updateVisualClipSpeed{speed}, resizeItems{durationSeconds}])`.

Note: handlers named in the assignment but defined beyond line 4400 (not covered here): `confirmProviderUploadForGeneration` :4556, `importMediaFromPicker` :4876, `importMediaPaths` :4886, `executionUsesTemporal` :5550, `openProjectInspectorDestination` :5709, `updateCodexDisclosure` :6453, `openViewerSourceDestination` :6523; transcription queueing, export/NLE/save-range, codex edit start/apply/reject, and `queueGeneratedClipVariationSet` (:4425) / `queueMediaGeneration` (:4493) also start after 4400. No autosave/debounce and no poll interval constants exist in lines 1-4400 (polling is inside `createGeneratedAssetPoller` in src/lib).

### editor-workspace.tsx (lines 4401-8585)

**retryGeneratedAssetDownload** (`src/components/workspace/editor-workspace.tsx:4370-4423`, P05)
1. (pre-4401 guards) asset exists; requested output exists; pick output: requested ?? first output whose media is missing from project ?? first output with sourceUrl.
2. No `sourceUrl` -> error "...has no provider retry URL. Rerun generation to recreate the file."; schemaVersion < 2 or no projectDir -> "...can only be retried from a split project folder.".
3. `setTimelinePatchError(null)`; `retryGeneratedAssetOutputDownloadInSplitProjectFolder({projectDir, assetId, outputMediaId})` (@/lib/project).
4. Success -> `setProject(result.project)`; failure -> timeline patch error = message.

**queueGeneratedClipVariationSet** (`src/components/workspace/editor-workspace.tsx:4425-4491`, P05)
1. Find asset; `validVariationDrafts(drafts)`; abort if < 2.
2. For each draft: `generatedVariationSetId(asset.id, index)`; `buildGenerateMediaJob(id, createdAt, {...})` (local, :2299); push `recordJob` + `recordGeneratedAsset`.
3. `applyProjectActions(actions)` (one batch).
4. If project returned: for each `recordJob` sequentially `startGenerateMediaWorkflow(job, runningProject)` (local, :2415), threading returned project (fallback previous).

**queueMediaGeneration** (`src/components/workspace/editor-workspace.tsx:4493-4542`, P05)
1. `confirmProviderUploadForGeneration(request)` -> `loadAppSettingsPreferences()`; window.confirm if required; cancel -> abort.
2. `generatedMediaAssetId()`; `buildGenerateMediaJob(assetId, createdAt, request fields)`.
3. `generatedTimelinePlaceholderAction(project, assetId, request)` (optional).
4. `applyProjectActions([recordJob, recordGeneratedAsset(status "queued"), placeholder?])`.
5. If project -> `startGenerateMediaWorkflow(job, nextProject)`.

**queueReferencedMediaGeneration / queueMediaUpscale / queueVideoAudioGeneration** (`src/components/workspace/editor-workspace.tsx:4693-4810`, P05)
1. Guard media (not audio; video-audio requires video; prompt non-empty for referenced).
2. Build request (video: wan video-to-video with span; image: "fal-ai/wan-25-preview/text-to-video" first-frame; upscale: `upscaleGenerationRequest`; audio: `videoAudioGenerationRequest`).
3. `queueMediaGeneration(request)` (sequence above).

**completeMockGeneration** (`src/components/workspace/editor-workspace.tsx:4584-4639`, P05)
1. Abort if no projectDir; compute replacementItemId; clear patch error.
2. `completeMockGeneratedAssetInSplitProjectFolder({projectDir, assetId, updatedAt, replacementItemId})`.
3. `recordProjectHistory(project)`.
4. If no replacement && output && `generatedAssetPlacementIntent(asset) === "timeline"`: `generatedOutputTimelineActions(result.project, outputId)`; if insertion: 1 action -> `applyProjectActionToSplitProjectFolder`, else `applyProjectActionsToSplitProjectFolder`; null -> return; set project, select inserted item + output media; return.
5. Else `setProject(result.project)`. Errors -> patch error.

**failMockGeneration** (`src/components/workspace/editor-workspace.tsx:4641-4691`, P05)
1. Abort if no projectDir; find job by asset id; missing -> "Generated media workflow job is missing for {assetId}.".
2. If `kind === "generate_media"` && `mockMode === false`: `cancelGenerateMediaInProcess({projectDir, jobId, updatedAt})` -> setProject; if cancelUrl && provider fal.ai/replicate -> `cancelGenerateMediaProviderRequestInSplitProjectFolder({projectDir, jobId})` (failure -> "Generation was cancelled locally, but the provider cancellation request failed: ..."); return.
3. Else `buildTemporalGenerateMediaFailureActions({job, assetId, runId, updatedAt})` -> `applyProjectActions(actions)`.
4. Errors -> patch error.

**insertGeneratedOutputOnTimeline / insertSelectedMediaOnTimeline** (`src/components/workspace/editor-workspace.tsx:4830-4874`, P05)
1. `generatedOutputTimelineActions(project, mediaId)`; if present: 1 action -> `applyProjectAction`, else `applyProjectActions`; success -> select item + media.
2. (selected media only) fallback `mediaTimelineAction(project, mediaId)` -> `applyProjectAction(insertion.action)` -> select item + media.

**importMediaFromPicker -> importMediaPaths** (`src/components/workspace/editor-workspace.tsx:4876-4907`, P05)
1. `openMediaFilePaths()` (@/lib/media-import).
2. Empty -> return; importStatus "importing", error null.
3. `importMediaToProject({projectDir, project: projectRef.current, sourcePaths})`.
4. Success: setProject; select first imported; status "idle"; error = `formatSkippedMediaImports(skipped)`. Failure: status "failed", error message.

**createMatte** (`src/components/workspace/editor-workspace.tsx:4909-4925`, P05)
1. Guard schema v2 + projectDir else throw "Save this project before creating a matte.".
2. `createMatteInSplitProjectFolder({projectDir, request: {hex, aspectRatio, folderId?}})`.
3. setProject; select `result.media.id`; destination "media"; open source panel.

**generateCodexEdit** (`src/components/workspace/editor-workspace.tsx:4927-5016`, P06)
1. Bump epoch; set latest request; clear proposal/issues/error; status "running".
2. If schema v2 + projectDir: `analyzeMediaForEditInSplitProjectFolder({projectDir, mediaId})` -> `mergeProjectMediaAnalysis` into local and state (errors warned only).
3. `buildTemporalJobSummary("codex_edit", projectId, generatedCodexEditJobId(), updatedAt)`.
4. If split project: attach `startRequest = buildTemporalCodexEditStartRequest({projectId, projectDir, jobId, request})`.
5. `recordQueuedWorkflowJob(queuedJob, projectForEdit)` (local :2269); null -> idle + "Codex edit workflow job could not be queued.".
6. `startCodexVideoEditForProject({projectDir? (split only), project: queuedProject, request})`.
7. `mergeCodexProjectMetadata(queuedProject, result.project)` then merge into current state.
8. No proposal -> idle + "Codex did not return a structured edit proposal."; else set proposal, validation issues ?? null, status "ready".
9. Epoch checked after every await; error -> idle + message (only if current epoch).

**cancelCodexEdit** (`src/components/workspace/editor-workspace.tsx:5018-5032`, P06)
1. Bump epoch; status idle; error null.
2. `cancelCodexVideoEditForProject({projectDir?})`; error shown if epoch unchanged.

**applyCodexProposal** (`src/components/workspace/editor-workspace.tsx:5034-5071`, P06)
1. Guard proposal; `codexProposalReviewModel.canApply` else blocked copy.
2. Status "applying"; `projectHistorySnapshot(project)`.
3. `materializeCodexProposalActions(project, proposal)` (throw -> ready + message).
4. `applyProjectActions(actions)`; null -> ready + "Codex proposal actions were rejected.".
5. Append `{before, after: projectHistorySnapshot(nextProject)}` to agent history; clear proposal; status idle.

**runPreviewRenderComparison** (`src/components/workspace/editor-workspace.tsx:5098-5135`, P07)
1. Guard request/projectDir/not running; capture runId; running true.
2. `runPreviewRenderComparisonRequestInSplitProjectFolder({projectDir, request: {...request, status:"pending"}, updatedAt})`.
3. Success (same runId): setProject, setRenderReport, passed/differences message. Failure: "Preview/render review failed: ...". Finally running false if same runId.

**exportInDesktopProcess** (`src/components/workspace/editor-workspace.tsx:5141-5223`, P07)
1. `inProcessExportLabel(profile, quality)`; runId+1; `generatedRenderJobId(quality)`; `generatedRenderAttemptId()`; set active job/attempt ids.
2. Local `applyProjectActionLocally(recordJob {kind:"render_draft", status:"running"})`; status exporting; webm quality; "{label} rendering"; clear path/notification.
3. `recordRenderReport` -> `renderMediaToSplitProjectFolder({projectDir, projectId, profile, quality, width, height, jobId, attemptId, updatedAt})`.
4. Success (same runId): clear active ids; setProject, report, path; status "exported"; "{label} rendered"; `openProjectInspectorDestination("render-review")`; notification if `loadAppSettingsPreferences().renderCompletionNotifications`.
5. Failure (same runId): if projectDir `Promise.all([loadSplitProjectFromFolder, loadRenderPipelineReportFromSplitProjectFolder({projectDir, jobId})])` each catch null; re-check runId; clear ids; set project if loaded; set report (maybe null); status failed; message; open render-review.

**cancelRender** (`src/components/workspace/editor-workspace.tsx:5233-5265`, P07)
1. Guard exporting; capture job/attempt; runId+1; status "cancelled", "Render cancelled"; clear path/notification/ids.
2. If projectDir && ids: `cancelRenderJobInSplitProjectFolder({projectDir, jobId, attemptId, updatedAt})` -> setProject (same runId) / error message.

**saveTimelineRangeAsMedia** (`src/components/workspace/editor-workspace.tsx:5267-5332`, P07)
1. Guard projectDir; `generatedSaveRangeJobId()`, `generatedRenderAttemptId()`; export "exporting" + import "importing".
2. `draftExportDimensions(w, h)`; `renderMediaToSplitProjectFolder({profile:"webm", quality:"draft", ..., rangeStartSeconds, rangeEndSeconds})`.
3. `safeProjectMediaPath(projectDir, outputPath)` else throw.
4. `importMediaToProject({projectDir, project: renderResult.project, sourcePaths: [path]})`.
5. setProject(import project); select first imported ?? current; report; path; "exported"; "Timeline range saved as media"; import idle; skipped copy. Failure: both statuses failed with message.

**exportNleXml** (`src/components/workspace/editor-workspace.tsx:5334-5363`, P07)
1. Guard canExportNleXml; "{label} export running".
2. `exportNleXmlToSplitProjectFolder({projectDir, format, jobId: generatedNleExportJobId(format), updatedAt})`.
3. setProject; "exported"; "{label} exported"; path = `result.exportPath`. Failure -> failed + message.

**exportMediaProfile** (Temporal codec) (`src/components/workspace/editor-workspace.tsx:5365-5426`, P07)
1. Skip palmier; resolve jobId (`generatedMediaExportJobId`), outputPath, input; guard availability.
2. Status exporting; "{codec} {quality} queued at {w}×{h}"; path = outputPath.
3. `buildTemporalJobSummary("export_media", projectId, jobId, updatedAt)` -> `buildTemporalExportMediaStartRequest(input)` -> `exportJobWithStartRequest(job, startRequest)`.
4. `recordQueuedWorkflowJob(queuedJob)`; null -> failed "{label} workflow could not be recorded".
5. `startQueuedTemporalWorkflow(queuedJob)` (local :2375); status "idle". Failure -> failed + message.

**exportPalmierProjectPackage** (`src/components/workspace/editor-workspace.tsx:5428-5492`, P07)
1. Guard canExportMediaProfiles; jobId, outputPath (`.palmier`).
2. Non-Temporal: `exportPalmierProjectPackageToSplitProjectFolder({projectDir, jobId, outputPath, updatedAt})` -> setProject, exported, path = exportPath.
3. Temporal: `buildTemporalJobSummary("export_media", ...)` -> `buildTemporalExportProjectBundleStartRequest({projectId, projectDir, jobId, outputPath})` -> `exportJobWithStartRequest` -> `recordQueuedWorkflowJob` (null -> "Palmier Project package workflow could not be recorded") -> `startQueuedTemporalWorkflow` -> status idle.

**submitExportSheet / startMediaProfileExport** (`src/components/workspace/editor-workspace.tsx:5494-5548`, P07)
1. "webm" -> `exportWebm` -> `exportInDesktopProcess("webm", opts)`.
2. NLE ids -> `exportNleXml`; "palmierProject" -> package export.
3. Else look up profile availability -> `startMediaProfileExport`: Temporal -> `exportMediaProfile(profile, explicit input with sheet quality/dims)`; else (available, quality available, split project) -> `exportInDesktopProcess(profile, opts)`.

**queueSelectedMediaTranscription** (`src/components/workspace/editor-workspace.tsx:5555-5607`, P05)
1. Guard selected media exists; `generatedTranscribeMediaJobId(mediaId)`; clear error.
2. `buildTemporalJobSummary("transcribe_media", projectId, jobId, updatedAt)`.
3. `buildTemporalTranscribeMediaStartRequest({projectId, projectDir, mediaId, jobId, languageMode:"auto"})`; on `isBackendUnavailableError` -> `buildFallbackTranscribeMediaStartRequest(same)`; other errors rethrow.
4. `recordQueuedWorkflowJob({...job, startRequest})`; null -> "Transcription workflow could not be recorded.".
5. `startQueuedTemporalWorkflow(queuedJob)`. Errors -> patch error.

**analyzeItemSpeech** (`src/components/workspace/editor-workspace.tsx:5616-5647`, P05)
1. Resolve item, `timelineItemSourceMediaId`, media, projectDir; prepared PCM path.
2. Status "analyzing"; clear error.
3. `analyzeProjectSpeech({projectDir, mediaId, preparedPcmPath})`.
4. `refreshSpeechProject(baseProject)` -> `loadSplitProjectFromFolder` + `recordProjectHistory` + setProject.
5. `getProjectSpeakerRegistry({projectDir})` -> speakers (non-array -> []); delete status entry.
6. Failure: status "failed"; error; `applyProjectAction(updateItemProperties speechAnalysis failed/production, baseProject)`.

**renameSpeakerIdentity / recolorSpeakerIdentity** (`src/components/workspace/editor-workspace.tsx:5649-5671`, P05)
1. `renameProjectSpeaker({projectDir, speakerId, name})` / `recolorProjectSpeaker({projectDir, speakerId, color})` -> set registry speakers.
2. `refreshSpeechProject(baseProject)`. Errors -> patch error.

**assignItemSpeaker** (`src/components/workspace/editor-workspace.tsx:5673-5707`, P05)
1. Guard item + projectDir; fingerprint required; compute overlap range and source-time mapping.
2. `assignProjectMediaSpeaker({projectDir, fingerprint, startSeconds, endSeconds, speakerId})`.
3. `refreshSpeechProject(baseProject)`. Errors -> patch error.

**cancelActivityJob** (`src/components/workspace/editor-workspace.tsx:6070-6100`, P07)
1. Guard cancellable; active render -> `cancelRender()`.
2. Job running/progress + generated asset + real generate + projectDir: `cancelGenerateMediaInProcess({projectDir, jobId, updatedAt})` -> setProject; provider cancel via `cancelGenerateMediaProviderRequestInSplitProjectFolder` if cancelUrl and fal.ai/replicate.

**openActivityRenderRecord** (`src/components/workspace/editor-workspace.tsx:6138-6173`, P07)
1. Guard reportId/logPath; open "render-review"; same report loaded -> return; clear report.
2. No projectDir -> status/message/path from record.
3. `loadRenderPipelineReportFromSplitProjectFolder({projectDir, jobId})` -> set report, status, message, path. Error -> failed + message.

**requestTimelineFilmstrip** (`src/components/workspace/editor-workspace.tsx:6968-7001`, P03)
1. Guard projectDir/browser; dedupe key.
2. `cacheTimelineFilmstripInSplitProjectFolder({projectDir, ...request})`.
3. Map frames through `previewUrlForMedia(projectDir, relativePath)` into `timelineFilmstripFrames[itemId]`; error -> drop key.

**CaptionsWorkbench onBuildCaptions** (`src/components/workspace/editor-workspace.tsx:6595-6609`, P05)
1. No transcript or range -> `queueSelectedMediaTranscription()`.
2. Else `buildCaptionItems({...})` -> `addBuiltCaptionItems(items)`.

**EffectCatalogPanel onApply** (`src/components/workspace/editor-workspace.tsx:6889-6909`, P05)
1. Guard target/descriptor (no resourceKey)/not already applied.
2. `nextCatalogEffectInstanceId(descriptor.id, existing)`; `applyVisualClipEffects(itemId, [...existing, newEffect])`.

_Note: handlers named in the assignment but defined before line 4401 (outside this part): `recordQueuedWorkflowJob` :2269, `buildGenerateMediaJob` :2299, `startQueuedTemporalWorkflow` :2375, `startGenerateMediaWorkflow` :2415, `undoProjectEdit` :2481, `undoAgentProjectEdit` :2503, `redoProjectEdit` :2532, `applyTimelinePatch` :2120, `applyProjectAction(s)` :2143/:2226, `removeDetectedSilence` :3204, `removeCurrentDeadAir` :3216, `applyAudioDenoise` :3428, `retryAudioDenoise` :3485, drag-drop listener :1222, native menu switch :1257-1309, `queueGeneratedClipVariation` :4304, `rerunGeneratedAsset` :4359. Only their wiring in the JSX is recorded here._

### timeline-editor.tsx

**finishInteraction** (`src/components/workspace/timeline-editor.tsx:3603`, P03)
1. Flush frame coalescer with final pointer → preview; if no interaction or preview → cancelTimelineInteraction.
2. Move: record target row; if passedThreshold && state != "rejected": duplicate → onDuplicateItemsAtPositions(patches); >1 patches → onMoveItems(patches); else onTimelinePatch(createMovePatchFromDrag(patch[0])).
3. Ripple resize: if preview.error === null → onRippleTrim(request).
4. Resize: record placement track; if evaluation.patch → onTimelinePatch(patch).
5. Always cancelTimelineInteraction (cancel coalescer, clear interaction, preview, snap).

**evaluateTimelineInteraction** (`src/components/workspace/timeline-editor.tsx:3450`, P03)
1. Move: resolveMoveTarget → clamp delta to earliest start >= 0 → snapInteractionAtPointer (timelineSnapTargets + resolveTimelineSnap, sticky) → proposed start (guide delta or snapTimelineSeconds, round3) → unavailable target ⇒ rejected "Destination unavailable" else evaluateTimelineMove.
2. Ripple: request {itemId, edge, round3 delta}; if delta != 0 call onPlanRippleTrim, catch → error string.
3. Resize: snap → resizeLeft start/duration, resizeRight end >= start+0.1 → evaluateTimelineResize.

**dropTemplate** (`src/components/workspace/timeline-editor.tsx:3638`, P03)
1. Read shader background id and template id (template or text/plain); none → return (no preventDefault).
2. preventDefault; locked track → dropEffect "none", return.
3. Compute start from explicit start-seconds data or clientX/pps, max 0, snapTimelineSeconds, round3.
4. Shader id → onShaderBackgroundDrop(id, start, trackId); else onTemplateDrop(id, start, trackId).

**adjustResizeHandleFromKeyboard** (`src/components/workspace/timeline-editor.tsx:3734`, P03)
1. Only ArrowLeft/Right without Alt/Ctrl/Meta.
2. Shift: requires onRippleTrim; dismiss error; request delta ±0.25; try onPlanRippleTrim(request) then onRippleTrim(request); swallow errors.
3. Else requires onTimelinePatch; dismiss error; evaluateTimelineResize (left: start+delta, duration end-start-delta; right: duration+delta); clamped/rejected → keyboard feedback copy; patch → onTimelinePatch.

**Shift+ArrowUp/Down vertical move** (`src/components/workspace/timeline-editor.tsx:2182`, P03)
1. Guard canNudgeSelected; find nearest unlocked compatible track in direction (selectedVerticalMoveTargetTrackId); none → return.
2. dismissInteractionErrorForNewInteraction(target); compute per-item targetTrackIds (pinned linked/incompatible companions).
3. evaluateTimelineMove(start unchanged); rejected → setKeyboardTimelineFeedback reason ?? "Invalid destination", stop.
4. >1 patches → onMoveItems; else onTimelinePatch(createMovePatchFromDrag).

**Shift+ArrowLeft/Right nudge** (`src/components/workspace/timeline-editor.tsx:2254`, P03)
1. Guard canNudgeSelected; delta ±0.25 clamped (start >= 0; single item end <= duration), round3.
2. evaluateTimelineMove on same track; delta 0 or rejected → stop (feedback on reject).
3. >1 patches → onMoveItems ?? onNudgeSelectedItems(delta); single → onTimelinePatch(createMovePatchFromDrag).

**runTimelineContextMenuAction** (`src/components/workspace/timeline-editor.tsx:3204`, P03)
1. Close menu, clear ripple insert preview.
2. addRangeToChat/saveRangeAsMedia → handler(selectedRange) if range; clearRange → setTimelineRangeSelection(null); paste/insert → onPasteItems/onInsertItems(menu seconds).
3. Item actions require itemId: copy/cut/duplicate/delete → single-item handler; queueUpscale/createVideo re-validate kind + source media; fallthrough → onRippleDeleteItem.

**executeTimelineEditCommand** (`src/components/workspace/timeline-editor.tsx:1967`, P07)
1. selectForward*: onSelectItems forward items (track or all), clear gap.
2. trimStart/trimEnd: multi → onTrimSelectedItemsToPlayhead(edge, playhead) if splittable; single → guards then onTimelinePatch(createLeftTrimPatchFromDrag \| createRightTrimPatchFromDrag ?? createResizePatchFromDrag).
3. delete: multi onRemoveSelectedItems else onRemoveItem if canRemoveSelected.
4. rippleDelete: selected gap → onRippleDeleteGap + clear gap; multi → onRippleDeleteSelectedItems; else onRippleDeleteItem.
5. split: canSplitSelected → onSplitSelectedItems(seconds) or onSplitItem(id, seconds).

**finishTimelineMarquee** (`src/components/workspace/timeline-editor.tsx:3156`, P03)
1. Clear marquee state; none → false.
2. Movement < 3px → setSelectedGap(timelineGapAtSeconds(track at startY, startX/pps) or null).
3. Else clear gap; intersecting ids non-empty → onSelectItems({itemIds, additive}).

### source-clip-inspector.tsx, caption-inspector.tsx, template-inspector.tsx, text-overlay-inspector.tsx, keyframe-lane-editor.tsx

None of these files call Tauri invoke or backend APIs. All side effects go through callback props. Multi-step handlers:

**Apply effects onClick** (`src/components/workspace/source-clip-inspector.tsx:2072`, P04)
1. Guard: item, onApplyEffects, canApplyEffects and effectDraftsAreValid; otherwise return.
2. editedInstanceIds = Set of every draft's effectInstanceId.
3. untouched = projectActionEffectsForItem(item) (lib), keeping effects without an effectInstanceId or not in the edited set.
4. For each catalog descriptor with an enabled draft:
   - Numeric params = Number(draft.params[key]); add resourceKey = trimmed resource.
   - color.curves: params become {masterCurve:[[0,0],[0.5,mid],[1,1]]}.
   - color.hueCurves: params become {targets:[{targetHue,hueShift,satScale,lumShift}]}.
   - Emit {effectInstanceId, effectType, enabled:true, params}.
5. Call onApplyEffects(item.id, [...untouched, ...known]). No local state or copy is set.

**Apply Lottie inputs onClick** (`src/components/workspace/source-clip-inspector.tsx:2401`, P04)
1. inputs = {slots: []}.
2. If theme is trimmed non-empty, set themeId.
3. If marker is trimmed non-empty, set marker. Otherwise, if Number(end) > Number(start), set segment micros (round(x*1e6)).
4. If slot id is trimmed non-empty, parse #rrggbb into micros/255 and push a color slot with alpha 1e6.
5. If state machine id is trimmed non-empty:
   - Build at most one input (numeric micros / boolean === "true" / string).
   - Split events on commas, trim, drop empties.
6. Call onApplyLottieInputs(item.id, inputs).

**Referenced generation onClick** (`src/components/workspace/source-clip-inspector.tsx:1503`, P05)
1. Guard: mediaId, onQueueReferencedGeneration and canQueueReferencedGeneration.
2. prompt = trimmed text.
3. context = sourceClipGenerationContextForItem(item) only when media kind is "video"; otherwise undefined.
4. If referencedGenerationPlacementIntent exists, call onQueueReferencedGeneration(mediaId, prompt, intent, context). Otherwise call it with (mediaId, prompt, undefined, context).
5. The prompt is not cleared after queueing.

**Imported upscale onClick** (`src/components/workspace/source-clip-inspector.tsx:1454`, P05)
1. If mediaId exists: call onQueueUpscale(mediaId, sourceClipUpscaleContextForItem(item, mediaAsset)) when the context is non-null; otherwise call onQueueUpscale(mediaId).

**addAtPlayhead** (`src/components/workspace/keyframe-lane-editor.tsx:127`, P04)
1. localPlayhead = rounded(clamp(playhead, 0, duration)).
2. value = rounded(clamp(suggestedValue(config, keyframes, localPlayhead) (lib), min, max)).
3. Call onUpsert(property, {atSeconds, value, easing:"linear"}).
4. selectKeyframe: set selectedAtSeconds, timeInput, valueInput, easing.

**startDrag / updateDrag / finishDrag** (`src/components/workspace/keyframe-lane-editor.tsx:143-183`, P04)
1. pointerdown (button 0, not disabled): preventDefault and stopPropagation, setPointerCapture, store {pointerId, fromSeconds, previewSeconds, startClientX, laneWidth=max(1,rect.width)}, select the keyframe.
2. pointermove (same pointer): previewSeconds = rounded(clamp(from + dx/width*duration, 0, duration)).
3. pointerup (same pointer): releasePointerCapture. If preview !== from, call onMove(property, from, preview) and set selectedAtSeconds = preview. Clear drag.
4. pointercancel: clear drag without moving.

**Build timed captions onClick** (`src/components/workspace/caption-inspector.tsx:463`, P05)
1. Guard: buildTranscript, buildRange and onBuild.
2. Call buildCaptionItems({transcript, range, wordsPerCue, stylePreset, groupId: `caption-group-${Date.now().toString(36)}`}) (lib).
3. Call onBuild(items). Existing cues are untouched (per copy).

**Apply template onClick** (`src/components/workspace/template-inspector.tsx:273`, P04)
1. Guard: item and template.
2. nextFields = trimmed values for template.fieldDefinitions names only.
3. Call onApply(item.id, {fields: nextFields, startSeconds: Number, durationSeconds: Number, override: {templateId: template.id, name: template.name, fields: nextFields, style: {accent, background, text trimmed}, visualTreatment, motion, safeZone, avoid trimmed}}).

### media-bin.tsx, matte-sheet.tsx

**submitGeneration** (`src/components/workspace/media-bin.tsx:1256`, P05)
1. `trimmed = prompt.trim()`; if placement "timeline" and no `timelineGenerationTargets[timelineTargetMode(mode)]` -> return.
2. `model = selectedGenerationModel(mode, modelValues[mode], optionsByMode)`; `acceptsSelectedSourceVideo`/`requiresSelectedSourceVideo` via provider-rules; `selectedSourceVideoMediaId = accepts ? generationSourceVideoId : ""`.
3. `timelineSourceRange = validGenerationTimelineSourceRange(mode, model, range)`; `usesTimelineSourceVideo = requires && !selectedSource && range`.
4. Return if no prompt and no selected source video and not using timeline source; return if requires source and neither; return if `generationModelRequiresFirstFrame` and no first frame.
5. First/last frame ids cleared when exclusive-frame model and pane "reference" (else validated frame-ref ids); reference ids = [] when exclusive model and pane "first-last", else filtered to `generationReferenceMediaForModel` ids.
6. `typedGenerationReferenceMediaRefs(...)`; return if `generationReferenceLimitMessage(...)` non-null. Image refs = submit ids if typed-image model else typed.referenceImageMediaRefs.
7. `baseSettings = selectedGenerationSettings(mode, model, duration, aspect, resolution, count, generateAudio, instrumental, voice, lyrics, style, quality)`.
8. Settings branch: video with selected source video and model doesn't preserve settings -> duration/aspect/resolution null; timeline source video -> `category: videoInputAudioCategory(model)`, `durationSeconds = round(end-start)`, `videoSourceStartSeconds/EndSeconds`, `timelineStartSeconds = start` (all `roundGenerationTimelineSeconds`); else base.
9. `onGenerateMedia({kind: audio?"audio":"generated", name: name.trim()\|\|null, targetFolderId, placementIntent: usesTimelineSourceVideo?"timeline":placement, prompt: trimmed, model: generationRequestModel(model), references: {mediaIds: source?[source]: supportsRefMedia?refIds:[], sourceVideoMediaRef?, firstFrameMediaId/lastFrameMediaId (only if supports frame refs AND ref media), referenceImageMediaRefs (typed-image & supports ref media; or non-typed with length>0), referenceVideoMediaRefs/referenceAudioMediaRefs if non-empty}, settings})`.
10. Clear name, prompt, lyrics, style (keeps model/settings/references).

**rebuildSearchIndex** (`src/components/workspace/media-bin.tsx:865`, P05)
1. Guard: no callback or already rebuilding -> return.
2. Set rebuilding true, clear status; `await onRebuildProjectSearchIndex()`; status "Search index rebuilt".
3. If search callback and normalized query: `await onSearchProjectMedia({query: trim, limit: 20, scope})` -> set result, clear error.
4. Catch: status "Search index rebuild failed", error message stored (UI "Search index unavailable"). Finally rebuilding false.

**Indexed search effect** (`src/components/workspace/media-bin.tsx:813`, P05)
1. No callback or empty normalized query -> clear result/error.
2. Else clear result/error, call `onSearchProjectMedia({query: searchQuery.trim(), limit: 20, scope})`; ignore if superseded (cancelled flag); success sets result; failure sets message.

**extractVisualFrames** (`src/components/workspace/media-bin.tsx:902`, P05)
1. Guard: selected media, callback, not busy.
2. Busy true, clear status; `await onExtractVisualFrames(id)` -> "Visual frames already cached (n)" or "Visual frames extracted (n)".
3. Catch -> error.message or "Visual frame extraction failed"; finally busy false.

**captionVisualFrames** (`src/components/workspace/media-bin.tsx:929`, P05)
1. Guard: selected media, callback, not busy.
2. Busy true, clear status; `await onCaptionVisualFrames(id)` -> "Captioned <n> visual frames with <provider>".
3. Catch -> error.message or "Visual captioning failed"; finally busy false.

**createMatte** (`src/components/workspace/media-bin.tsx:3715`, P05)
1. Guard: `onCreateMatte` present and not busy (sheet also requires valid hex via `normalizedHex`).
2. Busy true, clear error; `await onCreateMatte({hex, aspectRatio, folderId?: activeFolder.id})`.
3. Success: close sheet (focus returns to "More media actions" trigger). Failure: error.message or "The matte could not be created." shown in sheet alert. Finally busy false.

**Folder drop** (`applyFolderDrop`, `src/components/workspace/media-bin.tsx:1205`, P05)
1. Require `onAssignMediaFolder`; `droppedMediaId` from "application/x-video-creater-media-id" or "text/plain", must exist in media.
2. preventDefault, dropEffect "move", clear highlight, `onAssignMediaFolder(mediaId, folderId \| null)`.

**External file drop** (`src/components/workspace/media-bin.tsx:3754`, P05)
1. `externalFilePathsFromDataTransfer(dataTransfer)`; empty -> return.
2. preventDefault, clear drop highlight, `onImportPaths(paths)`.

### agent-panel.tsx, export-sheet.tsx, activity-panel.tsx, render-report-panel.tsx

**generateEdit** (`src/components/workspace/agent-panel.tsx:405-596`, P06)
1. First matching branch wins, each calling a local helper then returning: caption text → overlay text → template field → template timing → template style → template metadata → track state → visual opacity → audio fade → audio volume → delete clip → reorder (lib `timelineClipReorderUpdate`) → split → trim → timeline generated clip variation (`onQueueSelectedVariation`) → mention timeline insert (`onInsertSelectedMedia`) → source variation set (`defaultVariationDrafts` → `onQueueSelectedVariationSet`).
2. Each branch calls its prop callback (e.g. `onApplySelectedCaptionText(itemId, text)`) then `recordQueueTranscript(...)` with tool "project_action" (generation branches use `generationToolNameForKind`).
3. Fallback: `buildEditJobRequest({mediaId: effectiveMediaId, preset: "trailer_cut", prompt (untrimmed), languageMode: "en"})` → `onGenerateEdit(request)` → `recordGenerateEditTranscript(request)` (sets localGenerateRequestKey, appends 6 entries).
4. Note: button label precedence puts mention insertion first and reorder after trim, whereas dispatch checks mention insertion after variation and reorder before split.

**trimSelectedTimelineClipFromPrompt** (`src/components/workspace/agent-panel.tsx:795-824`, P06)
1. If clip has source range: sourceIn = round(sourceIn + (start - clipStart)), sourceOut = round(sourceOut - (clipEnd - end)).
2. Build update {startSeconds: round(start), durationSeconds: round(end-start), + sourceIn/sourceOut if both non-null}.
3. `onTrimSelectedTimelineClip(itemId, update)` → transcript "Trimmed {label} to {A}s-{B}s.".

**queueSelectedSourceVariationSetFromPrompt** (`src/components/workspace/agent-panel.tsx:1795-1815`, P06)
1. Guard: return if no generatedAssetId, no selectedMediaContext, or empty prompt.
2. `defaultVariationDrafts(trimmedPrompt)` → `onQueueSelectedVariationSet(assetId, drafts)`.
3. Transcript "Queued {n} variations for {label}." + followup "Directions: {names}.".

**queueSelectedTimelineClipVariation** (`src/components/workspace/agent-panel.tsx:1817-1842`, P06)
1. Guard: generated asset id, clip context, non-empty prompt, handler.
2. `onQueueSelectedVariation(assetId, prompt)` → transcript "Queued a variation for {label}.".

**startNewChat + onCreateAgentSession** (`src/components/workspace/agent-panel.tsx:396-403,2257`, P06)
1. Clear prompt, transcript state, `clearStoredTranscriptEntries(key)`, local request key, mention state.
2. `onCreateAgentSession()`.

**selectVideoProfile** (`src/components/workspace/export-sheet.tsx:221-240`, P07)
1. Store profile id for destination.
2. `profileQualityAvailability(profile)` → choose quality (current → final → draft → current).
3. `setQuality` if changed; `defaultResolutionForQuality(nextQuality, {width,height}, resolutionOverridden, current)`.

**submitExport** (`src/components/workspace/export-sheet.tsx:286-297`, P07)
1. Guard: return if !outputProfile.available, !canSubmit, or busy.
2. `onExport({destination, profileId, quality, resolution, width, height (from dimensionsForResolution), resolutionOverridden})` (promise not awaited; busy/error/progress come back via props).

**Export footer Cancel** (`src/components/workspace/export-sheet.tsx:517-524`, P07)
1. If busy && onCancel → `onCancel()` (cancel render), sheet stays open.
2. Else `onOpenChange(false)`.

**Activity Start workflow** (`src/components/workspace/activity-panel.tsx:222-238`, P07)
1. Visible only for job with startRequest in queued/blocked.
2. Disabled if no handler, Temporal report not ready/feature disabled, or job id in startingWorkflowIds.
3. `onStartWorkflow(job)`.

### preview-panel.tsx, timeline-preview-compositor.tsx

**retryCanonicalPreview** (`src/components/workspace/preview-panel.tsx:821`, P04)
1. `setFailedCanonicalFrameUrl(null)`.
2. `onRetryCanonicalPreview?.()` (parent prop, no direct invoke). This handler is passed to the compositor only when the parent provides `onRetryCanonicalPreview`.

**Compositor "Retry preview"** (`src/components/workspace/timeline-preview-compositor.tsx:814-820`, P04)
1. If `missingCanonicalLayers.length > 0`, call `onRetryCanonicalPreview?.()` (routed to PreviewPanel's `retryCanonicalPreview`).
2. Otherwise `setFailedLayerIds(new Set())`, which remounts and reloads the failed media and audio layers.

**toggleSourcePlayback** (`src/components/workspace/preview-panel.tsx:640`, P04)
1. Guard: toggle allowed and a media element exists.
2. If `previewPlaying`: call `el.pause()` and `setPreviewPlaying(false)`.
3. Otherwise `el.play()`: on resolve `setPreviewPlaying(true)`, on reject `setPreviewPlaying(false)`.

**finishCanvasInteraction** (`src/components/workspace/timeline-preview-compositor.tsx:608`, P04)
1. Pointer id must match. Call preventDefault and `releasePointerCapture`.
2. If `moved` (>= 2 px): mode rotate calls `onRotationCommit(itemId, currentRotationDegrees)`; a crop edge calls `onCropCommit(itemId, current)`; otherwise `onTransformCommit(itemId, current)`. These become `onRotateTimelineItem`, `onCropTimelineItem` and `onTransformTimelineItem` in PreviewPanel.
3. `setCanvasInteraction(null)`.

**selectCanvasLayer** (`src/components/workspace/timeline-preview-compositor.tsx:624`, P04)
1. Left button only and no active interaction.
2. `topmostTimelinePreviewLayerAtPoint(interactive layers with transient transforms, normalized point, bounds size)`.
3. On a hit: `onSelectItem(itemId)`, then `startCanvasInteraction(event, layer, "move")`. The move is ignored if the layer isn't editable or there is no transform commit.

**Canonical frame preload effect** (`src/components/workspace/preview-panel.tsx:752-802`, P04)
1. Skip if not in timeline mode or there is no window.
2. For each active sequence, collect URLs from index `max(0, cur-2)` to `min(len-1, cur+12)`.
3. For each URL not already loaded or pending: create `new Image()` with async decoding, add it to pending, set `src`, and call `decode()`.
4. On load or decode: move the URL from pending to loaded and evict the oldest while loaded is over 480. On error: remove from pending only.

**useTimelineMediaSynchronization returned callback (onLoadedMetadata)** (`src/components/workspace/timeline-preview-compositor.tsx:227-230`, P04)
1. `synchronizeClock()`: seek if paused, non-finite, or drift > 0.1 s; set clamped volume.
2. `synchronizeTransport()`: pause, or `play()` with a generation guard and a re-pause if desired playing turned false while the play was pending.

**Notes for the rebuild:**
- Neither file has fullscreen, inline text editing, audio fade math, preparation triggers or debounce. Canonical preparation is only received as props (status/message and frame sequences), so preparation triggers must live in the parent.
- The numbers above are for the current HEAD `9854bad8`, not `2c544965` as the brief says.
- The local helpers here (`useTimelineMediaSynchronization`, `renderCaptionTextWithEmphasis`, `sourceViewerTreatment`, `PreviewTransport`, `CanvasSelectionOverlay`) have no copies in `src/lib`, so their logic would be lost with these files.

### captions-workbench.tsx, transcript-panel.tsx, speech-workbench.tsx, effect-catalog-panel.tsx, project-timeline-inspector.tsx, timeline-selection-toolbar.tsx, timeline-item.tsx

No direct invoke/Tauri calls exist in these files; all side effects go through props.

**applyChanges** (`src/components/workspace/transcript-panel.tsx:84`, P05)
1. Return early if changedInputs is empty or any changed word is invalid.
2. Call onApplyMany(changedInputs) once with every changed word as a TranscriptWordRepairInput.
3. Reset drafts to canonicalDrafts(transcript), using the lib helper from `@/lib/captions/transcript-drafts`.

**Start workflow onClick** (`src/components/workspace/project-timeline-inspector.tsx:709`, P07)
1. Re-check canStartWorkflow: startRequest present, canShowStartWorkflowAction(job.status), startRequestStatus(job) is not "mismatch", Temporal is ready and featureEnabled, and the job is not in startingWorkflowIds.
2. If all checks pass, call onStartWorkflow(job). Otherwise do nothing.

**Caption agent menu item** (`src/components/workspace/captions-workbench.tsx:283`, P06)
1. Call onDraftAgentPrompt(action.prompt).
2. Set agentMenuOpen to false.

**Compact selection menu item** (`src/components/workspace/timeline-selection-toolbar.tsx:202`, P03)
1. Call action.run().
2. Set menuOpen to false.
