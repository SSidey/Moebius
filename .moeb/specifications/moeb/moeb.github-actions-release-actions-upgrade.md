---
domain: moeb
slug: github-actions-release-actions-upgrade
status: active
supersedes:
  - path: specifications/moeb/moeb.github-actions-nodejs24-upgrade.md
    decision: "Decision 1 — Use `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24=true` rather than upgrading individual action versions"
---

# GitHub Actions: Upgrade Release Actions to Native Node.js 24 Versions

## Raw Requirement

Github actions presents the following: Node20 will reach end-of-life (EOL) in April of 2026. As a result we have started the deprecation process of Node20 for GitHub Actions. We plan to migrate all actions to run on Node24 in the fall of 2026.

We need to update the actions used, a fix was previously attempted by signal fixing a1000001-0529-4000-8000-000000000001, however the release action in run 9d5a69df-ac3a-4af4-a140-9aae58bd25d1 still resulted in the node warning on release

The explicit warning was: Node.js 20 is deprecated. The following actions target Node.js 20 but are being forced to run on Node.js 24: actions/cache@v4, actions/checkout@v4, softprops/action-gh-release@v2. For more information see: https://github.blog/changelog/2025-09-19-deprecation-of-node-20-on-github-actions-runners/

A suggestion for the fix would be to upgrade the version of our actions used from v4, v4, v2 to their most recent stable version

## Description

The previous specification (`moeb.github-actions-nodejs24-upgrade`) added `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true` at the workflow level as an interim measure. Although this causes the runner to execute actions under Node.js 24, GitHub still emits the deprecation warning because the action manifests themselves declare `node20` as their runtime. Run `9d5a69df-ac3a-4af4-a140-9aae58bd25d1` confirms the warning persists for all three affected actions — `actions/checkout@v4`, `actions/cache@v4`, and `softprops/action-gh-release@v2` — despite the environment variable being present.

The definitive fix is to upgrade the three action version pins in `.github/workflows/release.yml` to the most recent stable versions that natively declare `node24` (or later) as their runtime. Once all three actions natively target Node.js 24, the `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true` environment variable is redundant and must be removed to keep the workflow minimal.

```mermaid
graph TD
    A[release.yml — current] --> B[actions/checkout@v4 node20]
    A --> C[actions/cache@v4 node20]
    A --> D[softprops/action-gh-release@v2 node20]
    B --> E[determine latest stable Node24-native version]
    C --> E
    D --> E
    E --> F[patch release.yml with new version pins]
    F --> G[remove FORCE_JAVASCRIPT_ACTIONS_TO_NODE24]
    G --> H[release.yml — no Node20 deprecation warning]
```

## Backlinks

### Parents

| Label | Path | Purpose |
|-------|------|---------|
| Harness README | README.md | Governing harness for all specifications |
| GitHub Actions: Node.js 24 Actions Upgrade | specifications/moeb/moeb.github-actions-nodejs24-upgrade.md | Prior specification whose Decision 1 this spec supersedes |

## Steps

1. **Read the workflow file.** Call `read_file` on `.github/workflows/release.yml`. Confirm the three action pins (`actions/checkout@v4`, `actions/cache@v4`, `softprops/action-gh-release@v2`) and the presence of `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true` in the workflow-level `env:` block.

2. **Determine target versions.** Call `query_agent` with the following parameters to identify the latest stable Node.js 24-native version for each action:
   - `role`: `run`
   - `prompt`: "Identify the latest stable release tag for each of the following GitHub Actions that natively declares `node24` (or later) as the `runs.using` value in its `action.yml`: (1) actions/checkout — currently pinned at v4; (2) actions/cache — currently pinned at v4; (3) softprops/action-gh-release — currently pinned at v2. Return your answer as a JSON object: { \"checkout\": \"<tag>\", \"cache\": \"<tag>\", \"action-gh-release\": \"<tag>\" }. If a native Node.js 24 version is not yet available for a given action, set its value to null and add a `notes` key with a brief explanation."
   - `expected_response_type`: `json`
   - Parse the returned JSON. For any action whose value is `null`, that action cannot be upgraded in this run and `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true` must remain.

3. **Patch action versions.** For each action with a non-null version from Step 2, call `patch_file` on `.github/workflows/release.yml` to replace the current version pin with the Node.js 24-native version. Apply one minimal-diff patch per action to keep each change independent and reviewable.

