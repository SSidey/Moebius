---
review: true
---
IMPORTANT — DO NOT narrate, plan, or summarise before calling tools. Your FIRST action
must be a tool call. Do not write "let me start", "I will now", "here is my plan", or
any equivalent preamble. Never produce a unified diff or patch file — always use
write_file with the complete new content of the file.

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

## Phase 1 — Plan

Call `create_task_list` as your very first tool call. Derive one task per numbered Step
in the specification's `## Steps` section. Each task entry must state which file(s) it
touches and what change is required.

## Phase 2 — Scope

Before modifying anything, locate all relevant code:

1. Call `list_directory` on `src/` to understand the top-level project layout.
2. Call `search_files` with path `src/` and an appropriate extension (e.g. `rs`, `toml`)
   to enumerate source files relevant to the specification.
3. Call `grep_files` to locate the specific functions, types, or modules that need to
   change. Note the file path and line number in each result.
4. Call `read_file_range` with the file path, a start line a few lines before the match,
   and an end line that covers the complete function or block. Prefer `read_file_range`
   over `read_files` — only read a full file when you cannot determine the relevant range
   from grep results or when writing a complete file replacement.

## Phase 3 — Implement

For each task in your task list, in order:

1. State the exact items you will modify in the target file and which specification step
   requires each change (pre-write declaration — HARD RULE 4).
2. Read the current file content. Use `read_file_range` for targeted sections; use
   `read_file` only when writing a complete file replacement.
3. Write the change:
   - If changing fewer than ~20 lines in a file already read in full, use `patch_file`
     with a unified diff — only the changed lines are transmitted.
   - Otherwise use `write_file` with the complete new content.
   Never use `patch_file` on a file you have not read via `read_file` or `read_files`
   in this run.
4. Call `update_task` with `status: "done"`.

### Per-Step Review Sub-Loop

This sub-loop runs when `{{no_review}}` is `"false"` (the default). Skip ONLY if
`{{no_review}}` is the exact string `"true"`.

After every `write_file` or `patch_file` call within a step:

0. **Error preflight.** If the tool call immediately preceding this sub-loop invocation
   was a file-modification tool (`write_file` or `patch_file`) and its result indicates
   a failure (the result string contains "failed", "error", or "could not"):
   - Record StepMetric with `iteration_count = 0`, `acceptance_rate = 1.0`, `delta_scores = []`.
   - Do NOT call `complete_review` for this path.
   - Proceed to the next task. The kernel-injected `tool-errors` criterion records
     a Fail for this call when `verify_rubrics` is invoked.
   Skip steps 1–9 below for this invocation.

1. **Inline Reviewer.** Without calling any tool, adopt the **Reviewer Persona**
   pre-loaded in your context. Evaluate the artifact against the step intent and the
   applicable rubric criteria. Produce a unified diff if improvements are needed, or
   an empty string if the artifact fully satisfies all criteria. Hold this result in
   working memory — do not output it as a standalone message before continuing.

2. If the returned diff is empty or whitespace: terminate sub-loop. Record StepMetric
   with `iteration_count = 0`, `acceptance_rate = 1.0`, `delta_scores = []`.

3. **Inline Moderator.** Without calling any tool, adopt the **Moderator Persona**
   pre-loaded in your context. Evaluate the proposed diff against the artifact content,
   rubric criteria, and iteration history. Produce a JSON verdict:
   `{ "accepted": bool, "delta_score": float, "rationale": string }`. Hold this result
   in working memory.

4. Parse the Moderator JSON. If `[PARSE_WARNING]` prefix is present, treat as
   `{ "accepted": false, "delta_score": 0.0, "rationale": "Moderator parse failure" }`.

5. If `accepted = true` and `delta_score >= 0.01` and `iteration_count < 2`:
   - Apply the diff: `patch_file` with the proposed diff on the artifact path.
   - If `patch_file` returns an error result, terminate the sub-loop immediately —
     do not return to step 1. Append `{ "delta_score": 0.0, "accepted": false }` to
     iteration history and proceed to step 6. The kernel records this as a tool error.
   - Otherwise append `{ "delta_score": <score>, "accepted": true }` to iteration
     history and return to step 1.

6. Otherwise: terminate sub-loop.

7. Record StepMetric:
   - `step_id`: current step identifier
   - `iteration_count`: length of iteration history
   - `acceptance_rate`: if history empty → `1.0`; else accepted-count / total-count
   - `delta_scores`: `[entry.delta_score for each entry]`

8. Evaluate any spec rubric criteria applicable to this artifact now (e.g. `no-drift`
   immediately after a spec file is written) and note the result before continuing.

9. Call `complete_review` with the path that was passed to `write_file` in this step.
   This acknowledges the review cycle and clears the review obligation for this path.
   Call it regardless of whether a diff was applied — once per write, after the
   sub-loop terminates. Omitting this call causes `verify_rubrics` to fail
   `write-review-compliance`.

