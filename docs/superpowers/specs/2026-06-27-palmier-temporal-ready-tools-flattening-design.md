# Palmier Temporal Ready Tools Flattening Design

## Context

The Temporal preflight setup-needed path is now flat, including the `Missing tools` rows. The ready path still renders `Required tools available` inside a rounded bordered background block, which reintroduces panel chrome when the workflow runtime is healthy.

For a Palmier-style editor rail, ready-state confirmations should be compact status text. They should not compete visually with timeline, media, generation, or workflow job actions.

## Goal

Flatten the `Required tools available` message shown when Temporal preflight has no missing tools.

## Requirements

- Keep the `Temporal worker preflight` status landmark and ready/setup-needed status pill.
- Keep the visible `Required tools available` message when no tools are missing.
- Remove the message wrapper's `rounded`, `border`, `bg-background`, `px-2`, and `py-1.5` shell classes.
- Keep the missing-tools branch, workflow job cards, start-request context, and `Start workflow` behavior unchanged.
- Do not change Temporal preflight data, worker setup logic, start dispatch behavior, project files, job sorting, or fal.ai generation behavior.

## Verification

- Extend the ready-preflight `ProjectTimelineInspector` test to locate `Required tools available`, assert the wrapper is flat, and prove the workflow start button remains enabled.
- Run the focused ready-preflight test, full inspector suite, TypeScript, `git diff --check`, and the full Vitest suite.

## Self-Review

- No placeholders or unresolved questions.
- Scope is limited to the ready-tools message presentation wrapper.
- Temporal runtime behavior and queue actionability remain unchanged.
