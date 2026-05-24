---
domain: moeb
slug: run-id-injection-and-output-guarantee
status: active
---

# Run ID Injection and Output Guarantee

## Raw Requirement

Every command operation must produce a signals file and a metrics file. Currently both
files are only written when the agent voluntarily executes the End-of-Skill Review and
Metrics Recording phases. Because the agent self-generates its `run_id` (a UUID it
fabricates during execution), the resulting files are not traceable to the kernel's
trace artefact. Two gaps must be closed:

1. **run_id traceability.** The kernel generates a stable UUID at the start of each
   `moeb run` or `moeb spec` invocation and injects it into the agent prompt as
   `{{run_id}}`. The skill files are updated to use this token when naming signals and
   metrics files. The trace envelope is updated to include the run_id in its JSON body,
   enabling cross-reference from signals/metrics back to the trace.

2. **Output guarantee.** After the agent loop completes, the kernel checks whether
   `.moeb/signals/<run_id>.signals.json` and `.moeb/metrics/<run_id>.metrics.json`
   exist. If either is absent, the kernel writes a minimal fallback file, emits a
   stderr warning, and continues normally. This ensures downstream consumers always
   find at least one signals file and one metrics file per completed run.

## Description

`TraceContext` gains a `run_id` field (UUID v4, generated at construction) and a
`pub fn run_id(&self) -> &str` accessor. The run_id is included in the serialised
`TraceEnvelope` so that a tool reading a trace file can locate the corresponding
signals and metrics files by run_id.

`domain/run.rs` and `domain/spec.rs` each add a `RUN_ID_TOKEN = "{{run_id}}"` constant
and substitute it from `trace.run_id()` when rendering the prompt. Both also add a
post-loop output-guarantee check: if the signals or metrics file is absent after the
agent loop exits, the kernel writes a fallback stub and logs a warning.

`tools/start_run.rs` (MCP path) generates a fresh UUID v4 and substitutes it for
`{{run_id}}` in the rendered prompt. The MCP server has no agent-loop end event, so
no fallback check is performed on that path.

`src/prompts/run.prompt` and `src/prompts/spec.prompt` each receive a
`run_id: {{run_id}}` line in a new "Session context" section near the top, making the
value visible in the prompt context the agent reasons about.

`src/moeb/internal/skills/run.skill.md` and `spec.skill.md` are updated to replace all
`<run_id>` placeholders with `{{run_id}}`, so the agent uses the kernel-injected UUID
for naming signals and metrics files rather than fabricating its own.

```mermaid
flowchart TD
    K[Kernel starts run] -->|uuid::Uuid::new_v4| UID[run_id generated\nin TraceContext]
    UID -->|{{run_id}} substituted| Prompt[rendered prompt\ncontains actual UUID]
    Prompt --> Agent[Agent loop\nwrites signals + metrics\nusing injected UUID]
    Agent --> Loop[loop completes]
    Loop --> Check1{.moeb/signals/\nrun_id.signals.json\nexists?}
    Check1 -->|yes| Check2{.moeb/metrics/\nrun_id.metrics.json\nexists?}
    Check1 -->|no| Stub1[write fallback stub\nwarn stderr]
    Stub1 --> Check2
    Check2 -->|yes| Done[done]
    Check2 -->|no| Stub2[write fallback stub\nwarn stderr]
    Stub2 --> Done
    UID -->|included in| Trace[TraceEnvelope\nrun_id field]
```

## Backlinks

### Parents

| Label | Path | Purpose |
|-------|------|---------|
| Harness README | README.md | Root index |
| Composable Review Wrapper | specifications/moeb/moeb.composable-review-wrapper.md | Established the signals and metrics file writing phases whose outputs this spec guarantees |
| Trace Capture, Replay, and Kernel Configuration | specifications/moeb/moeb.trace-and-replay.md | TraceContext and TraceEnvelope that this spec extends with run_id |
| MCP Serve / CLI Parity | specifications/moeb/moeb.serve-cli-parity.md | start_run.rs MCP tool that this spec updates to inject run_id |
| Agent Skills | specifications/moeb/moeb.agent-skills.md | Skill file injection mechanism via {{…}} template tokens that this spec extends with {{run_id}} |

## Steps

### Step 1 — Add `run_id` to `TraceContext` and `TraceEnvelope`

In `src/moeb/src/trace.rs`:

Add a `run_id: String` field to `TraceContext`. Generate it in `TraceContext::new()`:

```rust
run_id: uuid::Uuid::new_v4().to_string(),
```

Add a public accessor:

```rust
pub fn run_id(&self) -> &str {
    &self.run_id
}
```

Add a `pub run_id: String` field to `TraceEnvelope` (serialised as `"run_id"` in the
JSON output). Populate it from `self.run_id` in the `finalize()` method when
constructing the envelope.

