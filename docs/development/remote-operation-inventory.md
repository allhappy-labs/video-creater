# Remote operation inventory

The checked source of truth is
`src-tauri/src/app_service/operation.rs`. Every command registered in the production Tauri
handler list must have exactly one row. `pnpm test:remote-operation-inventory` also checks literal
frontend `backendRequest` calls, preventing a new operation from bypassing review.

Each row records remote support, authorization scope, mutation class, whether a project and an
expected revision are required, the maximum decoded request size, and any browser replacement.
`Remote` means eligible for the authenticated gateway; it does not make the command public.
`Internal` remains callable only by trusted host orchestration. `DesktopOnly` is intentionally not
served by the remote RPC gateway.

## Desktop-only behavior

| Desktop behavior | Browser behavior |
| --- | --- |
| Native menu synchronization | Browser keyboard shortcuts and visible controls |
| OS notification permission/capability | Browser Notification API, subject to browser permission |
| Reveal storage or export artifact in the file manager | Authenticated artifact details/download |
| Import a model through a native file picker | Authenticated bounded browser upload |
| Open/save native dialogs (Tauri plugin calls, not commands) | Project browser, upload, and download flows |

The initial boundary contains 121 commands: remote-capable commands plus explicit internal and
desktop-only exceptions. No network listener is introduced by this inventory.

The Phase 1 closeout count remains 121: 89 remote-capable, 26 internal, and 6 desktop-only;
no command was retired. Project create/load/save/validate, project actions and settings, agent
history reads, and job-progress reads now cross `ProjectService`. Media, job-event, export-review,
and agent-proposal contracts are transport-neutral and covered by focused service tests.

## Desktop compatibility defaults

Remote project mutations require the caller's explicit `expectedRevision`. The existing desktop
commands `apply_project_action_to_split_project_folder`,
`apply_project_actions_to_split_project_folder`, and
`update_project_settings_in_split_project_folder` predate that field. Their thin compatibility
adapters read the canonical revision while executing inside the existing per-project FIFO queue and
pass it to `ProjectService`. The remote gateway must never synthesize this value.
