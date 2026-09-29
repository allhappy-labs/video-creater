# Linux desktop special-source license audit

Date: 2026-09-12

Reviewed source-lock snapshot SHA-256:
`822f43f93d669f33a829eee310fcf223004ca6c54c38596fad583efd24bb2557`.
Reviewed Cargo lock SHA-256:
`c5895b55ac16c2d4f70aa79b84bf85f0d0981c58d556de7aaf89965badd3e7ed`.

Scope: the 58 selected records absent from the earlier 263-record exact-source
candidate set and 12-record evidence-gap set. Three current first-party records
are excluded for the private-development review route. This report closes the
source-license evidence for the other 55 exact records: 54 Cargo source trees
and one Roboto asset. It is an engineering evidence review, not legal advice,
patent clearance, a family-wide license interpretation, or release approval.

The ignored import candidate is
`.superpowers/sdd/2026-09-12-linux-desktop-home/task-1-special-source-review-candidates.json`,
SHA-256
`fad1830366f261f811bc992a45318acfc5f3458a20da054db4a327c59b090707`.
It contains every exact archive, upstream-base, patched/current tree, evidence,
content, patch, role, feature, and selected-expression binding. This report
records the reasoning and limits; the candidate is the machine-readable review
input.

## Result

All 55 third-party or asset records have exact-source candidates. Fifty are
ordinary exact-source approvals and five are limited
`development-only-host-tool-source` approvals under the existing MPL ruling.
There are no unresolved license-text or nested compiled-source evidence gaps in
this batch.

| Exact group | Records | Selected expression / scope |
| --- | ---: | --- |
| Legacy slash selections | 11 | exact MIT branch |
| Other special OR selections | 11 | 8 MIT, 3 Apache-2.0 |
| Patched Iced/cosmic source | 6 | MIT; `iced_core` also has compiled Apache-2.0 |
| ICU4X / Unicode source | 19 | 18 Unicode-3.0; `unicode-ident` is MIT AND Unicode-3.0 |
| `webpki-roots` certificate data | 1 | CDLA-Permissive-2.0 data role |
| `ring` and copied source | 1 | Apache-2.0 AND ISC, plus compiled MIT |
| Roboto Regular | 1 | Apache-2.0 asset |
| MPL host tools | 5 | exact MPL-2.0 source, development host only |
| **Total** | **55** | **50 exact-source + 5 development-only host-source** |

The expression count in the candidate is 25 MIT, 18 Unicode-3.0, five
MPL-2.0, four Apache-2.0, one MIT AND Unicode-3.0, one
Apache-2.0 AND ISC, and one CDLA-Permissive-2.0. Additional copied-source
obligations make `iced_core` effectively MIT AND Apache-2.0 and `ring`
effectively Apache-2.0 AND ISC AND MIT.

## Review method

I took the set difference from the current 328 selected records, rather than
reclassifying records already covered by the previous two batches. For each of
the 54 Cargo records I recomputed the existing source-audit tree hash over
sorted `path NUL bytes NUL` entries, excluding only `.git` and `target`. All 54
matched the reviewed source lock. Their archive identities were compared with
the pinned `Cargo.lock` checksums. Prior archive/base/patch evidence was reused
only where the current archive, base, and patched tree bindings still matched.
The Roboto files were hashed directly.

The recursive source scan inspected 79 license-like files and 146 license
header hits. The scan artifact is
`.superpowers/sdd/2026-09-12-linux-desktop-home/special-source-evidence/scan.json`,
SHA-256
`20356a60b184cb931ff66bd4345f1f9576681ba559e2ecad0810985fbc7ab878`.
The scan was compared with selected evidence, additional compiled obligations,
source-distribution-only material, and unselected alternate-branch texts. A
tree-hash match alone was not treated as a license approval.

## Permissive choices and exact grants

The 11 legacy slash-expression records are the exact records reviewed in
`task-1-source-review.md`: `bitflags`, `bs58`, `fnv`, `ident_case`,
`json-patch`, `rangemap`, `rustc-hash`, `same-file`, `serde_urlencoded`,
`version_check`, and `walkdir`. Their archive, current tree, and full MIT text
hashes still match, so only the MIT branch is selected. This is not a global
rewrite of slash syntax.

The other exact choices select MIT for `aho-corasick`, `byteorder-lite`,
`byteorder`, `jiff-core`, `jiff`, `linux-raw-sys`, `memchr`, and `rustix`;
they select Apache-2.0 for `dunce`, `ryu`, and `self_cell`. The candidate binds
the exact declaration and complete selected text for each source identity.

