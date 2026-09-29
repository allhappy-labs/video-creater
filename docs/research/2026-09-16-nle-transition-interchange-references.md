# NLE transition interchange references

Date: 2026-09-16. Backlog: VC-024. Plan: `docs/superpowers/plans/2026-09-16-gap-closure-05-nle-interchange.md`, Task 1.

Every effect id, parameter and structure the Premiere XMEML and DaVinci FCPXML writers emit for transitions must cite a row in this note. A **public reference export** is XML authored by Final Cut Pro, Premiere Pro or DaVinci Resolve and published publicly. Third-party generators can corroborate a reference, but they can't verify one.

Research was done with `gh search code` / `gh api search/code`, raw downloads pinned to commits, and web searches. The downloaded files were kept in `/tmp` only.

## Summary

| Item | Format | Verdict | Emitted text | Source |
| --- | --- | --- | --- | --- |
| Fade To Color effect | FCPXML | **emit** | `<effect … name="Fade To Color" uid="FxPlug:F779C565-486D-4633-8035-0374B4DB8F5C"/>` | R1 |
| Dip to black | FCPXML | **emit**, without a color param | `<transition name="Dip to Color" …><filter-video ref="…" name="Fade To Color"/><filter-audio …/></transition>` | R1 (uid), R6 (FCP default black) |
| Dip to white | FCPXML | **cut + note**: cross dissolve | `Cross Dissolve`, note "dip-to-white transitions (exported as cross dissolves)" | color encoding not verified (see Not verified) |
| Cross Dissolve | FCPXML | **emit** (unchanged) | `uid="FxPlug:4731E73A-8DAC-4113-9A30-AE85B1761265"` | R1, R2, R3 |
| Audio Crossfade | FCPXML | **emit** (unchanged) | `<effect … name="Audio Crossfade" uid="FFAudioTransition"/>` | R2, R3 |
| Basic Title | FCPXML | **emit** | `uid=".../Titles.localized/Bumper:Opener.localized/Basic Title.localized/Basic Title.moti"` | R4, R5 |
| Connected secondary storyline in a primary clip | FCPXML | **emit** | `<spine lane="N" offset="…">` as an anchored child of a primary `asset-clip` | R3 |
| Connected items hosted by a primary-spine gap | FCPXML | **emit** | `<gap name="Gap" offset="…" duration="…">` with lane children | R3 |
| Transitions inside a secondary storyline | FCPXML | **emit** | `<transition>` between clips inside `<spine lane="N">` | R3 (sibling samples), R7 |
| Anchor offset on a retimed (timeMap) host | FCPXML | **cut + note** | note "transitions connected to a retimed clip (exported as cuts)" | not verified |
| Transition in an audio-only storyline | FCPXML | **emit, fallback shape**: `<filter-audio>` only | `<filter-audio ref="vc-transition-audio-crossfade" name="Audio Crossfade"/>` | not verified; DTD allows `filter-video?` |
| Wipe | XMEML | **cut + note** | note "wipe transitions (exported as cuts)" | not verified |
| Wipe | FCPXML | **cut + note** | note "wipe transitions (exported as cuts)" | not verified |
| Transition touching a reversed clip | XMEML, FCPXML | **cut + note** | note "transitions touching a reversed clip (exported as cuts)" | neither format writes reverse, and a reversed clip's handles lie past the opposite ends of its forward source window |
| Dip to Color Dissolve with `dipcolor` | XMEML | already emitted; corroborated | `effectid` `Dip to Color Dissolve`, `dipcolor` alpha/red/green/blue 0–255 | R8 |

## References

### R1: DaVinci Resolve `Info.fcpxml` (Fade To Color, Cross Dissolve)