Continue until all tasks are marked done.

## Phase 4 — Verify

Collect all rubric criteria from two sources:

1. The injected `{{command_rubrics}}` section (the combined baseline and project layers).
   Every row in this section is mandatory and must receive a verdict.
2. The specification's `## Rubric / ### Structured` table. Every row is mandatory.

For each criterion, evaluate pass, fail, or na:

- `context-budget`: Check the line count of every file written during this run. If any
  implementation file exceeds 300 lines, or any `*_tests.rs` companion file exceeds
  400 lines, refactor it to meet the budget before assigning a verdict. This criterion
  is never `na`.

- `tool-errors`: This criterion is injected by the kernel from `RunState.tool_errors`.
  Do not supply a verdict — any agent-supplied entry is silently overridden. You may
  include it for completeness; the kernel replaces it.

- `rubric-qualification`: This criterion is injected by the kernel from the
  `acknowledged_failures` fields in your verdict objects. Do not supply this criterion
  directly — any agent-supplied entry is silently overridden. When you observe a failure
  but judge it acceptable for a Pass verdict, you MUST populate `acknowledged_failures`
  with a brief description of each failure. The kernel reads this field and produces the
  `rubric-qualification` verdict; the QA Architect surfaces it as a Critical signal.
  Do not leave `acknowledged_failures` empty when a failure was observed — omission is
  treated as "no failures observed" and the qualification will not be surfaced.

- `moeb-tool-origin`: Review this conversation for tool calls to non-moeb tools (tools whose names are not in the moeb tool schema, such as Bash, Edit, Write, Read, Glob, Grep, WebFetch, or WebSearch). If no external tool calls appear in the conversation, supply Pass. If external tool calls appear and MissingMoebTool signals were buffered for each one, supply Fail with `acknowledged_failures` listing each signal title. If external tool calls appear without corresponding buffered signals, supply Fail and list the unlogged tool names in the note field.

- `no-signal-reoccurrence` check:
  1. Use list_directory on `.moeb/signals/catalogue/` to enumerate *.signal.json files.
     If the directory does not exist or is empty, verdict = pass.
  2. For each file, read it. If any record has status == "reopened" and its last
     occurrences entry has run_id == current_run_id: verdict = fail, note each
     reopened signal title.
  3. If no such record found: verdict = pass.

- All other criteria: apply the stated Pass Condition. Mark `na` only when the criterion
  genuinely does not apply to this specification's scope.

Call `verify_rubrics` with the complete list of verdicts covering all criteria from both
sources. Do not call `verify_rubrics` with a partial list.

## Partial Resolution Check

Review the task list created at the start of this run. Identify any task whose status was never updated to `done`.

If all tasks are `done`, skip signal emission and proceed to End-of-Skill Review without further action.

If any task remains incomplete:

a. Assemble a signal record:
   - `category`: `"Error"`
   - `severity`: `"Major"`
   - `title`: `"Partial resolution: <domain>/<slug>"`
   - `description`: `"Run <run_id> did not complete all tasks for specification <spec_path>. Incomplete tasks: <comma-separated list of incomplete task titles>."`
   - `proposed_resolution`: `"Resume by invoking fix_signal with signal_id=<signal_id_of_this_record>. The signal description records which tasks remain."`
   - `gating_condition`: `null`

b. Write this record via the **Canonical Signal Dedup-and-Write Procedure** already present in run.skill.md. The UUID v4 assigned as `signal_id` during the procedure is the value to substitute into `proposed_resolution` before writing.

Continue to End-of-Skill Review regardless of whether a signal was emitted.

## Phase 5 — End-of-Skill Review

This phase runs when `{{no_review}}` is `"false"` (the default). Skip ONLY if
`{{no_review}}` is the exact string `"true"`.

Without calling any tool, adopt the **QA Architect Persona** pre-loaded in your context.
Evaluate the paths and contents of all artifacts produced in this run, the accumulated
StepMetrics as JSON, and the active specification's full `## Rubric` section. Produce a
ReviewSignalReport JSON matching the schema defined in the QA Architect Persona. Hold
the result in working memory and continue with parsing and signal processing below.

If `[PARSE_WARNING]` prefix is present in the response, treat it as a Critical error
signal: append a signal with title "QA Architect response parse failure" and
description containing the raw response, then continue.

Parse the returned `ReviewSignalReport` JSON:

1. Assign each signal:
   - `signal_id`: a fresh UUID v4
   - `run_id`: the current run identifier
   - `timestamp`: ISO 8601 current time

## Canonical Signal Dedup-and-Write Procedure

