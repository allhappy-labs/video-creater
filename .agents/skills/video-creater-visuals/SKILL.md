---
name: video-creater-visuals
description: Use when designing or changing Video Creater app UI, editor chrome, popovers, panels, timeline controls, preview surfaces, agent suggestions, shadcn components, Tailwind styling, or visual QA for the Tauri React video editor.
---

# Video Creater Visuals

## Overview

Design the app as a working video editor, not a marketing page. The first screen should make editing, previewing, accepting agent proposals, and rendering feel direct, dense, and reliable.

## When to Use

- React UI, Tailwind, shadcn/ui, or lucide icon changes.
- Popovers, menus, inspector panels, agent suggestion cards, render logs, media import states.
- Timeline rows, clip bodies, resize handles, captions, overlays, selected ranges, or drag previews.
- Preview player, draft artifact links, render status, or empty/error states.

## App Visual Rules

The editor follows the editor UI/UX redesign spec (`docs/superpowers/specs/2026-09-13-editor-ui-ux-redesign-design.md` in the Video Creater repository).

| Surface | Standard |
| --- | --- |
| Layout | One fixed layout: left tabs, center preview, Properties on selection, full-width timeline. No presets, rails, or maximize modes other than preview fullscreen. Below 1024 px the phone layout takes over. |
| Left tabs | Order is AI · Media · Audio · Text · Captions · Effects. AI is always first. Use product words: "AI", "Graphics", "Background tasks", never engine names or internal IDs such as `media-1`. |
| Properties | Opens only while something is selected and closes when the selection clears. It docks as a third column at ≥1280 px and overlays the preview edge from 1024 to 1279 px. Show meaningful controls, never timing fields (start, duration, source in/out); timing lives on the timeline. |
| Tokens | Colors come only from theme tokens: `background`, `foreground`, `panel`, `raised`, `hover`, `line`, `muted`, `muted-foreground`, `dim`, `primary`, `accent`, `accent-soft`, `border`, `input`, `ring`, `card`, `popover`, `secondary`, `destructive`, `warning`, `success`, `keyframe`, and `clip-video`, `clip-text`, `clip-caption`, `clip-audio`, `clip-graphics`. Radii are `rounded-panel`, `rounded-control`, and `rounded-clip`; times use `tabular-time`. No raw hex or `white/…` or `black/…` opacity classes (`scripts/editor-source-policy.test.ts` enforces this). Dark only. |
| Export | One entry point: the Export popover under the top-bar **Export** button (also ⌘/Ctrl+E). Do not add export buttons elsewhere. |
| Jobs | Background tasks (top-bar indicator, popover, and details) are the only job surface for transcription, analysis, generation, render, export, and agent jobs. No separate activity rails or render status buttons. |
| Controls | Use the shadcn/Radix primitives in `src/components/ui` for dialogs, popovers, menus, sheets, and tooltips. Never use `window.prompt` or `window.confirm`. |
| Icons | Use `lucide-react`. Every icon-only control needs an accessible name and a tooltip. |
| Cards | Use cards for repeated items, AI result and review cards, or framed tools only. Do not nest cards or make page sections into floating cards. |
| Density | Prefer compact professional controls over hero-scale text, decorative layouts, or generic SaaS landing patterns. Keep editor files under 600 lines. |
| Motion | Motion should clarify selection, drag, resize, preview, or status. Respect reduced motion. |
| Phone layout | Target the iPhone 17 Pro viewport (402×874). With nothing selected, the bottom tool bar holds the six tabs; with a selection it switches to clip tools. Tabs and tools open bottom sheets: only one sheet at a time, with a grab handle that swipes down to close. Touch targets, including trim handles, transition badge edges, and keyframes, are at least 24 px. Pad top and bottom chrome for `env(safe-area-inset-*)`. This is layout support only; there is no phone runtime. |

## Interaction States

Every editor surface needs states before it is complete:

- default, hover, focus-visible, disabled
- selected clip/range and multi-selection
- drag ghost, resize handle, invalid drop target
- loading/progress, queued, blocked, failed, completed
- empty media bin, no transcript, no draft, no selected item

## Timeline Guidance

- Tracks are dynamic: dropping above, between, or below tracks creates one, and an edit that
  empties a track removes it in the same undo step.
- Track headers show the kind icon and a short name ("Video 1", "Text 1", "Captions",
  "Graphics 1", "Audio 1") with visibility or mute and lock. Headers are hidden on phones, where
  clip color and icon carry track identity.
- Clips use their `clip-*` kind color. Labels truncate only when the clip is too short. Do not add
  status badges to clips.
- Transitions are small badges centered on the cut. Dragging a badge edge changes the duration,
  and clicking the badge opens transition Properties.
- AI change highlights mark the items and ranges an applied agent batch touched, behind the clips
  rather than over them.
- Time and duration are inspectable through tooltips and Set duration, not Properties fields.
- Invalid edits should be communicated at the target, not only in a toast.

## Popovers And Panels

- Popovers should expose one focused decision: choose, inspect, confirm, or fix.
- Keep destructive and render-expensive actions visually distinct.
- Use plain verbs: `Export video`, `Apply`, `Generate & place`, `Show changes`, `Undo`, `Retry`.
- Error copy states what failed and what can be done next.

## Visual QA

After meaningful UI work:

1. Run the narrowest relevant tests, then `pnpm lint` when TypeScript or Tailwind changed.
2. Run `pnpm test:browser` for the Playwright editor flows on the fixture transport, or a single
   spec with `pnpm exec playwright test e2e/<spec>.spec.ts`.
3. Inspect the acceptance captures in `output/editor-acceptance/`. Each flow is captured at
   desktop (1440×900) and iPhone 17 Pro (402×874).
4. Check desktop and phone widths for overlapping text, broken layout, unreadable controls, and blank preview areas.
5. Verify keyboard focus and icon-only labels on changed controls.

Browser fixture flows are the acceptance gate. They do not prove native Tauri or packaged behavior.

## Common Mistakes

- Building a landing page instead of the editor workspace.
- Using decorative cards, oversized headings, or one-note palettes for operational tools.
- Letting popovers describe implementation details instead of user decisions.
- Styling timeline items without considering duration, labels, drag handles, and validation states.
- Adding timing fields to Properties, a second export button, or a job status surface outside Background tasks.
