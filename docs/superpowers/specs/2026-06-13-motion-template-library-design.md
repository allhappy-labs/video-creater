# Motion Template Library Design

## Summary

Add a small motion template library to Video Creater, starting with one built-in kinetic title/lower-third template. Users can open a `Templates` tab beside the existing media bin, preview the template, and insert it onto the timeline as an overlay. Codex can reference the same template in structured proposals, but Rust remains the authority that validates template IDs, timing, fields, track compatibility, and visual metadata before canonical project state changes.

The first implementation is deliberately built-in rather than MCP-backed. The catalog shape is provider-oriented from day one so a later MCP server can supply the same template records without forcing the UI, Codex prompt, or Rust validation to change shape.

Jitter's template gallery is the product reference: a browsable, categorized set of customizable motion graphics templates. This design applies that pattern to the editor without importing Jitter templates directly.

## Goals

- Add a `Templates` tab next to `Media` in the left sidebar.
- Seed the catalog with one built-in template: `kinetic-lower-third-v1`.
- Let users insert the template onto the `Overlays` track manually.
- Store inserted template overlays as typed timeline items with template metadata and editable field values.
- Expose the built-in catalog to Codex as available visual layers.
- Let Codex proposals reference templates by ID after a real EDL exists.
- Keep the template contract shaped so a future MCP provider can return the same records.

## Non-Goals

- Importing, scraping, or cloning Jitter templates.
- Implementing a public template marketplace.
- Building the MCP server/provider in the first slice.
- Rendering polished animated template assets in the first slice.
- Building a full inspector for every possible template parameter.
- Allowing visual templates to replace EDL-first rough-cut generation.

## User Workflow

### Manual Template Insertion

1. The user imports or selects media.
2. The user opens the left sidebar `Templates` tab.
3. The app shows the `Kinetic Lower Third` template card with category, duration, intended track, and a compact preview.
4. The user inserts it onto the `Overlays` track, either by drag/drop or by an explicit `Insert` action for the MVP.
5. The timeline shows a distinct overlay item labeled `Kinetic Lower Third`.
6. Selecting the item exposes stored metadata through the existing timeline/inspector path. A richer template inspector can follow later.

### Codex Template Use

1. The user asks Codex to generate or revise an edit.
2. Rust sends Codex bounded project context, edit intent, transcript excerpt, and a summary of available templates.
3. Codex still proposes source clips first. Template overlays are allowed only after the proposal includes a valid EDL.
4. Codex may reference `templateId: "kinetic-lower-third-v1"` with text field overrides, timing, and required visual metadata.
5. Rust validates the template reference and converts it into an overlay timeline item.

## Layout

The left sidebar becomes a source panel with two tabs:

- `Media`: current imported media behavior.
- `Templates`: searchable/filterable template browser, initially with one card.

This keeps the right panel focused on Codex intent, generation state, and proposal review. It also matches the editor mental model: the left side contains things the user can place into the edit, the center contains preview/timeline, and the right side contains agent controls and review.

The template card should be compact and operational rather than decorative. It should show:

- name
- category, such as `Lower thirds`
- intended timeline track, such as `Overlays`
- default duration
- preview treatment
- insert affordance
- disabled or error state when no compatible timeline target exists

## Template Contract

The built-in TypeScript catalog should use an MCP-shaped record:

```ts
export interface MotionTemplateDefinition {
  id: string;
  name: string;
  category: "titles" | "lower_thirds" | "captions" | "callouts" | "transitions";
  kind: "overlay" | "caption" | "hyperframe_scene" | "transition";
  durationSeconds: number;
  defaultTextFields: Record<string, string>;
  preview: {
    thumbnailKind: "css";
    description: string;
  };
  placement: {
    trackKind: "overlay" | "caption" | "hyperframe_scene";
    defaultStartSeconds: number;
  };
  renderContract: {
    dimensions: "project";
    fps: "project";
    alpha: boolean;
  };
  visualTreatment: string;
  motion: string;
  safeZone: string;
  avoid: string;
}
```

The first record is:

