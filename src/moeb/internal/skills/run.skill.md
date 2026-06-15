---
review: true
---
IMPORTANT — DO NOT narrate, plan, or summarise before calling tools. Your FIRST action
must be a tool call. Do not write "let me start", "I will now", "here is my plan", or
any equivalent preamble. For targeted in-place modifications of existing file content, use patch_file with exact
old_string/new_string. For new files or complete rewrites, use write_file. If patch_file
fails (old_string not found), fall back to write_file with the complete file content.

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

Additionally, if a moeb tool call returns output that indicates silent failure, unexpected format, or required a workaround to proceed, buffer a ToolImprovement signal immediately and incorporate it via the Canonical Signal Dedup-and-Write Procedure during the End-of-Skill Review phase:

```json
{
  "category": "ToolImprovement",
  "severity": "Minor",
  "title": "Tool output issue: <tool_name> — <brief description of issue>",
  "description": "Tool <tool_name> returned output indicating <silent failure | unexpected format | required workaround>. Observed: <verbatim or summarised tool output>. Workaround applied: <what was done to compensate, or 'none' if the call was retried>.",
  "proposed_resolution": "Update <tool_name> to <fix description> so that <expected behaviour> without requiring agent workarounds.",
  "gating_condition": null
}
```

Escalate `severity` to `"Major"` when the tool's silent failure caused the intended operation not to complete correctly (e.g. a write that returned success but left the file unchanged, or a patch that silently applied to the wrong location).

### run_command Shell Syntax

`run_command` dispatches to `cmd.exe` on Windows and `sh` on Unix. Always use shell-appropriate syntax in `run_command` calls:

- **Windows (`cmd.exe`):** use built-in cmd commands: `dir`, `type`, `findstr`, `echo`, `del`, `copy`, `move`, `md`, `rd`, `set`, `where`. Do not use PowerShell cmdlets.
- **Unix (`sh`):** use POSIX sh commands: `ls`, `cat`, `grep`, `find`, `echo`, `rm`, `cp`, `mv`, `mkdir`, `which`.

**Never use PowerShell cmdlets** (`Get-ChildItem`, `Test-Path`, `Select-String`, `Get-Content`, `Set-Content`, `Remove-Item`, `ForEach-Object`, `Where-Object`) in `run_command` calls regardless of the host platform. `cmd.exe` does not interpret PowerShell syntax and will return a non-zero exit code or produce no output.

**ToolSearch is unconditionally exempt from this policy.** ToolSearch is a session-level
schema resolver built into the Claude Code MCP client — it loads deferred tool schemas from
`<system-reminder>` entries and cannot be replaced by any moeb tool. Do not buffer a
MissingMoebTool signal for ToolSearch calls. The moeb-tool-origin boundary applies from
Phase 1 onwards for all other external tools that have moeb equivalents (e.g. Read →
read_file, Bash → run_command, Glob → list_directory/search_files, Grep → grep_files).

### Pre-Read Discipline

Before calling `patch_file` on any file, that file must have been read via `read_file` or `read_file_range` earlier in the current session. If a file that will be patched has not been read, call `read_file` on it first. Skipping this step causes a scope-enforcement rejection that forces an unplanned retry and inflates `iteration_count`.

### grep_files Line-Boundary Limitation

`grep_files` matches within single lines only. If the text you intend to use as `old_string` in a `patch_file` call may span two or more lines in the file, `grep_files` will return no match. Before calling `patch_file` with a multi-line `old_string`, use `read_file_range` to retrieve and confirm the exact text (including whitespace, indentation, and line endings) from the file. Do not call `patch_file` based solely on a grep result when the target phrase is known or suspected to span lines.

## Phase 1 — Plan

1. **Bootstrap (if schemas not yet loaded).** If the moeb tool schemas are not yet active
   in this session, call ToolSearch with query `"select:start_run"` to load the `start_run`
   tool schema before proceeding. This is the canonical pre-skill setup step; skip if
   start_run was already called to initiate this skill session.

