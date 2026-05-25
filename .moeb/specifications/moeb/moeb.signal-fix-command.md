---
domain: moeb
slug: signal-fix-command
status: active
---

# Signal Fix Command: MCP-Driven Autonomous Signal Resolution

## Raw Requirement

Currently we have two commands in moeb that are directly invoked that result in skills 'spec' and 'run', we produce signals when failures are detected, we could use a new command, made available through mcp, that will start a new skill that checks all current unresolved signals and chooses the most significant by severity, once it has the signal it should mark the signal in a way to denote that it has been picked up to attempt a fix and create a branch as per our branching rules, once we have concluded this workflow, it should result in a 'start spec' directly for the signal chosen signal and then once we have the spec invocation completed, a start run should initiate for the new spec file

## Description

A new `fix_signal` MCP tool provides autonomous signal resolution by orchestrating the full spec-then-run pipeline for the most-critical unresolved issue in `.moeb/signals/`. The tool returns a rendered prompt backed by a new binary-bundled `fix_signal.skill.md` that drives the agent through: scanning signal files for unresolved signals (absence of `picked_up_at`), selecting the highest-severity signal (`Critical` > `Major` > any other, with oldest-timestamp tiebreak), marking it in-place with `picked_up_at` and `fix_branch` fields, creating an isolation branch (`feat/<derived-domain>-<derived-slug>`), then calling `start_spec` as a tool and following its embedded skill workflow to produce a specification, and finally calling `start_run` with the resulting spec path and following its embedded skill workflow to implement it. The `fix_signal.skill.md` carries all mandatory skill phase headings — Verify, End-of-Skill Review, Metrics Recording, Commit, Tag, and Complete — wrapping the two embedded workflows.

```mermaid
sequenceDiagram
    participant Agent as MCP Agent
    participant FST as fix_signal tool (Rust)
    participant Sigs as .moeb/signals/
    participant VCS as Git (create_branch)
    participant SST as start_spec tool
    participant SRT as start_run tool

    Agent->>FST: fix_signal {}
    FST->>Agent: rendered fix_signal.prompt + fix_signal.skill.md

    Note over Agent: Phase 1-2 — Scan and Select
    Agent->>Sigs: list_directory + read_file per *.signals.json
    Note over Agent: collect signals without picked_up_at; sort Critical>Major>other, oldest first

    Note over Agent: Phase 3 — Mark
    Agent->>Sigs: patch_file — add picked_up_at and fix_branch to chosen signal object

    Note over Agent: Phase 4 — Branch
    Agent->>VCS: create_branch(domain, slug derived from signal)

    Note over Agent: Phase 5 — Spec
    Agent->>SST: start_spec(requirement from signal proposed_resolution)
    SST->>Agent: rendered spec.prompt + spec.skill.md
    Note over Agent: follow all spec skill phases (Author → Write → Link → Verify → Commit → Branch …)

    Note over Agent: Phase 6 — Run
    Agent->>SRT: start_run(spec_path from completed spec workflow)
    SRT->>Agent: rendered run.prompt + run.skill.md
    Note over Agent: follow all run skill phases (Discover → Implement → Verify → Commit → Tag …)
```

## Backlinks

### Parents

| Label | Path | Purpose |
|-------|------|---------|
| Harness README | README.md | Root index and policy source |
| MCP Serve / CLI Parity | specifications/moeb/moeb.serve-cli-parity.md | Introduces start_spec and start_run MCP tools; fix_signal orchestrates both |
| Rubric Integrity Enforcement | specifications/moeb/moeb.rubric-integrity-enforcement.md | Defines signal schema and .moeb/signals/ conventions that fix_signal reads and patches |
| Specification Creation: Branch Type Correction to feat/ | specifications/vcs/vcs.spec-branch-type-feat.md | Establishes feat/<domain>-<slug> naming rules applied in Phase 4 |

## Steps

### Step 1 — Create `assets/skills/fix_signal.skill.md`

