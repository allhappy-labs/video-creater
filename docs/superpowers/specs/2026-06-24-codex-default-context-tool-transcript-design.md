# Codex Default Context Tool Transcript

## Problem

Palmier shows agent context work as compact tool rows inside the chat stream. Video Creater already
renders request-specific tool rows after the user queues edits or generations, and it keeps a static
`Codex context tools` panel for scanability. In the idle/default rail, however, the chat stream jumps
from a Codex greeting to the generic readiness message, so the visible conversation does not show the
same context checks that the static panel says are loaded.

## Goal

Show the default context checks as chat-stream tool rows when there is no active local transcript yet:

- `list_models` loaded with `3 model families ready`
- `get_timeline` loaded with the current project/timeline summary
- `project_context` loaded with the current selected media or clip context

## Behavior

- Default tool rows appear after the initial Codex context greeting and before the readiness message.
- Once a user action records its own transcript entries, the default rows hide so request-specific
  tool rows remain unambiguous.
- The static `Codex context tools` panel remains unchanged.
- No backend calls, Temporal workflows, project actions, provider calls, or persisted chat history
  are added.

## Verification

- `AgentPanel` tests prove the idle `Codex chat` region contains the three loaded context tool rows.
- Existing edit/generation transcript tests keep passing with a single request-specific tool row
  where they currently expect one.
- Browser QA confirms the default chat rail shows compact tool rows without text overlap.
