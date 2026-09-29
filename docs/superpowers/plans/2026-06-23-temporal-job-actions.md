# Temporal Job Actions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:test-driven-development for this implementation. Keep project actions atomic and validate before mutating canonical project state.

**Goal:** Make Temporal-backed jobs writable through the validated project action contract.

**Architecture:** Extend the existing `ProjectAction` enum with `RecordJob` and `UpdateJobStatus`. Validation lives in `src-tauri/src/project/action.rs` next to render report and generated asset validation. The frontend mirrors the wire contract in `src/lib/project.ts`.

**Tech Stack:** Rust, serde, TypeScript, Vitest.

## Tasks

- [x] Add failing Rust tests for recording and updating workflow-backed jobs.
- [x] Add failing TypeScript contract coverage for typed `ProjectJobSummary` and job actions.
- [x] Implement Rust action variants, validation errors, and mutation helpers.
- [x] Implement frontend TypeScript job and action types.
- [x] Run focused Rust and frontend tests.