# Identity key: "<category>-<title-slug>"  (slug = lowercase, [^a-z0-9]+ → '-', strip edges, max 80 chars)
# Canonical path: .moeb/signals/catalogue/<identity-key>.signal.json
# Severity order: Info < Minor < Major < Critical
# current_run_id: the run identifier from the current run session ({{run_id}} in the rendered prompt)
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

2. Write the complete array of signals to `.moeb/signals/{{run_id}}.signals.json`.

3. Do not fail or abort the skill if critical signals are present. Continue to the
   Metrics Recording phase.

## Phase 6 — Metrics Recording

### Part A — Record

1. Assemble `RunMetrics`:
   - `run_id`: the current run identifier
   - `timestamp`: ISO 8601 run-start time
   - `rubric_score`: written by the kernel from RunState after verify_rubrics completes — do NOT compute or write this field.
   - `step_metrics`: all StepMetric records accumulated across steps
   - `end_review_error_count`: written by the kernel from RunState — do NOT compute or write this field.
   - `wall_time_ms`: elapsed milliseconds since run start

2. Write the `RunMetrics` object to `.moeb/metrics/{{run_id}}.metrics.json` as JSON.

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

### Part B — Regression Detection

4. Load the last `{{metrics_window}}` `.metrics.json` files from `.moeb/metrics/`
   ordered by `timestamp` ascending (most recent last). If fewer than 2 files exist,
   skip regression detection (insufficient baseline).

5. Compute `rolling_avg = mean(rubric_score for each loaded file)`.

6. If `rubric_score < rolling_avg * (1 - {{metrics_degradation_margin}})`:
   Append a DegradationSignal to `.moeb/signals/{{run_id}}.signals.json`:
   ```json
   {
     "signal_id": "<fresh UUID v4>",
     "run_id": "{{run_id}}",
     "timestamp": "<ISO 8601>",
     "category": "Error",
     "severity": "Critical",
     "title": "Rubric score degraded below rolling baseline",
     "description": "Current rubric_score is more than {{metrics_degradation_margin}}% below the rolling average.",
     "proposed_resolution": "Revert to the git tag preceding this run, create a diagnostic spec identifying the regression cause, and branch from the pre-regression version.",
     "gating_condition": "NoCandidateBranch"
   }
   ```

7. Emit a `MetricsEvent { metrics: <RunMetrics> }` to the trace.

## Phase 7 — Commit

Extract `domain` and `slug` from the YAML frontmatter at the top of the active
specification (the content between the opening `---` and closing `---` markers). Call
`git_commit` with:
- `kind`: `"run"`
- `domain`: the value of the `domain` field from the frontmatter
- `slug`: the value of the `slug` field from the frontmatter

Do not pass `spec_path` or `readme_path` — they are not used for run commits. The tool
stages all working-tree changes and commits with the message
`feat(<domain>): execute <slug> specification`.

## Phase 8 — Tag

Call `tag_run` with:
- `run_id`: the current run identifier (`{{run_id}}`)
- `domain`: the value of the `domain` field from the spec frontmatter
- `slug`: the value of the `slug` field from the spec frontmatter

The tool creates an annotated git tag `run/{{run_id}}` with annotation body
`moeb <domain>/<slug> @ <ISO 8601 timestamp>`. This tag is independent of semantic
versioning and provides per-run traceability: any commit produced by moeb can be
identified by its run tag.

## Phase 9 — Version and Tag

Classify the change implemented in Phases 1–3 as one of `"major"`, `"minor"`, or
`"patch"` using SemVer 2.0.0 semantics:

- **major**: a publicly visible behaviour is removed or altered incompatibly (a tool is
  removed, an argument is renamed, a command changes its output format in a breaking way).
- **minor**: new capability is added without breaking existing behaviour (a new tool,
  command, flag, or workflow phase is introduced).
- **patch**: a bug fix, refactor, or internal improvement that does not alter the public
  interface.

Consult the active specification's `## Description` and `## Steps` sections to classify
correctly.

1. Call `bump_version` with the determined bump class:
   ```json
   { "bump_type": "<major|minor|patch>" }
   ```

2. Call `create_candidate_tag` with `domain` and `slug` from the spec frontmatter:
   ```json
   { "domain": "<domain>", "slug": "<slug>" }
   ```
   If the tool returns an error indicating the tag already exists, retry with
   `"force": true`:
   ```json
   { "domain": "<domain>", "slug": "<slug>", "force": true }
   ```

## Phase 10 — Complete

Respond with a concise summary of every file created or updated.

## Rubric

### Structured

| Criterion | Description | Pass Condition | Verification Method |
|-----------|-------------|----------------|---------------------|
| `no-preferred-patch-file` | No skill, prompt, or role file written or modified during this run instructs agents to call `patch_file` by preference or by default for writing new file content | Zero violations | grep for instructional `patch_file` references in all skill, prompt, and role files written during this run; verify no match designates `patch_file` as the primary or preferred write tool for new content |
