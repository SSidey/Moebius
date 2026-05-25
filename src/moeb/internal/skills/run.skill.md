---
review: true
---
IMPORTANT — DO NOT narrate, plan, or summarise before calling tools. Your FIRST action
must be a tool call. Do not write "let me start", "I will now", "here is my plan", or
any equivalent preamble. Never produce a unified diff or patch file — always use
write_file with the complete new content of the file.

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

- All other criteria: apply the stated Pass Condition. Mark `na` only when the criterion
  genuinely does not apply to this specification's scope.

Call `verify_rubrics` with the complete list of verdicts covering all criteria from both
sources. Do not call `verify_rubrics` with a partial list.

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
