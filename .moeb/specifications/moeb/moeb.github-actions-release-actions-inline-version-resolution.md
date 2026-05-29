---
domain: moeb
slug: github-actions-release-actions-inline-version-resolution
status: active
supersedes:
  - path: specifications/moeb/moeb.github-actions-release-actions-upgrade.md
    decision: "Decision 3 — Use `query_agent` to determine target versions at implementation time"
---

# GitHub Actions: Release Actions Upgrade via Inline Version Resolution

## Raw Requirement

The previous specification `moeb.github-actions-release-actions-upgrade` prescribes calling `query_agent` in Step 2 to determine the latest stable Node.js 24-native version tags for `actions/checkout`, `actions/cache`, and `softprops/action-gh-release`. When moeb runs as an MCP server driven by an external coordinator, `query_agent` requires a locally configured adapter which is absent in MCP serve mode (per `moeb.inline-persona-review.md` Decision 3). This renders the upgrade specification non-executable in MCP mode.

Two changes are required:
1. When running as an MCP server, `query_agent` should not cause a tool error — it should return immediately with a diagnostic message directing the caller to use inline reasoning instead.
2. The version determination step in the release actions upgrade should not call `query_agent`. The implementing agent already has training knowledge sufficient to identify current stable action versions; it should apply that knowledge directly, without tool-call indirection.

## Description

`moeb.github-actions-release-actions-upgrade` addressed the Node.js 20 deprecation warning by upgrading three action pins in `.github/workflows/release.yml` to Node.js 24-native versions and removing the `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24` interim override. That approach was sound; the implementation flaw was in Step 2, which called `query_agent` to resolve version tags. In MCP mode, this call fails because no adapter is configured at server startup (per the revert in `moeb.inline-persona-review.md`).

This specification replaces Decision 3 of the prior spec with two changes. First, it adds a mode-aware guard to `query_agent.rs`: when the tool is invoked in a context without a configured adapter (the MCP serve path), it returns a graceful `ToolResult` message rather than propagating an adapter resolution error. This prevents `query_agent` calls in any spec or skill from generating spurious tool errors in MCP mode. Second, Step 3 of the upgrade workflow instructs the implementing agent to use its own training knowledge directly — identical in quality to what a `query_agent` sub-agent would produce, but without the adapter dependency.

`moeb.inline-persona-review.md` established that frontier LLMs are capable of multi-perspective reasoning within their own context window. The same reasoning applies here: version tag lookups from training knowledge are within the implementing agent's capability and do not require sub-agent delegation.

```mermaid
graph TD
    K[Step 1: query_agent.rs — add MCP guard] --> G[Returns Ok diagnostic if no adapter configured]
    A[release.yml — current] --> B[actions/checkout@v4 node20]
    A --> C[actions/cache@v4 node20]
    A --> D[softprops/action-gh-release@v2 node20]
    B --> E[Step 3: implementing agent inline AI knowledge lookup]
    C --> E
    D --> E
    E --> F[Step 4: patch release.yml with new version pins]
    F --> H[Step 5: remove FORCE_JAVASCRIPT_ACTIONS_TO_NODE24]
    H --> I[release.yml — no Node20 deprecation warning]
```

## Backlinks

### Parents

| Label | Path | Purpose |
|-------|------|---------|
| Harness README | README.md | Governing harness for all specifications |
| GitHub Actions: Upgrade Release Actions to Native Node.js 24 Versions | specifications/moeb/moeb.github-actions-release-actions-upgrade.md | Prior specification whose Decision 3 this spec supersedes |
| Inline Persona Review | specifications/moeb/moeb.inline-persona-review.md | Established inline reasoning as the MCP-compatible alternative to query_agent; motivates the guard added in Step 1 |
| Query Agent Tool | specifications/moeb/moeb.query-agent-tool.md | query_agent.rs modified in Step 1 |

## Steps

### Step 1 — Add MCP mode guard to `query_agent.rs`

