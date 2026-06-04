---
domain: moeb
slug: run-files-must-record-signal-id-and-candidate-tag
status: active
signal_id: a1000046-0604-4000-8000-000000000046
---

# Run Files: Record signal_id and candidate_tag

## Raw Requirement

Run files currently record run_id, spec_path, signals_path, and metrics_path but carry no
link to the signal that triggered the run or the candidate tag produced at the end. Without
these two fields the only way to connect a run to its origin and its output artefact is to
search the git log or signal catalogue — both are fragile and expensive for any automated
tool watching moeb artefacts.

Note: this signal replaces the in-scope portion of a1000006. That signal's proposed
workflow_id, spec_id, and pr_id fields are outside moeb's boundary (workflow_id and pr_id
are orchestrator-owned; spec_id introduces a UUID scheme for specs that moeb does not have).
Those fields belong in the conductor's own state (see Ouro signal b0000009).

Extend the run file schema with two nullable fields: `signal_id` (string | null) and
`candidate_tag` (string | null). Populate `signal_id` when `start_run` is invoked with a
signal context — the value is passed in as an optional parameter and written to the run file
at creation time. Populate `candidate_tag` when `create_candidate_tag` is called — the
agent writes the tag name back into the run file at that point. No other external IDs
(workflow, PR number) are included. Update the fallback stub in `check_run_outputs` to
initialise both fields as null. Document the schema extension in `.moeb/README.md`.

## Description

Two nullable fields are added to the run file JSON schema:

- **`signal_id`** (string | null): identifies the signal that triggered this run. Set when
  `start_run` is invoked with an optional `signal_id` parameter (MCP path). Null for CLI
  `moeb run` invocations where no signal context is provided. Injected into `run.prompt` as
  `{{signal_id}}` and written to the run file during the Metrics Recording phase.

- **`candidate_tag`** (string | null): the annotated git tag created by
  `create_candidate_tag` if QA passed. Set during the Version and Tag phase after
  `create_candidate_tag` returns successfully, by re-reading the run file at
  `{{run_file_path}}`, setting the field, and re-writing. Null when the Version and Tag
  phase is skipped (QA failed) or when `create_candidate_tag` returns an error. Always null
  for `moeb spec` runs.

Both fields are also added to the `check_run_outputs` fallback stub in both `domain/run.rs`
and `domain/spec.rs` with `null` values, keeping the stub schema consistent with the
agent-written version.

**Affected files:**

- `src/moeb/src/tools/start_run.rs` — new optional `signal_id` input parameter; token injection
- `src/moeb/src/domain/run.rs` — token replacement with empty string for CLI path; fallback stub extension
- `src/prompts/run.prompt` — new `signal_id` context line
- `src/moeb/internal/skills/run.skill.md` — Metrics Recording writes `signal_id`; Version and Tag phase updates `candidate_tag`
- `src/moeb/src/domain/spec.rs` — fallback stub extension only

```mermaid
flowchart TD
    A[start_run called\noptional signal_id param] -->|{{signal_id}} injected| B[run.prompt rendered]
    C[moeb run CLI] -->|{{signal_id}} replaced with empty| B
    B --> D[Agent loop: Phases 1..n]
    D --> E[Metrics Recording\nwrite run file\nsignal_id from context OR null\ncandidate_tag: null]
    E --> F{QA passed?}
    F -->|no| G[Version + Tag skipped\ncandidate_tag remains null]
    F -->|yes| H[create_candidate_tag\nreturns tag_name]
    H --> I[re-read run file\nset candidate_tag = tag_name\nre-write run file]
    I --> J[Agent loop exits]
    G --> J
    J --> K{run file present?\nkernel post-loop check}
    K -->|yes| L[done]
    K -->|no| M[check_run_outputs\nfallback stub\nsignal_id: null\ncandidate_tag: null]
    M --> L
```

## Backlinks

### Parents

| Label | Path | Purpose |
|-------|------|---------|
| Harness README | README.md | Root harness index and policy source |
| Run File Artifact | specifications/moeb/moeb.run-file-artifact.md | Defines the run file schema, `check_run_outputs` fallback, and `{{run_file_path}}` injection; this spec extends the schema |
| Signal Status Lifecycle Tracking | specifications/moeb/moeb.signal-status-lifecycle-tracking.md | Establishes `signal_id` in spec frontmatter and run skill candidate status update; the run file `signal_id` field is the parallel link in the run artefact |
| Run Command: SemVer Version Bump and Candidate Tag | specifications/moeb/moeb.run-semver-bump-and-candidate-tag.md | Defines `create_candidate_tag` tool whose return value is written to the run file |
| Run Skill: Gate Candidate Tag on QA Review Pass | specifications/moeb/moeb.run-skill-must-only-emit-candidate-tag-when-qa-rev.md | Defines the QA gate; `candidate_tag` is only written when this gate passes |
| MCP Serve / CLI Parity | specifications/moeb/moeb.serve-cli-parity.md | Establishes `start_run` MCP tool structure that this spec extends with an optional parameter |

