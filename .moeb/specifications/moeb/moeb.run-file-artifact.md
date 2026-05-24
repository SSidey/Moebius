---
domain: moeb
slug: run-file-artifact
status: active
---

# Run File Artifact

## Raw Requirement

Every completed command operation (`moeb run` or `moeb spec`) must produce a run file
that ties together all artifacts produced during that operation: the spec file, signals
file, metrics file, and identifying metadata. The run file serves as the primary index
for a command execution — a reader can locate all related artifacts from it alone.

The file must be named for uniqueness and ordering: `<timestamp>_<command>_<slug>.json`
where timestamp is the run start in compact ISO 8601 UTC format (for lexicographic
ordering), command is `run` or `spec`, and slug uniquely identifies the subject. Files
live in `.moeb/runs/`.

The run file is written by the agent atomically at the end of the Metrics Recording
phase in both `run.skill.md` and `spec.skill.md`. The kernel injects the expected run
file path as `{{run_file_path}}` so the agent uses the kernel-computed name rather than
deriving it independently. After the agent loop, the kernel extends the existing
`check_run_outputs` function (from `moeb.run-id-injection-and-output-guarantee.md`) to
also check for the run file and write a minimal fallback stub if absent.

## Description

The run file is a small JSON document written to `.moeb/runs/`. Its content links the
run_id, timestamp, command, spec path, signals path, metrics path, rubric score, and
critical error count. It is not a summary of the run's content — it is a manifest of
the run's artifacts.

**Naming.** The kernel computes the expected path at prompt-rendering time and injects
it as `{{run_file_path}}`:

- For `moeb run`: `<timestamp>_run_<spec_file_stem>.json` where `spec_file_stem` is the
  stem of the spec file (e.g. `moeb.kernel-thin-and-mcp-parity-rubrics`).
- For `moeb spec`: `<timestamp>_spec_<sanitized_input>.json` where `sanitized_input` is
  the slug derived from the requirement string (same logic as the trace spec field).
- For `start_run` MCP path: the same pattern as `moeb run`, derived from the resolved
  spec file stem.

Timestamp format: the `started_at` value already used by the trace, rendered as
`%Y%m%dT%H%M%SZ` (uppercase Z for UTC). Example filename:
`20260523T142301Z_run_moeb.kernel-thin-and-mcp-parity-rubrics.json`.

**Content schema:**

```json
{
  "run_id": "...",
  "timestamp": "2026-05-23T14:23:01Z",
  "command": "run",
  "spec_path": ".moeb/specifications/moeb/moeb.kernel.md",
  "signals_path": ".moeb/signals/<run_id>.signals.json",
  "metrics_path": ".moeb/metrics/<run_id>.metrics.json",
  "rubric_score": 0.95,
  "end_review_error_count": 0
}
```

`spec_path` is the path of the spec being executed (run command) or authored (spec
command). For the spec command, the agent fills this in from the frontmatter it authored;
the kernel fallback stub omits it.

**`moeb init`.** Creates `.moeb/runs/` as an empty directory, following the pattern
established for `.moeb/signals/` and `.moeb/metrics/`.

**Fallback guarantee.** The kernel's `check_run_outputs` function (defined in
`moeb.run-id-injection-and-output-guarantee.md`) is extended to also check for the run
file at `{{run_file_path}}`. If absent, the kernel writes a minimal stub and logs a
warning. The stub omits `spec_path` and sets `rubric_score` to `0.0`.

```mermaid
flowchart TD
    K[Kernel renders prompt] -->|{{run_file_path}} injected| Agent[Agent loop]
    Agent --> MR[Metrics Recording phase\n write signals\n write metrics\n write run file\n at run_file_path]
    MR --> Loop[loop completes]
    Loop --> C1{run file\nexists?}
    C1 -->|yes| Done[done]
    C1 -->|no| Stub[write fallback stub\nwarn stderr]
    Stub --> Done
    MR --> RF[.moeb/runs/\n20260523T142301Z_run_\nmoeb.kernel.json]
    RF --> Links[links to\nsignals + metrics]
```

## Backlinks

### Parents