Read `src/moeb/src/tools/query_agent.rs`. At the beginning of the `execute` method, before any adapter resolution attempt, add a guard that detects when no adapter is configured. Use the same mechanism the existing code uses to resolve the adapter (e.g. `resolve_adapter()` or equivalent). If the resolution would fail with a "no adapter configured" condition — which is the state produced by `RealToolExecutor::new_mcp` — return immediately with:

```
Ok(ToolResult { content: "query_agent is not available in MCP serve mode. Use inline reasoning from your training knowledge instead of calling this tool." })
```

The guard must return `Ok(ToolResult { ... })`, not `Err(...)`, so that the call does not register as a tool error in `RunState.tool_errors` and does not fail the `tool-errors` rubric criterion.

Apply the change as a single `patch_file` call on `src/moeb/src/tools/query_agent.rs`.

### Step 2 — Read the workflow file

Call `read_file` on `.github/workflows/release.yml`. Confirm:
- The three action pins `actions/checkout@v4`, `actions/cache@v4`, and `softprops/action-gh-release@v2` are present.
- `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true` is present in the workflow-level `env:` block.

### Step 3 — Determine target versions using inline AI knowledge

Without calling any tool, apply training knowledge to identify the latest stable release tag for each of the following GitHub Actions that natively declares `node24` (or later) as the `runs.using` value in its `action.yml`:

- `actions/checkout` — currently pinned at `v4`
- `actions/cache` — currently pinned at `v4`
- `softprops/action-gh-release` — currently pinned at `v2`

Record the result as a JSON object in working memory:
```json
{ "checkout": "<tag>", "cache": "<tag>", "action-gh-release": "<tag>" }
```

If the Node.js 24-native version for any action is uncertain or unavailable from training knowledge, set its value to `null` and note the reason. Do not fabricate a tag — a `null` value means that action cannot be upgraded in this run and `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true` must be retained for it.

### Step 4 — Patch action versions

For each action with a non-null version from Step 3, call `patch_file` on `.github/workflows/release.yml` to replace the current version pin with the determined Node.js 24-native tag. Apply one minimal-diff patch per action.

### Step 5 — Remove `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24`

If all three actions returned non-null versions in Step 3, call `patch_file` on `.github/workflows/release.yml` to remove the `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true` line from the `env:` block. If the `env:` block is then empty, remove the entire block including its key. If any action could not be upgraded, skip this step and retain `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true` unchanged.

### Step 6 — Verify

Call `read_file` on `.github/workflows/release.yml` and confirm:
- Each upgraded action pin matches the tag determined in Step 3.
- `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24` is absent if all three actions were upgraded; still present if any action could not be upgraded.
- No structural YAML regressions: no duplicate keys, no indentation errors, `jobs:` key and all workflow steps remain intact.

## Decisions

### Decision 1 — Upgrade action versions to eliminate the Node.js 20 deprecation warning natively

**Rationale:** `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true` causes the runner to execute actions under Node.js 24 but does not silence the deprecation warning, which fires because the action manifests themselves declare `node20`. Run `9d5a69df-ac3a-4af4-a140-9aae58bd25d1` confirmed the warning persists despite the override. Upgrading to versions whose `action.yml` natively declares `node24` removes the warning at its source.

**Rejected alternatives:**
- _Retain `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true` without upgrading action versions_: Demonstrated to be insufficient — the deprecation warning fires in every release run despite the override.
- _Pin to explicit patch versions rather than the latest stable major tag_: The existing workflow convention uses floating major tags; changing that convention is out of scope for this fix.

**Consequences:** The implementing agent selects version tags using its inline training knowledge at implementation time (per Decision 3). If a native Node.js 24 version is unavailable for any action, `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true` is retained for that action and a follow-up signal should be authored.

### Decision 2 — Remove `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true` once all three actions are upgraded

