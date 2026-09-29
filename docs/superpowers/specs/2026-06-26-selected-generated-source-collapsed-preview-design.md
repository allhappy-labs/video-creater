# Selected Generated Source Collapsed Preview Design

## Context

Palmier keeps generated source identity visible while editing: selecting a generated output shows the source tab, preview context, generation details, references, and prompt without forcing the editor away from the timeline. Video Creater already has rich generated source details behind the expandable media-panel source card, but the collapsed selected-source header is mostly text and action buttons. That makes a selected AI output harder to visually confirm before inserting, replacing, rerunning, or opening it in the composer.

## Requirements

- The selected generated source card must show a compact preview of the selected output while collapsed.
- When a safe local preview URL exists, the preview should render the real generated output media.
- If the preview URL is missing or fails, the card must fall back to the existing generated media thumbnail treatment.
- The preview must preserve existing source actions: replace selected clip, insert on timeline, rerun same prompt, and use in composer.
- The expandable details, AI Edit tab, reference cards, workflow status, and prompt copy behavior must remain unchanged.

## Design

- Reuse `selectedGeneratedOutputMedia` and `mediaPreviewUrls` inside `renderSelectedGeneratedSource`.
- Add a compact aspect-video preview block to the selected generated source header when the selected output has a media asset.
- Use the existing `mediaThumbnailTreatment` helper so video/image/audio/generated fallback behavior stays consistent with media tiles and generation history cards.
- Store preview failures in the existing `failedPreviewMediaIds` set so failed real previews automatically revert to deterministic generated treatment.
- Keep labels accessible with an explicit `aria-label` on the preview block; the card's region and action button labels stay stable.

## Verification

- Add a media-bin test that selects a generated output and asserts the collapsed selected-source card shows the real preview URL.
- Add a fallback test that fires a preview error and verifies the selected-source card keeps a generated thumbnail fallback.
- Run the focused media-bin tests.
- Run `pnpm lint`.
- Browser-smoke the editor media panel to check the selected source preview does not crowd the action buttons or details toggle.
