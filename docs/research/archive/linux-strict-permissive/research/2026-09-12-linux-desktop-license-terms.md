# Linux desktop exact license-term and source-role review

Date: 2026-09-12

Source checkpoint: `f54a1c3103853dcef8f18ea6138450950f94e4a9`

Scope: bounded review of `Unicode-3.0`, `CDLA-Permissive-2.0`, five
`MPL-2.0` host-build packages, and the two unlicensed first-party roots in the
prepared Linux desktop source closure. This is a project-policy review of the
exact sources below, not a general approval of a license family, package, or
later version. It does not close the 327-record source review or establish
patent clearance.

## Decision

The exact `Unicode-3.0` and `CDLA-Permissive-2.0` terms reviewed here meet this
project's permissive-license boundary. Their grants allow commercial use,
modification, and redistribution without reciprocal source-code licensing. The
approval must remain tied to the exact archives, vendored trees, license-text
hashes, roles, and expressions recorded below.

`unicode-ident 1.0.24` is not licensed under Unicode alone. Its declared
expression is `(MIT OR Apache-2.0) AND Unicode-3.0`; an implementation should
record the exact selection `MIT AND Unicode-3.0` (or separately review and
record `Apache-2.0 AND Unicode-3.0`) and preserve both obligations. It must not
turn the `AND` into an `OR` or approve the package merely because one leaf is
already allowlisted.

`webpki-roots 1.0.9` is a generated, compiled-in certificate-data package. Its
exact manifest applies `CDLA-Permissive-2.0`, and the exact upstream commit
links the package license to the repository's `LICENSE-CCADB`. It may be
accepted for this data-bearing role with the full agreement made available in
the installed application's notices. This decision does not approve CDLA for
arbitrary software code or approve any other `webpki-roots` release.

The five `MPL-2.0` packages are not eligible application dependencies and are
not approved for the target or shipped payload. The current receipts classify
them only as host-build tools and show no target-normal reachability. The
specification records build-time-only tools separately and permits them for
development when they do not contribute code or artifacts to the shipped
application. On the current source evidence, these five may compile in the host
tool graph; they are not source-gate blockers solely because the development
machine compiles them. No generated or copied MPL code path into the target was
found in this graph. A final target-binary, generated-output, and package-payload
map must prove that no MPL contribution or artifact ships before the gate can
close. This role decision is not an MPL allowlist.

`linux-desktop 0.1.0` and `video-creater 0.1.0` should be classified as exact
`first-party-private` repository sources rather than third-party packages with
invented MIT terms. That classification requires a separate ownership and
distribution-authorization decision before release; the present development
authorization is not a public-license grant. Any copied or generated
third-party material inside those trees remains subject to its own license
review.

## Evidence snapshot and limits

The reviewed records are bound to these files as they existed at the source
checkpoint:

| Evidence | SHA-256 / count |
| --- | --- |
| `native/linux-desktop/source-lock.json` | `30ed5d014ae7093755a2ff29498ab9305d526bcb6a48e382fe1b6d54bcb7493e` |
| `native/linux-desktop/audit/license-review-required.json` | `8c98b5326a499963a9cb8fac763c8d7f3e956f49f42a7cf692b889e1b4ed3a35`; 327 selected records |
| `native/linux-desktop/audit/license-review-required.md` | `1079ac658d139fdf16dd3816bc45d04d7776d5ef3521de0e0901b49c228e8046` |
| `native/linux-desktop/audit/target-normal.json` | `4008553d9c8dd8fe90ee3173bb5a58c2012a9f8dacd9c5de71bbf6509d0e1778`; 186 records |
| `native/linux-desktop/audit/host-build.json` | `eeee6ee87055e96214af954bb8c5e0431a00ca4c8baa3ca9c27eff2a363fc1be`; 205 records |