- Source: `https://raw.githubusercontent.com/CommandPost/FCPCafe/b694abba077858a673754e484c85ec69e4964aff/docs/learn/immersive.md`, section "Example FCPXML Metadata". Repository license: MIT.
- Authoring app: DaVinci Resolve. The prose says "When you export an Immersive file from DaVinci Resolve, it gives you a `FCPXML`", and the XML carries `<event name="Timeline 1 (Resolve)">` and Resolve's attribute order. The page contains two exports (`version="1.11"` and `version="1.13"`); both carry the same effects.
- Verbatim lines:
  - `<effect name="Fade To Color" id="r2" uid="FxPlug:F779C565-486D-4633-8035-0374B4DB8F5C"/>`
  - `<effect name="Cross Dissolve" id="r4" uid="FxPlug:4731E73A-8DAC-4113-9A30-AE85B1761265"/>`
  - `<transition offset="19/2s" name="Dip To Color Dissolve" duration="3/1s"><filter-video ref="r2" name="Transition"><param value="0 0 1 1" name="color" key="3"/></filter-video></transition>`
- The page advises setting the dip "to black", but the exported value is `0 0 1 1`. FCP writes FxPlug colors as space-separated RGB floats (for example `<param name="Color" key="3" value="1 0.839877 0.48642"/>` in the FCP-authored R3 samples), where `0 0 1` would read as blue. The dip color of this timeline therefore can't be established, and the `color` value encoding stays unverified.
- Corroboration only: `DareDev256/fcp-mcp-server` `fcpxml/writer.py` (third-party generator) uses the same uid.

### R2: Final Cut Pro `transitions-1-13.fcpxmld/Info.fcpxml`

- Source: `https://raw.githubusercontent.com/limulus/tcx2webvtt/d501129aea387570779359e2437bf2fbfc1a4f1e/fixtures/fcp/transitions-1-13.fcpxmld/Info.fcpxml`. Repository license: MIT.
- Authoring app: Final Cut Pro (an `.fcpxmld` bundle, which only FCP 10.6+ writes; FCP `effectConfig` plist data in the transitions).
- Verbatim lines: `<effect id="r4" name="Cross Dissolve" uid="FxPlug:4731E73A-8DAC-4113-9A30-AE85B1761265"/>`, `<effect id="r5" name="Audio Crossfade" uid="FFAudioTransition"/>`.

### R3: Final Cut Pro sample exports in OpenFCPXMLKit

- Source: `https://github.com/TheAcharya/OpenFCPXMLKit/blob/7e81f55d2ea4914f685314cf2c5d08cdeeb3b6e8/Tests/FCPXML%20Samples/FCPXML/TimelineWithSecondaryStoryline.fcpxml`, and the sibling samples `23.98.fcpxml`, `24.fcpxml`, `30.fcpxml`, `50.fcpxml`, `59.94.fcpxml`, `60.fcpxml`, `TimelineSample.fcpxml` in the same folder (checked at HEAD `bfc95f8e9631453643762e6b97470ccf164468e2`). Repository license: MIT.
- Authoring app: Final Cut Pro (`<library location="file:///Users/user/Movies/Keywords-Within-Folders.fcpbundle/">`, FCP `adjust-colorConform` attributes, `<!DOCTYPE fcpxml>`, `version="1.13"`).
- Structures verified:
  - A connected secondary storyline anchored in a primary clip: an `asset-clip` in the sequence spine contains, after its intrinsic `adjust-*` params, `<spine lane="1" offset="1388387/12000s">` whose children (`asset-clip`, `gap`) carry no `lane` and use offsets relative to the storyline start (the first child at `offset="0s"`).
  - Connected clips hosted by a primary-spine gap: `<gap name="Gap" offset="964964/24000s" start="86400314/24000s" duration="46046/24000s">` contains an `asset-clip` with `lane="1"`.
  - Transitions inside secondary storylines: the frame-rate samples have `<spine lane="1">` holding two `asset-clip`s joined by `<transition>`s with `filter-video` and `filter-audio`; `TimelineSample.fcpxml` has lane 1 and lane 2 storylines with up to three transitions each.
  - `Audio Crossfade` / `FFAudioTransition` is declared in 14 of the samples in that folder.

### R4: CommandPost `empty.fcpxml` (Basic Title)