Create `assets/skills/fix_signal.skill.md`. This file is binary-bundled and drives the 12-phase fix_signal workflow. All mandatory heading strings must appear: "Verify", "End-of-Skill Review", "Metrics Recording", "Commit", "Tag", "Complete".

**Phase 1 — Scan**

Call `list_directory` on `.moeb/signals/`. For each entry ending with `.signals.json`, call `read_file` on that path. Parse the returned JSON array. Collect every signal object across all files where the `picked_up_at` field is absent or null. These are unresolved signals. Record each signal alongside its source file path.

If no `.signals.json` files exist or all signals have `picked_up_at` set, skip directly to Phase 12 with the message: `"No unresolved signals found."`.

**Phase 2 — Select**

Assign each unresolved signal a numeric severity rank: `"Critical"` → 2, `"Major"` → 1, any other value → 0. Sort the collected signals by severity rank descending, then by `timestamp` ascending (oldest first among equal severity). Select the first signal after sorting. Record its `signal_id`, `severity`, `title`, `proposed_resolution`, and its source file path.

**Phase 3 — Mark**

Compute the `fix_branch` name before patching:
1. Derive `domain`: if the signal's `category` is `"Error"`, use `"fix"`; otherwise use `"moeb"`.
2. Derive `slug`: take `title`, lowercase all characters, replace every character that is not `a-z`, `0-9`, or `-` with a hyphen, collapse consecutive hyphens into one, strip leading and trailing hyphens, truncate to 50 characters.
3. `fix_branch` = `"feat/<domain>-<slug>"`.

Call `read_file` on the source file path (required before patch_file per scope enforcement). Construct a minimal unified diff that adds `"picked_up_at": "<current ISO 8601 timestamp>"` and `"fix_branch": "<fix_branch>"` as new fields after the last existing field of the chosen signal object, before its closing `}`. Call `patch_file` with this diff on the source file path. If `patch_file` returns an error, read the current file content, add the two fields to the chosen signal object in memory, and call `write_file` to overwrite — recording a StepMetric for `step_id: "mark-signal"` regardless of which path was taken.

**Phase 4 — Branch**

Call `create_branch` with the `domain` and `slug` derived in Phase 3. This creates `feat/<domain>-<slug>` and switches the working tree to it. Record a StepMetric for `step_id: "create-branch"`.

**Phase 5 — Spec**

Construct the `requirement` string:
- If the selected signal's `proposed_resolution` is a non-null, non-empty string: use it as-is.
- Otherwise: concatenate `title + ". " + description`.

Call `start_spec` with this requirement string. The tool returns a rendered `spec.prompt` containing the full `spec.skill.md` workflow. **Follow the returned skill phases completely** — including Author (Phase 2), Write (Phase 4), Per-Step Review sub-loops, Link README (Phase 5), Verify (Phase 6), End-of-Skill Review (Phase 7), Metrics Recording (Phase 8), Commit (Phase 9), Tag (Phase 10), Branch (Phase 11), and Complete (Phase 12).

After the embedded spec workflow concludes, record the spec file path written (`.moeb/specifications/<domain>/<domain>.<slug>.md` derived from the spec frontmatter).

**Phase 6 — Run**

Call `start_run` with the `spec_path` recorded in Phase 5. The tool returns a rendered `run.prompt` containing the full `run.skill.md` workflow. **Follow the returned skill phases completely** — including all discovery, implementation, Per-Step Review sub-loops, Verify, End-of-Skill Review, Metrics Recording, Commit, Version Bump, Tag, and Complete phases.

**Phase 7 — Verify**

Evaluate the fix_signal workflow for rubric compliance. Supply verdicts for all criteria from the injected rubric baseline and the specification's `## Rubric / ### Structured` table. Call `verify_rubrics` with the complete verdict list. Do not supply verdicts for kernel-authoritative criteria (`tool-errors`, `rubric-qualification`).

**Phase 8 — End-of-Skill Review**

