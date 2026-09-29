# Gap Closure 05 — NLE Interchange Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Detail level:** task-level with bite-sized TDD steps. Every step names its files, its test command and its commit.

**Goal:** Close VC-024. Exported Premiere XMEML and DaVinci FCPXML should carry clip transitions as faithfully as the formats allow, and DTD validation plus structure checks should prove the XML is well formed:
- FCPXML dips become Fade To Color transitions, using an effect uid verified from a public reference export.
- Wipes become real wipe transitions in XMEML and FCPXML wherever a public reference export verifies the effect id. Otherwise they stay cuts with a note.
- Transitions on FCPXML lanes other than the primary storyline (upper video lanes and audio lanes) are written inside connected secondary storylines, so they survive.
- Transitions on audio tracks are exported as audio crossfades in both formats.
- Exported XMEML and FCPXML are validated against their DTDs with `xmllint`, and a parser checks their structure. The evidence is recorded under `output/`.
- Projects without transitions still export byte-identical XML (golden tests).

**Architecture:**
- **Writer layout.** The writer lives in `src-tauri/src/project/nle_export.rs` (2,706 lines, over the size limit before this plan) plus `src-tauri/src/project/nle_export/transitions.rs`. Task 2 moves the FCPXML spine, title and transition writers into new submodules under `src-tauri/src/project/nle_export/`, with no change in behavior. New logic then goes into files under 600 lines, and `nle_export.rs` only shrinks.
- **Transition spans.** `attach_nle_transitions` already resolves every transition to an `NleTransitionSpan` on its two `NleClip`s. The span's `primary_storyline: bool` becomes `lane: i64` plus the track kind. Each writer then decides per format and kind what to emit.
- **FCPXML storylines.** Per DTD 1.10, `<!ELEMENT spine (%clip_item; | transition)*>`: a `spine` can't sit directly inside a spine. It is an anchor item and has to live inside a clip or a `gap`. A "chain" is a maximal run of clips on one non-zero lane joined by emitted transitions. Each chain is written as `<spine lane="N" offset="…">` and anchored in one of two places:
  - inside the lane-0 primary clip that covers the chain start, with `offset` in that clip's local time;
  - otherwise inside a new `<gap>` in the primary spine that fills the primary-storyline hole at the chain start.

  Clips that belong to no chain keep today's flat `lane="N"` layout. That keeps projects without transitions byte-identical.
- **Validation.**
  - A new Node driver fetches `xmllint` (from the `libxml2-utils` `.deb`, MIT) and the pinned DTDs into `/tmp/vc-smoke-tools`. It then runs an ignored Rust test that exports a corpus, runs `xmllint --dtdvalid` on every file, and writes a report.
  - An always-on Rust test checks the corpus structure with `roxmltree`, which is added as a dev-dependency.
  - The DTDs are Apple-copyrighted, so they are downloaded and pinned by SHA-256 and never committed.

**Tech Stack:** Rust (writer and integration tests in the `project_nle_export` target), `roxmltree` 0.20 (dev-only, already in `Cargo.lock`, MIT OR Apache-2.0), Node 24 scripts with `node --test`, `xmllint` 2.9.14 from Ubuntu noble `libxml2-utils`.

**Spec:** `docs/superpowers/specs/2026-09-16-editor-redesign-gap-closure-design.md`, Decision 17 and workstream 05.
**Backlog:** VC-024.
**Depends on:** nothing in workstreams 01–04. Workstream 06 runs after this plan and re-runs the desktop smoke "NLE XML export" step.

## Global Constraints

- **Commands.** Prefix every shell command with `rtk` (for example `rtk cargo test`, `rtk pnpm …`, `rtk git …`, `rtk grep -rn`). `rg` isn't installed. Run long commands (cargo, the validation driver) in the foreground.
- **Commits.**
  - Use Conventional Commits. End every commit message with the trailer `Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>`.
  - Stage only the files the task names (`rtk git add <paths>`). Never `git add -A`.
  - Never push.
- **File size.** Keep every new or touched source file under 600 lines. The exceptions are files already over the limit (`src-tauri/src/project/nle_export.rs`, `src-tauri/tests/project_nle_export.rs`): add no net lines to them beyond `mod` declarations and call-site wiring.
- **UI tokens.** Use tokens only: no hex colors and no `white/` or `black/` utility fragments. This plan changes no UI; the rule applies if a task ever touches `src/`.
- **Rust/TS lockstep.** New project actions must land in one commit with the TS union member, `applyProjectActionLocally`, the Codex schema, MCP support and the risk classification. This plan adds no project actions and changes no command signatures. If a task finds it needs one, stop and re-plan.
- **License policy.**
  - LGPL GStreamer and WebKitGTK are fine. Nothing GPL may be linked into the app.
  - Test tooling may be MIT/BSD/LGPL: `libxml2-utils` is MIT, and `roxmltree` is a dev-dependency only.
  - The FCPXML and XMEML DTDs are Apple-copyrighted. Download them to `/tmp` for validation and never commit them.
