# Palmier Composer Footer Metadata Removal Design

## Intent

Palmier's chat composer is compact: the prompt field stays primary, with lightweight controls around it. Video Creater currently adds a separate `EDL agent` footer row inside the composer that repeats edit preset and language metadata already available in the `Edit setup` section above. This consumes vertical space and makes the prompt surface feel like an internal status panel instead of a direct assistant input.

## Requirements

- Remove the `EDL agent` metadata row from the `Codex composer`.
- Keep the prompt label, active `@media` target badge, textarea placeholder, mention suggestions, resolved mention actions, blocked model settings state, and primary action button behavior unchanged.
- Keep edit preset and language controls in the `Edit setup` section.
- Keep the composer as the final rail section.

## Testing

- Update `AgentPanel` coverage to assert the composer no longer renders `EDL agent`.
- Keep coverage proving the composer still shows the active target badge and primary action button.
- Run focused `AgentPanel` tests, lint/typecheck, and browser QA for the left rail.