| Label | Path | Purpose |
|-------|------|---------|
| Harness README | README.md | Root index |
| Run ID Injection and Output Guarantee | specifications/moeb/moeb.run-id-injection-and-output-guarantee.md | Established check_run_outputs and {{run_id}} injection; this spec extends both |
| Composable Review Wrapper | specifications/moeb/moeb.composable-review-wrapper.md | Established the Metrics Recording phase whose output this spec extends with a run file write |
| Moeb Init Rubric Storage Boundary | specifications/harness/harness.init-rubric-storage-boundary.md | Pattern for directory creation during moeb init that this spec follows for .moeb/runs/ |
| Agent Skills | specifications/moeb/moeb.agent-skills.md | Skill file injection mechanism via {{…}} tokens that this spec extends with {{run_file_path}} |

## Steps

### Step 1 — Add `.moeb/runs/` to `moeb init`

In `src/moeb/src/commands/init.rs`, after the creation of `.moeb/metrics/`, add:

```rust
let runs_dst = moeb.join("runs");
fs::create_dir_all(&runs_dst).context("Failed to create .moeb/runs/")?;
```

### Step 2 — Compute and inject `{{run_file_path}}` in `domain/run.rs`

In `src/moeb/src/domain/run.rs`, add the constant:

```rust
const RUN_FILE_PATH_TOKEN: &str = "{{run_file_path}}";
```

After `let trace = Arc::new(TraceContext::new(...))`, compute the run file path using the
spec file stem already computed as `spec_slug` and a fresh timestamp:

```rust
let run_ts = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
let run_file_path = format!(".moeb/runs/{}_run_{}.json", run_ts, spec_slug);
```

Add `.replace(RUN_FILE_PATH_TOKEN, &run_file_path)` to the prompt rendering chain.

Pass `run_file_path` to `check_run_outputs` (extended in Step 6 of this spec).

### Step 3 — Compute and inject `{{run_file_path}}` in `domain/spec.rs`

In `src/moeb/src/domain/spec.rs`, add:

```rust
const RUN_FILE_PATH_TOKEN: &str = "{{run_file_path}}";
```

The spec command's subject identifier is derived from the input requirement. Re-use the
same `sanitize_slug(input)` already called when constructing `trace_config.spec`. The
run file path:

```rust
let run_ts = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
let input_slug = spec_parser::sanitize_slug(input);
let run_file_path = format!(".moeb/runs/{}_spec_{}.json", run_ts, input_slug);
```

Add `.replace(RUN_FILE_PATH_TOKEN, &run_file_path)` to the prompt rendering chain in
`run_in()`. Pass `run_file_path` to `check_run_outputs`.

### Step 4 — Compute and inject `{{run_file_path}}` in `tools/start_run.rs`

In `src/moeb/src/tools/start_run.rs`, add:

```rust
const RUN_FILE_PATH_TOKEN: &str = "{{run_file_path}}";
```

After resolving the spec path and extracting the file stem, compute:

```rust
let run_ts = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
let spec_stem = abs_spec_path
    .file_stem()
    .map(|s| s.to_string_lossy().to_string())
    .unwrap_or_else(|| spec_path_arg.to_string());
let run_file_path = format!(".moeb/runs/{}_run_{}.json", run_ts, spec_stem);
```

Add `.replace(RUN_FILE_PATH_TOKEN, &run_file_path)` to the prompt rendering chain.

### Step 5 — Compute and inject `{{run_file_path}}` in `tools/start_spec.rs`

In `src/moeb/src/tools/start_spec.rs`, add:

```rust
const RUN_FILE_PATH_TOKEN: &str = "{{run_file_path}}";
```

The MCP spec tool receives a `requirement` string. The `sanitize_slug` function in
`spec_parser.rs` is `pub(super)` and is not accessible from `tools/`. Apply the same
logic inline:

```rust
let input_slug: String = requirement
    .chars()
    .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
    .collect::<String>()
    .trim_matches('-')
    .to_string()
    .chars()
    .take(40)
    .collect();
let run_ts = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
let run_file_path = format!(".moeb/runs/{}_spec_{}.json", run_ts, input_slug);
```

Add `.replace(RUN_FILE_PATH_TOKEN, &run_file_path)` to the prompt rendering chain.
No kernel fallback stub is written for the MCP spec path.

### Step 6 — Extend `check_run_outputs` in `domain/run.rs` and `domain/spec.rs`

