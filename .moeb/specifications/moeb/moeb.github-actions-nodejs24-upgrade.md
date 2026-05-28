---
domain: moeb
slug: github-actions-nodejs24-upgrade
status: active
---

# GitHub Actions: Node.js 24 Actions Upgrade

## Raw Requirement

Update GitHub Actions workflows to use Node.js 24-compatible action versions before the 2026-06-02 deprecation deadline. Actions affected: actions/cache@v4, actions/checkout@v4, softprops/action-gh-release@v2. Resolution: upgrade to versions that natively support Node.js 24, or set FORCE_JAVASCRIPT_ACTIONS_TO_NODE24=true as an interim measure. All three actions must be verified.

## Description

GitHub Actions deprecated Node.js 20 as the JavaScript action runtime on 2026-06-02, with full removal on 2026-09-16. The release workflow at `.github/workflows/release.yml` uses three actions that execute on Node.js 20: `actions/checkout@v4`, `actions/cache@v4`, and `softprops/action-gh-release@v2`. Without intervention the build and release pipeline will fail on the deprecation date.

This specification resolves the signal by adding a `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true` environment variable at the workflow level. This is the officially supported GitHub interim migration path: the runner executes all Node.js 20-based JavaScript actions under Node.js 24 when the variable is present, allowing the existing action version pins to remain unchanged. The single workflow-level `env:` block covers all three affected actions across both the Linux and Windows matrix jobs in one minimal-diff change.

```mermaid
graph TD
    A[.github/workflows/release.yml] --> B[env: FORCE_JAVASCRIPT_ACTIONS_TO_NODE24=true]
    B --> C[actions/checkout@v4]
    B --> D[actions/cache@v4]
    B --> E[softprops/action-gh-release@v2]
    C --> F[Node.js 24 runtime]
    D --> F
    E --> F
```

## Backlinks

### Parents

| Label | Path | Purpose |
|-------|------|---------|
| Harness README | README.md | Governing harness for all specifications |
| Windows Release Target | specifications/moeb/moeb.windows-release-target.md | Established the release workflow matrix that this spec modifies |

## Steps

1. **Read the workflow file.** Call `read_file` on `.github/workflows/release.yml`. Confirm the three affected action references are present: `actions/checkout@v4` (Checkout step), `actions/cache@v4` (Cache Rust build artefacts step), and `softprops/action-gh-release@v2` (Upload release asset step).

2. **Patch the workflow to add the `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24` environment variable.** Call `patch_file` on `.github/workflows/release.yml` with a unified diff that inserts the following block between the `permissions:` block and the `jobs:` key:

   ```yaml
   env:
     FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true
   ```

   The resulting file structure in the affected region must be:
   ```yaml
   permissions:
     contents: write

   env:
     FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true

   jobs:
   ```

3. **Verify the patch.** Call `read_file` on `.github/workflows/release.yml` and confirm:
   - The `env:` block containing `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true` is present at workflow level, between `permissions:` and `jobs:`.
   - All three action references (`actions/checkout@v4`, `actions/cache@v4`, `softprops/action-gh-release@v2`) remain unchanged.
   - No structural YAML regressions are present (no duplicate keys, no indentation errors).

## Decisions

### Decision 1 — Use `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24=true` rather than upgrading individual action versions

**Rationale:** GitHub officially supports `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24=true` as the interim migration path. It covers all three affected actions in a single one-line change and eliminates the risk of behavioural regressions from upgrading individual action versions (changed input or output contracts). At the time of authoring, no stable Node.js 24-native release exists for all three actions.

**Rejected alternatives:**
- _Upgrade actions/checkout, actions/cache, and softprops/action-gh-release to Node.js 24-native versions_: Action maintainers have not yet published stable Node.js 24-native releases for all three. Piecemeal upgrades require per-action validation against both Linux and Windows runners.
- _Wait until native Node.js 24 action versions are published_: Deferring past 2026-06-02 causes build and release pipeline failures on that date.

**Consequences:** The `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24` variable is an interim measure. A follow-up signal or specification should be authored once the three action maintainers publish stable Node.js 24-native versions to remove the override.

### Decision 2 — Apply the environment variable at workflow level, not job or step level