- **Cargo environment.** Every cargo command runs with `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}'`.
- **GES and export runtime** (only for tests that render; `project_nle_export` doesn't):
  - GES tests need `VIDEO_CREATER_RENDER_RUNTIME_ROOT=$HOME/.local/share/com.olhapi.video-creater/render-runtime/linux-3451661e032bbcf9d529a468ad23f69a`. That directory symlinks into `/tmp/vc-deb-root`; if it's missing, rebuild it with `rtk pnpm build:linux-media-runtime --output <dir>`.
  - `project_export` also needs `VIDEO_CREATER_COMPATIBILITY_DECODER`.
- **Local tools.** Missing tools go under `/tmp/vc-smoke-tools` and are extracted from Ubuntu `.deb`s without sudo (`apt-get download` + `dpkg-deb -x`). Never install system packages.
- **Evidence.** Never claim evidence you didn't observe. A validation run that was skipped or couldn't reach its tools is reported as "not run", with the missing prerequisite. Keep output under `output/nle-xml-validation/` (git-ignored).
- **Golden output.** `projects_without_transitions_export_byte_identical_xml` must pass unchanged after every task. The fixture files under `src-tauri/tests/fixtures/nle_export/adjacent-clips-without-transitions.*` are never edited.
- **Reference rule.** A "public reference export" is XML authored by Final Cut Pro, Premiere Pro or DaVinci Resolve and published publicly. Third-party generators (for example `DareDev256/fcp-mcp-server`) may corroborate a reference but can't verify one. An effect id or parameter the writer emits must cite a reference recorded in Task 1's research note.

---

## File Map

### Created
- **`docs/research/2026-09-16-nle-transition-interchange-references.md`** (Task 1): the verified effect ids, parameter shapes, structure references, pinned DTD sources, and an emit-or-cut verdict for each.
- **`src-tauri/src/project/nle_export/fcpxml_spine.rs`** (Task 2): moved `render_davinci_fcpxml`, `push_fcpxml_video_clip`, `push_fcpxml_audio_clip`, `push_fcpxml_asset`, `push_fcpxml_asset_clip`, `fcpxml_visual_clip_children`, `fcpxml_audio_clip_children`, `fcpxml_enabled_attribute`.
- **`src-tauri/src/project/nle_export/fcpxml_titles.rs`** (Task 2): moved `push_fcpxml_text_overlay`, `push_fcpxml_caption`, `push_fcpxml_title`, `fcpxml_text_style_attributes`, `fcpxml_font_family`, `fcpxml_font_face`, `fcpxml_color_string`.
- **`src-tauri/src/project/nle_export/fcpxml_transitions.rs`** (Task 2): moved `push_fcpxml_transition_effects`, `push_fcpxml_transition`, `fcpxml_emits` and the `FCPXML_*_ID` constants out of `transitions.rs`.
- **`src-tauri/src/project/nle_export/fcpxml_storylines.rs`** (Task 7): chain planning, host selection, gap and storyline writing.
- **`src-tauri/tests/project_nle_export/goldens.rs`** (Task 2), plus fixtures `src-tauri/tests/fixtures/nle_export/{caption,text-overlay,effect-keyframes}.{xml,fcpxml}`.
- **`src-tauri/tests/project_nle_export/structure.rs`** (Task 3b): `roxmltree` checkers `check_fcpxml_structure(&str) -> Vec<String>` and `check_xmeml_structure(&str) -> Vec<String>`.
- **`src-tauri/tests/project_nle_export/validation.rs`** (Task 3b): corpus, the always-on structure test, and the ignored DTD test.
- `src-tauri/tests/project_nle_export/fade_to_color.rs` (Task 5), `wipes.rs` (Task 6), `storylines.rs` (Task 7).
- **`scripts/nle-xml-validation-sources.mjs`** (Task 3a): pure helpers `PINNED_DTDS`, `extractXmemlV5Dtd(html)`, `sha256Hex(bufferOrString)`, `summarizeValidationResults(results)`.
- **`scripts/nle-xml-validation-sources.test.ts`** (Task 3a).
- **`scripts/nle-xml-validation.mjs`** (Task 3a): the driver, covering tools, DTDs, the cargo run and the report.

### Modified
- `src-tauri/src/project/nle_export.rs`: `mod` declarations and moves in Task 2. Afterwards only call-site wiring.
- `src-tauri/src/project/nle_export/transitions.rs`: XMEML transitions, the attachment, `transition_limitations`.
- `src-tauri/tests/project_nle_export.rs`: `#[path] mod` lines only.
- `src-tauri/tests/project_nle_export/transitions.rs`: helper visibility (`pub(super)`), updated FCPXML expectations.
- `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`: `roxmltree = "0.20"` under `[dev-dependencies]` (Task 3b).
- `package.json`: `verify:nle-xml` script, and the new test file appended to `test:source-quality` (Task 3a).
- `docs/development/runtime-and-verification.md`: a short "NLE XML validation" section (Task 9).
- `docs/product-backlog.md`: the VC-024 row and "Last updated" (Task 9).

### Not touched
`src-tauri/src/main.rs`, `src/lib/project.ts`, `knip.jsonc`, `src/**`. No command, TS wrapper or UI changes.

---

## Parallelization

| Task | Owns (exclusive while running) | Runs after | Can run in parallel with |
| --- | --- | --- | --- |
| 1 Reference research | `docs/research/2026-09-16-nle-transition-interchange-references.md` | none | 2, 3a |
| 2 Goldens + module split | `nle_export.rs`, `nle_export/transitions.rs`, new `fcpxml_{spine,titles,transitions}.rs`, `tests/project_nle_export.rs`, `tests/project_nle_export/goldens.rs`, new fixtures | none | 1, 3a |
| 3a Validation tooling (Node) | `scripts/nle-xml-validation*.{mjs,test.ts}`, `package.json` | none | 1, 2 |
| 3b Structure + DTD harness (Rust) | `tests/project_nle_export.rs`, `tests/project_nle_export/{structure,validation}.rs`, `tests/project_nle_export/transitions.rs`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock` | 2, 3a | 1 |
| 4 FCPXML DTD fixes | `fcpxml_spine.rs`, `fcpxml_titles.rs`, the FCPXML goldens from Task 2, `tests/project_nle_export.rs` (existing caption and overlay assertions) | 3b | 1 |
| 5 Fade To Color dips | `fcpxml_transitions.rs`, `transitions.rs`, `tests/project_nle_export.rs`, `tests/project_nle_export/{transitions,fade_to_color}.rs` | 1, 4 | none |
| 6 Wipes + audio-track kinds | `transitions.rs`, `fcpxml_transitions.rs`, `tests/project_nle_export.rs`, `tests/project_nle_export/{transitions,wipes}.rs` | 5 | none |
| 7 Secondary storylines | `fcpxml_storylines.rs`, `fcpxml_spine.rs`, `fcpxml_transitions.rs`, `transitions.rs`, `tests/project_nle_export.rs`, `tests/project_nle_export/{transitions,storylines}.rs` | 6 | none |
| 8 Evidence run | `output/nle-xml-validation/` (git-ignored); commits only fixes, with the files each fix names | 3a, 7 | none |
| 9 Docs + backlog | `docs/development/runtime-and-verification.md`, `docs/product-backlog.md` | 8 | none |

**Shared with other workstreams:**
- `package.json` (Task 3a), `src-tauri/Cargo.toml` and `src-tauri/Cargo.lock` (Task 3b), and `docs/product-backlog.md` (Task 9) must be serialized with any other workstream task that edits them.
- This plan doesn't touch `src-tauri/src/main.rs`, `src/lib/project.ts` or `knip.jsonc`.

---

### Task 1: Verify the reference effect ids and structures

No product code. This task produces the research note every later task cites. Use `rtk gh search code` (code search allows 10 requests a minute; on HTTP 403, check `rtk gh api rate_limit --jq .resources.code_search` and wait for `reset`), plus `rtk curl -sfL` on raw URLs.

Starting points found while writing this plan. Each must be re-checked, not trusted:
- **FCPXML 1.10 DTD:**
  - URL: `https://raw.githubusercontent.com/CommandPost/CommandPost/e0698cbd6ac0bc13cd8279e115ff05dec296f5a9/src/extensions/cp/apple/fcpxml/dtd/FCPXMLv1_10.dtd`
  - SHA-256: `32cbad28022f9a2033acdc25d0583b16d2e12745dc5efe4fa8f16e27aa59ff53`
  - Header: "FCP XML Interchange Format, Version 1.10", "Copyright (c) 2011-2021 Apple Inc."
  - The writer emits `<fcpxml version="1.10">`, and `src-tauri/src/workflows/mod.rs:5131` depends on that.
- **XMEML v5 DTD:** inline `<pre>` blocks on `https://developer.apple.com/library/archive/documentation/AppleApplications/Reference/FinalCutPro_XML/DTD/DTD.html`. The fifth DTD starts at the block containing "Copyright 2009 Apple Inc." / "Interchange Format v5.0". A Python extraction produced SHA-256 `8b2702330d6739eb97b3f523cb7916c5fa52de53bb3371b57cd31acfd5977921`, but the value depends on how the blocks are joined, so Task 3a pins the value its own extractor produces.
- **Fade To Color:**
  - `https://raw.githubusercontent.com/CommandPost/FCPCafe/main/docs/learn/immersive.md` quotes a DaVinci Resolve-authored `Info.fcpxml` (version 1.11). It contains `<effect name="Fade To Color" id="r2" uid="FxPlug:F779C565-486D-4633-8035-0374B4DB8F5C"/>` and `<transition offset="19/2s" name="Dip To Color Dissolve" duration="3/1s"><filter-video ref="r2" name="Transition"><param value="0 0 1 1" name="color" key="3"/></filter-video></transition>`.
  - The dip color used in that timeline isn't known, so the `value` encoding is unverified.
- **Cross Dissolve:** `FxPlug:4731E73A-8DAC-4113-9A30-AE85B1761265` appears in the same Resolve export. `FFAudioTransition` ("Audio Crossfade") appears in `TheAcharya/OpenFCPXMLKit` `Tests/FCPXML Samples/FCPXML/TimelineWithSecondaryStoryline.fcpxml` (FCP-authored, MIT repo).
- **Secondary storylines:** that OpenFCPXMLKit sample shows `<spine lane="1" offset="1388387/12000s">` inside a primary `asset-clip`, with a `<gap>` inside it. It also shows connected clips hosted by a primary-spine `<gap>`.
- **Basic Title:** `.../Titles.localized/Bumper:Opener.localized/Basic Title.localized/Basic Title.moti` appears in `CommandPost/CommandPost` `src/plugins/finalcutpro/toolbox/titlestokeywords/templates/empty.fcpxml` and in several FCP exports on GitHub.
- **Wipes:**
  - No public export with FCP's basic "Wipe" FxPlug uid was found. `vjeux/fcp-headless-transitions` `fct/slug_map.json` lists only Motion-template wipes (for example `Wipes.localized/Mask.localized/Mask.motr`).
  - No XMEML export with a wipe `effectid` was found. `OpenTimelineIO/otio-fcp-adapter` `tests/sample_data/premiere_example.xml` has only `Cross Dissolve`.

Steps:
- [ ] **Confirm each item** against the raw source and record, for every item:
  - the exact element text to emit;
  - the source URL, pinned to a commit where the host allows it;
  - the authoring app, and how you know it (file comments, `<!DOCTYPE>`, app-specific metadata, surrounding prose);
  - the repository license;
  - a verdict: **emit** or **cut + note**.
- [ ] **Fade To Color color encoding.** Find a second reference that pins down the `color` param encoding and key: a Resolve or FCP export whose dip color is known (black or white). Record the `value` for black and for white. If you can't find one, the verdict is:
  - dip to black emits Fade To Color **without** a color param (black is the default of FCP's Fade To Color and of Resolve's Dip To Color Dissolve; record the source for that default);
  - dip to white keeps a cross dissolve, with the note "dip-to-white transitions (exported as cross dissolves)".
- [ ] **Wipes, XMEML.** Look for a Premiere Pro or Final Cut Pro 7 export containing a `<transitionitem>` with a wipe. Candidates:
  - effectid `Wipe` or `Edge Wipe`, category `Wipe`;
  - the `wipecode` and the parameters that give a hard-edged left-to-right wipe, which is the renderer's wipe: the incoming clip is revealed where `x < p * W`, per `src-tauri/src/render_pipeline/gstreamer_transitions.rs`.

  Suggested searches:
  - `rtk gh search code "<effectcategory>Wipe</effectcategory>"`
  - `rtk gh search code "Edge Wipe" --extension xml`
  - the Apple FCP7 XML reference archive
  - Premiere XML samples in OTIO / `otio-fcp-adapter`
- [ ] **Wipes, FCPXML.** Look for an FCP or Resolve export with a wipe transition, and record its uid and direction params. If none is found, the verdict is cut + note.
- [ ] **Anchored offsets on a retimed parent.** Find an FCP export where a connected clip or storyline hangs off a clip with a `timeMap`, and record whether the anchor `offset` is in the parent's adjusted local time. Adjusted local time matches `fcpxml_clip_start`, which is `source_in / speed`. If this can't be verified, the verdict is: chains whose covering primary clip is retimed stay cuts, with the note "transitions connected to a retimed clip (exported as cuts)".
- [ ] **Audio-only storylines.** Find a reference transition in an audio-only connected storyline and record whether it carries only `<filter-audio>`. If none is found, emit `<filter-audio>` only (the DTD allows `filter-video?`) and record that this is the fallback.
- [ ] **Write the note:** `docs/research/2026-09-16-nle-transition-interchange-references.md`.
  - A summary table: item, format, verdict, emitted text, source.
  - A "Not verified" section listing every item whose verdict is cut + note.
  - A DTD sources section with URL, SHA-256 and copyright, and the rule "download only, never commit".
- [ ] **Commit** (stage only the note): `docs(research): record verified NLE transition effect references`

### Task 2: Capture more goldens, then split the FCPXML writer

- [ ] **Capture goldens at the current HEAD, before moving any code.**
  1. Add a temporary test at the end of `src-tauri/tests/project_nle_export.rs` that writes both formats for `sample_project_with_caption()`, `sample_project_with_text_overlay()` and `sample_project_with_effect_keyframes()` to `src-tauri/tests/fixtures/nle_export/{caption,text-overlay,effect-keyframes}.{xml,fcpxml}`.
  2. Run it once with `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_nle_export zz_capture -- --test-threads=1`.
  3. Delete the temporary test.
  4. Check that `rtk git status --short` shows only the six new fixture files.
- [ ] **Golden test.** Create `src-tauri/tests/project_nle_export/goldens.rs` with `feature_projects_export_byte_identical_xml`. It uses `assert_eq!` against `include_str!` of the six fixtures and calls the parent module's sample builders through `super::`. Add `#[path = "project_nle_export/goldens.rs"] mod goldens;` to `src-tauri/tests/project_nle_export.rs`. Run the target; it must pass.
- [ ] **Move code with no behavior change.**
  - `nle_export.rs` declares `mod fcpxml_spine; mod fcpxml_titles; mod fcpxml_transitions;`.
  - Move the functions listed in the File Map. Give the moved items `pub(super)` and import their helpers from `super::` (`push_text_element`, `push_number_element`, `xml_escape`, `seconds_to_frames`, `clip_export_note`, `nle_clip_limitation_note`, `fcpxml_time_map_node`, `fcpxml_clip_start`, `fcpxml_static_crop_node`, `fcpxml_transform_node`, `fcpxml_opacity_node`, `fcpxml_static_audio_volume_node`, `file_name`).
  - `transitions.rs` keeps `NleTransitionSpan`, `attach_nle_transitions`, `expanded_track_transitions`, the XMEML functions and `transition_limitations`. Update its module docs to point to `fcpxml_transitions.rs` for FCPXML.
- [ ] **Verify:**
  - `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_nle_export -- --test-threads=1` (all 59 existing tests plus the new golden test pass)
  - `rtk cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`
  - `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' rtk cargo clippy --manifest-path src-tauri/Cargo.toml --lib --test project_nle_export -- -D warnings`
  - `rtk wc -l src-tauri/src/project/nle_export.rs src-tauri/src/project/nle_export/*.rs`: every new file is under 600 lines, and `nle_export.rs` is shorter than 2,706.
- [ ] **Commit** (stage `src-tauri/src/project/nle_export.rs`, `src-tauri/src/project/nle_export/{transitions,fcpxml_spine,fcpxml_titles,fcpxml_transitions}.rs`, `src-tauri/tests/project_nle_export.rs`, `src-tauri/tests/project_nle_export/goldens.rs`, the six fixtures): `refactor(nle): split the FCPXML writer into modules behind golden exports`

### Task 3a: Validation tooling (xmllint and pinned DTDs)

- [ ] **Failing tests first.** Write `scripts/nle-xml-validation-sources.test.ts` (`node:test` and `node:assert/strict`, following `scripts/linux-desktop-smoke-denoise.test.ts`):
  - `extractXmemlV5Dtd(html)`:
    - Given a small HTML string with two `<pre>` DTD groups (a v4 header and a "Copyright 2009 Apple Inc." v5 header, entity-escaped `&lt;!ELEMENT xmeml …&gt;`), it returns only the v5 text, unescaped, joined with `\n`.
    - It throws `"XMEML v5 DTD not found"` when the header is missing.
    - It throws when the result lacks `<!ELEMENT xmeml` or `<!ELEMENT transitionitem`.
  - `sha256Hex("abc")` equals `ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad`.
  - `summarizeValidationResults([{ file, format, exitCode, stderr }…])` returns `{ passed, failed, failures: [{ file, stderr }] }`, and an empty list yields `{ passed: 0, failed: 0, failures: [] }`.
  - `PINNED_DTDS.fcpxml` has the pinned CommandPost commit URL and SHA-256 from Task 1. `PINNED_DTDS.xmeml` has the Apple URL and a 64-hex `sha256`.
- [ ] **Run the tests; they fail:** `rtk node --test scripts/nle-xml-validation-sources.test.ts`.
- [ ] **Implement `scripts/nle-xml-validation-sources.mjs`.** Use only Node built-ins (`node:crypto`). Unescape `&lt; &gt; &amp; &quot; &#39;`, and strip tags inside `<pre>` blocks.
  - Pin the XMEML SHA-256 to the value your extractor produces on the live page, and assert it in the test. If Task 1 is already committed, Task 9 adds the pinned value to the research note. Never amend.
  - Tests pass.
- [ ] **Implement `scripts/nle-xml-validation.mjs`** (driver, `shell: false` everywhere).
  - **Arguments:** `--tools-dir` (default `/tmp/vc-smoke-tools`), `--output` (required, for example `output/nle-xml-validation/2026-09-16`) and `--help`. Ignore a literal `--`, which pnpm may forward.
  - **xmllint:**
    1. Use `xmllint` from `PATH` if `xmllint --version` exits 0.
    2. Otherwise use `<tools>/libxml2-utils/usr/bin/xmllint` if it exists.
    3. Otherwise run `apt-get download libxml2-utils` with `cwd` `<tools>/debs`, then `dpkg-deb -x <deb> <tools>/libxml2-utils`.

    Record the path, the `--version` first line, the `.deb` name and its SHA-256. This was checked while planning: `libxml2-utils_2.9.14+dfsg-1.3ubuntu3.8_amd64.deb` extracts an `xmllint` that runs against the system `libxml2.so.2`.
  - **DTDs:** download both into `<tools>/nle-dtd/FCPXMLv1_10.dtd` and `<tools>/nle-dtd/xmeml-v5.dtd` with `fetch`, and verify the pinned SHA-256s. A mismatch fails with both hashes.
  - **Run:** `cargo test --manifest-path src-tauri/Cargo.toml --test project_nle_export validation::nle_exports_validate_against_the_dtds -- --ignored --exact --test-threads=1`, with these environment variables:
    - `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}'`
    - `VIDEO_CREATER_XMLLINT=<abs path>` (new)
    - `VIDEO_CREATER_NLE_DTD_DIR=<tools>/nle-dtd` (new)
    - `VIDEO_CREATER_NLE_VALIDATION_OUTPUT=<abs output>` (new)
  - **Report:** read `<output>/results.json` and write `<output>/report.json` with `generatedAt`, `xmllint`, `dtds` (url, sha256, `"Apple copyright; downloaded for validation, not committed"`), `cargo` (argv, exit code), `summary`.
  - **Exit code:** non-zero when a download fails, the cargo run fails, `results.json` is missing, or any file failed. The console states "not run" plus the reason whenever validation didn't actually execute.
- [ ] **`package.json`:** add `"verify:nle-xml": "node scripts/nle-xml-validation.mjs"`, and append `scripts/nle-xml-validation-sources.test.ts` to `test:source-quality`.
- [ ] **Verify:**
  - `rtk pnpm test:source-quality`
  - `rtk pnpm check:tooling-source`
  - `rtk pnpm check:unused`
  - `rtk node scripts/nle-xml-validation.mjs --help` exits 0
- [ ] **Commit** (stage `scripts/nle-xml-validation-sources.mjs`, `scripts/nle-xml-validation-sources.test.ts`, `scripts/nle-xml-validation.mjs`, `package.json`): `build(nle): fetch xmllint and pinned NLE DTDs for XML validation`

### Task 3b: Structure checks and the DTD harness (Rust)

- [ ] **Dev-dependency.** Add `roxmltree = "0.20"` to `[dev-dependencies]` in `src-tauri/Cargo.toml`. Build with `TAURI_CONFIG=… rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_nle_export --no-run`, then confirm `rtk git diff src-tauri/Cargo.lock` only adds `roxmltree` to the app package's dependency list (the version is already locked at 0.20.0).
- [ ] **Helper visibility.** In `src-tauri/tests/project_nle_export/transitions.rs`, make `media_clip`, `adjacent_clips_project`, `add_transition`, `audio_track_index` and `export` `pub(super)`. There is no other change.
- [ ] **Checker tests first.** `structure.rs` gets unit tests over hand-written XML strings. Each rule gets one passing and one failing example.

  **FCPXML rules:**
  1. The root is `fcpxml` with `version="1.10"`.
  2. Every `ref` attribute resolves to an `id` under `resources`.
  3. Every `<transition>` has a `spine` parent and is neither that spine's first nor last element child, and both neighbors are story elements (`asset-clip`, `gap`, `title`, `clip`).
  4. Every `<spine>` other than the sequence's spine has a `lane` attribute and a parent that is `asset-clip`, `gap`, `title` or `clip`.
  5. Children of a lane storyline carry no `lane` attribute.
  6. Handles:
     - For a transition between asset-clips A and B in one spine, with offset `t0` and duration `d` (all times parsed from `N/Ds`, `Ns` or `0s` as exact rationals), A's media must extend through the transition: `A.start + (t0 + d - A.offset) <= asset(A).duration`.
     - B must have head room: `B.start - (B.offset - t0) >= asset(B).start`, which defaults to 0.
     - Skip this check for asset-clips with a `timeMap`.
  7. A transition whose neighbors are all audio-only assets (no `hasVideo`) carries no `filter-video`.

  **XMEML rules:**
  1. The root is `xmeml` with `version="5"`.
  2. In every `<track>`, each `<transitionitem>` is immediately preceded by a `<clipitem>` whose `<end>` is `-1` and followed by one whose `<start>` is `-1`.
  3. Every `-1` edge touches a `<transitionitem>`.
  4. A transition effect has `effecttype` `transition`, and a `mediatype` that matches the enclosing `video`/`audio` section.
  5. A transition's `start < end`.
  6. `clipitem` ids are unique, and every `linkclipref` resolves to a `clipitem` id.
  7. `0 <= in < out <= file duration` for every clipitem.

  Keep `structure.rs` under 600 lines.
- [ ] **Corpus.** Implement `validation.rs` `fn corpus() -> Vec<(String, VideoProject)>`:
  - Sample builders: `sample_project_with_caption`, `_text_overlay`, `_effect_keyframes`, `_generated_clip`, `_audio_clip`, `_image_clip`, `_lottie_clip`.
  - `adjacent_clips_project(false)` and `adjacent_clips_project(true)` without transitions.
  - For each `TransitionKind` (`Crossfade`, `DipToBlack`, `DipToWhite`, `Wipe`):
    - a transition on track 0 of `adjacent_clips_project(false)` (primary);
    - the same on `adjacent_clips_project(true)` (lane 1);
    - a transition on the audio track (`a-1`/`a-2`, 0.5 s).
  - A three-clip chain with two crossfades on the lane-1 track.
  - A retimed host: `up-1` with `speed` 2 covering the chain start.
  - The nested-sequence case from `nested_sequence_transitions_are_carried_into_the_flattened_export`.
- [ ] **Always-on test `nle_exports_have_consistent_structure`.** For every corpus case and both formats, run the matching checker and collect `"{case}.{ext}: {problem}"`. Assert the list is empty.
  - The checkers must pass on today's output. The rules don't check clip overlap inside an XMEML track, because the current writer puts every video track into one `<track>` (see Risks).
  - If a rule fails on today's output, record the failure in the commit body and hand the fix to the task that owns that behavior: Task 4 for FCPXML DTD issues, Tasks 5–7 for transitions. Exclude the case from this assertion through a `const KNOWN_UNTIL_TASK_N: &[&str]` list, which the owning task deletes. Don't mark the case `#[ignore]`.
- [ ] **Ignored test `nle_exports_validate_against_the_dtds`** (`#[ignore = "needs VIDEO_CREATER_XMLLINT, VIDEO_CREATER_NLE_DTD_DIR and VIDEO_CREATER_NLE_VALIDATION_OUTPUT; run rtk pnpm verify:nle-xml"]`).
  1. Panic with that message when any variable is unset. Never pass silently.
  2. Write `corpus/<case>.xml|.fcpxml`.
  3. Run `Command::new(xmllint).args(["--noout", "--dtdvalid", dtd, file])` with `FCPXMLv1_10.dtd` or `xmeml-v5.dtd`.
  4. Write `results.json` (`[{case, format, file, dtd, exitCode, stderr}]`).
  5. Assert every exit code is 0, listing the failures.
- [ ] **Register** the modules in `src-tauri/tests/project_nle_export.rs` (`#[path = "project_nle_export/structure.rs"] mod structure;` and the same for `validation`).
- [ ] **Observe the red run.** Run `rtk pnpm verify:nle-xml --output output/nle-xml-validation/task-3b`. The DTD run is expected to fail on today's FCPXML. These violations were observed while planning:
  - `element title: validity error : Element title does not carry attribute ref` (caption and text overlay);
  - `Element asset-clip content does not follow the DTD … got (adjust-volume note )` (any clip with a note: dips, wipes, lane notes, effect keyframes).

  All XMEML files were valid against the v5 DTD. Paste the observed summary into the commit body.
- [ ] **Verify:**
  - `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_nle_export -- --test-threads=1` (the always-on tests pass)
  - fmt
  - clippy as in Task 2
- [ ] **Commit** (stage `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `src-tauri/tests/project_nle_export.rs`, `src-tauri/tests/project_nle_export/{structure,validation,transitions}.rs`): `test(nle): check exported XMEML and FCPXML structure and DTD validity`

### Task 4: Fix the FCPXML DTD violations that predate this plan

These fixes change output for projects that have notes or titles. They don't affect `adjacent-clips-without-transitions.*`.

- [ ] **Failing tests.** In `src-tauri/tests/project_nle_export.rs`, update the existing assertions:
  - `exports_davinci_fcpxml_with_caption_titles` expects `<title name="Hook caption" ref="vc-title-basic" offset="12/24s" duration="36/24s">`.
  - `exports_davinci_fcpxml_with_text_overlay_titles_above_video` expects `<title name="Manual callout" ref="vc-title-basic" lane="1" offset="72/24s" duration="48/24s">`.
  - Add `fcpxml_declares_the_basic_title_effect_once_when_titles_exist`: it expects `<effect id="vc-title-basic" name="Basic Title" uid=".../Titles.localized/Bumper:Opener.localized/Basic Title.localized/Basic Title.moti"/>`, using the uid verified in Task 1, exactly once. A project without titles has no such line.
  - Add `fcpxml_asset_clip_notes_come_first`: in `sample_project_with_effect_keyframes()` output, `<note>` directly follows the `<asset-clip …>` open tag.
- [ ] **Run; they fail:** `TAURI_CONFIG=… rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_nle_export -- --test-threads=1`.
- [ ] **Implement.**
  - `push_fcpxml_asset_clip` (`fcpxml_spine.rs`) writes the note before the children.
  - `push_fcpxml_title` (`fcpxml_titles.rs`) writes `ref="vc-title-basic"` after `name`.
  - `render_davinci_fcpxml` declares the Basic Title effect after the transition effects and before the assets, only when captions or text overlays exist.
- [ ] **Regenerate only the affected FCPXML goldens** (`caption.fcpxml`, `text-overlay.fcpxml`, `effect-keyframes.fcpxml`) with the capture method from Task 2. Then `rtk git diff --word-diff src-tauri/tests/fixtures/nle_export/` must show only the added `ref`, the added effect line and the moved note. The `.xml` goldens and `adjacent-clips-without-transitions.*` must be unchanged.
- [ ] **Remove** any Task 3b `KNOWN_UNTIL_TASK_4` exclusions.
- [ ] **Verify:**
  - The target passes.
  - `rtk pnpm verify:nle-xml --output output/nle-xml-validation/task-4` shows no `title` or `note` validity errors. The remaining failures, if any, must all be transition cases owned by Tasks 5–7; list them in the commit body.
- [ ] **Commit** (stage `src-tauri/src/project/nle_export/{fcpxml_spine,fcpxml_titles}.rs`, `src-tauri/tests/project_nle_export.rs`, `src-tauri/tests/project_nle_export/validation.rs` if exclusions changed, and the three `.fcpxml` goldens): `fix(nle): write DTD-valid FCPXML notes and titles`

### Task 5: FCPXML dips as Fade To Color

Follow the Task 1 verdict. The shapes below assume the Resolve-authored reference, and the Task 1 note wins where it differs.

- [ ] **Failing tests** in the new `src-tauri/tests/project_nle_export/fade_to_color.rs`, registered in `tests/project_nle_export.rs`:
  - `fcpxml_dip_to_black_writes_a_fade_to_color_transition` (`adjacent_clips_project(false)`, 1 s dip on `item-1`/`item-2`):
    - resources contain `<effect id="vc-transition-fade-to-color" name="Fade To Color" uid="FxPlug:F779C565-486D-4633-8035-0374B4DB8F5C"/>` and the audio crossfade effect, and **no** cross dissolve effect;
    - the spine has `<transition name="Dip to Color" offset="84/24s" duration="24/24s">` with `<filter-video ref="vc-transition-fade-to-color" name="Fade To Color">`, the verified black color param or none, and the `<filter-audio …>` line;
    - no "dip-to-color" note remains.
  - `fcpxml_dip_to_white_follows_the_verified_color_param`:
    - if Task 1 verified the white value: the same shape with the white `<param …/>`;
    - otherwise: a cross dissolve, plus the note "dip-to-white transitions (exported as cross dissolves)" on both clips.
  - `fcpxml_declares_only_the_transition_effects_it_uses`:
    - a crossfade-only project keeps today's two effect lines byte for byte;
    - a crossfade and a dip on separate cuts (three clips) declare all three effects once.
  - Update `fcpxml_places_cross_dissolve_transitions_in_the_spine` in `transitions.rs` to loop over `Crossfade` only. Its expected text is unchanged.
- [ ] **Run; they fail.**
- [ ] **Implement in `fcpxml_transitions.rs`.**
  - Add an `FcpxmlVideoTransition` enum (`CrossDissolve`, `FadeToColor { color: Option<&'static str> }`), resolved from `NleTransitionSpan.kind`.
  - `push_fcpxml_transition_effects` scans the emitted spans and declares the effects in a fixed order: cross dissolve, fade to color, audio crossfade.
  - `push_fcpxml_transition` writes the kind-specific `name`, `filter-video` and param.
  - In `transitions.rs`, `transition_limitations` drops the dip label and adds the white fallback label only when that fallback applies.
  - Update the module docs in both files and cite the research note.
- [ ] **Verify:**
  - The target passes, including `goldens` and `projects_without_transitions_export_byte_identical_xml`.
  - `rtk pnpm verify:nle-xml --output output/nle-xml-validation/task-5`: the dip cases pass the DTD check.
- [ ] **Commit** (stage `src-tauri/src/project/nle_export/{fcpxml_transitions,transitions}.rs`, `src-tauri/tests/project_nle_export.rs`, `src-tauri/tests/project_nle_export/{fade_to_color,transitions}.rs`): `feat(nle): export FCPXML dips as Fade To Color transitions`

### Task 6: Wipes, and audio-track transitions of every kind

The renderer applies an equal-power crossfade to the audio of every transition kind (`gstreamer_transitions.rs`: "Audio: outgoing gain `cos(p * pi / 2)`…"), and project validation allows any kind between audio clips. XMEML therefore writes `Cross Fade (+3dB)` for audio-track and linked-source-audio transitions of every kind, wipes included. FCPXML audio lanes get the same treatment in Task 7.

- [ ] **Failing tests.** Create `src-tauri/tests/project_nle_export/wipes.rs` and move `wipes_export_as_cuts_with_a_limitation_note` there from `transitions.rs`. Then split it by verdict:
  - XMEML wipe verified: `xmeml_wipes_write_the_verified_wipe_transition`.
    - The `<transitionitem>` sits between `item-1` and `item-2` with the Task 1 element text (name, effectid, effectcategory, wipecode, direction params) and `mediatype` `video`.
    - The clip bounds match the crossfade case: `item-1` `0,-1,48,156`; `item-2` `-1,192,132,240`.
    - The linked source audio gets `Cross Fade (+3dB)`.
  - XMEML wipe not verified: `xmeml_wipes_export_as_cuts_with_a_limitation_note`, keeping today's XMEML assertions. The linked source audio still gets its crossfade.
  - The same split for FCPXML: `fcpxml_wipes_write_the_verified_wipe_transition`, or `fcpxml_wipes_export_as_cuts_with_a_limitation_note` (today's FCPXML assertions).
  - `xmeml_audio_track_transitions_of_every_kind_write_cross_fades`: on the audio track, `Crossfade`, `DipToBlack`, `DipToWhite` and `Wipe` each give a `Cross Fade (+3dB)` between `a-1` and `a-2`, with no limitation note.
- [ ] **Run; they fail.**
- [ ] **Implement.**
  - In `transitions.rs`, replace `xmeml_emits(span)` with `xmeml_emits(span, media)`. Audio media always emits. Video emits unless the kind is a wipe without a verified XMEML effect.
  - `push_xmeml_transition` writes the verified wipe effect.
  - `xmeml_source_audio_transitions` uses audio media, so linked source audio crossfades under wipes too.
  - `transition_limitations(clip, format)` labels wipes per format, and only where that format cuts them.
  - `fcpxml_transitions.rs` gets the FCPXML wipe effect if verified.
  - Update the "Wipes" module docs.
- [ ] **Verify:**
  - The target passes, including both golden tests.
  - `rtk pnpm verify:nle-xml --output output/nle-xml-validation/task-6`
- [ ] **Commit** (stage `src-tauri/src/project/nle_export/{transitions,fcpxml_transitions}.rs`, `src-tauri/tests/project_nle_export.rs`, `src-tauri/tests/project_nle_export/{wipes,transitions}.rs`). Use `feat(nle): export verified wipe transitions and audio-track transitions of every kind`, or, if neither wipe verdict is "emit", `fix(nle): export audio-track transitions of every kind as crossfades`.

### Task 7: FCPXML connected secondary storylines for upper and audio lanes

- [ ] **Failing tests** in the new `src-tauri/tests/project_nle_export/storylines.rs`, registered in `tests/project_nle_export.rs`. Times are 24 fps frames. In `adjacent_clips_project(true)`, `item-1`/`item-2` sit on lane 1, `up-1` on lane 0 at `[24, 72)`, and `a-1`/`a-2` on lane -1.
  - `fcpxml_upper_lane_transition_writes_a_gap_hosted_storyline` (a 1 s crossfade on `item-1`/`item-2`). No primary clip covers frame 0, so the primary spine gains the lines below, and `up-1` stays at `offset="24/24s"`:
    ```xml
    <gap name="Gap" offset="0/24s" duration="24/24s">
      <spine lane="1" offset="0/24s">
        <asset-clip name="Clip item-1" ref="media-1" offset="0/24s" duration="96/24s" start="48/24s">…
        <transition name="Cross Dissolve" offset="84/24s" duration="24/24s">…
        <asset-clip name="Clip item-2" ref="media-1" offset="96/24s" duration="96/24s" start="144/24s">…
      </spine>
    </gap>
    ```
    Neither clip has a `lane` attribute inside the storyline, and no "outside the primary storyline" note remains.
  - `fcpxml_storyline_anchors_inside_the_covering_primary_clip`: `up-1` is moved to `[0, 192)` with `sourceIn` 2 s.
    - Its `asset-clip` contains `<spine lane="1" offset="48/24s">`, which is the parent `start` (48) plus the delta (0).
    - A second case starts the chain 1 s into `up-1` and expects `offset="72/24s"`.
    - No gap is written.
  - `fcpxml_storyline_on_a_retimed_host` (`up-1` with `speed` 2), following the Task 1 verdict:
    - verified: the offset equals `format_fcpxml_rational_time(source_in_frames * den + delta_frames * num, fps * num)`, consistent with `fcpxml_clip_start`;
    - not verified: the transitions stay cuts, with the retimed-host note on both clips.
  - `fcpxml_audio_lane_transition_writes_an_audio_storyline` (a 0.5 s crossfade on `a-1`/`a-2`): `<spine lane="-1" offset="0/24s">` with `a-1`, then `<transition name="Audio Crossfade" offset="66/24s" duration="12/24s">` containing only `<filter-audio ref="vc-transition-audio-crossfade" name="Audio Crossfade"/>` (per the Task 1 audio verdict), then `a-2` with `audioRole="dialogue"` kept. Audio-lane transitions of every kind produce this shape.
  - `fcpxml_chained_transitions_share_one_storyline`: three clips and two crossfades on lane 1 produce one `<spine lane="1">` with two `<transition>`s.
  - `fcpxml_unchained_lane_clips_keep_the_flat_layout`: a lane-1 clip that isn't joined by a transition stays a flat `<asset-clip … lane="1" …>` in the primary spine.
  - `fcpxml_primary_lane_transitions_are_unchanged`: this is the existing `fcpxml_places_cross_dissolve_transitions_in_the_spine` expectation, kept as is.
  - Replace `fcpxml_transitions_outside_the_primary_storyline_export_as_cuts` in `transitions.rs` with `fcpxml_upper_and_audio_lane_transitions_are_emitted`: `adjacent_clips_project(true)` with both transitions gives 2 `<transition` in FCPXML and zero "outside the primary storyline" notes, and XMEML still has 3 `<transitionitem>`.
- [ ] **Run; they fail.**
- [ ] **Implement.**
  - **`transitions.rs`.** `NleTransitionSpan` replaces `primary_storyline` with `lane: i64` and `track_kind: TrackKind`, both set in `attach_nle_transitions`. `transition_limitations` drops the non-primary label, and keeps the retimed-host label only for the unverified fallback.
  - **`fcpxml_storylines.rs`** (new, under 600 lines):
    - `pub(super) struct FcpxmlChain { lane: i64, start_frames: i64, clip_indices: Vec<usize> }` holds maximal runs on `lane != 0` where every consecutive pair shares an emitted span.
    - `pub(super) enum FcpxmlChainHost { Clip { clip_index: usize }, Gap { gap_index: usize } }`. The host is the first lane-0 video clip with `start <= chain.start < end`. Otherwise the host is a gap covering `[earliest chain start in that hole, next lane-0 clip start or the sequence end)`, shared by every chain starting in the same hole.
    - `pub(super) struct FcpxmlStorylinePlan { chains, hosts, gaps, chained: BTreeSet<usize> }`
    - `pub(super) fn plan_fcpxml_storylines(clips: &[NleClip<'_>], sequence_frames: i64) -> FcpxmlStorylinePlan`
    - `pub(super) fn push_fcpxml_gap(…)`
    - `pub(super) fn push_fcpxml_storyline(xml, chain, host_local_offset, clips, fps, indent)`: writes clips and transitions with offsets relative to the chain start.
    - Unit-test the planner inside this file (`#[cfg(test)]`): hole detection, shared gaps, covering clip, chains split by a missing transition.
  - **`fcpxml_spine.rs`.**
    - Plan the storylines once in `render_davinci_fcpxml`.
    - Write gaps before the video clip loop, ordered by offset.
    - Skip chained clips in the flat video and audio loops.
    - `push_fcpxml_video_clip` and `push_fcpxml_audio_clip` take an `FcpxmlPlacement { lane: Option<i64>, offset_frames: i64, indent: usize }`. Existing call sites pass today's values, so their bytes don't change.
    - The children builders append anchored storylines after the intrinsic params (DTD order: `note?, timing, intrinsic params, anchor items`).
  - **`fcpxml_transitions.rs`.** `push_fcpxml_transition` takes the offset and media (video writes `filter-video` + `filter-audio`; audio writes `filter-audio` only), and `fcpxml_emits` no longer checks the lane.
  - **Docs.** Update the module docs in `transitions.rs` and `fcpxml_transitions.rs` ("# DaVinci FCPXML") to describe the storylines and cite the research note.
- [ ] **Verify:**
  - `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_nle_export -- --test-threads=1` (all tests, including `goldens::feature_projects_export_byte_identical_xml`, `transitions::projects_without_transitions_export_byte_identical_xml` and `validation::nle_exports_have_consistent_structure`)
  - `TAURI_CONFIG=… rtk cargo test --manifest-path src-tauri/Cargo.toml --lib project::nle_export -- --test-threads=1` (the planner unit tests)
  - fmt and clippy as in Task 2
  - `rtk pnpm verify:nle-xml --output output/nle-xml-validation/task-7`: every corpus file passes
- [ ] **Commit** (stage `src-tauri/src/project/nle_export/{fcpxml_storylines,fcpxml_spine,fcpxml_transitions,transitions}.rs`, `src-tauri/tests/project_nle_export.rs`, `src-tauri/tests/project_nle_export/{storylines,transitions}.rs`): `feat(nle): write upper and audio lane FCPXML transitions as connected storylines`

### Task 8: Evidence run on the final code

- [ ] **Clean validation run.** From a clean worktree (`rtk git status --short` is empty), run `rtk pnpm verify:nle-xml --output output/nle-xml-validation/2026-09-16-final`. Read `report.json` and record:
  - the xmllint version and source;
  - both DTD SHA-256s;
  - the passed and failed counts;
  - the number of corpus files.

  The run must exit 0. If it doesn't, fix the cause in the owning module, with a failing test first, and commit the fix as `fix(nle): <what>` with only its files staged. Then re-run into a new folder. Never edit the report.
- [ ] **Rust suites that consume the writer:**
  - `TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}' rtk cargo test --manifest-path src-tauri/Cargo.toml --test project_nle_export --test project_split --test codex_mcp_server --test codex_app_server --test temporal_workflows -- --test-threads=1`
  - `TAURI_CONFIG=… rtk cargo test --manifest-path src-tauri/Cargo.toml --lib project::nle_export -- --test-threads=1`
  - `TAURI_CONFIG=… rtk cargo run --manifest-path src-tauri/Cargo.toml --bin video-creater-nle-roundtrip-evidence -- --help`. `parse_arguments` handles `--help`, so this confirms the evidence binary still builds and starts.
- [ ] **Frontend gate** (for `package.json` and the scripts): `rtk pnpm verify:frontend`.
- [ ] **Lint gates:**
  - `rtk cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`
  - `TAURI_CONFIG=… rtk cargo clippy --manifest-path src-tauri/Cargo.toml --lib --tests -- -D warnings`
- [ ] **Size check:** `rtk wc -l src-tauri/src/project/nle_export/*.rs src-tauri/tests/project_nle_export/*.rs scripts/nle-xml-validation*.mjs`. Every file this plan created is under 600 lines.
- [ ] **Record** each command's observed result, with pass counts, for Task 9. There is no commit unless a fix was needed.

### Task 9: Documentation and backlog (only after Task 8 passed)

- [ ] **`docs/development/runtime-and-verification.md`.** Add "## NLE XML validation", at most 25 lines, covering:
  - what `rtk pnpm verify:nle-xml --output <dir>` does;
  - the three `VIDEO_CREATER_*` variables;
  - that `/tmp/vc-smoke-tools` holds `xmllint` and the DTDs;
  - that the DTDs are Apple-copyrighted and never committed;
  - that XMEML's v5 DTD is permissive, so the structure test carries most of the XMEML evidence.
- [ ] **`docs/product-backlog.md`.**
  - Set "Last updated" to the date of the run.
  - VC-024 stays `implemented`: the user outcome is opening the XML in Premiere and Resolve, which can't be verified on Linux. Rewrite its Next action to state:
    - what now exports: Fade To Color dips, per the verdict; wipes, per the verdict; lane and audio storylines; audio-track transitions of every kind;
    - the evidence folder `output/nle-xml-validation/2026-09-16-final/`, with the observed pass counts;
    - what remains: a real-app round-trip in Premiere Pro and DaVinci Resolve; every Task 1 "Not verified" item that stayed a cut; and, if still present, the XMEML layout that writes all video tracks into one `<track>` and all audio into one `<track>`.
  - Don't touch other rows.
- [ ] **Commit** (stage those two files; also stage the Task 1 note if Task 3a pinned a new XMEML SHA to record there): `docs(backlog): record NLE transition interchange and DTD validation evidence`

---

## Acceptance

| Gap (VC-024) | Proof |
| --- | --- |
| FCPXML dips use Fade To Color with a verified uid | Task 1 note cites the Resolve-authored reference. `fade_to_color::fcpxml_dip_to_black_writes_a_fade_to_color_transition` and `fcpxml_dip_to_white_follows_the_verified_color_param` pass. Dip corpus cases pass `xmllint --dtdvalid FCPXMLv1_10.dtd` in `output/nle-xml-validation/2026-09-16-final/report.json`. |
| Wipes export as real wipes where verified, otherwise cuts with a note | Task 1 verdicts for XMEML and FCPXML. `wipes::*` tests match each verdict. Wipe corpus cases pass the DTD and structure checks. |
| Non-primary FCPXML lanes keep their transitions | `storylines::*` tests (gap host, clip host, retimed host per verdict, chains, flat unchained clips). `validation::nle_exports_have_consistent_structure` checks anchoring rules 3–6. The DTD run passes the lane corpus cases. |
| Audio lane transitions | XMEML: `transitions::xmeml_audio_transitions_write_equal_power_cross_fades` and `wipes::xmeml_audio_track_transitions_of_every_kind_write_cross_fades`. FCPXML: `storylines::fcpxml_audio_lane_transition_writes_an_audio_storyline`. The audio corpus cases pass DTD validation. |
| DTD/schema validation as evidence | `report.json` from Task 8 with the xmllint version, pinned DTD SHA-256s and zero failures. `nle-xml-validation-sources.test.ts` passes in `test:source-quality`. |
| Byte-identical output without transitions | `transitions::projects_without_transitions_export_byte_identical_xml` (fixtures never edited). `goldens::feature_projects_export_byte_identical_xml`: XMEML goldens unchanged since Task 2; FCPXML goldens changed only by the reviewed Task 4 diff. |
| No regressions | The Task 8 Rust suites, `verify:frontend`, fmt and clippy, all as observed. |

## Risks

- **Fade To Color color encoding.** The only reference found (Resolve `Info.fcpxml`) has `<param name="color" key="3" value="0 0 1 1"/>`, with an unknown dip color. If Task 1 can't pin the encoding, dip to white stays a cross dissolve with a note.
- **Wipe effect ids.** No public reference export with a wipe was found for XMEML or FCPXML while planning. Wipes will likely stay cuts with a note, which closes that part of VC-024 as a documented deviation rather than real wipes.
- **XMEML track layout.** The XMEML writer puts every video track in one `<track>` and every audio track (plus linked source audio) in another, so clips from different tracks overlap inside one track. This predates the plan, the goldens lock it, and it's outside Decision 17. The structure checker skips overlap, and Task 9 records it in VC-024.
- **Real imports.** Resolve and Premiere behavior for gap-hosted storylines and Fade To Color can't be observed on Linux (spec: out of scope). The DTD and structure checks are the substitute evidence.
- **Retimed hosts.** Anchor offsets on retimed hosts depend on the Task 1 verdict. The fallback keeps those transitions as cuts.
- **XML IDs.** Asset `id`s come from media ids and are DTD `ID`s. An id starting with a digit would fail validation (current ids are `media-<uuid>`). The corpus uses fixture ids only.
- **Tooling access.** Network access is needed for `apt-get download` and the DTD downloads, and GitHub code search is limited to 10 requests a minute.
- **Shared files.** `src-tauri/Cargo.toml`, `Cargo.lock` and `package.json` edits must be serialized with other workstreams.
