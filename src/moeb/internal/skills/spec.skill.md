---
review: true
---
The harness README, specification schema, and rubrics catalogue are already in your
context. Do not re-read any of them.

## Tool Origin Policy

Prefer moeb tools for every operation during this run. The complete list of available moeb tools is visible in your tool schema (read_file, read_files, read_file_range, write_file, patch_file, grep_files, list_directory, search_files, create_task_list, update_task, verify_rubrics, complete_review, query_agent, git_commit, tag_run, create_branch, bump_version, get_version). If you need an operation and no moeb tool exists for it:

1. Use the external tool (e.g. Bash, Edit, Read) to complete the operation.
2. Immediately after the external call, buffer a MissingMoebTool signal entry to be incorporated into the signals file during the End-of-Skill Review phase:

```json
{
  "category": "NewCapability",
  "severity": "Major",
  "title": "Missing moeb tool: <external_tool_name>",
  "description": "Used <external_tool_name> to perform <description of operation>. No moeb equivalent exists.",
  "proposed_resolution": "Run moeb spec to introduce a moeb equivalent of <external_tool_name> covering <operation>.",
  "gating_condition": null
}
```

## Phase 1 — Contradiction check

Before authoring, verify that the proposed specification does not contradict any active
decision recorded in an existing specification visible in the README index. If a
contradiction exists, stop and surface it explicitly rather than proceeding.

## Phase 2 — Author

Write the complete specification document conforming to the schema:

- Begin with YAML frontmatter between `---` markers. Required fields: `domain`, `slug`,
  `status`. Include `supersedes` only when overriding a named decision.
  - `status` must be `active` for all newly authored specifications. Use `draft` only
    when explicitly instructed by the user. Never set `status: superseded` when authoring
    a new specification.
  - When `supersedes` is included, use this syntax:
    ```
    supersedes:
      - path: specifications/<domain>/<domain>.<slug>.md
        decision: "Decision N — Exact title of the overridden decision"
    ```
    The overridden decision must also be addressed in the Decisions section with rationale.
    Both the frontmatter entry and the prose rationale are required.

- Include all required sections in this exact order:
  1. `# <Title>` (level-1 heading)
  2. `## Raw Requirement`
  3. `## Description`
  4. A fenced Mermaid diagram block (opened with ` ```mermaid `, closed with ` ``` `)
  5. `## Backlinks`
  6. `## Steps`
  7. `## Decisions`
  8. `## Rubric`

  Each step must be detailed enough for an agent or developer to execute without
  ambiguity. Each decision must record rationale, rejected alternatives, and consequences.

- Backlinks must include at minimum one Parents entry pointing to `README.md`.

- For the `## Rubric / ### Structured` table, apply criteria in two passes:

  **Pass 1 — baseline rows (mandatory):**
  Copy every row from the injected `{{command_rubrics}}` section verbatim into the
  `## Rubric / ### Structured` table. These rows are mandatory in every specification.

  **Pass 2 — catalogue-selected rows (trait-driven):**
  Read the `catalogue.rubrics.md` section pre-loaded above. Filter by Domain to match
  the domain of the specification being authored, then identify traits present in the
  raw requirement and description. For each matching catalogue entry:
  - If `Applies At` contains `run`, `All`, or any command other than `spec`: copy the
    criterion row verbatim into the `## Rubric / ### Structured` table.
  - If `Applies At` contains `spec` or `All`: verify this criterion against the
    specification you are authoring before marking it complete. Do not copy it into
    the rubric table.

- The `### Qualitative` section is always spec-specific and is never drawn from the
  rubrics catalogue.

## Phase 3 — Validate before writing

Before writing the file, verify:
- All required frontmatter fields are present.
- All required sections exist and are non-empty and appear in the correct order.
- The Mermaid diagram block is syntactically plausible.
- The Rubric section contains at least one structured criterion.
- The `## Rubric / ### Structured` table contains at minimum every row copied from the
  `{{command_rubrics}}` baseline (Pass 1 above).