2. Call `create_task_list` as your very first tool call. Derive one task per numbered Step
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
   - **Preferred for targeted modifications:** When changing a specific substring of an
     existing file, use `patch_file` with the exact `old_string` and `new_string`. This
     is the preferred path for in-place edits.
   - **Preferred for new files or complete rewrites:** When creating a new file or
     replacing the entire file content, use `write_file`.
   - **Fallback:** If `patch_file` fails (old_string not found), read the file, apply
     the change in working memory, and write the complete updated content using
     `write_file`. Do not retry `patch_file`.
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
   - Apply the patch: call `patch_file` with `old_string` set to the content being
     replaced and `new_string` set to the updated content. If `patch_file` returns an
     error, call `write_file` with the complete updated artifact content immediately —
     do not retry `patch_file`.
   - If the write tool returns an error result, terminate the sub-loop immediately —
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

- `moeb-tool-origin`: Review this conversation for tool calls to non-moeb tools (tools whose names are not in the moeb tool schema, such as Bash, Edit, Write, Read, Glob, Grep, WebFetch, or WebSearch). If no external tool calls appear in the conversation, supply Pass. If external tool calls appear and MissingMoebTool signals were buffered for each one, supply Fail with `acknowledged_failures` listing each signal title. If external tools were used without corresponding buffered signals, supply Fail and list the unlogged tool names in the note field.

- `no-signal-reoccurrence` check:
  1. Use list_directory on `.moeb/signals/catalogue/` to enumerate *.signal.json files.
     If the directory does not exist or is empty, verdict = pass.
  2. For each file, read it. If any record has status == "reopened" and its last
     occurrences entry has run_id == current_run_id: verdict = fail, note each
     reopened signal title.
  3. If no such record found: verdict = pass.

- All other criteria: apply the stated Pass Condition. Mark `na` only when the criterion
  genuinely does not apply to this specification's scope.

### Structural Inline Fix Sub-Phase

Before calling `verify_rubrics`, apply inline fixes for any structural rubric criterion annotated with `phase: structural` and `skill_ref`:

1. Identify all criteria with `phase: structural` in `{{command_rubrics}}` and the spec's `## Rubric / ### Structured` table.
2. Evaluate all structural criteria as a group, recording pass/fail for each.
3. For each structural criterion that **fails** and has a `skill_ref` value:
   a. Locate the skill block at `.moeb/rubric-skills/<skill_ref>.md`. If absent, treat as no `skill_ref` and proceed to item 4.
   b. Invoke the skill block inline: the block receives the failing criterion text and the observed failure evidence. The block may call write_file, patch_file, read_file, and run_command tools to apply a targeted fix. The block **must not** call `verify_rubrics`, `enter_phase`, or any orchestration tool.
   c. After the block exits, re-run structural criteria only (step 2 above, restricted to structural criteria). Increment an iteration counter per criterion.
   d. If `skill_ref_max_iterations` (default 2) is reached and the criterion still fails, collect the failure and stop retrying this criterion.
4. For structural criteria that fail with no `skill_ref`, collect the failure as-is.
5. Do not emit individual signals for structural failures here. All rubric failures are emitted as a batch in Phase 5 via the ReviewSignalReport. Include structural verdicts in the `verify_rubrics` call as pass or fail.

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

## Canonical Signal Dedup-and-Write Procedure

# Identity key: "<category>-<title-slug>"  (slug = lowercase, [^a-z0-9]+ → '-', strip edges, max 80 chars)
# Canonical path: .moeb/signals/catalogue/<signal_id>.signal.json
# Severity order: Info < Minor < Major < Critical
# current_run_id: the run identifier from the current run session ({{run_id}} in the rendered prompt)
# Note: .moeb/signals/catalogue/ is created on first use.
# write_file creates parent directories automatically — no explicit mkdir required.
#
# Canonical signal record schema (all fields in this order):
# { "signal_id", "category", "signal_source", "severity", "title", "description", "proposed_resolution",
#   "gating_condition": string[] | null  -- UUIDs of signals that must be resolved
#     before fix_signal may pick up this signal. null = no gate.
#     Example: ["a1000040-0601-4000-8000-000000000040"]
#   "status", "occurrence_count", "first_seen", "last_seen", "occurrences",
#   "resolution_summary": string | null,  -- Human-readable explanation of how the signal was resolved.
#                                            Set by accept_candidate or the post-run archive hook. Null until resolved.
#   "superseded_by": string | null,       -- signal_id of the signal that supersedes this one.
#                                            Null unless status is "superseded".
#   "archived_on": string | null          -- ISO 8601 timestamp set by archive_signal when the file is
#                                            moved to archive/catalogue/. Null until archived.
#   "candidate_tag": string | null,       -- The candidate tag created during the run that produced this signal's
#                                            candidate status. Set when status transitions to 'candidate'.
#                                            Null until then.
# }