### Step 2 — Inject `{{run_id}}` in `domain/run.rs`

In `src/moeb/src/domain/run.rs`:

Add the constant:

```rust
const RUN_ID_TOKEN: &str = "{{run_id}}";
```

In `RunService::run()`, after `let trace = Arc::new(TraceContext::new(...))`, obtain
the run_id: `let run_id = trace.run_id().to_string();`

Add `.replace(RUN_ID_TOKEN, &run_id)` to the `prompt` construction chain alongside the
existing `.replace(...)` calls.

After the agent loop call (after `trace.finalize()`), add the output-guarantee check:

```rust
check_run_outputs(&run_id);
```

Implement `check_run_outputs` as a private function in the same file:

```rust
fn check_run_outputs(run_id: &str) {
    let signals_path = format!(".moeb/signals/{}.signals.json", run_id);
    let metrics_path = format!(".moeb/metrics/{}.metrics.json", run_id);

    let signals_missing = !std::path::Path::new(&signals_path).exists();
    let metrics_missing = !std::path::Path::new(&metrics_path).exists();

    // Collect a critical signal for every absent artifact so the
    // signals file is never an empty array on a failed run.
    let mut absent_signals: Vec<serde_json::Value> = Vec::new();
    if signals_missing {
        eprintln!("[moeb] critical: signals file not written by agent — {}", signals_path);
        absent_signals.push(serde_json::json!({
            "signal_id": uuid::Uuid::new_v4().to_string(),
            "run_id": run_id,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "category": "Error",
            "severity": "Critical",
            "title": "Signals file not written by agent",
            "description": format!("End-of-Skill Review phase did not complete. Expected: {}", signals_path),
            "proposed_resolution": null,
            "auto_spec_path": null,
            "gating_condition": null
        }));
    }
    if metrics_missing {
        eprintln!("[moeb] critical: metrics file not written by agent — {}", metrics_path);
        absent_signals.push(serde_json::json!({
            "signal_id": uuid::Uuid::new_v4().to_string(),
            "run_id": run_id,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "category": "Error",
            "severity": "Critical",
            "title": "Metrics file not written by agent",
            "description": format!("Metrics Recording phase did not complete. Expected: {}", metrics_path),
            "proposed_resolution": null,
            "auto_spec_path": null,
            "gating_condition": null
        }));
    }

    if signals_missing {
        let _ = std::fs::create_dir_all(".moeb/signals");
        let _ = std::fs::write(
            &signals_path,
            serde_json::to_string_pretty(&absent_signals).unwrap_or_else(|_| "[]".to_string()),
        );
    }

    if metrics_missing {
        let _ = std::fs::create_dir_all(".moeb/metrics");
        let stub = serde_json::json!({
            "run_id": run_id,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "rubric_score": 0.0,
            "step_metrics": [],
            "end_review_error_count": absent_signals.len() as u32,
            "wall_time_ms": 0,
            "kernel_fallback": true
        });
        let _ = std::fs::write(&metrics_path, serde_json::to_string_pretty(&stub)
            .unwrap_or_else(|_| "{}".to_string()));
    }
}
```

### Step 3 — Inject `{{run_id}}` in `domain/spec.rs`

Apply the same changes as Step 2 to `src/moeb/src/domain/spec.rs`:

- Add `const RUN_ID_TOKEN: &str = "{{run_id}}";`
- After `let trace = Arc::new(TraceContext::new(...))`, obtain `let run_id = trace.run_id().to_string();`
- Add `.replace(RUN_ID_TOKEN, &run_id)` to the prompt rendering chain
- After the agent loop (in `run_in()`), call `check_run_outputs(&run_id)`. Place the
  `check_run_outputs` function at module level in `domain/spec.rs` with the same
  implementation as in Step 2. Do not share the function across modules — one copy per
  file, co-located with its caller, following the context-locality principle.

### Step 4 — Inject `{{run_id}}` in `tools/start_run.rs`

In `src/moeb/src/tools/start_run.rs`, add:

```rust
const RUN_ID_TOKEN: &str = "{{run_id}}";
```

In `StartRunTool::execute()`, generate a fresh UUID before rendering the prompt:

```rust
let run_id = uuid::Uuid::new_v4().to_string();
```

Add `.replace(RUN_ID_TOKEN, &run_id)` to the prompt rendering chain alongside the
existing token substitutions. No fallback check is performed on the MCP path — the
agent loop is external to the kernel in that mode.

### Step 5 — Inject `{{run_id}}` in `tools/start_spec.rs`

In `src/moeb/src/tools/start_spec.rs`, apply the same change as Step 4:

```rust
const RUN_ID_TOKEN: &str = "{{run_id}}";
```

In `StartSpecTool::execute()`, generate a fresh UUID before rendering the prompt:

```rust
let run_id = uuid::Uuid::new_v4().to_string();
```

Add `.replace(RUN_ID_TOKEN, &run_id)` to the prompt rendering chain. No fallback check
is performed on the MCP spec path — the agent loop is external to the kernel in that mode.

### Step 6 — Add `run_id` context line to `src/prompts/run.prompt`

After the opening `{{role_content}}` line and before the `=== .moeb/README.md ===`
section, add:

```
Session context:
- run_id: {{run_id}}
```

This makes the injected UUID visible to the agent as an explicit context variable
rather than a hidden substitution.

### Step 7 — Add `run_id` context line to `src/prompts/spec.prompt`

After the opening `{{role_content}}` line and before the pre-loaded files block, add:

```
Session context:
- run_id: {{run_id}}
```

### Step 8 — Update `{{run_id}}` references in `src/moeb/internal/skills/run.skill.md`

In the End-of-Skill Review phase and Metrics Recording phase, replace all occurrences of
`<run_id>` (the current placeholder notation) with `{{run_id}}` (the injected UUID
token). After template substitution the agent sees the actual UUID; it does not need to
fabricate one.

Specifically update:
- End-of-Skill Review step 3: `".moeb/signals/<run_id>.signals.json"` →
  `".moeb/signals/{{run_id}}.signals.json"`
- Metrics Recording step 2: `".moeb/metrics/<run_id>.metrics.json"` →
  `".moeb/metrics/{{run_id}}.metrics.json"`
- Metrics Recording step 5 (DegradationSignal): `"run_id": "<run_id>"` →
  `"run_id": "{{run_id}}"`

### Step 9 — Update `{{run_id}}` references in `src/moeb/internal/skills/spec.skill.md`

Apply the same replacements as Step 8 to `spec.skill.md`:
- End-of-Skill Review step 3: `".moeb/signals/<run_id>.signals.json"` →
  `".moeb/signals/{{run_id}}.signals.json"`
- Metrics Recording step 2: `".moeb/metrics/<run_id>.metrics.json"` →
  `".moeb/metrics/{{run_id}}.metrics.json"`
- Metrics Recording step 5: `"run_id": "<run_id>"` → `"run_id": "{{run_id}}"`

## Decisions

### Decision 1 — run_id generated in TraceContext, not separately

**Rationale:** The kernel already creates `TraceContext` at the very start of a `run`
or `spec` invocation — before any agent interaction. Attaching the run_id to
`TraceContext` ensures it is generated exactly once per command, is available for both
prompt injection and trace envelope serialisation, and requires no new lifecycle
management. A run_id generated outside `TraceContext` would require threading it
through additional call sites.

**Rejected alternatives:**
- Generate run_id in the prompt-rendering function and not in TraceContext: the run_id
  would not appear in the trace envelope, breaking the trace↔signals correlation.
- Use the trace filename (spec-slug + timestamp) as the run_id: the filename contains
  no UUID and cannot serve as a cross-reference key without parsing.

**Consequences:** `TraceContext` grows one field. `TraceEnvelope` JSON gains one field.
Existing trace files that do not have `run_id` will parse successfully via
`#[serde(default)]` if needed by replay or tooling.

### Decision 2 — MCP path generates fresh UUID in `start_run`, no fallback check

**Rationale:** In MCP mode there is no single "agent loop completes" moment visible to
the kernel — the MCP server is a long-lived process that handles many sessions. The
`start_run` tool generates a fresh UUID per call so the agent has a unique run_id for
the session. Adding a fallback check would require the MCP server to track session
boundaries, which is out of scope.

**Rejected alternatives:**
- Share a single run_id between the MCP server's internal TraceContext and the
  start_run tool call: start_run is a tool called by the agent; it does not have access
  to the session-level TraceContext without invasive plumbing.
- No run_id injection on MCP path: agents in MCP mode would continue self-generating
  UUIDs, preserving the current status quo for that path.

**Consequences:** MCP-mode runs do not benefit from the fallback guarantee. The run_id
from start_run is not correlated with any trace file — MCP sessions do not produce
trace files via the standard trace mechanism. This gap may be addressed in a future spec.

### Decision 3 — Absent artifacts surfaced as critical signals, not silent warnings

**Rationale:** A missing signals or metrics file means the End-of-Skill Review or
Metrics Recording phase was skipped entirely — a critical quality failure equivalent to
any QA-phase critical signal. Emitting only a stderr warning and writing an empty signals
file (`[]`) conceals the failure from downstream consumers reading the signals file: an
empty array is indistinguishable from a run that completed review with zero findings. The
kernel instead writes a proper `ReviewSignal` entry with `severity: "Critical"` for each
absent artifact so that any tool reading the signals file sees the failure, not silence.
The run is not terminated — implementation work is not discarded — but the failure is
surfaced through the same channel as all other critical review findings.