- The opening `---` will be the very first characters of the file content (no preamble,
  no BOM, no whitespace before it). The closing `---` on its own line is equally
  required — omitting it causes a parse failure.

## Phase 4 — Write

Write the authored specification to disk:

- Derive `domain` and `slug` from the YAML frontmatter you authored.
- Call `write_file` with path `.moeb/specifications/<domain>/<domain>.<slug>.md` and the
  complete specification document as content.
  The content must begin with `---` (the YAML opening delimiter) as its very first characters.

### Per-Step Review Sub-Loop (spec file)

Skip this sub-loop entirely if `{{no_review}}` is `"true"`.

After writing the spec file in Phase 4:

0. **Error preflight.** If the `write_file` call immediately preceding this sub-loop
   returned an error result (result string contains "failed", "error", or "could not"):
   - Record StepMetric with `step_id: "spec-file"`, `iteration_count = 0`,
     `acceptance_rate = 1.0`, `delta_scores = []`.
   - Do NOT call `complete_review` for this path.
   - Proceed to Phase 5. The error is recorded in RunState.tool_errors.
   Skip steps 1–8 below for this invocation.

1. **Inline Reviewer.** Without calling any tool, adopt the **Reviewer Persona**
   pre-loaded in your context. Evaluate the spec file against its `## Rubric / ###
   Structured` criteria and the specification title and description as step intent.
   Produce a unified diff if improvements are needed, or an empty string if the
   artifact fully satisfies all criteria. Hold this result in working memory.

2. If the returned diff is empty or whitespace: terminate sub-loop. Record StepMetric
   with `step_id: "spec-file"`, `iteration_count = 0`, `acceptance_rate = 1.0`,
   `delta_scores = []`.

3. **Inline Moderator.** Without calling any tool, adopt the **Moderator Persona**
   pre-loaded in your context. Evaluate the proposed diff against the spec content,
   rubric criteria, and iteration history. Produce a JSON verdict:
   `{ "accepted": bool, "delta_score": float, "rationale": string }`. Hold this result
   in working memory.

4. Parse the Moderator JSON. If `[PARSE_WARNING]` prefix is present, treat as
   `{ "accepted": false, "delta_score": 0.0, "rationale": "Moderator parse failure" }`.

5. If `accepted = true` and `delta_score >= 0.01` and `iteration_count < 2`:
   - Apply the reviewer's improvements: using the spec content already in context,
     incorporate the proposed changes in working memory to produce the complete updated
     content, then write it using `write_file` with the spec file path.
   - If `write_file` returns an error result, terminate the sub-loop immediately —
     do not return to step 1. Append `{ "delta_score": 0.0, "accepted": false }` to
     iteration history and proceed to step 6. The error is recorded in RunState.tool_errors.
   - Otherwise append `{ "delta_score": <score>, "accepted": true }` to iteration
     history and return to step 1.

6. Otherwise: terminate sub-loop.

7. Record StepMetric for `step_id: "spec-file"`.

8. Call `complete_review` with the spec file path (`.moeb/specifications/<domain>/<domain>.<slug>.md`).
   This is a no-op at the kernel level (path starts with `.moeb/`) but makes the review
   obligation explicit and maintains symmetry with the run skill.

## Phase 5 — Link README

Read `.moeb/README.md` using `read_file`. Locate the `### <domain>` section in the
Specification index. If the section does not exist, create it in alphabetical order
among the existing `###` domain subsections with a standard table header. Find the last
data row in the domain table and insert the new row immediately after it. Write the
complete updated file content to `.moeb/README.md` using `write_file`.

New row format (four columns, Status value `active`):
| <Title> | <one-sentence description> | [specifications/<domain>/<domain>.<slug>.md](specifications/<domain>/<domain>.<slug>.md) | active |