For each signal S in the ReviewSignalReport:
  1. Compute identity_key as described above.
  2. Read `.moeb/signals/index.md`. Scan for a row whose `Title` column matches S.title
     (case-insensitive) and `Category` column matches S.category (case-insensitive).
     If a matching row is found, extract its `Signal ID` cell as `existing_id`, then
     read `.moeb/signals/catalogue/<existing_id>.signal.json`; if the read succeeds,
     treat the signal as found with record E. If no matching row is found in index.md,
     or if the catalogue file read fails, treat as not_found.
     (The identity_key slug is retained for audit logging but must not be used as a
     filename.)
  3. If not_found:
       Assign S.signal_id = new UUID v4.
       Set S.status         = "open".
       Set S.signal_source = S.signal_source if present in the buffered signal definition, else determine by source:
         - If the signal was triggered by a project-domain rubric criterion (layers 3-4 in .moeb/rubrics/ for a non-moeb spec domain): "project"
         - All other signals (QA End-of-Skill Review, budget breach, missing moeb tool, uncommitted path,
           reasoning review, partial recovery, no-signal-reoccurrence): "moeb"
         - Default: "moeb"
       Set S.occurrence_count = 1.
       Set S.first_seen     = current ISO 8601 timestamp.
       Set S.last_seen      = current ISO 8601 timestamp.
       Set S.occurrences    = [{ "timestamp": S.first_seen, "run_id": current_run_id }].
       Write canonical record to `.moeb/signals/catalogue/<signal_id>.signal.json`.
  4. If found:
       Parse existing record E.
       If E.signal_source is absent (legacy record without field): set E.signal_source = "moeb".
       If severity_rank(S.severity) > severity_rank(E.severity): E.severity = S.severity.
       If E.status == "resolved": E.status = "reopened".
       E.occurrence_count += 1.
       E.last_seen = current ISO 8601 timestamp.
       Append { "timestamp": E.last_seen, "run_id": current_run_id } to E.occurrences.
       Write updated E back to `.moeb/signals/catalogue/<signal_id>.signal.json`.
       S.signal_id = E.signal_id.
  5. After processing all signals, update `.moeb/signals/index.md` (see Index Update below).

After all catalogue writes complete, add each emitted signal as { "id": signal_id, "description": title } to the emittedSignals array in the run file (written in the Metrics Recording phase).

### Index Update

After completing the Dedup-and-Write Procedure for all signals:

1. If `.moeb/signals/index.md` does not exist, create it with `write_file` using the standard header, then process each signal as Case 1 below:

   ```markdown
   # Signal Index

   | Signal ID | Title | Category | Source | Severity | Status | Occurrences | Last Seen | File |
   |-----------|-------|----------|--------|----------|--------|-------------|-----------|------|
   ```

2. For each canonical signal written in the current run, call `grep_files` with `path: ".moeb/signals/index.md"` and `pattern: "<signal_id>"` (the literal UUID string). If `grep_files` returns a match, treat as Case 2; otherwise treat as Case 1.

3. **Case 1 — row absent (new signal):** Call `grep_files` on `.moeb/signals/index.md` with `pattern: "| "` to locate the last `| ` line in the table body and obtain the exact last data row text. Call `patch_file` with `old_string` set to that last data row and `new_string` set to that same row followed immediately by a newline and the new row: `| <signal_id> | <title> | <category> | <signal_source> | <severity> | <status> | <occurrence_count> | <last_seen> | [catalogue/<signal_id>.signal.json](catalogue/<signal_id>.signal.json) |`. If `patch_file` fails (including when no data rows exist yet), fall back to `read_file` + `write_file` with the full updated content.

4. **Case 2 — row present (existing signal update):** Use the matching line returned by `grep_files` as the exact current row text. Call `patch_file` with `old_string` set to that exact row and `new_string` set to the updated row with refreshed values for `Severity`, `Status`, `Occurrences`, and `Last Seen`. If `patch_file` fails, fall back to `read_file` + `write_file` with the full updated content.

### archive_signal usage

