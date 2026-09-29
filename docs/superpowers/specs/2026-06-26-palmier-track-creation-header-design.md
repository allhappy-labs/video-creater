# Palmier Track Creation Header Design

## Intent

Palmier's timeline toolbar is a compact edit-tool strip. Video Creater still places track
management controls in that strip through the `New track kind` selector and `Add timeline track`
button. Keep manual track creation available, but move it into the track-list header so the primary
toolbar reads as edit tools first.

## Requirements

- The global `Timeline tools` toolbar must not contain the `New track kind` selector or
  `Add timeline track` button.
- The track header exposes a compact `Timeline track creation` group containing the same kind
  selector and add-track button.
- The selector continues to support video, HyperFrames, overlays, captions, and audio.
- The add-track callback still receives the selected track kind and remains disabled when track
  creation is unavailable.
- Existing toolbar controls for undo, redo, select, split, source marks, text, playhead navigation,
  and zoom remain unchanged.

## Verification

- Update `TimelineEditor` tests to assert the primary toolbar omits track creation controls and the
  track header owns them.
- Keep the add-track callback tests passing through the new track-header location.
- Update `EditorWorkspace` layout coverage to assert the center toolbar does not include track
  creation controls.
- Browser-smoke the editor and confirm the track creation controls sit in the left `Tracks` header
  rather than between text and playhead controls.
