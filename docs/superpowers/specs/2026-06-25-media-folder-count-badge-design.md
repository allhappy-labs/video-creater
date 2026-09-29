# Media Folder Count Badge

## Context

Palmier's media library makes folders feel like first-class project assets: folder tiles show a
large folder icon, the folder name, and a compact count badge in the corner. Video Creater already
groups media into folders and computes recursive descendant counts, but the folder count is rendered
as a small secondary text line. That is harder to scan in a dense media grid and reads less like the
reference editor screenshots.

## Goal

Make project library folder cards show their recursive item count as a compact badge while keeping
the existing navigation, grouping, and folder count semantics unchanged.

## Behavior

- Folder cards display the folder icon and folder name on the left.
- The recursive item count appears as a small badge on the right side of the folder card header.
- The badge has an accessible label that includes the full folder path and count.
- Counts remain recursive: a parent folder badge includes media in descendant folders.
- Active folder navigation, child folder cards, and media grouping continue to work as before.

## Non-Goals

- No folder schema change.
- No folder creation, rename, delete, or assignment behavior change.
- No drag-and-drop folder reordering.
- No changes to generated asset folders or generation destination behavior.

## Verification

- A Media Bin test asserts a folder card exposes an accessible count badge with the expected
  recursive count.
- Existing folder navigation and Media Bin tests continue to pass.
- Lint, build, diff, and secret scans pass.