Call `archive_signal` only after a signal's status has reached `"resolved"` or `"superseded"`.
The tool performs an integrity check before moving the file; if the active catalogue and index
are out of sync, the call returns an error rather than proceeding.
Pass the optional `resolution_summary` argument to record a human-readable explanation
alongside the archive timestamp. Archived signals are moved to
`.moeb/signals/archive/catalogue/` and their index rows are moved to
`.moeb/signals/archive/index.md`.

After all Phase 5 signals are processed, evaluate the ReviewSignalReport produced above. Set working memory `qa_passed = true` if the signals array contains no entry with `"severity": "Critical"`; otherwise set `qa_passed = false`. Signals from Phase 5b (Reasoning Review) and Phase 5c (Retrospective Review) do NOT contribute to `qa_passed`. Only Critical signals produced during Phase 5 (qualitative and invocation-specific rubric evaluation by the QA Architect) affect `qa_passed`.

2. Do not fail or abort the skill if critical signals are present. Continue to the
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
   - `tools_used`: sorted array of tool names from `get_run_status`.
   - `token_usage`: object with fields `input_tokens`, `output_tokens`, `cache_read_tokens`, `cache_creation_tokens`, `total_tokens` — read from the `token usage:` line in `get_run_status` output.

2. Write the `RunMetrics` object to `.moeb/metrics/{{run_id}}.metrics.json` as JSON.

3. Write the run file to `{{run_file_path}}` with the following JSON content:

   ```json
   {
     "run_id": "{{run_id}}",
     "timestamp": "<ISO 8601 start time>",
     "command": "run",
     "signal_id": "<signal_id session context value if non-empty, else JSON null>",
     "candidate_tag": null,
     "spec_path": "<path of the spec file being executed>",
     "metrics_path": ".moeb/metrics/{{run_id}}.metrics.json",
     "emittedSignals": [
       { "id": "<signal_id_1>", "description": "<title_1>" }
     ],
     "rubric_score": <from verify_rubrics output>,
     "end_review_error_count": <count of Critical signals>,
     "qa_passed": <qa_passed>,
     "tools_used": <sorted array from get_run_status>,
     "token_usage": {
       "input_tokens": <from get_run_status>,
       "output_tokens": <from get_run_status>,
       "cache_read_tokens": <from get_run_status>,
       "cache_creation_tokens": <from get_run_status>,
       "total_tokens": <from get_run_status>
     }
   }
   ```

   > `tools_used` and `token_usage`: call `get_run_status` immediately before writing the run file and extract the `tools used:` line as a sorted JSON array and the `token usage:` line for the token counts.

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
     "gating_condition": null
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

**QA Gate.** Before proceeding, check working memory `qa_passed`. If `qa_passed` is `false`:
1. Collect all Critical signals from the Phase 5 ReviewSignalReport. Format each as `"<title>"`.
2. Emit a signal via the Canonical Signal Dedup-and-Write Procedure with:
   - `category`: `"Error"`
   - `severity`: `"Major"`
   - `title`: `"Candidate tag suppressed: QA gate failed"`
   - `description`: `"QA Architect review produced Critical signals; qa_passed is false. Failing signals: <comma-separated list of formatted Critical signal titles>."`
   - `proposed_resolution`: `"Resolve all Critical signals listed above and re-run to obtain a candidate tag."`
   - `gating_condition`: `null`
3. Append this signal to the run file `emittedSignals` array.
4. Skip the remainder of Phase 9 entirely (do not call `bump_version` or `create_candidate_tag`). Proceed directly to Phase 10 — Complete.

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

**Run file `candidate_tag` update:**
After `create_candidate_tag` returns a tag name (not an error):
1. Call `read_file` with path `{{run_file_path}}`.
2. Parse the JSON content.
3. Set `"candidate_tag"` to the tag name returned by `create_candidate_tag`.
4. Write the updated JSON back to `{{run_file_path}}` using `write_file`.

Skip this update if `create_candidate_tag` returned an error string or if the QA gate
prevented the tag from being created.

### Signal Status Update <!-- condition: signal_id non-empty, candidate tag created -->

If the session context variable `signal_id` is empty or absent, skip this sub-procedure entirely and proceed to Phase 10.

1. Call `read_file` on `.moeb/signals/catalogue/<signal_id>.signal.json` where `<signal_id>` is the `signal_id` context variable. If the file does not exist, skip the remaining steps and proceed to Phase 10.
2. Parse the returned JSON. Set `"status"` to `"candidate"`. Set `"candidate_tag"` to the tag value returned by `create_candidate_tag`. Do not alter any other field.
3. Write the updated JSON back to the same path using `write_file`.
4. Run the **Index Update sub-procedure**: read `.moeb/signals/index.md`, find the row whose `Signal ID` cell matches `signal_id`, update its `Status` cell to `candidate`, write the complete updated `index.md` using `write_file`. If no matching row is found, append a new row with current field values.