4. **Remove `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24`.** If all three actions returned non-null versions in Step 2 (all three pins have been upgraded), call `patch_file` on `.github/workflows/release.yml` to remove the `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true` line from the `env:` block. If the `env:` block is then empty, remove the entire `env:` block including its key. If any action could not be upgraded, skip this step and retain `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true` unchanged.

5. **Verify.** Call `read_file` on `.github/workflows/release.yml` and confirm:
   - Each upgraded action pin matches the version returned in Step 2.
   - `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24` is absent if all three actions were upgraded; still present if any action could not be upgraded.
   - No structural YAML regressions: no duplicate keys, no indentation errors, `jobs:` key and all workflow steps remain intact.

## Decisions

### Decision 1 — Upgrade action versions to eliminate the Node.js 20 deprecation warning natively

**Rationale:** `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true` causes the runner to execute actions under Node.js 24, but GitHub still emits the deprecation warning because the action manifest's `runs.using` field declares `node20`. Run `9d5a69df-ac3a-4af4-a140-9aae58bd25d1` confirms this: the warning fires for all three actions even with the environment variable present. Upgrading to versions that natively declare `node24` in their manifest removes the warning at its source. This supersedes Decision 1 from `moeb.github-actions-nodejs24-upgrade.md`, which chose `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24=true` on the grounds that no stable Node.js 24-native releases existed at that time.

**Rejected alternatives:**
- _Retain `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true` without upgrading action versions_: Demonstrated to be insufficient — the deprecation warning fires in every release run despite the override.
- _Pin to explicit patch versions (e.g., `actions/checkout@v4.2.3`) rather than the latest stable major tag_: Patch pinning provides strong reproducibility but stalls on security patches unless the spec is updated. The existing workflow convention uses floating major tags; changing that convention is out of scope for this fix.

**Consequences:** The implementing agent must determine the correct version tags using its AI knowledge at implementation time via `query_agent`. If a native Node.js 24 version is not yet available for one or more actions, those actions retain the `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24` mitigation and a follow-up signal should be authored.

### Decision 2 — Remove `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true` once all three actions are upgraded

**Rationale:** The environment variable was an explicitly interim measure. Retaining it after all three actions natively target Node.js 24 adds unnecessary configuration noise and could silently suppress future deprecation warnings for any new actions added to the workflow. Removing it once all three actions are upgraded keeps the workflow minimal and transparent.

**Rejected alternatives:**
- _Keep `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true` as a defensive override_: If all action manifests declare `node24`, the variable has no effect. If a future action manifest declares an older runtime, retaining the variable would silently suppress the associated deprecation warning rather than surfacing it for remediation.

**Consequences:** Partial upgrades (where one or more actions cannot be upgraded) retain `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true` as a partial mitigation. A follow-up signal or specification is implicitly required to complete the migration once the remaining actions publish Node.js 24-native versions.

### Decision 3 — Use `query_agent` to determine target versions at implementation time

**Rationale:** Hardcoding version tags in this specification risks staleness: if newer Node.js 24-native versions are published between spec authoring and implementation, the spec would prescribe an outdated tag. Delegating version determination to `query_agent` ensures the implementing agent uses the most current information available from its training knowledge, while the spec captures the intent (Node.js 24-native runtime) and acceptance criteria rather than a point-in-time version string.

**Rejected alternatives:**
- _Hardcode specific version tags_: Accurate at authoring time (2026-05-29) but potentially stale at implementation time. The spec's intent — native Node.js 24 support — is better expressed as a goal than a specific tag.
- _Use web search to look up the latest version at implementation time_: No moeb tool provides web search capability; `query_agent` using AI training knowledge is the closest available mechanism.

**Consequences:** The implementing agent bears responsibility for confirming that the version tags it selects are syntactically valid GitHub Actions tags. If `query_agent` returns `null` for any action, the implementing agent must not invent a tag and must retain `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true` for that action.

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
| `no-signal-reoccurrence` | No canonical signal in `.moeb/signals/catalogue/` has `status: reopened` due to this run | Zero reopened signals introduced by this run | Agent scans all signal writes during this run; fails if any canonical signal file has `status: reopened` after processing |

### Qualitative

- All three action pins must reference the version tags returned by `query_agent` in Step 2 after patching; any deviation constitutes a failure.
- If all three actions are upgraded, `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24` must be absent from the workflow file.
- If any action cannot be upgraded, `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true` must remain and the commit message must identify which actions were not upgraded and why.
- The YAML structure of `release.yml` must remain valid after all patches: no duplicate keys, no indentation errors, workflow-level sections (`on:`, `permissions:`, `env:` if retained, `jobs:`) in their correct positions.
