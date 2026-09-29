# Linux permissive inventory preflight

The Linux permissive inventory preflight validates a declared dependency inventory against
the project's fixed license policy. It is an early feasibility gate. An
`inventory-eligible` result is not a package audit, a Linux compatibility result, or release
evidence.

Under the approved option 1 boundary, this inventory describes the app, its
helpers, and their linked/loaded libraries. `system` means an application-loaded
system component and receives no license exemption. Independent OS services
(display/audio servers, portals, credential services) are documented separately
by protocol and availability; their internal libraries do not belong in this
application dependency graph. An app-owned helper or a client library cannot be
reclassified as an independent service to bypass the gate.

## Run the preflight

Pass exactly one JSON inventory file:

```bash
node scripts/linux-permissive-preflight.mjs path/to/inventory.json
```

The command writes the evaluation as JSON to standard output. It reads only the named file;
it does not discover, install, execute, or probe any declared component.

Exit codes are:

| Code | Meaning |
| --- | --- |
| `0` | The declaration is `inventory-eligible`. |
| `1` | The declaration is `blocked` by schema, graph, selection, or license policy failures. |
| `2` | Invocation, file reading, or JSON parsing failed. Details are written to standard error. |

## Inventory schema

The top-level object has this shape:

```json
{
  "schemaVersion": 1,
  "roots": ["app"],
  "components": [
    {
      "id": "app",
      "name": "app",
      "version": "1",
      "sourceUrl": "https://example.org/app",
      "license": "MIT OR Apache-2.0",
      "selectedLicense": "Apache-2.0",
      "classification": "bundled",
      "dependencies": ["libc"]
    },
    {
      "id": "libc",
      "name": "permissive libc",
      "version": "1.2.5",
      "sourceUrl": "https://example.org/libc",
      "license": "MIT",
      "selectedLicense": "MIT",
      "classification": "system",
      "dependencies": []
    }
  ]
}
```

`schemaVersion` must be `1`. `roots` and `components` must both be nonempty arrays. Every
root and dependency must resolve to a component `id`, and component IDs must be unique.
Dependency cycles are allowed because shared-library graphs can contain cycles.

Every component must contain all fields shown above for evaluation. All string fields must be
nonempty, `sourceUrl` must be a valid HTTPS URL, `classification` must be `bundled` or `system`,
and `dependencies` must be an array of nonempty component IDs. The evaluator checks every
component, including entries disconnected from the roots.

## License expressions and selection

The policy began with six eligible simple IDs: `MIT`, `Apache-2.0`, `BSD-2-Clause`,
`BSD-3-Clause`, `ISC`, and `Zlib`. Following the explicit
[LLVM term review](../research/2026-09-12-linux-llvm-license-policy.md), the exact combined
atomic term `Apache-2.0 WITH LLVM-exception` is also eligible. The same rule applies to
bundled and system components. Extra inventory properties cannot extend the policy or exempt
system libraries.

The bounded parser also recognizes these denied IDs so a declaration can explicitly select a
different branch from an `OR` expression:

```text
0BSD
AGPL-1.0-only, AGPL-1.0-or-later, AGPL-3.0-only, AGPL-3.0-or-later
BSD-4-Clause, BSL-1.0, CC0-1.0, CDDL-1.0, CDDL-1.1
EPL-1.0, EPL-2.0
GPL-1.0-only, GPL-1.0-or-later, GPL-2.0-only, GPL-2.0-or-later,
GPL-3.0-only, GPL-3.0-or-later
LGPL-2.0-only, LGPL-2.0-or-later, LGPL-2.1-only, LGPL-2.1-or-later,
LGPL-3.0-only, LGPL-3.0-or-later
MPL-1.0, MPL-1.1, MPL-2.0
Unlicense
```

Recognition does not make these licenses eligible. Any other license ID fails as unknown even
when it appears only in an unselected branch.

`license` and `selectedLicense` use SPDX-style license IDs with uppercase `AND`, uppercase
`OR`, and parentheses. `AND` binds tighter than `OR`. The bounded parser recognizes the exact
exception ID `LLVM-exception` after one recognized license ID, for example
`Apache-2.0 WITH LLVM-exception`. It retains the license and exception as one atomic leaf.
The earlier recognition-only stage left exception-bearing leaves denied. The subsequent
source review approved only `Apache-2.0 WITH LLVM-exception`; attaching the exception to
`MIT`, GPL, LGPL, or another recognized base does not make that combination eligible. The
combined term also remains separate from the parser's recognized simple license IDs.

For example, this declaration and selection are eligible at the declaration-only gate:

```json
{
  "license": "Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT",
  "selectedLicense": "Apache-2.0 WITH LLVM-exception"
}
```

Selecting the `MIT` alternative is also eligible.

Any other exception ID fails as unknown, including in an unselected `OR` branch. Missing or
repeated `WITH`, an exception attached to a parenthesized compound expression, standalone
exception IDs, unknown tokens, and malformed expressions fail closed. This is not a complete
SPDX expression parser. Expressions remain bounded to 512 characters, 64 tokens, and 64
enumerated choices.

`selectedLicense` records the chosen expression. It can select one branch from a declared
`OR`, but it must be a complete branch derived from `license`. It cannot drop a declared
`AND` obligation. Every leaf retained by the selection must appear in the fixed eligibility
policy.
For example, selecting `MIT` from `MIT OR LGPL-2.1-or-later` is eligible, while retaining the
whole `OR` is blocked because the selection still contains an LGPL choice.

## Result contract and limits

The evaluator and CLI return this shape:

```json
{
  "status": "inventory-eligible",
  "failures": [],
  "componentCount": 2,
  "limitations": [
    "Declaration-only evaluation; inventory entries are not independently verified.",
    "No artifact, binary, dependency-closure, or package verification was performed.",
    "No Linux runtime, desktop integration, compatibility, or acceptance testing was performed."
  ]
}
```

`status` is exactly `blocked` or `inventory-eligible`; the tool never reports `passed` or
`compatible`. Failures identify the affected component where possible and are sorted for
deterministic output. Limitations are always present, including in blocked results.

The inventory is a declaration supplied by its author. This tool does not verify source
metadata, artifacts, hashes, binaries, static code, loaded libraries, dependency closure,
packaging contents, desktop services, or runtime behavior. Test fixtures exercise declaration
handling only and are not evidence that any represented dependency is redistributable or
eligible in an actual package.