The source policy status is `review-required`, with one finding for every one
of the 327 selected records. Sixty-five records have both target-normal and
host-build roles, and Roboto is the one distributed-only asset. Consequently,
the top-level metadata and this bounded review do not establish complete source
closure. No build was run for this review, so claims about final ELF contents,
build-output copying, or installer payloads remain unverified.

## Unicode License v3

Unicode states that its software and data files are generally under the
OSI-approved, highly permissive Unicode License v3 and that the license differs
from MIT principally by expressly covering data files. Its FAQ confirms the
SPDX identifier `Unicode-3.0`. The exact terms grant use, copying, modification,
publication, distribution, and sale, subject to placing the copyright and
permission notice in copies or associated documentation. They disclaim
warranties and restrict promotional use of a copyright holder's name without
authorization. [Official Unicode License v3
text](https://www.unicode.org/license.txt), [Unicode licensing
policy](https://www.unicode.org/policies/licensing_policy.html), and [Unicode
license FAQ](https://unicode.org/faq/unicode_license.html).

The license contains no express patent grant. Its non-infringement language is
a warranty disclaimer, not patent clearance. This source-license decision must
therefore remain separate from any feature-specific patent analysis.

### Exact Unicode source binding

All 18 ICU4X-family license files below are byte-identical, SHA-256
`f367c1b8e1aa262435251e442901da4607b4650e0e63a026f5044473ecfb90f2`.
That text has copyright years 2020-2024, includes the `Unicode-3.0` SPDX marker,
and carries the ICU4C/ICU4J adaptation notice. The official ICU4X repository
publishes the same form and describes ICU4X as released under the Unicode
License. [ICU4X LICENSE](https://github.com/unicode-org/icu4x/blob/main/LICENSE)
and [ICU4X repository licensing statement](https://github.com/unicode-org/icu4x#licensing-and-copyright).

| Exact component and role | Crate archive SHA-256 | Vendored-tree SHA-256 |
| --- | --- | --- |
| `icu_collections 2.3.0` — target + host | `fa68d21081c4a05d5a901a1c62add574c77048b6a1c67be3b50ce0b60d4ca513` | `948868883d85f46c9f35e99aedb19e3a361e4a8d0c7372b8b674e79ea83338c2` |
| `icu_locale_core 2.3.0` — target + host | `d56e28588da92eee5c3201a6eff33fabdd49b62269c8938d4ff050ce4d900deb` | `2f374cbe9a35080f4737a6b2cd915c0bb182cd74276ccf3772483c4b9f59bcb7` |
| `icu_normalizer_data 2.3.0` — target + host | `1563da1ed3e0b3bf3d74c9b85917ac9c56464d2f57242270c09c9e752f8021a0` | `ad9bee28b16f065fdbf26e0b8545262a52bb00f9e71da6d562cc7673a1e73c5e` |
| `icu_normalizer 2.3.0` — target + host | `12f9cf5f235641ed274641dd81c3f28d870e276763d0797aeeab72317b1c646f` | `d7bb6f684058af828b0ee5d5cbe40083f11c6e4db2f7dc5b6d289df6edf45b90` |
| `icu_properties_data 2.3.0` — target + host | `e590f038c1464a96894fd6d10127e90a8be4509f56ff7ecef851b15cee0b7caa` | `ea14c1b0cda0b958902210d00408c3ef6a625d58f455e95ae2d1c917d3092552` |
| `icu_properties 2.3.0` — target + host | `7e7ca276ad3145661a65914e6daf131ca5120cd3dcee8f8f3214b8875184a148` | `eef49589275a940e8ec1f418e25bbf604119c5125fe2380db557604ffebc8d31` |
| `icu_provider 2.3.1` — target + host | `d27bbb9d3abbefac45d55f647c9de1d44aafcd1186eb91879afef17c396c3e73` | `5df3f060dc5794938f83e16f400f1391fb1feb7f5a34d3820cb7865481e19f77` |
| `litemap 0.8.3` — target + host | `47d9d19d1d6efa0109d2f65ff4c85cddd50bd572e5a00127ab10987290bcefae` | `eb90c232b182db39c86b74078a3b0f5cd7ab3aea5f40c565e736ecfe4c0bed0c` |
| `potential_utf 0.1.6` — target + host | `d83eb9bc6d8e5cf568e7a1101d60ee05e81ed50ea106026f3d18deeb046d7661` | `eedc2dbd2c89f319f93a7f95fe3297d4c39fff0302f2b05226df995098eb0539` |
| `tinystr 0.8.4` — target + host | `b1e27c91459209c2986af3dcf603a5a74a4368754ce37414f59acc971167f643` | `50fce0fa0acd893e774f6ab3e14b4558843ef14e21fa61377c066930410564d8` |
| `writeable 0.6.4` — target + host | `3ad82d2a33cdc9674dc7465672f271e096168fcdbe0f799d9e6db8c5892679dc` | `f9c04b2779143d22655ad993a5dc110a65986eb5c9de3186da5528eacf46e004` |
| `yoke-derive 0.8.2` — host | `de844c262c8848816172cef550288e7dc6c7b7814b4ee56b3e1553f275f1858e` | `738903483ec57eca7f0d5121b1518846ef52ff625a8649ccdd2690985c24ac03` |
| `yoke 0.8.3` — target + host | `709fe23a0424b6a435d82152b1bd3fdfb0833487d5fa90d05d42762a9891fef5` | `084aaf1d02288718f315dc712294721f79c91a2bbe39243c10500da60de23662` |
| `zerofrom-derive 0.1.7` — host | `11532158c46691caf0f2593ea8358fed6bbf68a0315e80aae9bd41fbade684a1` | `f653891599c9aec79d7133b28e700efd4e47e58f6a5129b23c728b456ad54b3f` |
| `zerofrom 0.1.8` — target + host | `0ec05a11813ea801ff6d75110ad09cd0824ddba17dfe17128ea0d5f68e6c5272` | `4a3bc89c057e4a4e080a9e0f783f7271c91afaa17ade9b0fb2047ee6c45b871a` |
| `zerotrie 0.2.5` — target + host | `4ea269c3bd32f0a32c321907a2ae912ba6f4649bb0fc764a15627e99a7095a3f` | `af0b983f50653829b801314423fda4260732268bd053ae093d3704fc237bdb8a` |
| `zerovec-derive 0.11.6` — host | `34df6fc39dbd26ddc9c10e6a2984476e13acce22e64e4487636ef494369225da` | `6a71a156c99d01276b169af568ed683f0acde48ad550bb742fbbf0cf01814fb6` |
| `zerovec 0.11.8` — target + host | `bb0464e17806c1d976d5cba29399c7f08e516e279e2ba493f63123b5fca67dd8` | `e0eef6edb339720fc0b848237793aafd1860a8e295250d26b68e172d6904fc3f` |

`unicode-ident 1.0.24` has archive SHA-256
`e6e4313cd5fcd3dad5cafa179702e2b244f760991f45397d14d4ebf38247da75`
and vendored-tree SHA-256
`ead54142a438823e631690d3094bc8b7b2bc4fdc14ce891f5713e1f4c13e7c05`.
Cargo's embedded VCS record binds it to upstream commit
`5b54a632702b5744a1c40ea01c127c0ac0498172`. Its three exact evidence
files hash as follows:

| Evidence file | SHA-256 |
| --- | --- |
| `LICENSE-MIT` | `23f18e03dc49df91622fe2a76176497404e46ced8a715d9d2b67a7446571cca3` |
| `LICENSE-APACHE` | `62c7a1e35f56406896d7aa7ca52d0cc0d272ac022b5d2796e7d6905db8a3636a` |
| `LICENSE-UNICODE` | `f7db81051789b729fea528a63ec4c938fdcb93d9d61d97dc8cc2e9df6d47f2a1` |

The Unicode file is the same substantive v3 license but has the 1991-2023
Unicode copyright notice rather than the ICU4X 2020-2024 notice. The exact
upstream 1.0.24 file confirms those terms. [unicode-ident 1.0.24
LICENSE-UNICODE](https://github.com/dtolnay/unicode-ident/blob/1.0.24/LICENSE-UNICODE).
Both Unicode notice variants must be retained; matching only the SPDX ID would
lose component-specific attribution.

## CDLA-Permissive-2.0 and `webpki-roots 1.0.9`

CDLA-Permissive-2.0 allows a recipient to use, modify, and share Data. Sharing
Data, modified or unmodified, requires making the agreement text available with
the shared Data; Results have no use, modification, or sharing restriction.
The agreement also contains warranty and liability disclaimers. [Official CDLA
Permissive 2.0 text](https://cdla.dev/permissive-2-0/).

The exact component binding is:

| Field | Value |
| --- | --- |
| Component / role | `webpki-roots 1.0.9`; target-normal |
| Source archive | `https://static.crates.io/crates/webpki-roots/webpki-roots-1.0.9.crate` |
| Archive SHA-256 | `7dcd9d09a39985f5344844e66b0c530a33843579125f23e21e9f0f220850f22a` |
| Vendored-tree SHA-256 | `ee0f305c7410f03ae36996e0f692289ae8a468f60008bf2236c899b181e426ee` |
| Cargo VCS identity | commit `0a553dbc8b3f18ea05c4f881cffa3f2d005d0d30`, path `webpki-roots` |
| Vendored `LICENSE` SHA-256 | `e271993808fec50ab29350b39539cdec611a9103f827e0aa26d61da70e2d33f8` |
| Generated `src/lib.rs` SHA-256 | `581232b6fb8d5b8df315d34c798099ee759cb4930ffed77c331e7b38fed82b15` |
| Direct dependency | `rustls-pki-types 1.15.1` |

The exact upstream manifest declares version 1.0.9 and
`CDLA-Permissive-2.0`; its package `LICENSE` symlink resolves to the repository
`LICENSE-CCADB`, whose text matches the vendored agreement. The generated source
identifies itself as a compiled-in copy of Mozilla root certificates generated
from CCADB data. [Exact upstream manifest](https://github.com/rustls/webpki-roots/blob/0a553dbc8b3f18ea05c4f881cffa3f2d005d0d30/webpki-roots/Cargo.toml),
[exact license target](https://github.com/rustls/webpki-roots/blob/0a553dbc8b3f18ea05c4f881cffa3f2d005d0d30/LICENSE-CCADB),
and [exact generated source](https://github.com/rustls/webpki-roots/blob/0a553dbc8b3f18ea05c4f881cffa3f2d005d0d30/webpki-roots/src/lib.rs).

The installed notices must include the full exact CDLA agreement. A short
attribution is insufficient because section 2.1 requires the agreement text to
be made available with shared Data. CDLA-Permissive-2.0 is a data agreement and
contains no express patent grant; this approval is limited to the exact
certificate-data role and is not patent clearance.

## MPL host-build reachability

Each package below is recorded as `host-build` and absent from
`target-normal.json`:

| Package | Archive SHA-256 | Vendored-tree SHA-256 | Exact license evidence |
| --- | --- | --- | --- |
| `cssparser-macros 0.6.1` | `13b588ba4ac1a99f7f2964d24b3d896ddc6bf847ee3855dbd4366f058cfcd331` | `f7b9cfb0095f406e91b4940fb3244090691243ec5a8196e100e143a30b2a9908` | `LICENSE`, SHA-256 `fab3dd6bdab226f1c08630b1dd917e11fcb4ec5e1e020e2c16f83a0a13863e85` |
| `cssparser 0.36.0` | `dae61cf9c0abb83bd659dab65b7e4e38d8236824c85f0f804f173567bda257d2` | `5b480a0eae400073d06441295ef3314bb4f8d377c7651b7e8d1e886a84ef0202` | `LICENSE`, same `fab3dd...63e85` text |
| `dtoa-short 0.3.5` | `cd1511a7b6a56299bd043a9c167a6d2bfb37bf84a6dfceaba651168adfb43c87` | `e0e150c3c70a173ee9b1f055a77ce2c01f06bbcb5ef8eae48ac132cb49426d6d` | `LICENSE`, SHA-256 `1f256ecad192880510e84ad60474eab7589218784b9a50bc7ceee34c2b91f1d5` |
| `option-ext 0.2.0` | `04744f49eae99ab78e0d5c0b603ab218f515ea8cfe5a456d7629ad883a3b6e7d` | `a0eb2ad139f259196e024cf269fcdadb065b377a046eb546f84111845fc563d4` | `LICENSE.txt`, SHA-256 `66a3107d5ad6a058aab753eaac2047ccb2ed0e39465dd0fe5844da3e300d5172` |
| `selectors 0.36.1` | `c5d9c0c92a92d33f08817311cf3f2c29a3538a8240e94a6a3c622ce652d7e00c` | `07e8c60b19aa2001b7f3a953097069b02e55d83adb68ea96ed2eaf4ab91900fc` | manifest declares MPL-2.0; source files carry MPL headers; the crate has no standalone license file |

Their observed paths are:

- `video-creater` build dependency `tauri-build -> dirs -> dirs-sys ->
  option-ext`;
- `video-creater` build dependency `tauri-build -> tauri-utils -> dom_query ->
  cssparser -> cssparser-macros + dtoa-short`;
- the same `dom_query` branch reaches `selectors -> cssparser`.

`selectors` has a build script that writes
`ascii_case_insensitive_html_attributes.rs` into its `OUT_DIR`; `selectors`
then includes that table in its own library. Because `selectors` itself exists
only in the host-build closure, the table is compiled into the host library,
not the Linux target. `cssparser-macros` expands internal cssparser macros into
the host-only `cssparser` library. `dtoa-short` is an ordinary cssparser host
dependency, and `option-ext` is an ordinary `dirs-sys` host dependency. No
copy-to-target instruction or invocation by a target-normal crate was found.

This is source-role evidence, not a license waiver. MPL-2.0 is copyleft at the
covered-file level and remains denied for application code, linked or loaded
libraries, generated target code, and package contents. The specification
already distinguishes nonshipping build tools: these five exact packages may
be recorded as reviewed `development-only-host-tool-source` and compiled while
building the app, without adding MPL-2.0 to the application-license allowlist.

The cost of a wrong role classification is material: a host executable could
be copied into the installer, a build script or procedural macro could emit MPL
covered code into a target crate, or an MPL object could be linked into the
application while top-level metadata still says `host-build`. The final gate
must therefore reconcile target objects, host artifacts, generated outputs,
the target binary, and the complete package payload. Any MPL contribution or
shipped artifact fails that gate and requires removal or replacement.

## First-party private roots

Both `native/linux-desktop/Cargo.toml` and `src-tauri/Cargo.toml` lack a license
declaration, and the repository has no top-level `LICENSE`. The npm package is
marked `private: true`; the Rust package names author `olhapi`; the configured
origin is the private Forgejo namespace
`ssh://git@git.home.olhapi.com:2222/olhapi/video-creater.git`; and nearly all
recorded commits use the `Oleh Vdovenko <o@olhapi.com>` identity. These are
strong provenance indicators but do not independently prove legal ownership or
public-distribution authority.

The exact prepared records are:

| Component / role | Prepared tree SHA-256 | Current source-lock finding |
| --- | --- | --- |
| `linux-desktop 0.1.0` — target-normal | `bc9e07b9c5e8cf32a8b92a8d57cec81b2620db2d141e5c13edd176f6901f4095` | missing license |
| `video-creater 0.1.0` — target-normal + host-build | `77bcc5a3289c9ec235fac46aabcb94aa7e19ef814b8daedc92a45936d72dc431` | missing license |

The source-policy schema should distinguish `first-party-private` provenance
from third-party license eligibility. Such a record should include the exact
repository identity, revision and tree hash, a first-party path boundary, and
separate ownership and release-distribution decisions. A null third-party
license expression may be acceptable only after the owner makes that provenance
decision; it must never become a generic missing-license exception. Nested,
copied, generated, vendored, or submodule content keeps its own third-party
record and cannot inherit first-party status from its directory.

The unresolved user decision is ownership/distribution authorization for these
two first-party components, not a forced MIT relicensing decision. Until that
decision is recorded, both stay `review-required` for release. No existing app
code should be assigned MIT, Apache-2.0, or another public license by inference.

## Bounded policy-change contract

A follow-up policy implementation can use this review without broadening it:

1. Recognize `Unicode-3.0` and `CDLA-Permissive-2.0` as exact atomic terms, but
   require a component review keyed by source ID, version, archive SHA-256,
   vendored-tree SHA-256, exact license-file SHA-256, role, and selected
   expression before clearing a source finding.
2. Approve the 18 enumerated ICU4X-family records under `Unicode-3.0`, with
   their shared license hash and component-specific archive/tree hashes.
3. For `unicode-ident 1.0.24`, record `MIT AND Unicode-3.0`, retain both exact
   texts, and keep it host-build-only. The raw declared `AND` expression cannot
   be shortened to either leaf.
4. Approve only `webpki-roots 1.0.9` at the hashes above for the target-normal
   certificate-data role under `CDLA-Permissive-2.0`; require the complete
   agreement in installed notices.
5. Record the five exact MPL records as reviewed development-only host tools,
   not eligible application dependencies. Permit host compilation only while
   target-binary, generated-output, and package-payload exclusion remains a
   mandatory unverified release gate. Do not introduce an MPL allowlist or an
   app-contribution waiver.
6. Introduce an exact first-party provenance route rather than a license ID for
   the two roots. Require the user's ownership/distribution decision before the
   release gate and continue third-party scanning within those paths.
7. Leave every other one of the 327 review-required records unchanged. This
   memo provides no library-wide or license-family approval beyond the listed
   hashes and roles.

## Pitfalls for the remaining closure review

The remaining audit should account for at least these concrete issues:

- Fifteen records use legacy, non-SPDX slash alternatives (`MIT/Apache-2.0`,
  `Apache-2.0/MIT`, or `Apache-2.0 / MIT`). Their exact license files may support
  an MIT or Apache selection, but the checker must normalize and record that
  choice per source. It must not globally reinterpret `/` or allow the raw
  metadata string. Two more records use `Unlicense/MIT` and can only clear under
  this policy by exact evidence and an explicit MIT selection.
- `ring 0.17.14` declares `Apache-2.0 AND ISC` and is both target-normal and
  host-build. Its prepared source-lock lists `LICENSE`, `LICENSE-BoringSSL`, and
  `LICENSE-other-bits`, but the vendored source also contains
  `third_party/fiat/LICENSE` plus
  `src/polyfill/once_cell/LICENSE-APACHE` and `LICENSE-MIT`. Those omitted files
  must be reconciled with compiled source and notices. The current three-path
  record is not enough to close `ring`.
- `selectors 0.36.1` demonstrates that absence of a standalone license file is
  not absence of license evidence: its manifest and file headers identify
  MPL-2.0. Conversely, a permissive manifest string without matching files and
  source headers is not sufficient approval.
- Every `OR` selection remains component-specific, while every `AND` retains
  all obligations. The current prepared source lock has null selections and
  review decisions, so the 327-record gate remains open after this memo.
