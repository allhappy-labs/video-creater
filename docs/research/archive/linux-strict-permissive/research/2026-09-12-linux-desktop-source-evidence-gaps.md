# Linux desktop source-evidence gap review

**Reviewed:** 2026-09-12  
**Scope:** twelve exact third-party Cargo source records left outside the first
selected-source batch  
**Source-lock snapshot:**
`30ed5d014ae7093755a2ff29498ab9305d526bcb6a48e382fe1b6d54bcb7493e`  
**Cargo.lock snapshot:**
`7c70b2b9454d99b4767c4b39d86bc568df5bad2cf73f468727b68983ee4a50d2`

This is an engineering evidence review, not a legal opinion. It binds findings
to the immutable crate archives, vendored tree hashes, Cargo VCS identities,
and exact upstream commit files. It does not infer approval from Cargo metadata
alone. The downloaded evidence and structured candidates are ignored files at
`.superpowers/sdd/2026-09-12-linux-desktop-home/source-evidence-gaps/` and
`.superpowers/sdd/2026-09-12-linux-desktop-home/task-1-source-evidence-gap-candidates.json`.

## Outcome

Nine records now have complete evidence for their already-allowed MIT branch:
`defmt-parser`, `iced_debug`, `iced_futures`, `siphasher`, and the five UNIC
crates. Two copied-source terms were identified precisely and approved by the
coordinator as exact atomic policy terms: `BSD-2-Clause-Views` for
`futures-channel`, and `Unicode-DFS-2016` for host-only `regex-syntax`. They must
still pass the separate policy-code review before import. `image` is eligible
only for the feature-bound compiled-target-source route; distributing its whole
vendored archive remains held because that archive includes IJG-licensed JPEG
source.

For `futures-channel` and `regex-syntax`, `MIT` remains the selected branch of
the crates' declared `MIT OR Apache-2.0`. The copied-source terms are typed
additional obligations. The effective artifact obligations are therefore MIT
plus the copied-source term; the conjunction does not replace or reinterpret
the original Cargo declaration.

## Exact upstream license recovery