Adopt the QA Architect Persona pre-loaded in your context. Evaluate the fix_signal workflow outputs: the signal marking patch, the branch creation, the embedded spec and run outcomes, and the accumulated StepMetrics. Produce a `ReviewSignalReport` JSON. Assign `signal_id`, `run_id`, and `timestamp` to each signal. Write signals to `.moeb/signals/<run_id>.signals.json`.

**Phase 9 — Metrics Recording**

Assemble `RunMetrics` for the fix_signal invocation (distinct from the embedded spec and run invocations which write their own metrics files):
- `run_id`: the current fix_signal run identifier (injected as `{{run_id}}`)
- `timestamp`: ISO 8601 run-start time
- `step_metrics`: StepMetric records from mark-signal and create-branch steps
- `wall_time_ms`: elapsed milliseconds since fix_signal run start
- `rubric_score`: written by kernel from RunState — do NOT compute or write this field
- `end_review_error_count`: written by kernel from RunState — do NOT compute or write this field

Write to `.moeb/metrics/{{run_id}}.metrics.json`. Write the run file to `.moeb/runs/{{run_file_path}}` with fields: `run_id`, `timestamp`, `command: "fix_signal"`, `spec_path` (from Phase 5), `signals_path`, `metrics_path`, `rubric_score`, `end_review_error_count`.

**Phase 10 — Commit**

The fix_signal workflow does not produce implementation artifacts requiring a dedicated commit. All spec and code commits are issued by the embedded spec and run skill phases. No git_commit call is made in this phase.

**Phase 11 — Tag**

Call `tag_run` with the current fix_signal `run_id`, `domain` (from Phase 3), and `slug` (from Phase 3).

**Phase 12 — Complete**

Respond with a concise summary: which signal was selected (signal_id, title, severity), which branch was created (Phase 4), which spec was authored (path and domain/slug), and whether the run completed successfully.

---

### Step 2 — Create `src/prompts/fix_signal.prompt`

Create `src/prompts/fix_signal.prompt`. This template delivers context for `fix_signal` invocations.

Required content structure (in order):
1. `{{role_content}}` — persona pre-load (run role).
2. Session context block:
   ```
   Session context:
   - run_id: {{run_id}}
   - run_file_path: {{run_file_path}}
   ```
3. Pre-loaded README section:
   ```
   The following file has been pre-loaded — do not call read_file for it:

   === .moeb/README.md ===
   {{readme_content}}
   ```
4. `=== Workflow ===\n{{skill_content}}` — injects `fix_signal.skill.md`.
5. `{{command_rubrics}}` — injected rubric criteria.
6. Do-not-re-read directive (verbatim from `spec.prompt`): "Do not re-read any file already present in your context window. If you need to refer to content from a pre-loaded section or from a tool result returned in an earlier turn, locate it in your context directly rather than making another tool call."
7. Repository boundary note (verbatim from `spec.prompt`): the working directory is the repository root; use `.moeb/`-prefixed paths for harness artefacts.

No `{{input}}` placeholder is required — the requirement is derived from the signal in Phase 5 of the skill.

Ensure `fix_signal.prompt` is bundled in the binary by the same mechanism used for `spec.prompt` and `run.prompt` (typically `include_str!` in `Prompts::get` or a `build.rs` glob).

### Step 3 — Create `src/moeb/src/tools/fix_signal.rs`

Create `src/moeb/src/tools/fix_signal.rs` implementing `ToolHandler` for `FixSignalTool`.