## Steps

### Step 1 — Add optional `signal_id` parameter to `start_run.rs`

Read `src/moeb/src/tools/start_run.rs`.

**1a — Extract parameter.** In the `execute` method, after the existing argument extraction
(e.g. after extracting `spec_path`), add:

```rust
let signal_id = args["signal_id"].as_str().unwrap_or("").to_string();
```

**1b — Token constant and substitution.** Add the constant:

```rust
const SIGNAL_ID_TOKEN: &str = "{{signal_id}}";
```

In the prompt rendering chain, add `.replace(SIGNAL_ID_TOKEN, &signal_id)` alongside the
existing `.replace(RUN_FILE_PATH_TOKEN, &run_file_path)` and `.replace(RUN_ID_TOKEN, &run_id)` calls.

**1c — Update `definition()` input schema.** Add the optional `signal_id` property to the
`"properties"` map. Do NOT add it to the `"required"` array:

```json
"signal_id": {
  "type": "string",
  "description": "Optional UUID of the signal in .moeb/signals/catalogue/ that triggered this run. Written to the run file when provided."
}
```

Write the updated file using `write_file`.

### Step 2 — Add `{{signal_id}}` token replacement to `domain/run.rs` (CLI path)

Read `src/moeb/src/domain/run.rs`.

**2a — Token constant and substitution.** Add the constant alongside the existing
`RUN_FILE_PATH_TOKEN` and `RUN_ID_TOKEN` constants:

```rust
const SIGNAL_ID_TOKEN: &str = "{{signal_id}}";
```

In the prompt rendering chain in `run_in()`, add `.replace(SIGNAL_ID_TOKEN, "")` alongside
the other token replacements. This ensures the CLI path renders `signal_id: ` (empty) in
the session context block rather than the literal `{{signal_id}}` string.

**2b — Extend `check_run_outputs` fallback stub.** Locate the `serde_json::json!` stub in
`check_run_outputs`. After the `"end_review_error_count": 0` and `"kernel_fallback": true`
entries, add:

```rust
"signal_id": serde_json::Value::Null,
"candidate_tag": serde_json::Value::Null,
```

Write the updated file using `write_file`.

### Step 3 — Add `signal_id` to `run.prompt` session context block

Read `src/prompts/run.prompt`.

In the `Session context:` block (which currently contains `run_id` and `run_file_path`
lines), add a third line:

```
- signal_id: {{signal_id}}
```

Place it immediately after `- run_file_path: {{run_file_path}}`.

Write the updated file using `write_file`.

### Step 4 — Update `run.skill.md`: Metrics Recording and Version and Tag

Read `src/moeb/internal/skills/run.skill.md`.

**4a — Metrics Recording phase: add `signal_id` and `candidate_tag` to run file template.**

Locate the run file JSON template in the Metrics Recording phase. Add two fields to the
template, immediately after the `"command"` field:

```
"signal_id": <write the value of the `signal_id` session context variable as a JSON string
              if non-empty, or JSON null if the variable is empty or absent>,
"candidate_tag": null,
```

The `candidate_tag` field is always written as JSON `null` in this initial write. It is
updated to the actual tag string in the Version and Tag phase (Change 4b).

**4b — Version and Tag phase: update run file after successful `create_candidate_tag`.**

In the Version and Tag phase, locate the step that calls `create_candidate_tag`. After the
tool returns a tag name successfully (not an error string), add the following instruction
block:

```markdown
**Run file `candidate_tag` update:**
After `create_candidate_tag` returns a tag name (not an error):
1. Call `read_file` with path `{{run_file_path}}`.
2. Parse the JSON content.
3. Set `"candidate_tag"` to the tag name returned by `create_candidate_tag`.
4. Write the updated JSON back to `{{run_file_path}}` using `write_file`.

Skip this update if `create_candidate_tag` returned an error string or if the QA gate
prevented the tag from being created.
```

Write the complete updated file using `write_file`.

### Step 5 — Extend `check_run_outputs` fallback stub in `domain/spec.rs`

Read `src/moeb/src/domain/spec.rs`.

In `check_run_outputs`, locate the `serde_json::json!` stub. After `"end_review_error_count": 0`
and `"kernel_fallback": true`, add:

```rust
"signal_id": serde_json::Value::Null,
"candidate_tag": serde_json::Value::Null,
```

This mirrors the change in Step 2b, ensuring schema parity between the spec-command fallback
and the run-command fallback.

Write the updated file using `write_file`.

### Step 6 — Verify

Run `cargo build --release` — zero errors expected.