| Record(s) | Immutable source identity and evidence | Review result |
| --- | --- | --- |
| `defmt-parser@1.0.0` | archive `10d60334…fee3e`, tree `40c81e66…14df9`, `.cargo_vcs_info.json` `7ab8f010…86a05` identifies commit `4a8cdb44891ed57b8ff5a023b6bec7137c48708f` and path `parser`. The exact [parser manifest](https://github.com/knurling-rs/defmt/blob/4a8cdb44891ed57b8ff5a023b6bec7137c48708f/parser/Cargo.toml) declares `MIT OR Apache-2.0`; the exact root [MIT license](https://github.com/knurling-rs/defmt/blob/4a8cdb44891ed57b8ff5a023b6bec7137c48708f/LICENSE-MIT) was retained as evidence SHA-256 `2710a622…b50`. | Select MIT. Preserve the complete upstream MIT text. |
| `iced_debug@0.14.0`, `iced_futures@0.14.0` | archives `25035ab0…a0be` and `8c0c85cc…fdf0`; trees `cb9055e8…5bca` and `36d39dcd…85cd`; VCS evidence `e3fd1b45…aecf` and `4a247950…4709` identifies commit `3997291f318a8bc06fa522f5579836fb3feb94df`, paths `debug` and `futures`. The exact [`iced_debug` manifest](https://github.com/iced-rs/iced/blob/3997291f318a8bc06fa522f5579836fb3feb94df/debug/Cargo.toml) and [`iced_futures` manifest](https://github.com/iced-rs/iced/blob/3997291f318a8bc06fa522f5579836fb3feb94df/futures/Cargo.toml) inherit the [workspace manifest's MIT value](https://github.com/iced-rs/iced/blob/3997291f318a8bc06fa522f5579836fb3feb94df/Cargo.toml); the exact [workspace license](https://github.com/iced-rs/iced/blob/3997291f318a8bc06fa522f5579836fb3feb94df/LICENSE) is evidence SHA-256 `fc9086ba…0398`. | Select MIT for each exact tree. Preserve the complete Iced MIT text. |
| five UNIC `0.9.0` crates | `unic-char-property`, `unic-char-range`, `unic-common`, and `unic-ucd-version` bind to commit `5878605364af97a3358368a6eaef02104af2e016`; `unic-ucd-ident` binds to `8a6ce83063d90b91ae2ce59eddb803edd393fca9`. Both commits contain the same exact [MIT text](https://github.com/open-i18n/rust-unic/blob/5878605364af97a3358368a6eaef02104af2e016/LICENSE-MIT), SHA-256 `23f18e03…4fb50`. Downloaded source headers are byte-identical to the vendored `src/lib.rs` files: property `200af936…0201b`, range `955ee0b9…c3ec`, common `726d9222…9ea0`, ident `b855a13e…c1fea`, version `51d52da8…0f37`. Each header selects MIT or Apache at the user's option and points to the root license files. | Select MIT for each exact host-build tree. Preserve the upstream MIT text and the component header/copyright notice. |
| `siphasher@1.0.3` | archive `8ee5873e…2e649`, tree `b06a9f52…53588`, VCS evidence `67d171b7…b3c5` identifies commit `451f67d73a772cba325728109bbfa247750ed076`. Its exact [COPYING](https://github.com/jedisct1/rust-siphash/blob/451f67d73a772cba325728109bbfa247750ed076/COPYING), SHA-256 `c962ee4d…e1365`, grants the MIT/Apache choice and links the MIT term, while the commit tree contains no separate full license files. The linked [OSI MIT term](https://opensource.org/license/mit) and pinned SPDX 3.28 MIT text, evidence SHA-256 `b05785f9…92b5`, provide the complete selected term. The upstream `src/lib.rs` is byte-identical to the vendored file. | Select MIT for this exact host-build tree. Preserve both `COPYING` (including its two copyright lines) and the full MIT term. This closure depends on the explicit URL incorporated by the commit-bound choice notice; it does not pretend the missing `LICENSE-MIT` exists upstream. |

The five UNIC archive/tree pairs remain those previously measured: property
`a8c57a40…e221` / `8e88785a…116a4`; range `0398022d…1fbc` /
`c5499884…5d51d`; common `80d7ff82…35bc` / `d6843eac…6dcda`; ident
`e230a37c…b6987` / `d7ee6cc7…60f1b`; version `96bd2f22…7b0c4` /
`056b806f…28399`.

## Copied-source term findings

### `futures-channel@0.3.34`

The archive is `b1f9e3d6…393c4`, the tree is `7763fc1c…0c169`, and VCS
evidence `3b311c27…63d1` identifies commit
`705e6b5c0f06535b1aac1cb1989a172b3d45be8c`, path `futures-channel`.
Vendored `src/mpsc/queue.rs` is byte-identical to the [exact upstream
file](https://github.com/rust-lang/futures-rs/blob/705e6b5c0f06535b1aac1cb1989a172b3d45be8c/futures-channel/src/mpsc/queue.rs),
SHA-256 `22034085…ca18e`. It contains the two BSD redistribution conditions,
disclaimer, and final views-and-conclusions sentence. SPDX 3.28's exact
[`BSD-2-Clause-Views`](https://spdx.org/licenses/BSD-2-Clause-Views.html)
definition says this identifier is BSD-2-Clause with that sentence added; its
pinned XML (SHA-256 `ffd93db1…4a31`) marks holder substitutions.

The queue module is compiled: `src/lib.rs` exposes `mpsc`, `src/mpsc/mod.rs`
loads `queue`, and the selected record enables `alloc`, `default`,
`futures-sink`, `sink`, and `std`. The coordinator approved only this exact
atomic term, with no wildcard approval for other BSD variants. The import must
record:

- selected declared branch: `MIT`;
- typed additional compiled-source obligation: `BSD-2-Clause-Views`;
- effective artifact obligations: `MIT AND BSD-2-Clause-Views`;
- source retention of the Dmitry Vyukov notice, conditions, and disclaimer;
- binary reproduction of the same notice, conditions, and disclaimer in the
  documentation or other supplied materials;
- the views sentence in the installed full notice.

If misclassified as ordinary BSD-2-Clause, the shipped notice would omit an
exact upstream sentence and the policy would lose the distinction needed to
reject unreviewed BSD variants. Import is therefore blocked until the separate
policy-code review accepts this exact atom and validates its full notice.

### `regex-syntax@0.8.11`

The archive is `d6f6ff9a…145d4`, the tree is `3ca24675…8e71`, and VCS
evidence `aa8fc3e6…e1b` identifies commit
`140167995737fa11dfe11b8af8b9aa143b790b4e`, path `regex-syntax`.
Vendored `src/unicode_tables/LICENSE-UNICODE` is byte-identical to the [exact
upstream file](https://github.com/rust-lang/regex/blob/140167995737fa11dfe11b8af8b9aa143b790b4e/regex-syntax/src/unicode_tables/LICENSE-UNICODE),
SHA-256 `74db5baf…cfa3`.

The substantive text matches
[`Unicode-DFS-2016`](https://spdx.org/licenses/Unicode-DFS-2016.html). The only
plain-text differences from SPDX 3.28 are wrapping and the copyright year
`1991-2018` in place of `1991-2016`; replacing that year and removing whitespace
makes both byte streams hash to
`354dc1af25a7545f6ea578569a6ce7c9c8a83e62c6e31b714f28c2936aae18f6`.
SPDX's [matching guidelines](https://spdx.github.io/spdx-spec/v3.0.1/annexes/license-matching-guidelines-and-templates/)
allow copyright dates to vary, and the pinned SPDX XML (SHA-256
`41652776…f037`) marks the copyright paragraph separately.

This crate is classified only as `host-build`, with distribution
`development-only-host-tool-source`. `src/lib.rs` unconditionally loads
`unicode_tables`; the enabled `unicode-*` features select the generated table
modules used by `unicode.rs`. The coordinator approved only exact
`Unicode-DFS-2016`, not other Unicode variants. The import must record selected
declared branch `MIT`, typed additional host-source obligation
`Unicode-DFS-2016`, and the effective host artifact obligation
`MIT AND Unicode-DFS-2016`. Preserve the complete 1991-2018 Unicode notice with
copies or in associated documentation, and preserve its name-use restriction;
the license supplies no trademark grant. Import remains blocked until the
separate policy-code review accepts this exact atom and validates its full
notice.

If the table sources were treated as metadata or the newer Unicode-3.0 term,
the result would attach the wrong license to compiled host code and could omit
the exact 1991-2018 notice.

## `image@0.25.10` feature and distribution boundary

The archive is `85ab8039…a6104`, tree `51c66550…4d66`, and VCS evidence
`f231a3e6…6a693` identifies commit
`76e57184f22772dad1138e96954e57945406b15e`. The selected features are exactly
`bmp` and `png`. Vendored `src/lib.rs` is byte-identical to the [exact upstream
module root](https://github.com/image-rs/image/blob/76e57184f22772dad1138e96954e57945406b15e/src/lib.rs),
SHA-256 `6e47fec5…b6419`, and declares `codecs::jpeg` only under
`#[cfg(feature = "jpeg")]`. The IJG-bearing
`src/codecs/jpeg/transform.rs` is byte-identical to the [exact upstream
file](https://github.com/image-rs/image/blob/76e57184f22772dad1138e96954e57945406b15e/src/codecs/jpeg/transform.rs),
SHA-256 `65490ae7…70db`; it is loaded only by the disabled JPEG module.

The coordinator approved a compiled-target-source decision for the exact tree
only when the gate binds the `bmp,png` feature set and records the IJG file as
source-distribution-only evidence. Select MIT for compiled target code. Do not
approve distribution of the crate archive, full vendor tree, source bundle, or
SDK from this finding. Any such distribution requires separate review of the
IJG terms. If the feature binding drifts or JPEG becomes enabled, reset the
approval before build or release. Treating the tree-level scan as compiled-code
reachability would either block eligible BMP/PNG runtime code or, in the other
direction, falsely approve IJG-bearing source redistribution.

## Remaining gates

No source fact remains unresolved in this bounded batch. Three implementation
conditions remain before import: the exact `BSD-2-Clause-Views` policy atom and
notice check, the exact `Unicode-DFS-2016` policy atom and notice check, and the
feature-bound/source-distribution separation for `image`. The two first-party
protocol crates from the earlier seven-record gap list are outside this report
and must use the private-development route.
