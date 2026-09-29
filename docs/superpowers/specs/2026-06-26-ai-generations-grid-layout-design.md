# AI Generations Grid Layout

## Context

Palmier shows generated assets inside the media panel as compact thumbnail cards with AI
badges, so generated media feels like part of the editable project library. Video Creater
currently renders imported media in a two-column grid, then renders generated assets below
as a full-width vertical feed. The generated section carries important workflow actions,
so it should not be removed, but its layout can move closer to the media-grid pattern.

## Goal

Restyle the `AI generations` section as compact generated media cards in an adaptive
grid while preserving every existing generated asset action.

## Requirements

- The generated assets region remains accessible as `AI generations`.
- Generated assets render inside a labeled `AI generation grid`.
- Each generated asset card keeps its preview, status, title, placement/destination
  badges, prompt, workflow status, output selection, composer, insert, replace, mock
  complete/fail, and retry actions.
- Completed generated output buttons remain accessible by their existing
  `Select generated output ...` names.
- The generated grid uses media-card spacing and creates multiple columns only when the
  media panel is wide enough for readable generated action cards.
- At the current compact media panel width, generated cards remain readable and do not
  clip action labels.
- No generated asset schema, project action, Temporal workflow, or provider changes.

## Verification

- Media bin tests assert generated assets render in the `AI generation grid`.
- Existing generated asset action tests continue to pass.
- EditorWorkspace integration tests continue to pass for generated media selection and
  workflow actions.
- Browser QA confirms the media panel reads as one compact library rather than an
  imported-media grid followed by a separate generated feed.