Update the signature of `check_run_outputs` in both `domain/run.rs` and `domain/spec.rs`
to accept `run_file_path: &str` as a third parameter. Inside the function, after the
signals and metrics checks, add:

```rust
if !std::path::Path::new(run_file_path).exists() {
    eprintln!("[moeb] warning: run file not written by agent — writing fallback stub");
    let _ = std::fs::create_dir_all(".moeb/runs");
    let stub = serde_json::json!({
        "run_id": run_id,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "command": command,
        "signals_path": format!(".moeb/signals/{}.signals.json", run_id),
        "metrics_path": format!(".moeb/metrics/{}.metrics.json", run_id),
        "rubric_score": 0.0,
        "end_review_error_count": 0,
        "kernel_fallback": true
    });
    let _ = std::fs::write(run_file_path, serde_json::to_string_pretty(&stub)
        .unwrap_or_else(|_| "{}".to_string()));
}
```

The `command` parameter should be `"run"` in `domain/run.rs` and `"spec"` in
`domain/spec.rs`. Pass it as a `&str` parameter alongside `run_id` and `run_file_path`.

Update all call sites of `check_run_outputs` to pass the extended parameters.

### Step 7 — Add run file to `src/prompts/run.prompt` context section

In the `Session context:` section added by `moeb.run-id-injection-and-output-guarantee.md`,
add a second line:

```
Session context:
- run_id: {{run_id}}
- run_file_path: {{run_file_path}}
```

### Step 8 — Add run file to `src/prompts/spec.prompt` context section

Apply the same addition as Step 7 to `src/prompts/spec.prompt`.

### Step 9 — Add run file write to `src/moeb/internal/skills/run.skill.md`

In the Phase — Metrics Recording section, after step 2 (write metrics to
`.moeb/metrics/{{run_id}}.metrics.json`), add:

```
3. Write the run file to `{{run_file_path}}` with the following JSON content:

   ```json
   {
     "run_id": "{{run_id}}",
     "timestamp": "<ISO 8601 start time>",
     "command": "run",
     "spec_path": "<path of the spec file being executed>",
     "signals_path": ".moeb/signals/{{run_id}}.signals.json",
     "metrics_path": ".moeb/metrics/{{run_id}}.metrics.json",
     "rubric_score": <from verify_rubrics output>,
     "end_review_error_count": <count of Critical signals>
   }
   ```

   Use `write_file` with path `{{run_file_path}}`. The `spec_path` is the spec file
   path as provided in the prompt context. The timestamp is the session start time (the
   time at which this run began, not the time this file is written).
```

Renumber the existing steps 3–6 (rolling average, degradation, trace emit) to 4–7.

### Step 10 — Add run file write to `src/moeb/internal/skills/spec.skill.md`

In the Phase — Metrics Recording section of `spec.skill.md`, apply the same addition as
Step 9, with two adaptations:

- `"command"` is `"spec"`.
- `"spec_path"` is the path of the spec file authored in Phase 4 of the skill (the
  `.moeb/specifications/<domain>/<domain>.<slug>.md` path derived from frontmatter).

Renumber following steps accordingly.

## Decisions

### Decision 1 — Agent writes the run file; kernel provides fallback

**Rationale:** The run file contains `spec_path` — for the spec command this is the
path of the spec the agent just authored. The kernel does not know this path at
prompt-rendering time (the agent chooses domain and slug during Phase 2). The agent is
the only party that knows the full content at write time. The kernel provides a fallback
for the common fields it does know (run_id, signals_path, metrics_path) when the agent
skips the write.

**Rejected alternatives:**
- Kernel writes the run file after the loop: cannot include `spec_path` for the spec
  command; the kernel's `check_run_outputs` fallback already covers the case where the
  agent skips it.
- Kernel writes a stub before the loop and the agent overwrites it: ordering complexity;
  the agent's overwrite must occur after the metrics file is written, which is already
  the last step of the Metrics Recording phase.

**Consequences:** For spec command runs where the agent fails before authoring the spec
file, the fallback stub omits `spec_path`. This is acceptable — a run that produced no
spec file has no spec to link.

### Decision 2 — `.moeb/runs/` as a dedicated directory

**Rationale:** Run files are distinct from signals (per-signal observability records)
and metrics (per-run performance data). A consumer that wants to enumerate all runs
reads `.moeb/runs/`; a consumer that wants all signals reads `.moeb/signals/`. Mixed
directories require filename-extension filtering and make consumer glob patterns fragile.