**Rationale:** The environment variable was an explicitly interim measure. Retaining it after all three actions natively target Node.js 24 adds configuration noise and could silently mask future deprecation warnings for any new actions added to the workflow.

**Rejected alternatives:**
- _Keep `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true` as a defensive override_: If all manifests declare `node24`, the variable has no effect. If a future action manifest declares an older runtime, retaining the variable would suppress the warning rather than surfacing it for remediation.

**Consequences:** Partial upgrades retain `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true` as a partial mitigation. A follow-up signal or specification is implicitly required to complete the migration once remaining actions publish Node.js 24-native versions.

### Decision 3 — Use inline AI knowledge for version determination; do not call `query_agent`

**Rationale:** The original Decision 3 (in `moeb.github-actions-release-actions-upgrade.md`) delegated version lookups to `query_agent` to avoid hardcoding potentially stale version tags. In MCP serve mode, `query_agent` requires a locally configured adapter that is absent by design (per `moeb.inline-persona-review.md` Decision 3). The `query_agent` approach also introduced unnecessary indirection: the information a sub-agent returns from training knowledge is identical to the information available directly from the implementing agent's own training knowledge. `moeb.inline-persona-review.md` established the principle that frontier LLMs are capable of multi-perspective reasoning within their own context window; version tag lookup is a straightforward knowledge retrieval task well within this capability. Using inline knowledge eliminates the adapter dependency and produces identical results across both CLI and MCP serve modes.

**Rejected alternatives:**
- _Retain `query_agent` with a mode-conditional branch in the spec steps_: `moeb.inline-persona-review.md` Decision 2 explicitly rejected mode-specific branching in skill and spec instructions on the grounds that skill files must describe a single coherent workflow. The same principle applies here.
- _Hardcode specific version tags in the spec_: Accurate at authoring time (2026-05-29) but potentially stale at implementation time. Inline knowledge achieves the same freshness goal without the staleness risk.
- _Add web search capability_: No moeb tool provides web search. Inline training knowledge is the appropriate mechanism for this lookup.

**Consequences:** The implementing agent bears responsibility for confirming that the version tags it selects are syntactically valid GitHub Actions tags. If it is uncertain about any tag, it must set that value to `null` rather than fabricating a tag.

### Decision 4 — Add a graceful no-op guard to `query_agent` in MCP mode

**Rationale:** Even though this specification eliminates the `query_agent` call from the release actions upgrade workflow, other specs or custom skills could inadvertently call `query_agent` in MCP mode. Without a guard, such calls propagate an adapter resolution error that registers in `RunState.tool_errors` and fails the `tool-errors` rubric. A graceful `Ok(ToolResult)` return with a diagnostic message is preferable: it does not fail the rubric, clearly identifies the issue, and directs the author to use inline reasoning instead. This is consistent with the principle from `moeb.inline-persona-review.md` that MCP mode should not require adapter configuration for the standard workflow.

**Rejected alternatives:**
- _Remove `query_agent` from the MCP tool registry_: Violates the `kernel-thin-and-parity` rubric criterion, which requires every tool registered in the CLI tool registry to also be registered in the MCP stdio server tool list.
- _Return `Err(...)` from the guard_: Counts as a tool error, fails the `tool-errors` rubric, and provides no guidance on what to do instead.
- _Do nothing; rely on the existing adapter resolution error message_: The existing error "No adapter configured. Run `moeb use <adapter>` first." is accurate for CLI mode but misleading in MCP mode — configuring an adapter is not the intended fix. A context-aware `Ok` response is more helpful and rubric-safe.

**Consequences:** `query_agent` in MCP serve mode is effectively a no-op that returns a diagnostic string. The tool remains in the MCP tool registry, satisfying the `kernel-thin-and-parity` rubric. Skill authors and spec authors who encounter this message know to rewrite their `query_agent` call as an inline reasoning step.

## Rubric

### Structured

