# Palmier Timeline Context Metric Rows

## Context

The compact `Timeline context` block appears beside source-focused editing so editors and agents can keep project facts in view while inspecting media or timeline clips. Its outer shell has already been flattened, but the individual `Duration`, `Format`, `Contents`, `Workflows`, and optional `Viewer` facts still render as bordered mini boxes. In the Palmier reference, right-rail project facts read as direct key/value rows instead of a grid of cards.

## Goal

Render `Timeline context` facts as compact inline rows so the right rail feels like one continuous inspector surface.

## Requirements

- Keep the `Timeline context` region and accessible name.
- Preserve the existing facts and ordering: duration, format, contents, workflows, and optional viewer status.
- Replace the boxed metric presentation with direct key/value rows.
- Metric rows must not use `rounded-md`, `border`, `bg-background`, `p-2`, or shadow classes.
- Keep project identity, split/embedded state, and all `ProjectTimelineInspector` workflow sections unchanged.

## Non-Goals

- No changes to project schema, timeline state, render settings, Temporal workflows, fal.ai generation, or source viewer behavior.
- No changes to the full `Project timeline inspector` project metric boxes in the empty-selection rail.
- No changes to the order or wording of the timeline-context facts.

## Acceptance Tests

- `ProjectTimelineContext` still renders duration, format, contents, workflows, and optional viewer status.
- The timeline-context fact rows are addressable as a `Timeline context facts` group.
- The fact rows do not carry mini-card shell classes such as `rounded-md`, `border`, `bg-background`, `p-2`, or `shadow-sm`.
