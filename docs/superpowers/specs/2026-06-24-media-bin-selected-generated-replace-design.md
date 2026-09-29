# Media Bin Selected Generated Replace Design

Palmier keeps generated source inspection and timeline swap actions in the same editing context. Video Creater already lets editors replace a selected timeline clip from generated asset list rows and from the right-rail Source Inspector. The remaining media-panel gap is the selected generated-source detail block: after an editor opens a generated output for inspection, the list-row replacement action is no longer adjacent to the inspected prompt, references, and variation controls.

## Scope

- When a generated output is selected in the media bin and a retained timeline replacement target exists, show `Replace selected clip` in the selected generated-source detail block.
- The action uses the selected generated output media id and the existing `onReplaceGeneratedOutput` callback.
- The accessible name must include the replacement target label and media id for parity with the generation list row.
- Keep the action on the Details tab so replacement sits beside prompt/provenance inspection; AI Edit remains focused on variation prompting.

## Out of Scope

- New backend replacement project actions.
- Automatic replacement after queued variation completion.
- Imported media replacement.

## Acceptance

- MediaBin tests prove the selected generated-source detail block can replace the retained timeline target.
- EditorWorkspace tests prove the retained timeline target is passed into the selected generated-source detail block after selecting a generated output.
- Existing list-row replacement, insertion, rerun, and variation behavior remains unchanged.