### Per-Step Review Sub-Loop (README patch)

Skip this sub-loop entirely if `{{no_review}}` is `"true"`.

After writing `.moeb/README.md`:

0. **Error preflight.** If the `write_file` call on `.moeb/README.md` immediately
   preceding this sub-loop returned an error result (result string contains "failed",
   "error", or "could not"):
   - Record StepMetric with `step_id: "readme-link"`, `iteration_count = 0`,
     `acceptance_rate = 1.0`, `delta_scores = []`.
   - Call `complete_review` with `.moeb/README.md` (no-op at kernel level; included for
     symmetry).
   - Proceed to Phase 6 — Verify. The error is recorded in RunState.tool_errors.
   Skip steps 1–5 below for this invocation.

1. **Inline Reviewer.** Without calling any tool, adopt the **Reviewer Persona**
   pre-loaded in your context. Evaluate the README patch for correctness: the row must
   be correctly formatted and present in the right `### <domain>` section. Produce a
   unified diff if the row is incorrectly formatted or missing, or an empty string if
   the row is correct. Hold this result in working memory.

2. If the returned diff is empty or whitespace: terminate sub-loop. Record StepMetric
   with `step_id: "readme-link"`.

3. **Inline Moderator.** Without calling any tool, adopt the **Moderator Persona**
   pre-loaded in your context. Evaluate the proposed diff against the README content,
   rubric criteria, and iteration history. Produce a JSON verdict:
   `{ "accepted": bool, "delta_score": float, "rationale": string }`. Hold this result
   in working memory.

4. Parse Moderator JSON. If `accepted = true` and `delta_score >= 0.01` and
   `iteration_count < 2`:
   - Re-apply the reviewer's correction: read `.moeb/README.md` using `read_file`,
     incorporate the proposed changes in working memory to produce the complete updated
     content, and write it using `write_file`.
   - If `write_file` returns an error result, terminate the sub-loop immediately.
     Append `{ "delta_score": 0.0, "accepted": false }` to iteration history.
     The error is recorded in RunState.tool_errors. Proceed to step 5.
   - Otherwise append `{ "delta_score": <score>, "accepted": true }` to iteration
     history and return to step 1.
   Record StepMetric for `step_id: "readme-link"` when the loop terminates.

5. Call `complete_review` with `.moeb/README.md`.
   This is a no-op at the kernel level (path starts with `.moeb/`) but makes the review
   obligation explicit and maintains symmetry with the run skill.

## Phase 6 — Verify

Collect all rubric criteria from two sources:

1. The injected `{{command_rubrics}}` section. Every row is mandatory and must receive
   a verdict.
2. The `## Rubric / ### Structured` table of the specification authored in Phase 2.
   Every row is mandatory.

For each criterion, evaluate pass, fail, or na:

- `spec-schema-compliance`: verify the authored spec has all required frontmatter fields
  (domain, slug, status) and all required body sections in the correct order.
- `no-drift`: verify the authored spec does not contradict any decision in its linked
  parent specifications.
- `ai-first-org`: verify the spec's Steps prescribe file layouts, naming, and helper
  placement consistent with the four AI-First principles.
- `tool-errors`: kernel-authoritative — do not supply a verdict.
- `rubric-qualification`: kernel-authoritative — do not supply a verdict.
- `kernel-thin-and-parity`: verify no proposed step places workflow logic in Rust;
  verify any new tool is registered in both CLI and MCP server lists.
- `moeb-tool-origin`: Review this conversation for tool calls to non-moeb tools (tools whose names are not in the moeb tool schema, such as Bash, Edit, Write, Read, Glob, Grep, WebFetch, or WebSearch). If no external tool calls appear in the conversation, supply Pass. If external tool calls appear and MissingMoebTool signals were buffered for each one, supply Fail with `acknowledged_failures` listing each signal title. If external tool calls appear without corresponding buffered signals, supply Fail and list the unlogged tool names in the note field.
- All other criteria from the spec's own `## Rubric / ### Structured` table: apply the
  stated Pass Condition. Mark `na` only when the criterion genuinely does not apply.