- `name()` returns `"fix_signal"`.
- `definition()` declares an empty input schema (no required or optional parameters).
- `execute()`:
  1. Load template: `Prompts::get("fix_signal.prompt")`.
  2. Load rubrics using the four-layer order: `Internal::get("rubrics/global.rubrics.md")` (layer 1), no binary command rubric for fix_signal (layer 2 absent — skip), `working_dir.join(".moeb/rubrics/global.rubrics.md")` (layer 3), `working_dir.join(".moeb/rubrics/fix_signal.rubrics.md")` (layer 4, skip if absent). Concatenate non-empty layers into `command_rubrics`.
  3. Load `role_content` via `crate::skills::load_role(working_dir, "run")`.
  4. Load `skill_content` via `crate::skills::load_skill(working_dir, "fix_signal")` — checks `.moeb/skills/fix_signal.skill.md` first, falls back to `assets/skills/fix_signal.skill.md`.
  5. Load `readme_content` from `working_dir.join(".moeb/README.md")`; use empty string on read failure.
  6. Obtain `run_id` and `run_file_path` from `run_state` (same pattern as `start_run.rs` and `start_spec.rs`).
  7. Substitute tokens: `{{role_content}}`, `{{readme_content}}`, `{{skill_content}}`, `{{command_rubrics}}`, `{{run_id}}`, `{{run_file_path}}`.
  8. Return the rendered string.
- Add `#[cfg(test)]` block using `#[path = "fix_signal_tests.rs"]`.

### Step 4 — Create `src/moeb/src/tools/fix_signal_tests.rs`

Create `src/moeb/src/tools/fix_signal_tests.rs` with at least two unit tests:

1. `test_fix_signal_definition_no_required_params`: calls `definition()` and asserts the input schema's `required` field is empty or absent.
2. `test_fix_signal_name`: calls `name()` and asserts the result equals `"fix_signal"`.

### Step 5 — Register `FixSignalTool` in `src/moeb/src/tools/mod.rs`

In `src/moeb/src/tools/mod.rs`:

1. Add `pub mod fix_signal;` to the module declarations.
2. In `ToolRegistry::mcp()`, register `FixSignalTool` after `StartSpecTool`.
3. In `ToolRegistry::definitions()`, add `"fix_signal"` to the `order` array after `"start_spec"`.

Do **not** add `FixSignalTool` to `ToolRegistry::standard()` — it is MCP-only, following the same pattern as `start_spec` and `start_run`.

### Step 6 — Bundle `fix_signal.skill.md` and `fix_signal.prompt` in the binary

Verify that both new files are included in the binary:

- `assets/skills/fix_signal.skill.md`: confirm it is reachable via `crate::skills::load_skill(_, "fix_signal")`. If the bundling mechanism uses an explicit file list or a `match` arm (rather than a glob), add an entry for `"fix_signal"`.
- `src/prompts/fix_signal.prompt`: confirm it is reachable via `Prompts::get("fix_signal.prompt")`. Add to the explicit list or `match` arm if required.

Run `cargo build` to verify the binary compiles and `cargo test` to confirm no regressions.

## Decisions

### Decision 1 — fix_signal is MCP-only, not registered in ToolRegistry::standard()

**Rationale:** `start_spec` and `start_run` are MCP-only tools registered in `ToolRegistry::mcp()` per `moeb.serve-cli-parity` Decision 1. `fix_signal` orchestrates both and follows the same pattern. The `kernel-thin-and-parity` rubric states "every tool registered in the CLI tool registry must also be registered in the MCP list" — it does not require the inverse. MCP-only tools are permitted.

**Rejected alternatives:** Adding `fix_signal` to `standard()` would expose it inside CLI agent loops that are already following a spec or run skill; an agent mid-run calling `fix_signal` would recursively embed two full workflows, producing unpredictable context depth.

**Consequences:** `moeb run` and `moeb spec` CLI agents do not see `fix_signal`. Only MCP clients invoking `moeb serve` can call it.

### Decision 2 — fix_signal creates a preliminary isolation branch before calling start_spec

**Rationale:** The requirement specifies "create a branch as per our branching rules" before invoking `start_spec`. At that point the spec has not been authored, so the spec's domain and slug are unknown. The isolation branch is therefore created using domain and slug derived from the signal title, following the `feat/<domain>-<slug>` convention from `vcs.spec-branch-type-feat`.

When `start_spec` executes its Phase 11 (`create_branch` with the spec frontmatter domain/slug), it creates the definitive working branch. Because `create_branch` uses `git checkout -b` from current HEAD, both the preliminary and definitive branches point to the same spec commit. The definitive branch then carries the implementation commits added by `start_run`. The preliminary branch is a historical artifact and may be pruned after merge.