**Rejected alternatives:**
- `.moeb/artifacts/` as a unified directory: signals and metrics are already established
  in separate directories; merging now would require migrating them.
- Alongside signals/metrics: filename-extension filtering required; undermines the
  naming clarity achieved by separate directories.

**Consequences:** `moeb init` creates a third output directory. A future retention-policy
spec should clean up `.moeb/runs/` alongside `.moeb/signals/` and `.moeb/metrics/`.

### Decision 3 — Timestamp computed at prompt-rendering time, not inside TraceContext

**Rationale:** `TraceContext::new()` is called after the prompt is rendered in the
current code structure (`trace` is created after `template` substitution in
`domain/run.rs`). Adding a method to `TraceContext` for the run file path would require
restructuring the call order. Capturing `chrono::Utc::now()` before creating
`TraceContext` gives a timestamp that is within milliseconds of `TraceContext::started_at`
and is sufficient for the run file's ordering guarantee.

**Rejected alternatives:**
- Add `run_file_path()` accessor to `TraceContext`: requires reordering prompt rendering
  after `TraceContext::new()` or restructuring the token substitution flow — a larger
  diff than justified for this feature.
- Use `{{run_id}}` as the run file name: loses the timestamp ordering the user requires;
  UUIDs are not lexicographically ordered by creation time.

**Consequences:** The run file timestamp and the trace file timestamp may differ by a
few milliseconds (the time between `chrono::Utc::now()` before `TraceContext::new()` and
`TraceContext::started_at` inside). This is imperceptible for ordering purposes.

## Rubric

### Structured

| Name | Description | Threshold | Pass Condition |
|------|-------------|-----------|----------------|
| `context-budget` | All implementation files created or modified during this run are ≤ 300 lines; `*_tests.rs` companion files are ≤ 400 lines | Zero over-budget files after any required refactoring | Agent checks line counts of every file written; refactors any over-budget file |
| `binary-builds` | `cargo build --release` completes without error | Zero errors | CI build exits 0 |
| `all-tests-pass` | `cargo test` completes without failure | Zero failures | `cargo test` exits 0 |
| `no-test-regression` | All tests present before this change pass without modification to test code | Zero failures | `cargo test` exits 0; no test file edited beyond compilation-required changes |
| `thin-kernel` | New Rust in domain/ and commands/ must be primitive wrappers or thin dispatchers; run file path computation is format string operations only, no workflow logic | Zero workflow-logic functions | Spec review: run file path computation is a single format! call; check_run_outputs additions are existence checks and file writes |
| `runs-dir-created-on-init` | `moeb init` creates `.moeb/runs/` alongside `.moeb/signals/` and `.moeb/metrics/` | Directory present after init | Run `moeb init` in a fresh directory; confirm `.moeb/runs/` exists |
| `run-file-path-injection-coverage` | `{{run_file_path}}` is substituted in domain/run.rs, domain/spec.rs, tools/start_run.rs, and tools/start_spec.rs | All four paths updated | grep for RUN_FILE_PATH_TOKEN in all four files returns a match in each |
| `skill-run-file-write` | Both run.skill.md and spec.skill.md include a write_file call for the run file in the Metrics Recording phase | Both files updated | Read both files: write to {{run_file_path}} present in Metrics Recording in each |
| `fallback-coverage` | check_run_outputs in both domain/run.rs and domain/spec.rs checks for the run file and writes a stub when absent | Both callers updated | Code review: run file check present in check_run_outputs in both files |
| `run-file-content-schema` | The run file JSON written by the agent contains run_id, timestamp, command, spec_path, signals_path, metrics_path, rubric_score, end_review_error_count | All fields present in skill instruction | Read Step 9 and Step 10 in this spec; confirm all eight fields are listed in the JSON template in the skill update |

### Qualitative

- The run file path in `.moeb/runs/` is human-readable at a glance: the timestamp
  gives ordering and the command+slug identify the subject without opening the file.
- The fallback stub is distinguishable from an agent-written run file by the presence
  of `"kernel_fallback": true` and the absence of `"spec_path"`.
- A reader with only a run file can locate all three other artifacts: signals, metrics,
  and (via run_id cross-reference in the trace envelope) the trace file.
