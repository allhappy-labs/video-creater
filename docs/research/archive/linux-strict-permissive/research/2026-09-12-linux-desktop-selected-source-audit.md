# Linux desktop selected-source audit: modern permissive batch

Date: 2026-09-12

Source checkpoint: `9a834c31d9fc9414c79a3dfd7a0194c3319b60d8`

Status: **256 exact-source approval candidates; 7 records remain review-required**.
This is an engineering review of exact source and notice evidence for the
project's permissive-only policy. It is not a legal opinion, patent clearance,
release approval, or a blanket approval of any package or later version.

## Scope and result

The reviewed batch contains 263 selected Cargo records whose roles include
`target-normal` or `host-build` and whose declarations are well-formed SPDX
expressions composed only of `MIT`, `Apache-2.0`, `BSD-2-Clause`,
`BSD-3-Clause`, `ISC`, `Zlib`, and `0BSD`. The batch deliberately excludes the
six patched `cosmic-text`/Iced records, `ring`, `regex-syntax`, Roboto,
first-party roots, five MPL host tools, legacy slash declarations, Unicode,
CDLA, and other special expressions assigned to separate reviews.

The exact machine-readable decisions are in the ignored file
`.superpowers/sdd/2026-09-12-linux-desktop-home/task-1-exact-source-review-candidates.json`,
SHA-256 `b0cbd94e1ade87084059d70104a28680b781c3a2cffbf967c704d22ee12d2a4c`.
The file itself is the authoritative structured handoff.

| Result | Count |
| --- | ---: |
| Exact records inspected | 263 |
| Approval candidates | 256 |
| Review-required records | 7 |
| Approved target-only records | 100 |
| Approved target + host records | 46 |
| Approved host-only records | 110 |

The selected branches among the 256 candidates are 241 MIT, five
BSD-3-Clause, four Apache-2.0, three Zlib, two ISC, and one BSD-2-Clause. Each
choice is recorded per source identity; this does not create a global
expression rewrite.

## Evidence boundary and method

The review took an immutable snapshot before the concurrent source-gate
refresh:

| Input | SHA-256 |
| --- | --- |
| `native/linux-desktop/source-lock.json` | `30ed5d014ae7093755a2ff29498ab9305d526bcb6a48e382fe1b6d54bcb7493e` |
| `native/linux-desktop/Cargo.lock` | `7c70b2b9454d99b4767c4b39d86bc568df5bad2cf73f468727b68983ee4a50d2` |

The current working copies changed during the source-gate migration, so the
structured handoff intentionally retains the old hashes. An importer must
preserve a candidate only when its complete identity, archive, current tree,
roles, enabled features, and evidence hashes still match the refreshed record.

For every record, the audit recursively enumerated the exact source tree using
the source-lock algorithm: sorted UTF-8 relative path, NUL, file bytes, NUL,
excluding only `target` and `.git`. Hidden `.cargo-checksum.json` and
`.cargo_vcs_info.json` files therefore remain in the hash. All 263 recomputed
tree hashes match the snapshot. All 261 registry records match both the
`Cargo.lock` archive checksum and `.cargo-checksum.json`; the two local protocol
crates have no registry archive and are among the unresolved first-party gaps.

The inspection covered 11,481 files, including 465 recursively discovered
license-like files and 1,054 files with copyright, SPDX, or license-header
evidence. Full license texts were classified only when the complete grant,
notice-retention condition, disclaimer, and license-specific clauses were
present. Exact evidence hashes are attached to each record. Duplicate license
families were grouped by full-file SHA-256; nonmatching texts, nested files, and
copied/generated-source headers were inspected separately. A mere Cargo
declaration, filename, or tree-hash match was not treated as approval.

## Additional compiled-source obligations

Seven approved records need notices beyond their selected top-level license.
The exact paths and hashes below are also present in the structured handoff.

| Record | Additional obligation and exact evidence |
| --- | --- |
| `libm@0.2.16` | BSD-2-Clause FreeBSD notices in `src/math/exp2.rs` (`e7270f15…`) and `src/math/exp2f.rs` (`3b4b5cb2…`), in addition to selected MIT |
| `schemars_derive@0.8.22` | copied regex-syntax MIT notice in `src/regex_syntax.rs` (`ea417315…`) |
| `schemars@0.9.0` | copied regex-syntax MIT notice in `src/_private/regex_syntax.rs` (`ecbcab18…`) |
| `schemars@1.2.2` | copied regex-syntax MIT notice in `src/_private/regex_syntax.rs` (`ecbcab18…`) |
| `tracing-core@0.1.36` | copied spin MIT notice, `src/spin/LICENSE` (`58545fed…`) |
| `wayrs-client@1.3.1` | generated core-protocol MIT attribution and grant in `wayland.xml` (`d5d0d0b2…`) |
| `wayrs-protocols@0.14.11+1.45` | nested MIT `wayland-protocols/COPYING` (`f1a2b233…`) and the enabled `xdg-shell.xml` attribution/grant (`9d9e2703…`) |

`fontdb@0.23.0`, `harfrust@0.3.2`, and `ttf-parser@0.25.1` contain nested
licenses only in test fonts or helper scripts. They are not compiled in the
reviewed roles. Their exact hashes are retained as source-distribution-only
evidence; redistributing the vendor snapshot must preserve those notices even
though they are not installed-application notices.

## Unresolved records

| Record | Specific gap |
| --- | --- |
| `defmt-parser@1.0.0` | Exact crate has only a manifest declaration; no complete selected-license text or file-level grant. |
| `iced_debug@0.14.0` | Exact crate inherits the workspace license and carries no complete license text or file-level grant. |
| `iced_futures@0.14.0` | Same missing workspace-license evidence as `iced_debug`. |
| `futures-channel@0.3.34` | Compiled `src/mpsc/queue.rs` has a BSD two-clause notice plus an additional views-and-conclusions paragraph. That exact variant is outside the current allowlist. |
| `image@0.25.10` | Exact tree contains an IJG custom-license JPEG source. JPEG is disabled (`bmp` and `png` are enabled), but the source-distribution/reachability exclusion is not encoded in the gate. |
| `video-creater-compatibility-protocol@0.1.0` | Local first-party source needs the private-development identity and boundary route; its manifest MIT string is not third-party license evidence. |
| `video-creater-precompose-protocol@0.1.0` | Same first-party private-development gap. |

The 256 candidates remain conditional on importing their complete bindings,
including nested notice evidence. The seven gaps remain `review-required`.
This batch does not close the records handled by the special-term reviews, the
full selected source closure, installed notices, final ELF reconciliation, or
payload verification.