Call `verify_rubrics` with the complete list of verdicts. Do not call with a partial list.

## Phase 7 — End-of-Skill Review

Skip this phase entirely if `{{no_review}}` is `"true"`.

Without calling any tool, adopt the **QA Architect Persona** pre-loaded in your context.
Evaluate the spec file, README.md, their contents, and the accumulated StepMetrics as
JSON. Produce a ReviewSignalReport JSON matching the schema defined in the QA Architect
Persona. Hold the result in working memory and continue with parsing and signal processing
below.

When issuing a Pass verdict for a criterion where failures were observed but judged
acceptable, populate `acknowledged_failures` with a brief description of each failure:

  "acknowledged_failures": ["cargo test: 2 failures in pre-existing suite", "..."]

Do not leave this field empty when a failure was observed. The QA Architect scans
verdict objects in context for Pass entries with non-empty `acknowledged_failures` and
raises a Critical signal for each. This mechanism replaces qualification language in
`note` as the surface for declared-but-passing failures — the note field should remain
a criterion evaluation; failure acknowledgement goes in `acknowledged_failures`.

If `[PARSE_WARNING]` prefix is present in the response, treat it as a Critical error
signal: append a signal with title "QA Architect response parse failure" and description
containing the raw response, then continue.

Parse the returned `ReviewSignalReport` JSON:

1. Assign `signal_id`, `run_id`, `timestamp` to each signal.

## Canonical Signal Dedup-and-Write Procedure

# Identity key: "<category>-<title-slug>"  (slug = lowercase, [^a-z0-9]+ → '-', strip edges, max 80 chars)
# Canonical path: .moeb/signals/catalogue/<identity-key>.signal.json
# Severity order: Info < Minor < Major < Critical
# current_run_id: the run identifier injected via {{run_id}} in the spec prompt template
# Note: .moeb/signals/catalogue/ is created on first use.
# write_file creates parent directories automatically — no explicit mkdir required.
#
# Canonical signal record schema (all fields in this order):
# { "signal_id", "category", "severity", "title", "description", "proposed_resolution",
#   "gating_condition", "status", "occurrence_count", "first_seen", "last_seen", "occurrences" }

For each signal S in the ReviewSignalReport:
  1. Compute identity_key as described above.
  2. Attempt to read `.moeb/signals/catalogue/<identity_key>.signal.json` using read_file.
     If the file does not exist (read returns an error), treat as not_found.
  3. If not_found:
       Assign S.signal_id = new UUID v4.
       Set S.status         = "open".
       Set S.occurrence_count = 1.
       Set S.first_seen     = current ISO 8601 timestamp.
       Set S.last_seen      = current ISO 8601 timestamp.
       Set S.occurrences    = [{ "timestamp": S.first_seen, "run_id": current_run_id }].
       Write canonical record to `.moeb/signals/catalogue/<identity_key>.signal.json`.
  4. If found:
       Parse existing record E.
       If severity_rank(S.severity) > severity_rank(E.severity): E.severity = S.severity.
       If E.status == "resolved": E.status = "reopened".
       E.occurrence_count += 1.
       E.last_seen = current ISO 8601 timestamp.
       Append { "timestamp": E.last_seen, "run_id": current_run_id } to E.occurrences.
       Write updated E back to `.moeb/signals/catalogue/<identity_key>.signal.json`.
       S.signal_id = E.signal_id.
  5. After processing all signals, update `.moeb/signals/index.md` (see Index Update below).

The run-based signals file `.moeb/signals/{{run_id}}.signals.json` is then written as normal,
using the signal_id values assigned in steps 3–4 above.

### Index Update

