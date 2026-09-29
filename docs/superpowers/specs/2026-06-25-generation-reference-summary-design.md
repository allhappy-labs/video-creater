# Generation Reference Summary Design

## Context

Palmier's generation drawer keeps the selected first frame, last frame, and reference media visible while the user prepares a generation request. Video Creater already supports selecting those media IDs, shows thumbnail slots, serializes them into generation jobs, and now exposes the selected reference state in the active generation recipe.

Superseded by `2026-06-27-palmier-generation-reference-summary-removal-design.md`: the standalone `Generation references` block was removed after the active recipe became the canonical read-only summary.

## Requirements

- Use `Active generation recipe` as the read-only reference summary when the selected mode can use references.
- For video, list First frame, Last frame, and Reference in a compact block.
- For image, list Reference only.
- Each populated entry must show a human label and the media ID or filename so text-file-backed project state is inspectable.
- Empty entries should clearly say `None` without blocking generation.
- Audio generation should not show the reference summary.
- Do not change the project action payload; this is a UI confidence and inspection layer for existing `references`.

## Design

- Add a small helper that formats a selected media ID into a concise label from the media library.
- Render reference state in the active recipe instead of between the prompt field and submit footer.
- Use dense editor styling: small uppercase labels, one row per reference, truncation for long filenames, no nested cards.
- Keep the existing thumbnail slots and "Use selected" buttons as the editing controls; the summary is read-only.

## Acceptance

- A video generation request with selected first frame, last frame, and reference shows all three in the summary.
- An image generation request shows only the reference row.
- Audio mode hides the summary.
- Existing generation request tests continue to prove the same `references` payload is submitted.
