---
domain: moeb
slug: domain-spec-dead-code-removal
status: active
---

# Domain Spec Dead Code Removal

## Raw Requirement

There are unused elements in the codebase, last warnings were from src/domain/spec_schema.rs and src/domain/spec_parser.rs

## Description

After `moeb.serve-cli-parity.md` removed Rust-side spec validation from `domain/spec.rs`, the
functions `parse_frontmatter`, `validate_sections`, the constant `REQUIRED_SECTIONS`, and the
`use anyhow::{bail, Context, Result}` import in `spec_parser.rs` are no longer called by any
production code path. Similarly, the entire `spec_schema` module (`ValidationSchema`,
`FrontmatterSchema`, `BodySchema`, `load_validation_schema`) is declared in `spec.rs` but never
referenced outside `#[cfg(test)]` code. These become dead-code warnings in non-test builds.

All of these items are still exercised by the unit tests in `spec_tests.rs` and retain value as
correctness contracts for spec frontmatter parsing. The fix gates each item with `#[cfg(test)]`
rather than deleting it, so production builds are warning-free while all test coverage is
preserved. The `#[allow(dead_code)]` suppression on `FrontmatterSchema::optional` — which was
masking a pre-existing unused-field warning — becomes redundant once the whole module is
test-only and is removed.

## Diagram

```mermaid
graph LR
    subgraph non_test["Non-test build"]
        A["spec_parser.rs\n── sanitize_slug only"]
        B["spec.rs\n── mod spec_parser\n── use sanitize_slug\n── (no mod spec_schema)"]
    end

    subgraph test_build["Test build (#[cfg(test)])"]
        C["spec_parser.rs\n── sanitize_slug\n── REQUIRED_SECTIONS\n── parse_frontmatter\n── validate_sections"]
        D["spec_schema.rs\n── ValidationSchema\n── FrontmatterSchema\n── BodySchema\n── load_validation_schema"]
        E["spec_tests.rs\n── uses parse_frontmatter\n── uses validate_sections\n── uses REQUIRED_SECTIONS\n── uses load_validation_schema"]
    end

    B -->|always compiled| A
    E -->|cfg(test)| C
    E -->|cfg(test)| D
```

## Backlinks

### Parents

| Label | Path | Purpose |
|-------|------|---------|
| README | [README.md](../../README.md) | Root harness index |
| Split Domain Spec: Parser and Schema Extraction | [specifications/moeb/moeb.split-domain-spec.md](specifications/moeb/moeb.split-domain-spec.md) | Parent spec that introduced spec_parser.rs and spec_schema.rs with these items |
| MCP Serve / CLI Parity | [specifications/moeb/moeb.serve-cli-parity.md](specifications/moeb/moeb.serve-cli-parity.md) | Removed Rust-side spec validation from spec.rs, causing these items to become dead code |

### External

*(none)*

## Steps

### Step 1 — Gate test-only items in `src/moeb/src/domain/spec_parser.rs`

Read `src/moeb/src/domain/spec_parser.rs` in full.

Make the following changes:

**1a.** Remove the unconditional import at the top of the file:
```rust
use anyhow::{bail, Context, Result};
```

**1b.** After the closing brace of `sanitize_slug`, add a `#[cfg(test)]` guarded import and
gate the three test-only items with `#[cfg(test)]`:

The resulting file must have this structure (in order):

```rust
pub(super) fn sanitize_slug(input: &str) -> String {
    // ... unchanged body
}

#[cfg(test)]
use anyhow::{bail, Context, Result};

#[cfg(test)]
pub(super) const REQUIRED_SECTIONS: &[&str] = &[
    // ... unchanged body
];

#[cfg(test)]
pub(super) fn parse_frontmatter(content: &str) -> Result<(String, String, String, Vec<(String, String)>, String)> {
    // ... unchanged body
}

#[cfg(test)]
pub(super) fn validate_sections(body: &str, required: &[impl AsRef<str>]) -> Result<()> {
    // ... unchanged body
}
```

No logic changes. Only the placement of `#[cfg(test)]` attributes and the removal of the
top-level `use anyhow` import (replaced by a gated one after `sanitize_slug`).

### Step 2 — Gate `mod spec_schema` in `src/moeb/src/domain/spec.rs`

Read `src/moeb/src/domain/spec.rs` in full.

Locate the two lines:
```rust
#[path = "spec_schema.rs"]
mod spec_schema;
```

Replace them with:
```rust
#[cfg(test)]
#[path = "spec_schema.rs"]
mod spec_schema;
```

No other changes to `spec.rs`.

### Step 3 — Remove the `#[allow(dead_code)]` suppression in `src/moeb/src/domain/spec_schema.rs`

