# Luna editor session — local evidence (2026-09-25)

Scope: a Codex Luna agent independently explored the editor in local Chromium. The app's AI requests in these checks used deterministic fixtures, not a live Luna provider. No remote laptop or physical phone was involved.

## Session and findings

- The continuous desktop and 402 × 874 phone journeys passed twice after the final UI change. Each used one project for AI pacing edit, import, split, crossfade and duration, transcription, word repair, caption styling, and export. The regression lives in `e2e/editor-continuous-session.spec.ts`; screenshots are under `output/editor-acceptance/`.
- The existing editor acceptance flows plus the continuous journeys passed together: 18/18 in local Chromium. Luna's independent pass also passed 13/13 browser flows, including timeline, preview/properties, and transitions. No console error, horizontal overflow, or reproducible functional defect was found.
- The phone clip-tools back control was an unlabeled chevron visually. It now displays **Tools**; its accessible name remains **Back to editor tools**. Component tests and the phone Chromium screenshot (`continuous-phone-clip-tools.png`) verify the change.
- In the continuous journey, caption generation after clip splitting produced one cue, rather than the two from the unsplit sample. Code inspection confirmed that the current caption builder intentionally uses the selected forward clip of a source, or its first forward clip. This was a test-assumption mismatch, not a runtime failure. Captioning every occurrence of a source is a separate behavior change, not silently applied here.

## Real host and render

The real Rust-host Playwright suite passed 2/2: desktop edit/reconnect/offline-during-render/download and phone edit/render/download. These tests used a fresh isolated data root to preserve pre-existing E2E project data. Both pipeline reports show a successful 1280 × 720 MP4 at 0.833333 seconds, H.264 video, AAC audio, and present video/audio streams. Each downloaded artifact matched the host-reported output byte count; the reports list output, frame, report, and render-log artifacts. The agent response was scripted by the host E2E fixture, not produced by a live LLM.

Final local gates: 2,685/2,685 Vitest tests, 121/121 source-quality checks, TypeScript and Vite production build, and the continuous Chromium journeys all passed. The first unit and source-quality runs were invalidated by sandbox `EPERM` on child processes and a loopback bind; both suites passed on rerun with the required local permissions.

## Limits

T3 browser preview automation reported no available host, so these checks used local Playwright Chromium. Real second-device Tailscale access, a live app-server Luna conversation, macOS packaging/signing, and physical-device playback were not verified in this pass. The local browser fixture export checks UI state; the separate real-host run validates actual MP4 output.