| Name | Description | Threshold | Pass Condition |
|------|-------------|-----------|----------------|
| `tool-errors` | No ToolHandler errors were recorded during this command invocation. Covers all tool calls on both CLI and MCP paths via `RealToolExecutor::execute`. Applies to every moeb command. | Zero tool errors | Kernel-authoritative: injected by `verify_rubrics` from `RunState.tool_errors`. Do not supply a verdict — any agent-supplied entry is overridden. |
| `rubric-qualification` | No Pass verdicts with acknowledged failures were issued during this command invocation. An agent that observes a failure but passes a criterion must populate `acknowledged_failures` in the verdict object. Applies to every moeb command. | Zero qualified Pass verdicts | Kernel-authoritative: injected by `verify_rubrics` by scanning `acknowledged_failures` on incoming Pass verdicts. Do not supply a verdict — any agent-supplied entry is overridden. |
| `skill-mandatory-phases` | The skill loaded for this run contains all mandatory phase headings. The agent checks its own prompt context for the exact strings: "Verify", "End-of-Skill Review", "Metrics Recording", "Commit", "Tag", "Complete". No file read is required. | All six mandatory headings present | Agent scans skill content in its loaded context; fails if any mandatory heading string is absent |
| `no-drift` | The specification does not violate any decision recorded in a linked parent specification | Zero contradictions | Manual review of every decision in every parent spec listed in Backlinks |
| `spec-schema-compliance` | All required frontmatter fields and body sections are present and correctly ordered | 100% of required fields and sections | Validation in `domain/spec.rs` exits 0 during `moeb spec` |
| `ai-first-org` | The specification's Steps prescribe file layouts, naming, and helper placement that satisfy the four AI-first organisation principles: one concern per file, context locality, grep-discoverable names, no cross-cutting helpers. | All four principles followed | Manual review: no step produces a file that mixes concerns, no name is generic across the codebase, all helpers are co-located with their callers or promoted to a precisely named module |
| `kernel-thin-and-parity` | No domain-specific or workflow-specific coordination logic may be placed in kernel Rust code when it can live in skill markdown or role files. Every tool registered in the CLI tool registry must also be registered in the MCP stdio server tool list. | Zero violations | Spec review: no proposed step places review/coordination logic in Rust; tool lists in tools/mod.rs and the MCP server match exactly |
| `moeb-tool-origin` | All tool calls made during this invocation must originate from moeb's own tool registry. | Zero external tool calls | Agent reviews the conversation for calls to non-moeb tools. Supplies Pass if none were made. If external tools were used with MissingMoebTool signals, supplies Fail with `acknowledged_failures` listing each signal title. |
| `no-signal-reoccurrence` | No canonical signal in `.moeb/signals/catalogue/` has `status: reopened` due to this run | Zero reopened signals introduced by this run | Agent scans all signal writes during this run; fails if any canonical signal file has `status: reopened` after processing |
| `no-query-agent-call` | The implementing agent does not call `query_agent` at any point during this run | Zero `query_agent` invocations | Agent self-reports; `query_agent` absent from the run conversation's tool calls |
| `mcp-guard-returns-ok` | The MCP mode guard added in Step 1 returns `Ok(ToolResult { ... })`, not `Err(...)` | Guard path returns Ok | Code review of the patch from Step 1: the guard's return type is `Ok(ToolResult { ... })`; no `Err` or `?` propagation on the no-adapter path |

### Qualitative

- All three action pins must reference the version tags determined in Step 3 after patching; any deviation constitutes a failure.
- If all three actions are upgraded, `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24` must be absent from the workflow file.
- If any action cannot be upgraded, `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true` must remain and the commit message must identify which actions were not upgraded and why.
- The YAML structure of `release.yml` must remain valid after all patches: no duplicate keys, no indentation errors, workflow-level sections (`on:`, `permissions:`, `env:` if retained, `jobs:`) in their correct positions.
- The guard added in Step 1 must fire only when no adapter is configured (MCP mode). It must not interfere with `query_agent` calls in CLI mode where an adapter is present.
