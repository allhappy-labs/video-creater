# Browser Transcription Fallback Design

## Context

The standalone browser dev path renders the editor workspace at `127.0.0.1:1420`, but `App.refreshModelStatus` calls Tauri transcription commands on mount. Without the Tauri bridge, those commands reject with `Cannot read properties of undefined (reading 'invoke')`, producing repeated console errors. Palmier-style editing needs browser smoke tests to exercise the manual and agent-editing UI without unrelated runtime noise.

## Requirements

- Browser-only runs should treat missing Tauri transcription commands as a normal unavailable-runtime state.
- Desktop/Tauri command failures that are not bridge-missing errors must still be logged.
- The editor should continue to block source-media edit generation when no active transcription model or runtime is available.
- The model settings screen should render an empty/unavailable state without crashing in browser-only runs.
- The behavior should be covered at the `App` boundary because that is where model status is loaded and passed into editor/settings views.

## Design

- Add an `isTauriBridgeUnavailableError(error)` helper in `src/lib/transcription-models.ts`.
- Detect the missing bridge from known invoke rejection shapes, including the browser error message for undefined `invoke`.
- Update `App.refreshModelStatus` so bridge-missing errors set `models` to `[]`, `activeModel` to `null`, and `runtimeSelection` to `"unavailable"` without console logging.
- Leave all other rejection paths unchanged so real desktop command failures remain visible during development.
- Keep the editor state conservative: `transcriptionModelReady` remains false and `runtimeReady` remains false.

## Verification

- Add an `App` regression test where all transcription invokes reject with the missing-bridge error and assert no console errors are emitted.
- Add an `App` regression test where transcription invokes reject with a normal command failure and assert those failures are still logged.
- Run the new `App` tests, related model settings/agent panel tests, TypeScript lint, and a browser smoke that confirms the console no longer contains transcription bridge errors.