Run `cargo test` — zero failures expected.

Confirm `start_run.rs` includes `signal_id` in properties but NOT in required:
```
grep -n "signal_id" src/moeb/src/tools/start_run.rs
```

Confirm `run.prompt` includes the `signal_id` context line:
```
grep "signal_id" src/prompts/run.prompt
```

Confirm both fallback stubs include the new fields:
```
grep -n "signal_id\|candidate_tag" src/moeb/src/domain/run.rs
grep -n "signal_id\|candidate_tag" src/moeb/src/domain/spec.rs
```

Confirm run.skill.md includes both field additions:
```
grep -n "signal_id\|candidate_tag" src/moeb/internal/skills/run.skill.md
```

## Decisions

### Decision 1 — Agent re-reads and re-writes run file for `candidate_tag`; no new kernel tool

**Rationale:** The run file at `{{run_file_path}}` is already known to the agent when
`create_candidate_tag` returns. A read-parse-update-write sequence using `read_file` and
`write_file` requires no new Rust code and keeps all coordination logic in the skill file.
Adding a dedicated `update_run_file` kernel tool for this one-time mutation would violate
`thin-kernel` by placing a domain-aware file-update concern in compiled Rust. The agent's
existing file tools are sufficient.

**Rejected alternatives:**
- Extend `create_candidate_tag.rs` to write to `{{run_file_path}}` directly: requires the
  tool to receive the run file path (a coordination concern) and understand run file
  semantics. Violates the one-concern-per-file AI-first principle.
- Move the entire run file write to after the Version and Tag phase: restructures the
  established Metrics Recording phase ordering and removes the safety net that the fallback
  stub provides when the agent exits early.

**Consequences:** The run file contains `candidate_tag: null` for a brief interval between
Metrics Recording and the Version and Tag update. Any consumer must tolerate null values on
these fields.

### Decision 2 — `signal_id` is null on the CLI path; only `start_run` (MCP) injects it

**Rationale:** The signal's proposed resolution explicitly scopes `signal_id` to `start_run`
invocations with a signal context. CLI `moeb run` invocations are user-initiated and do not
carry a signal context by design. The `signal_id` is already available in the spec frontmatter
for the run agent (established by `moeb.signal-status-lifecycle-tracking.md`); adding a
`--signal-id` CLI flag would expose a harness-internal concern to CLI users unnecessarily.
The MCP path covers the primary automation use case (fix_signal pipeline) where traceability
is most needed.

**Rejected alternatives:**
- Add `--signal-id` CLI flag to `moeb run`: introduces a user-visible flag for a
  harness-internal concern; CLI users would need to know signal IDs to use it.
- Read `signal_id` from spec frontmatter in the Metrics Recording skill step: adds
  spec-file parsing logic to Metrics Recording when the value is already in context for MCP
  runs; creates asymmetry between the two paths for a field that is optional by design.

**Consequences:** CLI-initiated `moeb run` invocations produce run files with
`signal_id: null`. Only MCP-triggered runs via `start_run` with `signal_id` provided link a
run file to its originating signal. This is sufficient for the automated fix_signal pipeline
which exclusively uses MCP.

### Decision 3 — Both fallback stubs updated; schema parity between run and spec command paths

**Rationale:** The fallback stub is the schema authority for the minimum valid run file
structure. If the agent skips the run file write, the stub must produce a document that any
consumer can read without branching on field presence. Adding the new fields as `null` in
both `domain/run.rs` and `domain/spec.rs` ensures that every run file — whether
agent-written or kernel-fallback, from a run or spec command — shares an identical schema.
A stub missing the new fields would create observable divergence between normal and fallback
runs.

**Rejected alternatives:**
- Update only `domain/run.rs` stub: spec-command run files would lack the fields, creating
  schema divergence. Although spec runs never populate `candidate_tag`, it should be
  present as `null` for consumers that enumerate all run files uniformly.

**Consequences:** Two stub locations require identical additive changes. They must be updated
in the same implementation run to avoid a temporary schema inconsistency between the two
command paths.

## Rubric

### Structured