**Rejected alternatives:** Deferring branch creation entirely to `start_spec` Phase 11 would violate the explicit ordering in the requirement. Creating only one branch requires knowing the spec domain/slug before authoring — a circular dependency.

**Consequences:** Two branches exist after the workflow. Downstream merging targets the definitive spec branch (`feat/<spec-domain>-<spec-slug>`). The preliminary signal branch may be deleted manually.

### Decision 3 — Unresolved signals are identified by absence of picked_up_at; marking adds two optional fields in-place

**Rationale:** The existing signal schema has no status or resolved field. Adding optional `picked_up_at` (ISO 8601 string) and `fix_branch` (branch name string) fields in-place preserves backward compatibility: any signal lacking `picked_up_at` is unresolved. No Rust struct changes are required because the skill patches the JSON file directly and signal consumers use standard JSON parsing that tolerates unknown fields.

**Rejected alternatives:** A `status: "picked_up"` enum field would require updating the Rust `ReviewSignal` struct and all deserialization paths. A separate `signal-state/` directory for pickup tracking avoids in-place mutation but requires correlating two directories on every scan.

**Consequences:** Signal files are no longer purely append-only within a run; fields are added to existing objects. The `picked_up_at` and `fix_branch` fields are not committed in a dedicated commit (see Decision 4). Consumers must tolerate unknown fields.

### Decision 4 — Signal marking is not committed in a dedicated commit

**Rationale:** The current `git_commit` tool stages only `spec_path` and `readme_path`. There is no general-purpose file-staging tool. The in-tree `picked_up_at` marker is sufficient to prevent re-pickup: subsequent `fix_signal` invocations reading the same file will see `picked_up_at` as present and skip that signal. The signal modification will appear in the branch diff when reviewed for merge.

**Rejected alternatives:** Extending `git_commit` with an optional additional-paths parameter was considered but deferred as a separate concern that would benefit multiple workflows. Committing the signal change manually is possible but outside the scope of this automation.

**Consequences:** After `fix_signal` completes, the signal file shows as modified in `git status` but is not committed. It will be included in the branch diff at merge time. A follow-on spec may introduce a `git_stage` or `git_commit_files` tool to close this gap.

### Decision 5 — Requirement for start_spec is derived from proposed_resolution, falling back to title + description

**Rationale:** The `proposed_resolution` field is authored specifically to describe what a fixing specification should accomplish. It is the most concise and targeted input for `start_spec`. When absent or null (permissible per the schema), the title and description together provide sufficient context.

**Rejected alternatives:** Using only the title would produce too narrow a requirement. Passing the full raw signal JSON would include structured fields (signal_id, timestamps, category) that are noise in a spec-authoring context and could confuse the spec agent.

**Consequences:** The quality of the derived requirement depends on the quality of `proposed_resolution` text in the signal. Poorly authored resolutions will produce less targeted specs. This is inherent to autonomous operation and is not a code defect.

## Rubric

### Structured