**Rationale:** The three affected actions span two matrix jobs (Linux and Windows) and multiple steps. A top-level `env:` block applies the variable uniformly across all jobs and steps without duplication. Step-level `env:` applies only to `run:` shell steps, not to `uses:` action steps, making it unsuitable here.

**Rejected alternatives:**
- _Add `env:` under each job definition_: Duplicates the variable in both matrix jobs with no benefit.
- _Add `env:` on each affected `uses:` step_: The `env:` key on a `uses:` step does not pass environment variables into JavaScript action processes in the same way; workflow-level or job-level placement is required by GitHub Actions semantics.

**Consequences:** All future `uses:` action steps in this workflow will inherit `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true`, which is intentional until all actions are migrated to Node.js 24-native versions.

## Rubric

### Structured

| Name | Description | Threshold | Pass Condition |
|------|-------------|-----------|----------------|
| `tool-errors` | No ToolHandler errors were recorded during this command invocation. Covers all tool calls on both CLI and MCP paths via `RealToolExecutor::execute`. Applies to every moeb command. | Zero tool errors | Kernel-authoritative: injected by `verify_rubrics` from `RunState.tool_errors`. Do not supply a verdict — any agent-supplied entry is overridden. |
| `rubric-qualification` | No Pass verdicts with acknowledged failures were issued during this command invocation. An agent that observes a failure but passes a criterion must populate `acknowledged_failures` in the verdict object. Applies to every moeb command. | Zero qualified Pass verdicts | Kernel-authoritative: injected by `verify_rubrics` by scanning `acknowledged_failures` on incoming Pass verdicts. Do not supply a verdict — any agent-supplied entry is overridden. |
| `skill-mandatory-phases` | The skill loaded for this run contains all mandatory phase headings. The agent checks its own prompt context (the loaded skill content is already present) for the exact strings: "Verify", "End-of-Skill Review", "Metrics Recording", "Commit", "Tag", "Complete". No file read is required. | All six mandatory headings present | Agent scans skill content in its loaded context; fails if any mandatory heading string is absent |
| `no-drift` | The specification does not violate any decision recorded in a linked parent specification | Zero contradictions | Manual review of every decision in every parent spec listed in Backlinks |
| `spec-schema-compliance` | All required frontmatter fields and body sections are present and correctly ordered | 100% of required fields and sections | Validation in `domain/spec.rs` exits 0 during `moeb spec` |
| `ai-first-org` | The specification's Steps prescribe file layouts, naming, and helper placement that satisfy the four AI-first organisation principles: one concern per file, context locality, grep-discoverable names, no cross-cutting helpers. | All four principles followed | Manual review: no step produces a file that mixes concerns, no name is generic across the codebase, all helpers are co-located with their callers or promoted to a precisely named module |
| `kernel-thin-and-parity` | No domain-specific or workflow-specific coordination logic may be placed in kernel Rust code when it can live in skill markdown or role files. Every tool registered in the CLI tool registry must also be registered in the MCP stdio server tool list. | Zero violations | Spec review: no proposed step places review/coordination logic in Rust; Run review: grep confirms no skill-specific logic in kernel tools; tool lists in tools/mod.rs and the MCP server match exactly |
| `moeb-tool-origin` | All tool calls made during this invocation must originate from moeb's own tool registry. If an external tool (not in the moeb registry) was used because no moeb equivalent exists, the agent must write a MissingMoebTool signal immediately after each such call. The criterion always fails when any external tool is used, whether or not a signal was written. | Zero external tool calls | Agent reviews the conversation for calls to non-moeb tools (e.g. Bash, Edit, Write, Read, Glob, Grep, WebFetch, WebSearch in Claude Code MCP mode). Supplies Pass if none were made. If external tools were used with MissingMoebTool signals, supplies Fail with `acknowledged_failures` listing each signal title. If external tools were used without signals, supplies Fail with the unlogged tool names in the note. |

### Qualitative

- The workflow YAML file must remain syntactically valid after the patch; no indentation errors, no duplicate top-level keys.
- The `env:` block must be placed at the correct level — top-level workflow scope, not nested inside a job or step.
- Steps must be precise enough for an agent to construct the `patch_file` diff without ambiguity about hunk start lines or context.
- The specification must not widen scope beyond the three confirmed affected actions and the single workflow file.
