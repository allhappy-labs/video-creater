# LLVM runtime license policy review

Reviewed 2026-09-12 for the Linux permissive-runtime specification.

## Decision and scope

Approve the exact SPDX term `Apache-2.0 WITH LLVM-exception` for the declaration
policy. This is an additional permissive term reviewed under the spec's existing
procedure; the no-LGPL/no-copyleft requirement does not change. Implementation
at `4431322c` now reflects this decision. Independent source-policy,
specification, and code review accepted that checker update with zero findings;
31 focused and 40 combined tests passed. Recognition of other exception-bearing
terms does not approve them.

This decision concerns license terms only. It does not establish which source
files contributed to a binary, whether their notices are complete, or whether
an artifact matches a reviewed source/build recipe. LLVM explicitly identifies
separately licensed third-party material; those files remain separate inventory
obligations. The legacy license sections do not justify treating every current
LLVM source file as MIT.

## Primary source basis

The exact LLVM 18.1.8 component texts inspected are:

| Component | Pinned primary source | SHA-256 of retained text |
| --- | --- | --- |
| C++ ABI | [libc++abi license](https://github.com/llvm/llvm-project/blob/llvmorg-18.1.8/libcxxabi/LICENSE.TXT) | `e2b35be49f7284a45b7baca8fc7b3ab7440e7902392b2528a457816b5bb2a15c` |
| Compiler runtime | [compiler-rt license](https://github.com/llvm/llvm-project/blob/llvmorg-18.1.8/compiler-rt/LICENSE.TXT) | `1a8f1058753f1ba890de984e48f0242a3a5c29a6a8f2ed9fd813f36985387e8d` |
| Unwinding | [libunwind license](https://github.com/llvm/llvm-project/blob/llvmorg-18.1.8/libunwind/LICENSE.TXT) | `b5efebcaca80879234098e52d1725e6d9eb8fb96a19fce625d39184b705f7b6d` |

Each begins with Apache 2.0 and the LLVM exception. Apache's copyright grant
permits modification, sublicensing and source/object redistribution, with its
notice and other conditions; it does not require publishing application source.
The LLVM exception relaxes specified redistribution conditions for portions
embedded by compilation and provides a conditional GPLv2 compatibility waiver.
It adds no source-sharing obligation. The latter clause does not turn this
project's forbidden GPL dependencies into eligible ones. These observations
support classifying this exact combined term as permissive for this project.

[SPDX identifies LLVM-exception](https://spdx.org/licenses/LLVM-exception.html)
as the exception intended for Apache-2.0. Eligibility therefore applies to the
combined term, not to arbitrary licenses followed by that exception ID.

## Required checker behavior

- Keep the existing six eligible IDs and add this one complete atomic term.
- Preserve `WITH` during derivation; never replace it silently with bare Apache.
- Keep unknown exception IDs rejected, including in unselected alternatives.
- Deny LGPL/GPL and other unapproved combinations, including a GPL term followed
  by LLVM-exception. Preserve all selected `AND` obligations.
- Keep the evaluator declaration-only. Do not label source-license approval as
  a runtime, binary, complete dependency, or package audit.

The inspected text snapshots live with the ignored codec-runtime evidence.
The pinned URLs and hashes above identify the review inputs independently of
that scratch directory. No third-party license text is copied into this report.
