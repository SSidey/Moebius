---
domain: moeb
slug: fix-signal-event-emission
status: active
supersedes:
  - path: specifications/moeb/moeb.signal-fix-command.md
    decision: "Decision 2 — fix_signal creates a preliminary isolation branch before calling start_spec"
  - path: specifications/moeb/moeb.signal-fix-command.md
    decision: "Decision 5 — Requirement for start_spec is derived from proposed_resolution, falling back to title + description"
---

# Fix Signal: Event Emission Replaces spec/run Orchestration

## Raw Requirement

Reviewing the skills, each should have distinct separation, meaning that we need to reduce the responsibility of fix_signal: it should not call into `spec` or `run`, instead it needs to produce a new event artifact, for now this can be within the .moeb folder, may warrant .moeb/events, tentative structure (type would vary, here this would be running fix_signal and emitting an event that captures the next accepted signal by that skill):
```
{
  "type": "signal.accepted",
  "signal_id": "...",
  "repo": "...",
  "branch": "...",
  "timestamp": "..."
}
```

## Description

Reduces `fix_signal`'s responsibility to signal acceptance: scan `.moeb/signals/` for the most-critical unresolved signal, select it, mark it with `picked_up_at`, create the isolation branch, then emit a `signal.accepted` event artifact to `.moeb/events/<signal_id>.event.json`. The direct calls to `start_spec` and `start_run` in the current `fix_signal.skill.md` Phases 5–6 are removed entirely. A future consumer — whether a human operator, a scheduler, or a new dedicated skill — reads the event file and initiates the spec-and-run cycle independently. The `fix_signal.rs` Rust tool is unchanged; the behaviour change is confined to `fix_signal.skill.md`.

```mermaid
sequenceDiagram
    participant Agent as MCP Agent
    participant FST as fix_signal tool (Rust)
    participant Sigs as .moeb/signals/
    participant VCS as Git (create_branch)
    participant Evts as .moeb/events/

    Agent->>FST: fix_signal {}
    FST->>Agent: rendered fix_signal.prompt + fix_signal.skill.md

    Note over Agent: Phase 1 — Scan
    Agent->>Sigs: list_directory + read_file per *.signals.json
    Note over Agent: collect signals without picked_up_at

    Note over Agent: Phase 2 — Select
    Note over Agent: sort Critical>Major>other, oldest first; pick top signal

    Note over Agent: Phase 3 — Mark
    Agent->>Sigs: patch_file — add picked_up_at + fix_branch to chosen signal

    Note over Agent: Phase 4 — Branch
    Agent->>VCS: create_branch(domain, slug)

    Note over Agent: Phase 5 — Emit Event
    Agent->>Evts: write_file — .moeb/events/<signal_id>.event.json
    Note over Evts: {"type":"signal.accepted","signal_id":"...","repo":"...","branch":"...","timestamp":"..."}
```

## Backlinks

### Parents

| Label | Path | Purpose |
|-------|------|---------|
| Harness README | README.md | Root index and policy source |
| Signal Fix Command | specifications/moeb/moeb.signal-fix-command.md | Establishes fix_signal tool, skill structure, and signal-scanning Phases 1–4 that this spec preserves while superseding Phases 5–6 |
| Rubric Integrity Enforcement | specifications/moeb/moeb.rubric-integrity-enforcement.md | Defines signal schema and `.moeb/signals/` conventions that fix_signal reads and patches |
| Specification Creation: Branch Type Correction to feat/ | specifications/vcs/vcs.spec-branch-type-feat.md | Establishes `feat/<domain>-<slug>` naming rules applied in Phase 4 |

## Steps

### Step 1 — Read current `fix_signal.skill.md`

Call `read_file` on `src/moeb/internal/skills/fix_signal.skill.md` to load the current skill content into the read-scope before patching.

### Step 2 — Replace Phases 5–6 with Phase 5 — Emit Event

Apply a `patch_file` diff to `src/moeb/internal/skills/fix_signal.skill.md` that:

1. **Removes Phase 5 — Spec** (the block beginning `## Phase 5 — Spec` through the end of that phase's content, including the paragraph calling `start_spec` and following the embedded spec skill workflow).

2. **Removes Phase 6 — Run** (the block beginning `## Phase 6 — Run` through the end of that phase's content, including the paragraph calling `start_run` and following the embedded run skill workflow).

3. **Inserts the following new Phase 5 block** in their place:

```
## Phase 5 — Emit Event

Derive `repo` by calling `read_file` on `.git/config`. Search the returned content for
the line `url =` that appears beneath a `[remote "origin"]` stanza and extract the value
after `= ` (trimmed). If `.git/config` cannot be read or no origin url is found, use the
basename of the working directory as a fallback value for `repo`.

Construct the event object:
```json
{
  "type": "signal.accepted",
  "signal_id": "<signal_id from Phase 2>",
  "repo": "<repo URL or directory basename>",
  "branch": "<fix_branch from Phase 3>",
  "timestamp": "<current ISO 8601 timestamp>"
}
```

Call `write_file` with path `.moeb/events/<signal_id>.event.json` and the event JSON
content, pretty-printed with 2-space indentation. Record a StepMetric for
`step_id: "emit-event"`.
```

4. **Renumber all downstream phases** so they remain consecutive:
   - Old Phase 7 (Verify) → `## Phase 6 — Verify`
   - Old Phase 8 (End-of-Skill Review) → `## Phase 7 — End-of-Skill Review`
   - Old Phase 9 (Metrics Recording) → `## Phase 8 — Metrics Recording`
   - Old Phase 10 (Commit) → `## Phase 9 — Commit`
   - Old Phase 11 (Tag) → `## Phase 10 — Tag`
   - Old Phase 12 (Complete) → `## Phase 11 — Complete`

5. **Update the run file JSON** in the (renumbered) Phase 8 — Metrics Recording: replace the `"spec_path"` field with `"event_path": "<path of the event file written in Phase 5, i.e. .moeb/events/<signal_id>.event.json>"`.

6. **Update the Complete phase** summary instruction to read: `"Respond with a concise summary: which signal was selected (signal_id, title, severity), which branch was created (Phase 4), which event was emitted (path and type), and whether the run completed successfully."`

7. **Remove `start_spec` and `start_run` from the Tool Origin Policy tool list** at the top of the skill, since fix_signal no longer calls those tools.

After patching, verify the six mandatory heading strings remain present in the updated file: "Verify", "End-of-Skill Review", "Metrics Recording", "Commit", "Tag", "Complete".

### Step 3 — Verify no Rust changes are required

Confirm that `src/moeb/src/tools/fix_signal.rs` contains no references to `start_spec`, `start_run`, `spec_path`, or any Phase 5/6 workflow logic. The tool renders a prompt template; all workflow logic lives in the skill. No edit is required to any Rust source file.

### Step 4 — Build and test

Run `cargo build` from `src/moeb/` to verify the binary still compiles without errors. Run `cargo test` to confirm no regressions in the existing test suite.

## Decisions

### Decision 1 — fix_signal scope is reduced to signal acceptance; spec/run invocation is removed from the skill

**Rationale:** `fix_signal` calling `start_spec` and then `start_run` within the same skill creates a God-skill spanning three distinct workflow domains: signal triage, specification authoring, and implementation. Each domain carries its own rubric gates, review loops, metrics files, and commit checkpoints. Conflating them in one skill makes it impossible to verify or retry any phase independently. Restricting `fix_signal` to signal acceptance (Phases 1–5) ensures each skill addresses exactly one concern, consistent with the harness principle of minimal scope.

**Rejected alternatives:** Keeping `start_spec` and `start_run` in the skill was the original design (`moeb.signal-fix-command.md` Phases 5–6). Rejected because it couples triage to authoring and implementation, making individual phase quality gates impossible to isolate or restart.

**Consequences:** After a `fix_signal` run completes, no specification is authored and no code is implemented automatically. A human operator or event-driven consumer must read the emitted event and separately invoke the spec and run cycles. This is an intentional design boundary.

### Decision 2 — Events are written to `.moeb/events/<signal_id>.event.json`

**Rationale:** Following the established harness pattern of dedicated output subdirectories (`.moeb/signals/`, `.moeb/metrics/`, `.moeb/runs/`), a new `.moeb/events/` directory holds all event artifacts. Using `signal_id` as the filename makes the event trivially correlatable to the originating signal without parsing event content. The `.event.json` suffix prevents collision with signal files.

**Rejected alternatives:** Embedding the event inside the signal file alongside `picked_up_at` would co-locate producer and consumer data in the same file, complicating independent schema evolution. Using `run_id` as the filename would lose direct signal correlation.

**Consequences:** A `.moeb/events/` directory is created on first use by `write_file` (which creates missing parent directories). External consumers scan `.moeb/events/` for `*.event.json` files with `"type": "signal.accepted"` to discover pending work.

### Decision 3 — `repo` field is derived from `.git/config` remote origin URL

**Rationale:** The event's `repo` field identifies which repository accepted the signal, enabling future multi-repo or remote-consumer scenarios. The git remote origin URL is the canonical, human-readable identifier and is always present in a moeb-managed repository (required by `vcs.git-init`). Reading `.git/config` via `read_file` stays within the moeb tool set.

**Rejected alternatives:** Using the working directory basename is simpler but ambiguous — multiple repositories may share the same directory name. Omitting the field entirely was rejected because consumers need it for routing and display.

**Consequences:** If the repository has no `remote "origin"` configured, the skill falls back to the working directory basename. This case is documented in Phase 5 so agents handle it gracefully.

### Decision 4 — No Rust kernel changes are required

**Rationale:** `fix_signal.rs` is a thin prompt renderer that substitutes template tokens into `fix_signal.prompt`. It has no awareness of which phases the skill contains or which downstream tools the skill calls. The change from "call start_spec then start_run" to "emit event" is entirely a skill-level concern.

**Rejected alternatives:** A dedicated `emit_event` Rust tool was considered to encapsulate the write operation. Rejected because `write_file` already handles arbitrary JSON writes, and a new tool would violate the thin-kernel principle without adding capability.

**Consequences:** The skill uses `write_file` directly to emit events. Parent directory creation is handled by `write_file`'s existing behaviour of creating missing intermediate directories.

### Decision 5 — Branch creation (Phase 4) is retained; it is now the definitive branch, not a preliminary one

**Rationale:** The previous `moeb.signal-fix-command.md` Decision 2 described the Phase 4 branch as "preliminary" because `start_spec` Phase 11 would later create a definitive spec branch. With spec invocation removed, the Phase 4 branch — `feat/<domain>-<slug>` derived from the signal title — is the only branch created and is therefore the definitive working branch for any downstream operator who acts on the event.

**Rejected alternatives:** Removing branch creation entirely was considered. Rejected because the `branch` field in the `signal.accepted` event provides consumers with a ready-to-use isolation branch, avoiding a naming collision if multiple consumers process events concurrently.

**Consequences:** Supersedes Decision 2 of `moeb.signal-fix-command.md`. The Phase 4 branch is both the signal-isolation branch and the consumer's implementation branch. Consumers check out this branch before invoking their own spec and run cycles.

## Rubric

### Structured

| Name | Description | Threshold | Pass Condition |
|------|-------------|-----------|----------------|
| `tool-errors` | No ToolHandler errors were recorded during this command invocation. Covers all tool calls on both CLI and MCP paths via `RealToolExecutor::execute`. Applies to every moeb command. | Zero tool errors | Kernel-authoritative: injected by `verify_rubrics` from `RunState.tool_errors`. Do not supply a verdict — any agent-supplied entry is overridden. |
| `rubric-qualification` | No Pass verdicts with acknowledged failures were issued during this command invocation. An agent that observes a failure but passes a criterion must populate `acknowledged_failures` in the verdict object. Applies to every moeb command. | Zero qualified Pass verdicts | Kernel-authoritative: injected by `verify_rubrics` by scanning `acknowledged_failures` on incoming Pass verdicts. Do not supply a verdict — any agent-supplied entry is overridden. |
| `skill-mandatory-phases` | The skill loaded for this run contains all mandatory phase headings. The agent checks its own prompt context for the exact strings: "Verify", "End-of-Skill Review", "Metrics Recording", "Commit", "Tag", "Complete". | All six mandatory headings present | Agent scans skill content in its loaded context; fails if any mandatory heading string is absent |
| `no-drift` | The specification does not violate any decision recorded in a linked parent specification | Zero contradictions | Manual review of every decision in every parent spec listed in Backlinks |
| `spec-schema-compliance` | All required frontmatter fields and body sections are present and correctly ordered | 100% of required fields and sections | Validation in `domain/spec.rs` exits 0 during `moeb spec` |
| `ai-first-org` | The specification's Steps prescribe file layouts, naming, and helper placement that satisfy the four AI-first organisation principles: one concern per file, context locality, grep-discoverable names, no cross-cutting helpers. | All four principles followed | Manual review: skill change is confined to fix_signal.skill.md; event schema defined inline in Phase 5 instruction; no cross-cutting helpers introduced |
| `kernel-thin-and-parity` | No domain-specific or workflow-specific coordination logic may be placed in kernel Rust code when it can live in skill markdown or role files. Every tool registered in the CLI tool registry must also be registered in the MCP stdio server tool list. | Zero violations | Spec review: no step places workflow logic in Rust; event emission is entirely in the skill; no new tools are registered |
| `moeb-tool-origin` | All tool calls made during this invocation must originate from moeb's own tool registry. | Zero external tool calls | Agent reviews the conversation for calls to non-moeb tools. Supplies Pass if none were made. If external tools were used with MissingMoebTool signals, supplies Fail with `acknowledged_failures` listing each signal title. |
| `new-skill-mandatory-phases` | Any *.skill.md file written during this run contains all mandatory phase heading strings: "Verify", "End-of-Skill Review", "Metrics Recording", "Commit", "Tag", "Complete" | All six strings present in the written skill file | Confirm the updated fix_signal.skill.md contains each of the six mandatory heading strings as phase headings |
| `signal-accepted-event-completeness` | The `signal.accepted` event written to `.moeb/events/<signal_id>.event.json` contains all five required fields: `type`, `signal_id`, `repo`, `branch`, `timestamp` | All five fields present and non-empty in the emitted event JSON | Run review: after fix_signal completes, read the event file and confirm all five fields are present and non-empty |

### Qualitative

- The updated `fix_signal.skill.md` must contain no reference to `start_spec` or `start_run`.
- The event file path must be `.moeb/events/<signal_id>.event.json` where `signal_id` is the UUID from the accepted signal — not the `run_id` or any other identifier.
- The skill change must not alter the content of Phases 1–4 (Scan, Select, Mark, Branch) beyond renaming them if needed.
- All six mandatory phase heading strings must remain present in the updated skill after removal of Phases 5–6: "Verify", "End-of-Skill Review", "Metrics Recording", "Commit", "Tag", "Complete".
- After a complete `fix_signal` run, the repository must be on the branch created in Phase 4 (`feat/<domain>-<slug>`), with the event file present in `.moeb/events/`.