After completing the Dedup-and-Write Procedure for all signals:
- Attempt to read `.moeb/signals/index.md` using `read_file`. If missing, create it with
  `write_file` using this header:

  ```markdown
  # Signal Index

  | Signal ID | Title | Category | Severity | Status | Occurrences | Last Seen | File |
  |-----------|-------|----------|----------|--------|-------------|-----------|------|
  ```

- For each canonical signal written in the current run, search the existing table for a
  row whose `Signal ID` cell matches the canonical `signal_id`.
  - If found: replace that row in the in-memory content with updated values for
    `Severity`, `Status`, `Occurrences`, and `Last Seen`, then write the complete
    updated file using `write_file`.
  - If not found: append a new row to the in-memory content, then write the complete
    updated file using `write_file`:

    `| <signal_id> | <title> | <category> | <severity> | <status> | <occurrence_count> | <last_seen> | [catalogue/<identity_key>.signal.json](catalogue/<identity_key>.signal.json) |`

2. Write signals to `.moeb/signals/{{run_id}}.signals.json`.

3. Continue to Metrics Recording regardless of critical signal presence.

## Phase 8 — Metrics Recording

### Part A — Record

1. Assemble `RunMetrics`:
   - `run_id`: the current run identifier
   - `timestamp`: ISO 8601 run-start time
   - `rubric_score`: written by the kernel from RunState after verify_rubrics completes — do NOT compute or write this field.
   - `step_metrics`: all StepMetric records from spec-file and readme-link steps
   - `end_review_error_count`: written by the kernel from RunState — do NOT compute or write this field.
   - `wall_time_ms`: elapsed milliseconds since run start

2. Write to `.moeb/metrics/{{run_id}}.metrics.json`.

3. Write the run file to `{{run_file_path}}` with the following JSON content:

   ```json
   {
     "run_id": "{{run_id}}",
     "timestamp": "<ISO 8601 start time>",
     "command": "spec",
     "spec_path": "<path of the spec file authored in Phase 4>",
     "signals_path": ".moeb/signals/{{run_id}}.signals.json",
     "metrics_path": ".moeb/metrics/{{run_id}}.metrics.json",
     "rubric_score": <from rubric_score above>,
     "end_review_error_count": <count of Critical signals>
   }
   ```

   Use `write_file` with path `{{run_file_path}}`. The `spec_path` is the path of the
   spec file authored in Phase 4 (`.moeb/specifications/<domain>/<domain>.<slug>.md`
   derived from frontmatter). The timestamp is the session start time (the time at which
   this run began, not the time this file is written).

### Part B — Regression Detection

4. Load last `{{metrics_window}}` metrics files. If fewer than 2 exist, skip regression detection.

5. If `rubric_score < rolling_avg * (1 - {{metrics_degradation_margin}})`: append a
   DegradationSignal to the signals file.

6. Emit `MetricsEvent { metrics: <RunMetrics> }` to the trace.

## Phase 9 — Commit

Call `git_commit` with:
- `spec_path`: `.moeb/specifications/<domain>/<domain>.<slug>.md`
- `readme_path`: `.moeb/README.md`
- `domain` and `slug` from frontmatter.

## Phase 10 — Tag

Call `tag_run` with:
- `run_id`: the current run identifier (`{{run_id}}`)
- `domain`: the value of the `domain` field from the spec frontmatter
- `slug`: the value of the `slug` field from the spec frontmatter

The tool creates an annotated git tag `run/{{run_id}}` with annotation body
`moeb <domain>/<slug> @ <ISO 8601 timestamp>`.

## Phase 11 — Branch

After the file is written, call `create_branch` with `domain` and `slug` extracted from
the frontmatter. The tool creates the `feat/<domain>-<slug>` branch per Conventional
Branch 1.0.0. This branch is created after the spec commit so that it points to the
committed spec, making the implementation branch derivable from the spec's git history.

## Phase 12 — Complete

Respond with a concise summary of every file created or updated during this run.