```ts
{
  id: "kinetic-lower-third-v1",
  name: "Kinetic Lower Third",
  category: "lower_thirds",
  kind: "overlay",
  durationSeconds: 2.4,
  defaultTextFields: {
    headline: "Name / Role",
    subline: "Context label"
  },
  placement: {
    trackKind: "overlay",
    defaultStartSeconds: 0
  },
  renderContract: {
    dimensions: "project",
    fps: "project",
    alpha: true
  },
  visualTreatment: "compact lower-third block with translucent backing, accent rule, and strong hierarchy",
  motion: "slide-and-fade in over 8 frames, hold, then soft fade out",
  safeZone: "keep essential text inside 10% margins and below face/action priority areas",
  avoid: "full-width opaque black slabs, centered title-card layout, default-font template look, and long unmoving holds"
}
```

## Timeline Representation

Inserted templates use the existing timeline model:

- `kind: "overlay"`
- `source: { type: "generated", artifactId }`
- `label: "Kinetic Lower Third"`
- `properties.templateId`
- `properties.templateFields`
- `properties.visualTreatment`
- `properties.motion`
- `properties.safeZone`
- `properties.avoid`
- `properties.renderContract`

For the MVP, `artifactId` can be a stable synthetic ID derived from the template ID and item ID because the first slice is validating selection, placement, proposal shape, and timeline behavior. The render pipeline can later replace that synthetic reference with a real generated overlay artifact.

## Codex Proposal Contract

Codex proposal overlays should be extended from generic overlay briefs to also allow template references:

```json
{
  "kind": "lower_third",
  "templateId": "kinetic-lower-third-v1",
  "startSeconds": 4.2,
  "durationSeconds": 2.4,
  "fields": {
    "headline": "Olha API",
    "subline": "Founder"
  },
  "brief": "Introduce the speaker without covering hands or face.",
  "visualTreatment": "compact lower-third block with translucent backing, accent rule, and strong hierarchy",
  "motion": "slide-and-fade in over 8 frames, hold, then soft fade out",
  "safeZone": "keep essential text inside 10% margins and below face/action priority areas",
  "avoid": "full-width opaque black slabs, centered title-card layout, default-font template look, and long unmoving holds"
}
```

The app-server prompt should list available templates with IDs, categories, intended track kinds, default duration, required fields, and visual guardrails. Codex must not invent template IDs. Unknown IDs are validation errors.

## Rust Validation

Rust remains authoritative for canonical mutation. Validation rules:

- `templateId` must exist in the known catalog.
- Template kind must match the target track kind.
- This first template can only create `overlay` items on the `Overlays` track.
- `startSeconds` must be zero or positive.
- `durationSeconds` must be positive and should default to the template duration when omitted in a manual insert.
- Required template text fields must be present and non-empty after trimming.
- `visualTreatment`, `motion`, `safeZone`, and `avoid` are required for any Codex-proposed template layer.
- Codex template overlays are rejected unless the proposal also contains a valid EDL for the primary video.
- Overlay timing must fit inside the generated timeline duration or be explicitly clamped by Rust with predictable behavior.

## Error Handling

Manual UI errors should be local and specific:

- incompatible track: show the invalid drop state at the timeline row
- missing text fields: keep the item from being inserted and highlight the field problem
- no project timeline: disable insertion and explain that a timeline is required

Codex proposal errors should be surfaced as proposal validation failures, not silent fallbacks. The user should see that the template reference was invalid and that the canonical timeline was not mutated.

## Testing

TypeScript tests:

- catalog contains `kinetic-lower-third-v1` with required visual metadata
- insertion helper creates an overlay timeline item with template properties
- insertion helper rejects unknown IDs and incompatible target tracks
- timeline adapter displays template overlay items distinctly from captions and video clips

React tests:

- left sidebar switches between `Media` and `Templates`
- template card renders name, duration, category, and insert action
- inserting the template creates/selects an overlay item through the existing timeline patch path or a new narrow insertion callback

Rust tests:

- Codex proposal validation rejects unknown template IDs
- Codex proposal validation rejects template overlays without required visual metadata
- Codex proposal validation rejects template overlays when the EDL is invalid or missing
- valid Codex template overlay data can be converted into a canonical overlay item once the implementation adds that conversion boundary

Render tests are intentionally deferred until template metadata is consumed by the render pipeline.

## Rollout

1. Build the TypeScript catalog and insertion helper.
2. Add `Media`/`Templates` tabs to the left sidebar.
3. Render the first template card and insert it into the overlay track.
4. Style template overlay items in the timeline.
5. Add the catalog summary to Codex app-server context.
6. Extend the Codex proposal schema and Rust validation to accept template references.
7. Later, add a provider boundary for MCP and a renderer that turns template metadata into actual overlay artifacts.

This sequence proves the core product loop before adding external provider runtime complexity.