- Source: `https://raw.githubusercontent.com/CommandPost/CommandPost/9adff66ad81b6e3874cf12305053b572669adcd5/src/plugins/finalcutpro/toolbox/titlestokeywords/templates/empty.fcpxml`. Repository license: MIT.
- Authoring app: an FCP-exported template (FCP `version="1.10"` header and FCP attribute order). It is kept in a tool repository, so R5 carries the verification.
- Verbatim line: `<effect id="r2" name="Basic Title" uid=".../Titles.localized/Bumper:Opener.localized/Basic Title.localized/Basic Title.moti"/>`

### R5: Final Cut Pro exports using Basic Title

- Source: the same OpenFCPXMLKit folder as R3. 19 samples declare the same Basic Title uid, for example `TitlesRoles.fcpxml` (`<library location="file:///Users/user/Movies/FCPXMLTest.fcpbundle/">`, `version="1.11"`) and `TransitionMarkers1.fcpxml` (`version="1.13"`). Repository license: MIT.

### R6: Final Cut Pro Fade To Color defaults to black

- Source: FCP.co forum thread "Fade from/to black", `https://fcp.co/forum/4-final-cut-pro-x-fcpx/29573-fade-from-to-black`. Tom Wolsky (author of Final Cut Pro training books) writes: "You can use the Fade to Color transition, which defaults to black."
- This is an expert statement, not Apple documentation. No Apple page stating the default was found.
- Counter-evidence for Resolve: Resolve's own "Dip To Color Dissolve" defaults to **white** when applied inside Resolve (Envato Tuts+, "How to Quickly Fade To Black in DaVinci Resolve": "Now, it will fade to white. We'll fix that."). How Resolve fills a missing `color` param when it imports a `Fade To Color` transition wasn't observed. This is a known risk of the param-less dip to black, and it can only be settled by a real Resolve import (out of scope on Linux).

### R7: DTD 1.10 content models used by the storyline layout

From the FCPXML 1.10 DTD (see DTD sources):
- `<!ELEMENT spine (%clip_item; | transition)*>`: a `spine` can't be a direct child of a spine.
- `spine` is part of `%anchor_item;`, so a lane storyline is written inside a clip, `gap` or `title`, after that element's timing and intrinsic params.
- `<!ELEMENT transition (filter-video?, filter-audio?, marker*, …)>`: `filter-video` is optional, which allows audio-only transitions.

### R8: XMEML `Dip to Color Dissolve` (corroboration of the existing XMEML dips)

- Source: `https://github.com/bchapman/Qube/blob/97a6f8ebb51f982c23c637ece9d639b939425c53/Apps/Droplet/_extra/GN_L1_Bible.xml` (no repository license; cited, not copied).
- Authoring app: likely Final Cut Pro 7 or Premiere (XMEML `version="4"`, FCP7 `wipecode`/`wipeaccuracy` layout); not proven.
- It shows `effectid` `Dip to Color Dissolve`, category `Dissolve`, and a `dipcolor` parameter with `alpha`/`red`/`green`/`blue` 0–255, matching today's XMEML writer. This plan doesn't change XMEML dips.

## Not verified (cut + note, or fallback)

