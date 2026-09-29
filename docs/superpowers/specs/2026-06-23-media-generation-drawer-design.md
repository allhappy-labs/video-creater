# Media Generation Drawer Design

## Context

Palmier's media panel keeps generation close to the library and timeline. The screenshots show a compact generation drawer with Image, Video, and Audio modes, a First/Last tab, a Reference tab, prompt entry, model/settings summary, credits, and a direct queue action. The same top chrome exposes `Import`, `New Folder`, `Generate`, and search.

Video Creater already has the important project plumbing: the media panel can queue `MediaGenerationRequest` values, generation requests create Temporal-shaped workflow jobs, selected media can seed references, and folder actions persist through project actions. The current drawer is functional but shows every video reference slot at once and hides folder creation deeper in the panel.

## Goals

- Make generation feel more like a focused Palmier drawer without changing the project action contract.
- Keep Video generation centered on two decisions:
  - `First/Last` for opening and closing frame control.
  - `Reference` for style or subject consistency.
- Keep Image generation focused on reference selection and prompt/settings.
- Keep Audio generation as prompt/settings only.
- Promote `New Folder` into the media panel's top action row so project organization is one action away.

## Non-Goals

- No drag-and-drop reference assignment in this slice.
- No live provider execution; queued requests still use the existing workflow job and generated asset project actions.
- No persistent drawer preference.
- No changes to canonical project schemas.

## UI Behavior

The media panel top action row becomes:

- `Import`
- `New Folder`
- `Generate`

`New Folder` opens the folder manager when folder callbacks are available. The existing folder create, rename, and delete controls remain in the folder manager.

When the generation drawer is open:

- The Image, Video, and Audio mode controls include icons and accessible labels.
- Video mode shows a compact two-tab reference area:
  - `First/Last`: renders the first-frame and last-frame slots.
  - `Reference`: renders the style/subject reference slot.
- Image mode shows the `Reference` tab content directly.
- Audio mode hides visual slots.
- Queue payloads remain the same:
  - Video includes selected first frame, last frame, and reference media.
  - Image includes reference media only.
  - Audio includes no visual references.

## Data Flow

`MediaBin` continues to own local drawer state. Existing callbacks remain unchanged:

- `onGenerateMedia(request)`
- `onCreateMediaFolder(name)`
- `onRenameMediaFolder(folderId, name)`
- `onDeleteMediaFolder(folderId)`

`EditorWorkspace` remains responsible for turning `onGenerateMedia` into project actions and workflow job records.

## Acceptance Criteria

- Opening the drawer in Video mode shows `First/Last` as the active generation reference tab.
- Switching to `Reference` hides first/last slots and shows the selected reference media.
- Switching back preserves first-frame and last-frame selections.
- Queued video generation still includes first frame, last frame, and reference ids.
- Audio mode hides the reference tabs and visual slots.
- The top `New Folder` action reveals the folder manager and preserves existing folder creation behavior.
- Existing media-bin and editor-workspace tests keep passing.
