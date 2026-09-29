# Codex Mention Keyboard Navigation Design

## Context

Palmier-style chat editing depends on fast `@` references because the assistant can generate, place, and revise timeline media in context. Video Creater already shows dense mention suggestions and supports Enter/Tab insertion, but keyboard insertion always chooses the first suggestion. Editors need to move through matching media without leaving the prompt.

## Goal

Add keyboard navigation for the Codex composer mention list so editors can choose the intended media reference with ArrowUp/ArrowDown, insert it with Enter/Tab, or dismiss suggestions with Escape.

## Behavior

- Suggestions still derive from `mentionTargets`, match by id, label, or kind, and stay capped to five items.
- When suggestions are visible, the first suggestion is active by default.
- ArrowDown moves the active suggestion forward and wraps from the last suggestion to the first.
- ArrowUp moves the active suggestion backward and wraps from the first suggestion to the last.
- Enter and Tab insert the active suggestion, replacing the current trailing `@` token with `@media-id `.
- Escape hides suggestions for the current token without changing prompt text.
- Editing the prompt clears the dismissed state so suggestions can appear for the next token/query.
- The active option is exposed with `aria-selected` and a visible row state.

## Non-Goals

- No multi-mention request protocol changes.
- No thumbnail rendering in the suggestion list.
- No timeline or project selection side effects.

## Testing

- `AgentPanel` selects a non-first mention suggestion with ArrowDown before Enter insertion.
- `AgentPanel` wraps with ArrowUp from the first suggestion before Tab insertion.
- `AgentPanel` dismisses visible suggestions with Escape while preserving the prompt.