- **Fade To Color `color` param encoding.** R1 is the only public reference with the param (`key="3"`, `value="0 0 1 1"`), and its dip color is unknown. No FCP- or Resolve-authored export with a known black or white dip was found (`gh search code` for the uid finds only R1 and a third-party generator; "Fade To Color" with `extension:fcpxml` finds nothing). Verdict: dip to black emits Fade To Color with no color param (R6). Dip to white exports as a cross dissolve with the note "dip-to-white transitions (exported as cross dissolves)".
- **Wipes, XMEML.** No Premiere Pro or FCP7 export containing a wipe `<transitionitem>` was found. Searches covered `wipecode`, `xmeml wipecode`, `xmeml transitionitem Wipe`, `xmeml "Edge Wipe"`, `effectid Wipe extension:xml` and `"PR.ADBE" Wipe`. Public XMEML exports found with transitions (`OpenTimelineIO/otio-fcp-adapter` `tests/sample_data/premiere_example.xml`; `jacobmartinez3d/c1_otio` `src/OTIO/timeline.xml`; `nifra-s/short_maker` `maker/video_templates/video.xml`; `perlik/swieta-muzyka` FCP7 timelines; R8) contain only `Cross Dissolve`, `Cross Fade (+3dB)` and `Dip to Color Dissolve`. Apple's FCP XML reference (`Elements.html`) documents `wipecode` as "An integer specifying the SMPTE wipe code" but gives no wipe export. Apple's `AlbumToSlideshow` sample code names "Edge Wipe" only in a comment, and it is a generator. `markreidvfx/aaf_filter_samples` has Avid AAF dumps, not XMEML. Verdict: cut + note "wipe transitions (exported as cuts)".
- **Wipes, FCPXML.** No FCP- or Resolve-authored FCPXML with a wipe was found. `vjeux/fcp-headless-transitions` `fct/slug_map.json` (commit `02a6bbb06897a097faa6b1f624ec0f0319338876`) lists only Motion-template wipes (`Wipes.localized/Mask.localized/Mask.motr`, `Diagonal`, `360° Wipe`), not a hard-edged basic wipe, and has no export. `"Wipes.localized"` has no code-search hits. Verdict: cut + note "wipe transitions (exported as cuts)".
- **Anchored offsets on a retimed parent.** The R3 samples have no `timeMap` host with anchored children. The only public file found with that shape (`stts-se/dtw_markers` `timesync2fcpxml/Harald test - with speedpoints.fcpxml`, `version="1.9"`, Windows `Y:/` media paths, so not FCP and probably not a direct app export) has anchors exactly at timeMap points where `time == value`, so parent-local and source time can't be told apart. Verdict: chains whose covering primary clip is retimed stay cuts, with the note "transitions connected to a retimed clip (exported as cuts)".
- **Transitions in audio-only storylines.** No FCP-authored reference was found. One public file (`se1yu/Video-Editing` `bulletTrain/Info.fcpxml`, Windows media paths, so not FCP; the authoring app is unproven) writes audio-lane transitions named "Cross Fade -3 dB" that reference a Cross Dissolve `filter-video`. That shape isn't adopted. Fallback: audio-only storyline transitions carry only `<filter-audio ref="vc-transition-audio-crossfade" name="Audio Crossfade"/>`, which the DTD allows (R7).

## DTD sources (download only, never commit)

Both DTDs are Apple-copyrighted. They are downloaded into `/tmp/vc-smoke-tools/nle-dtd/` for validation, verified by SHA-256, and never committed.

| DTD | URL | SHA-256 | Copyright |
| --- | --- | --- | --- |
| FCPXML 1.10 | `https://raw.githubusercontent.com/CommandPost/CommandPost/e0698cbd6ac0bc13cd8279e115ff05dec296f5a9/src/extensions/cp/apple/fcpxml/dtd/FCPXMLv1_10.dtd` | `32cbad28022f9a2033acdc25d0583b16d2e12745dc5efe4fa8f16e27aa59ff53` (observed 2026-09-16) | "FCP XML Interchange Format, Version 1.10", "Copyright (c) 2011-2021 Apple Inc. All rights reserved." |
| XMEML v5 | `https://developer.apple.com/library/archive/documentation/AppleApplications/Reference/FinalCutPro_XML/DTD/DTD.html` (inline `<pre>` blocks from the "Copyright 2009 Apple Inc." / "Interchange Format v5.0" block onward) | `7be946ee484917dc1fd641ac3e0988a285a2ed6a3013395f9c9fbaa895169a2b` (the `extractXmemlV5Dtd` output: 289 `<pre>` blocks joined by `\n`, observed 2026-09-16; pinned in `scripts/nle-xml-validation-sources.mjs`) | "Copyright 2009 Apple Inc." |

The writer emits `<fcpxml version="1.10">`, and the DTD declares `<!ATTLIST fcpxml version CDATA #FIXED "1.10">`.