**Rejected alternatives:**
- Fail the run if signals or metrics are absent: discards implementation work
  unnecessarily; the code change may be correct even if review was skipped.
- Write an empty signals stub (`[]`): conceals the failure; identical to a run that
  completed review with zero findings.
- Write stubs before the agent loop: the agent might write the real file later,
  resulting in two files or a confusing overwrite scenario.

**Consequences:** Post-run stubs with `"kernel_fallback": true` are distinguishable from
agent-written files. The signals stub contains at least one `ReviewSignal` with
`severity: "Critical"` per absent artifact. The `end_review_error_count` in the metrics
stub is set to the number of absent artifacts the kernel detected at loop exit. When only
the metrics file is absent (the agent wrote signals), the existing signals file is not
modified; the metrics stub's `end_review_error_count` reflects only the kernel-detected
absence count, which may differ from the agent-written signals file's critical count.
Any tooling consuming metrics files should treat `kernel_fallback: true` as a marker
that `rubric_score` and `step_metrics` are unreliable.

### Decision 4 — `check_run_outputs` duplicated per caller file, not shared

**Rationale:** The context-locality AI-first organisation principle requires helpers
to be co-located with their callers. `domain/run.rs` and `domain/spec.rs` are in the
same directory but serve different concerns; a shared helper in `domain/mod.rs` or a
new `domain/output_check.rs` would introduce a cross-cutting helper. The function body
is small (~35 lines); duplication is preferable to premature abstraction.

**Rejected alternatives:**
- Single shared function in `domain/mod.rs`: violates context-locality; the function
  would appear as a dependency of every future domain module.
- A new `domain/output_check.rs`: one concern per file is satisfied, but the function
  is too small to warrant its own file — this is the premature abstraction case.

**Consequences:** Two copies of `check_run_outputs`. If the stub format changes,
both files must be updated in the same spec.

## Rubric

### Structured

| Name | Description | Threshold | Pass Condition |
|------|-------------|-----------|----------------|
| `context-budget` | All implementation files created or modified during this run are ≤ 300 lines; `*_tests.rs` companion files are ≤ 400 lines | Zero over-budget files after any required refactoring | Agent checks line counts of every file written; refactors any over-budget file |
| `binary-builds` | `cargo build --release` completes without error | Zero errors | CI build exits 0 |
| `all-tests-pass` | `cargo test` completes without failure | Zero failures | `cargo test` exits 0 |
| `no-test-regression` | All tests present before this change pass without modification to test code | Zero failures | `cargo test` exits 0; no test file edited beyond compilation-required changes |
| `thin-kernel` | New Rust code in tools/, commands/, or domain/ must be either a primitive operation wrapper or a thin dispatcher; no workflow logic in Rust | Zero workflow-logic functions in new Rust | Spec review: check_run_outputs performs only filesystem existence checks and writes; no branching logic beyond present/absent |
| `run-id-in-trace` | The run_id field appears in TraceEnvelope JSON output | Present in serialised output | Read a trace file produced after this change; confirm "run_id" key is present at the envelope level |
| `prompt-substitution-coverage` | {{run_id}} is substituted in all four prompt-rendering paths: domain/run.rs, domain/spec.rs, tools/start_run.rs, and tools/start_spec.rs | All four paths updated | grep for RUN_ID_TOKEN in run.rs, spec.rs, start_run.rs, and start_spec.rs returns a match in each |
| `fallback-guarantee` | After the agent loop, domain/run.rs and domain/spec.rs write stub files when signals or metrics are absent; the signals stub contains a `ReviewSignal` with `severity: "Critical"` for each absent artifact — never an empty array | Stubs written on both paths; signals stub non-empty | Code review: check_run_outputs called after agent loop in both domain/run.rs and domain/spec.rs; signals stub contains at least one Critical entry |
| `skill-run-id-consistency` | Both run.skill.md and spec.skill.md reference {{run_id}} (not <run_id>) for all signals and metrics file paths | No bare <run_id> placeholders remain | grep for '<run_id>' in run.skill.md and spec.skill.md returns no matches |

### Qualitative

- The `check_run_outputs` stderr messages are prefixed `[moeb] critical:` and include
  the missing file path, so operators can diagnose which phase was skipped without
  reading trace files.
- The fallback signals file is never an empty array; it always contains at least one
  `ReviewSignal` with `severity: "Critical"`, ensuring downstream consumers see a
  critical failure rather than a run that completed review with zero findings.
- `"kernel_fallback": true` is a top-level field in the metrics stub JSON, not nested
  under `step_metrics`, so it is discoverable by a simple key lookup without schema
  knowledge.
- The `run_id` in `TraceEnvelope` enables a single grep across `.moeb/traces/` to find
  the trace corresponding to a given signals or metrics file.