`dunce@1.0.5` needs an explicit evidence note. Its exact
`Cargo.toml.orig`, SHA-256
`14e322002ad961edfdd3494b2094b379de39ee9efbc196dbafc1e0606bdb0e36`,
grants `CC0-1.0 OR MIT-0 OR Apache-2.0`, while the source tree's only license
file contains the CC0 branch. The Apache selection is therefore bound to that
original, author-supplied source manifest and the official Apache 2.0 full text,
SHA-256
`cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30`,
from [Apache's canonical license page](https://www.apache.org/licenses/LICENSE-2.0.txt).
The exact tree's CC0 file is separately bound and retained at SHA-256
`a2010f343487d3f7618affe54f789f5487602331c0a8d03f49e9a7c547cf0499`;
it is classified as an unselected alternate-branch source file, not the chosen
installed notice. The original manifest retains the named author. The
branch-file inconsistency is explicit; this evidence is not an unattributed
replacement of the upstream license file or a decision about another dunce
release. This candidate does not approve distribution of the dunce source
snapshot.

## Patched Iced and cosmic-text

The six archive hashes, upstream-base hashes, patch intent, and patched tree
hashes remain exactly those verified in `task-1-source-review.md`. The current
trees independently match those bindings. All six select their exact MIT
branch and retain the complete Iced or cosmic MIT notice.

`iced_core@0.14.0` also compiles Druid-derived layout code. The candidate binds
`src/layout/DRUID_LICENSE`, SHA-256
`cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30`,
and the copied source header as an additional Apache-2.0 compiled obligation.
Its effective obligations are MIT AND Apache-2.0.

The ten files under `cosmic-text-0.15.0/fonts` are OFL-1.1 test assets and
license files. The only code references found are in two tests; no `src` use or
`include_bytes!` path includes them in the application. All ten exact hashes
are recorded as `source-distribution-only`, and source-snapshot distribution is
not approved by this candidate. These fonts must not enter the application,
runtime, or package payload. This finding does not approve redistribution of
the complete vendored source archive.

## `ring` copied-source closure

`ring@0.17.14` has exact archive SHA-256
`a4689e6c2294d81e88dc6261c768b63bc4fcdb852be6d1352498b114f61383b7`
and tree SHA-256
`7ab7c6b50291c748892bd7661721f983f15611cb740a18c7708990ce02a9cade`.
Its `.cargo_vcs_info.json` names commit
`2723abbca9e83347d82b056d5b239c6604f786df` and marks the source dirty, so
the archive and current tree are authoritative; this review does not call it a
clean checkout of that commit.

The declared selection remains Apache-2.0 AND ISC. The compiled source adds
selected MIT obligations from copied `once_cell` and slice code, so the
effective expression is Apache-2.0 AND ISC AND MIT. The candidate retains all
six complete top-level or nested notice files:

| Evidence | SHA-256 | Role |
| --- | --- | --- |
| `LICENSE` | `b3d734001a94efff3579978d953391aa7115f877657d25eb54037a43875d078a` | main license notice |
| `LICENSE-BoringSSL` | `005fc765ddc5115da796cca915baa9557abae13ff35e0a47c47affc56f6c414d` | compiled BoringSSL Apache notice |
| `LICENSE-other-bits` | `f025ccfb7dfb6bdfedc75ca0f67acc69e6fb4998143d834f7c2f38a29989680f` | ISC-origin source notice |
| `src/polyfill/once_cell/LICENSE-APACHE` | `a60eea817514531668d7e00765731449fe14d059d3249e0bc93b36de45f759f2` | copied source alternate branch text |
| `src/polyfill/once_cell/LICENSE-MIT` | `6ee2ed6c77710de911761acd5fc1ad1da00f476beb1a7ef27e78c2d1858deafc` | selected copied-source MIT notice |
| `third_party/fiat/LICENSE` | `9eacbcb81be660840c714a560a9d65ba07913db98dd4baf969f78dd499fdd60f` | compiled fiat Apache notice |

The compiled `once_cell` source and its selection are additionally bound by
`race.rs` SHA-256
`486c98b50b9db775178faaf6a80748ea06805f93987aabd79c6a453c775fbc10`.
The copied slice implementation contains its complete MIT header and is bound
at SHA-256
`491810bd2e2e3e933db93d24bc8ae78b381a3d6931ecb783173cfb34edc14edd`.
`LICENSE-BoringSSL` also carries a support-code notice for test-only Go code;
retaining the full file covers that source notice without classifying the
uncompiled test code as an application obligation.

## Unicode and certificate data

Eighteen ICU4X-family records select exact Unicode-3.0. Their exact source
trees carry the same complete text, SHA-256
`f367c1b8e1aa262435251e442901da4607b4650e0e63a026f5044473ecfb90f2`.
`unicode-ident@1.0.24` separately selects MIT AND Unicode-3.0 and retains both
full notices. These are exact source decisions under the term review in
`2026-09-12-linux-desktop-license-terms.md`; they do not approve other Unicode
license versions. The required notice must accompany copies or supporting
documentation, Unicode names may not be used for promotion without prior
written authorization, and the reviewed text has no express patent grant. The
primary term and policy sources are [Unicode's license](https://www.unicode.org/license.txt),
[licensing policy](https://www.unicode.org/policies/licensing_policy.html), and
[license FAQ](https://unicode.org/faq/unicode_license.html).

`webpki-roots@1.0.9` selects CDLA-Permissive-2.0 only for its exact generated,
compiled certificate-data role. Its archive, tree, generated library, and full
agreement hashes are bound in the candidate. Distribution of the shared Data
must make the complete agreement available. This is not approval for arbitrary
software code or another data snapshot. The primary term is the
[CDLA Permissive 2.0 agreement](https://cdla.dev/permissive-2-0/).

## MPL development-only host source

`cssparser-macros@0.6.1`, `cssparser@0.36.0`, `dtoa-short@0.3.5`,
`option-ext@0.2.0`, and `selectors@0.36.1` receive only exact
`development-only-host-tool-source` approval. Their current records are
host-build only. They are not approved for application code, target linkage,
generated target code, runtime loading, final binaries, or package contents,
and MPL-2.0 is not added to the application allowlist.

Four exact trees contain their complete MPL text. `selectors` instead has an
exact MPL header in `lib.rs`, SHA-256
`d54c6e13e9e952dac17d209171df8657e3cae93beaddace4906150cdec8d02e9`,
and an exact source manifest. Its evidence is completed by Mozilla's
[canonical MPL 2.0 text](https://www.mozilla.org/MPL/2.0/), saved at SHA-256
`3f3d9e0024b1921b067d6f7f88deb4a60cbe7a78e76c64e3f1d7fc3b779b9d04`.
The source header links this text; it is not an inference from package metadata
alone.

`selectors` generates a table only into its host `OUT_DIR`, and
`cssparser-macros` expands code only into the host cssparser library in the
reviewed graph. The cost of a mistaken classification remains material: an MPL
host artifact, generated target source, or linked object could enter the
application while metadata still calls the package host-only. The final gate
must reconcile generated outputs, target objects, host artifacts, the final
binary, and the complete payload. Any MPL reachability outside the exact host
scope fails this approval. Source-snapshot distribution is also outside it.

## Roboto Regular

The exact AOSP Roboto Regular asset at commit
`a781a172907fa1a48aead0911172988056f78e90` selects Apache-2.0. The candidate
binds `Roboto-Regular.ttf` SHA-256
`f10a4d95fb922a38aeeae842005dfdc4c7ef1e7a66308f6379d749c44133b876`,
the complete `NOTICE` SHA-256
`cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30`,
`README.android`, and the empty AOSP license marker. The empty marker is
classification evidence, not the license text. The installed application must
retain the complete NOTICE. This approval is limited to the exact file and does
not establish multilingual fallback coverage.

## Exclusions and importer conditions

The current difference set's three first-party records are excluded:

- `cargo:linux-desktop@0.1.0`
- `cargo:video-creater-permissive-media-protocol@0.1.0`
- `cargo:video-creater@0.1.0`

The previously separated `video-creater-compatibility-protocol` and
`video-creater-precompose-protocol` records remain in the same private-source
owner boundary. None receives a public MIT or Apache grant by inference. The
first-party gate owner must bind their exact private paths, repository identity,
revision, tree, development scope, and later distribution authority.

Importing the 55 candidates must preserve all exact source and evidence hashes,
selected branches, additional compiled obligations, roles, features, and scope
restrictions. It must install the complete required notices; prevent cosmic
test fonts and MPL material from entering the payload; and run final
binary/payload reconciliation. These are policy-import and release conditions,
not remaining evidence gaps in this source batch.

## Verification

- Exact set arithmetic: 58 remaining selected records = 55 third-party/assets
  + 3 excluded current first-party records.
- Independent Cargo tree hashing: 54/54 match the reviewed source-lock
  snapshot.
- Recursive evidence scan: 79 license-like files and 146 header hits inspected.
- Machine candidate: 55/55 records; 50 exact-source and five scoped MPL host
  approvals.
- Recursive candidate evidence validation: every recorded local evidence,
  content, patch, copied-source, and source-distribution-only path is required
  to match its SHA-256 before import.
- No Cargo build, native compilation, install, package, or runtime execution
  was performed or inferred from this source-license review.