| Name | Description | Threshold | Pass Condition |
|------|-------------|-----------|----------------|
| `tool-errors` | No ToolHandler errors were recorded during this command invocation. Covers all tool calls on both CLI and MCP paths via `RealToolExecutor::execute`. Applies to every moeb command. | Zero tool errors | Kernel-authoritative: injected by `verify_rubrics` from `RunState.tool_errors`. Do not supply a verdict — any agent-supplied entry is overridden. |
| `rubric-qualification` | No Pass verdicts with acknowledged failures were issued during this command invocation. An agent that observes a failure but passes a criterion must populate `acknowledged_failures` in the verdict object. Applies to every moeb command. | Zero qualified Pass verdicts | Kernel-authoritative: injected by `verify_rubrics` by scanning `acknowledged_failures` on incoming Pass verdicts. Do not supply a verdict — any agent-supplied entry is overridden. |
| `skill-mandatory-phases` | The skill loaded for this run contains all mandatory phase headings. The agent checks its own prompt context for the exact strings: "Verify", "End-of-Skill Review", "Metrics Recording", "Commit", "Tag", "Complete". No file read is required. | All six mandatory headings present | Agent scans skill content in its loaded context; fails if any mandatory heading string is absent |
| `no-drift` | The specification does not violate any decision recorded in a linked parent specification | Zero contradictions | Manual review of every decision in every parent spec listed in Backlinks |
| `spec-schema-compliance` | All required frontmatter fields and body sections are present and correctly ordered | 100% of required fields and sections | Validation in `domain/spec.rs` exits 0 during `moeb spec` |
| `ai-first-org` | The specification's Steps prescribe file layouts, naming, and helper placement that satisfy the four AI-first organisation principles: one concern per file, context locality, grep-discoverable names, no cross-cutting helpers. | All four principles followed | Manual review: no step introduces a cross-cutting helper; each file change addresses a single concern; the run file update logic is co-located in the Version and Tag phase with `create_candidate_tag` |
| `kernel-thin-and-parity` | No domain-specific or workflow-specific coordination logic may be placed in kernel Rust code when it can live in skill markdown or role files. Every tool registered in the CLI tool registry must also be registered in the MCP stdio server tool list. | Zero violations | Spec review: `start_run.rs` change is only token substitution; `domain/run.rs` change is token replacement with empty string and a null stub entry — both are primitive operations with no workflow logic; all coordination (conditional update of `candidate_tag`) lives in the skill |
| `moeb-tool-origin` | All tool calls made during this invocation must originate from moeb's own tool registry. If an external tool was used because no moeb equivalent exists, the agent must write a MissingMoebTool signal immediately after each such call. | Zero external tool calls | Agent reviews the conversation for calls to non-moeb tools. Supplies Pass if none were made. If external tools were used with MissingMoebTool signals, supplies Fail with `acknowledged_failures`. If used without signals, supplies Fail with the unlogged names. |
| `mcp-cli-behavioural-parity` | For any operation exposed through both the CLI agent loop and the MCP server, the observable outcome must be identical. Any tool added or modified must be registered identically in both registries. | Zero divergent outcomes | Run review: confirm `start_run.rs` definition schema change is visible via `ToolRegistry::mcp()`; confirm `run.prompt` template renders identically on CLI and MCP paths for the new `signal_id` token (CLI renders empty string; MCP renders provided value or empty) |
| `binary-builds` | `cargo build --release` completes without error after implementation | Zero errors | CI build exits 0 |
| `all-tests-pass` | `cargo test` completes without failure after implementation | Zero failures | `cargo test` exits 0 |
| `no-test-regression` | All tests present before this change pass without modification to test code | Zero failures | `cargo test` exits 0; no test file edited beyond compilation-required changes |
| `run-file-schema-extended` | The run file JSON written by the agent includes `signal_id` and `candidate_tag`; the fallback stub in both `domain/run.rs` and `domain/spec.rs` includes both fields as null | All four locations updated | Read `run.skill.md` Metrics Recording template: `signal_id` and `candidate_tag` present; read `check_run_outputs` in `domain/run.rs` and `domain/spec.rs`: `signal_id: null` and `candidate_tag: null` present in both stubs |
| `candidate-tag-update-in-version-phase` | The Version and Tag phase in `run.skill.md` contains an instruction to update `candidate_tag` in the run file after `create_candidate_tag` succeeds | Instruction present in skill file | Read `run.skill.md` Version and Tag phase: confirm read-parse-update-write instruction for `candidate_tag` is present after the `create_candidate_tag` call |

### Qualitative

- The `signal_id` field in the run file must be a JSON string when the session context
  variable is non-empty, or JSON `null` (not empty string) when the variable is empty or
  absent. The skill instruction must explicitly distinguish these two cases.
- The `candidate_tag` field must be JSON `null` in the initial Metrics Recording write and
  updated to a non-null string only after `create_candidate_tag` returns successfully. Any
  reader must tolerate `candidate_tag: null` in completed run files where QA failed.
- The two stub updates in `domain/run.rs` and `domain/spec.rs` must be identical for the
  new fields to maintain schema parity between the two command paths.
- `start_run.rs` must NOT add `signal_id` to the `"required"` array — it remains optional
  so callers without a signal context need not supply the field.
- The `{{signal_id}}` token must be replaced on both rendering paths: `start_run.rs`
  replaces it with the provided value (or empty string when absent); `domain/run.rs`
  replaces it with empty string unconditionally for the CLI path.