## Phase 9a — Archive Resolved Signals

For each `{ "id": signal_id, ... }` entry in the current run file's `emittedSignals` array:

1. Read `.moeb/signals/catalogue/<signal_id>.signal.json`. If the file does not exist
   (the signal may already be archived or the entry is stale), skip this entry.
2. Parse the `status` field.
   - If `status` is `"resolved"` or `"superseded"`:
     a. Call `archive_signal` with `signal_id` and the signal record's
        `proposed_resolution` value as `resolution_summary`.
     b. If `archive_signal` returns an error, emit a SkillImprovement signal:
        - `category`: `"SkillImprovement"`, `severity`: `"Minor"`
        - `title`: `"archive_signal failed during post-run hook: <signal_id>"`
        - `description`: the verbatim error returned by `archive_signal`
        - `proposed_resolution`: `"Investigate and manually archive or repair the signal record before the next run."`
     c. Continue to the next entry regardless.
   - Otherwise skip this entry.

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

### Budget Breach Check

After the QA Architect review and before writing signals, call `get_run_status` and
inspect the `budget breaches:` section of the output.

For each breach listed, apply the granularity rule:
- If any breach has `level=tool`: emit one tool-level signal per unique (phase, turn) combination.
- Else if any breach has `level=phase`: emit one phase-level signal per unique phase.
- Else if any breach has `level=run`: emit one run-level signal.

Tool-level signal template:
```json
{
  "category": "SkillImprovement",
  "severity": "Major",
  "title": "Token budget exceeded at tool-call level: phase=<phase_id> turn=<n>",
  "description": "Tool call in phase <phase_id> at turn <n> used <total> tokens, exceeding the per-tool budget of <threshold>.",
  "proposed_resolution": "Review the tool call in phase <phase_id> that produced the most output. Consider splitting the operation into smaller steps or raising TOKEN_BUDGET_TOOL if the threshold is too conservative.",
  "gating_condition": null
}
```

Phase-level signal template:
```json
{
  "category": "SkillImprovement",
  "severity": "Major",
  "title": "Token budget exceeded at phase level: phase=<phase_id>",
  "description": "Phase <phase_id> consumed <total> tokens in aggregate, exceeding the per-phase budget of <threshold>.",
  "proposed_resolution": "Review whether phase <phase_id> can be decomposed, or raise TOKEN_BUDGET_PHASE if the threshold is too conservative.",
  "gating_condition": null
}
```

Run-level signal template:
```json
{
  "category": "SkillImprovement",
  "severity": "Major",
  "title": "Token budget exceeded at run level",
  "description": "The run consumed <total> tokens in total, exceeding the run budget of <threshold>.",
  "proposed_resolution": "Enable conversation compaction (COMPACTION_ENABLED), reduce the number of phases, or raise TOKEN_BUDGET_RUN if the threshold is appropriate for this workload.",
  "gating_condition": null
}
```

## Phase 5a — Push Thinking Blocks

This phase runs when `{{no_review}}` is `"false"` (the default). Skip ONLY if
`{{no_review}}` is the exact string `"true"`.

Call `push_thinking_blocks` with the key reasoning text produced during this run.
Include:

- Decision rationale for any non-obvious implementation choice (why this signal
  category, why this fix approach, why a deviation from the skill default was taken).
- Deliberation about whether any signal should or should not be emitted.
- Any uncertainty that was resolved and how it was resolved.
- Reasoning about tradeoffs between alternatives that were considered.

Do not include mechanical steps: file reads, build output, cargo test results, or
descriptions of what was done rather than why.

Submit all reasoning as a single `push_thinking_blocks` call with one string element
per distinct decision point. Each element should be 2–6 sentences.

After `push_thinking_blocks` returns, proceed to Phase 5b — Reasoning Review.

## Phase 5b — Reasoning Review

> **MCP/inline mode note:** Do not call `query_agent` in this phase. When running
> inline (via `start_spec` or `start_run` MCP tools), the agent IS the model and must
> use only inline persona reasoning. The Reasoning Reviewer Persona is pre-loaded in
> your context for exactly this purpose.