Read `src/moeb/src/domain/spec_schema.rs` in full.

Locate:
```rust
    #[serde(default)]
    #[allow(dead_code)]
    pub(super) optional: Vec<String>,
```

Replace with:
```rust
    #[serde(default)]
    pub(super) optional: Vec<String>,
```

No other changes to `spec_schema.rs`. The `optional` field is retained; only the now-redundant
lint suppression attribute is removed.

### Step 4 — Verify

Run the following commands from `src/moeb/`:

```
cargo check 2>&1
cargo test 2>&1
```

Confirm:
- `cargo check` reports zero `dead_code` or `unused_imports` warnings whose source location
  is `spec_parser.rs` or `spec_schema.rs`.
- `cargo test` exits 0 with all tests in `spec_tests.rs` passing (no test deleted, commented
  out, or weakened).

## Decisions

### Decision 1 — Gate with `#[cfg(test)]` rather than delete

**Rationale:** `parse_frontmatter`, `validate_sections`, `REQUIRED_SECTIONS`, and the
`spec_schema` module are exercised by a comprehensive unit-test suite in `spec_tests.rs`. These
tests assert correct behaviour for frontmatter parsing, section validation, and schema
deserialization — contracts that remain meaningful even though the production code path no longer
calls these functions directly (validation is now agent-driven via MCP tools). Deleting the
functions would eliminate this coverage.

**Alternatives:**

| Option | Reason Rejected |
|--------|-----------------|
| Delete functions and their tests | Removes correctness coverage for spec frontmatter contracts; tests document the invariants that the AI spec agent is expected to uphold |
| Add `#[allow(dead_code)]` to each item | Suppresses the symptom without expressing that the items are test-only; the existing `#[allow(dead_code)]` on `optional` was already an example of this anti-pattern — extending it is strictly worse |
| Move test-only helpers into `spec_tests.rs` | Feasible but relocates 100+ lines of logic (parse_frontmatter) into a test companion file; test files should assert behaviour, not define substantial parsing logic |

**Consequences:** Items gated with `#[cfg(test)]` in a production Rust module are a recognised
idiom. Any future caller that needs these functions in a non-test context must remove the
`#[cfg(test)]` gate at that time.

### Decision 2 — Remove the `#[allow(dead_code)]` suppression on `FrontmatterSchema::optional`

**Rationale:** The suppression was added in `moeb.split-domain-spec.md` because `optional` is
deserialized from JSON but never read in production code. With the entire `spec_schema` module
now gated as `#[cfg(test)]`, the `dead_code` lint no longer fires for any item in that module.
The suppression attribute becomes misleading — it implies a production-code dead-code concern that
no longer exists — and should be removed.

**Alternatives:**

| Option | Reason Rejected |
|--------|-----------------|
| Retain the suppression attribute | Misleads future readers into believing a production dead-code issue exists; unnecessary noise |

**Consequences:** The `optional` field is retained and its value is accessible in tests. A future
reader can observe the field without the distraction of a lint suppression annotation.

## Rubric

### Structured

| Name | Description | Threshold | Pass Condition |
|------|-------------|-----------|----------------|
| `no-drift` | The specification does not violate any decision recorded in a linked parent specification | Zero contradictions | Manual review of every decision in every parent spec listed in Backlinks |
| `spec-schema-compliance` | All required frontmatter fields and body sections are present and correctly ordered | 100% of required fields and sections | Validation in `domain/spec.rs` exits 0 during `moeb spec` |
| `ai-first-org` | The specification's Steps prescribe file layouts, naming, and helper placement that satisfy the four AI-first organisation principles: one concern per file, context locality, grep-discoverable names, no cross-cutting helpers. | All four principles followed | Manual review: no step produces a file that mixes concerns, no name is generic across the codebase, all helpers are co-located with their callers or promoted to a precisely named module |
| `zero-dead-code-warnings` | `cargo check` reports no `dead_code` or `unused_imports` warnings from `spec_parser.rs` or `spec_schema.rs` | Zero warnings from target files | `cargo check 2>&1 \| grep -E "spec_parser\|spec_schema"` returns no warning output |
| `tests-green` | All tests in `spec_tests.rs` pass after the change | 100% pass rate | `cargo test` exits 0; test output shows no failures in the `domain::tests` module |

### Qualitative

- The change is minimal: three attribute additions, one import relocation, and one attribute removal — no logic changes anywhere.
- All test assertions in `spec_tests.rs` remain valid without modification to the test file.
- No public or crate-level API surfaces change; `sanitize_slug` remains the only item from these modules used in production code.