| Name | Description | Threshold | Pass Condition |
|------|-------------|-----------|----------------|
| `tool-errors` | No ToolHandler errors were recorded during this command invocation. Covers all tool calls on both CLI and MCP paths via `RealToolExecutor::execute`. Applies to every moeb command. | Zero tool errors | Kernel-authoritative: injected by `verify_rubrics` from `RunState.tool_errors`. Do not supply a verdict — any agent-supplied entry is overridden. |
| `rubric-qualification` | No Pass verdicts with acknowledged failures were issued during this command invocation. An agent that observes a failure but passes a criterion must populate `acknowledged_failures` in the verdict object. Applies to every moeb command. | Zero qualified Pass verdicts | Kernel-authoritative: injected by `verify_rubrics` by scanning `acknowledged_failures` on incoming Pass verdicts. Do not supply a verdict — any agent-supplied entry is overridden. |
| `skill-mandatory-phases` | The skill loaded for this run contains all mandatory phase headings. The agent checks its own prompt context for the exact strings: "Verify", "End-of-Skill Review", "Metrics Recording", "Commit", "Tag", "Complete". | All six mandatory headings present | Agent scans skill content in its loaded context; fails if any mandatory heading string is absent |
| `no-drift` | The specification does not violate any decision recorded in a linked parent specification | Zero contradictions | Manual review of every decision in every parent spec listed in Backlinks |
| `spec-schema-compliance` | All required frontmatter fields and body sections are present and correctly ordered | 100% of required fields and sections | Validation in `domain/spec.rs` exits 0 during `moeb spec` |
| `ai-first-org` | The specification's Steps prescribe file layouts, naming, and helper placement that satisfy the four AI-first organisation principles: one concern per file, context locality, grep-discoverable names, no cross-cutting helpers. | All four principles followed | Manual review: fix_signal.rs contains only thin dispatch; fix_signal.skill.md contains all workflow logic; no cross-cutting helpers introduced |
| `kernel-thin-and-parity` | No domain-specific or workflow-specific coordination logic may be placed in kernel Rust code when it can live in skill markdown or role files. Every tool registered in the CLI tool registry must also be registered in the MCP stdio server tool list. | Zero violations | Spec review: fix_signal.rs is a thin prompt-renderer with no workflow logic; all orchestration lives in fix_signal.skill.md; fix_signal is MCP-only so the CLI→MCP parity direction does not apply |
| `moeb-tool-origin` | All tool calls made during this invocation must originate from moeb's own tool registry. | Zero external tool calls | Agent reviews the conversation for calls to non-moeb tools. Supplies Pass if none were made. If external tools were used with MissingMoebTool signals, supplies Fail with `acknowledged_failures` listing each signal title. |
| `thin-kernel` | New Rust code in tools/, commands/, or domain/ must be either a primitive operation wrapper or a thin dispatcher. Workflow logic must live in skill markdown or role files. | Zero workflow-logic functions in new Rust | fix_signal.rs contains no conditional workflow logic; all orchestration lives in fix_signal.skill.md |
| `mcp-cli-behavioural-parity` | For any operation exposed through both CLI and MCP, outcomes must be identical. Any tool in standard() must be in MCP with identical schema. | Zero divergent outcomes | fix_signal is registered only in mcp() — no CLI path exists; the CLI→MCP parity constraint does not apply to this tool |
| `new-skill-mandatory-phases` | Any *.skill.md file written during this run contains all mandatory phase heading strings: "Verify", "End-of-Skill Review", "Metrics Recording", "Commit", "Tag", "Complete" | All six strings present in the written skill file | Confirm fix_signal.skill.md contains each of the six mandatory heading strings as phase headings |
| `signal-mark-prevents-re-pickup` | A signal marked with picked_up_at must not be selected again by a subsequent fix_signal invocation | Zero re-selections of previously marked signals | After one fix_signal run, a second invocation skips the previously marked signal and selects the next unresolved one (or exits cleanly if none remain) |

### Qualitative

- `fix_signal.rs` must contain no conditional branches implementing workflow logic; signal scanning, selection, marking, and orchestration are exclusively the skill's responsibility.
- The embedded spec workflow must follow all phases of `spec.skill.md` as returned by `start_spec`, including the Inline Reviewer/Moderator sub-loops, the git commit phase, and the branch creation phase.
- The embedded run workflow must follow all phases of `run.skill.md` as returned by `start_run`, including rubric verification, version bump, and tag phases.
- After a complete fix_signal run, the repository must be on the definitive spec branch (`feat/<spec-domain>-<spec-slug>`) created by `start_spec` Phase 11, with implementation commits from `start_run`. The preliminary signal branch created in Phase 4 will point to the same spec commit and may be pruned.
- When no unresolved signals exist, the skill must reach Phase 12 and return a clean completion message without error signals or non-zero rubric failures.