This phase runs when `{{no_review}}` is `"false"` (the default). Skip ONLY if
`{{no_review}}` is the exact string `"true"`.

This review is advisory — its findings are emitted as signals to the improvement
queue but do not affect the rubric verdict or run pass/fail.

1. If `RunState.thinking_blocks` is empty, skip this phase entirely and proceed to
   Phase 6.

2. Without calling any tool, adopt the **Reasoning Reviewer Persona** pre-loaded in
   your context. Evaluate the accumulated thinking block strings from this run against
   the reasoning quality criteria. Produce a raw JSON array of signal objects (each
   with `"category": "ReasoningImprovement"`) in working memory, or an empty array
   `[]` if no improvements are identified.

3. Parse the returned JSON array. If it is not valid JSON or is not an array, treat it
   as an empty array and proceed.

4. For each signal S in the array, assign:
   - `signal_id`: a fresh UUID v4
   - `run_id`: the current run identifier (`{{run_id}}`)
   - `timestamp`: ISO 8601 current time

   Then write S via the **Canonical Signal Dedup-and-Write Procedure** defined in
   this skill file.

5. After all signals are processed, run the Index Update sub-procedure.

6. Append each emitted signal's `{ "id": signal_id, "description": title }` to the
   run file's `emittedSignals` array.

Continue to Phase 6 — Metrics Recording regardless of signal count.

## Phase 5c — Retrospective Review

This phase always runs regardless of the value of `{{no_review}}`. It is advisory —
signals emitted here never gate pass/fail and never affect `qa_passed`.

1. Without calling any tool, adopt the **Retrospective Reviewer Persona** pre-loaded
   in your context. Evaluate the run process using the observational lenses defined in
   the Retrospective Reviewer Persona. Produce a raw JSON array of signal objects (each with
   `"signal_source": "moeb"`, `"severity": "Major"` or `"Minor"` — never Critical).
   Hold the result in working memory.

2. Parse the returned JSON array. If it is not valid JSON or is not an array, treat it
   as an empty array and proceed.

3. For each signal S in the array, assign:
   - `signal_id`: a fresh UUID v4
   - `run_id`: the current run identifier (`{{run_id}}`)
   - `timestamp`: ISO 8601 current time

   Then write S via the **Canonical Signal Dedup-and-Write Procedure** defined in
   this skill file.

4. After all signals are processed, run the Index Update sub-procedure.

5. Append each emitted signal's `{ "id": signal_id, "description": title }` to the
   run file's `emittedSignals` array.

Continue to Phase 6 — Metrics Recording regardless of signal count.

## Cleanup Commit

Call `git_commit` with `kind: "run"`. This stages all uncommitted working-tree changes — signals, metrics, run file, and signal catalogue entries written during the review phases — and commits them, keeping the branch clean before the Complete event fires.

If the working tree is already clean (nothing to commit), `git_commit` with `kind: "run"` is a no-op; proceed to Complete without error.

## Rubric

### Structured

| Criterion | Description | Pass Condition | Verification Method |
|-----------|-------------|----------------|---------------------|

## Phase 10 — Complete

Derive the compact ISO timestamp as `yyyyMMddTHHmmssZ` from the current time (e.g.
`20260604T132045Z`). Using the `run_id`, `signal_id`, and `candidate_tag` values already
in context from the Metrics Recording phase (no additional file read is required):

If `qa_passed` is `true`, construct `.moeb/events/{compact_timestamp}_{run_id}.event.json`
and write using `write_file`:

```json
{
  "event_type": "run_completed",
  "run_id": "<run_id>",
  "signal_id": "<signal_id from run context, or null>",
  "candidate_tag": "<candidate_tag from create_candidate_tag return value, or null>",
  "timestamp": "<current ISO 8601 timestamp>"
}
```

If `qa_passed` is `false`, construct the same path and write:

```json
{
  "event_type": "run_failed",
  "run_id": "<run_id>",
  "signal_id": "<signal_id from run context, or null>",
  "candidate_tag": null,
  "timestamp": "<current ISO 8601 timestamp>",
  "reason": "QA gate: Critical signals detected in End-of-Skill Review"
}
```

The event write must not interrupt subsequent phases. The closing summary must be issued
regardless of whether the event write succeeds.

Respond with a concise summary of every file created or updated.
