---
review: true
---
IMPORTANT — DO NOT narrate, plan, or summarise before calling tools. Your FIRST action
must be a tool call.

## Tool Origin Policy

Prefer moeb tools for every operation. Available tools: read_file, read_files,
read_file_range, write_file, patch_file, grep_files, list_directory, search_files,
create_task_list, update_task, verify_rubrics, complete_review, query_agent, git_commit,
tag_run, tag_signal, create_branch, bump_version, get_version.
If an external tool must be used, buffer a MissingMoebTool signal immediately after:

```json
{
  "category": "NewCapability",
  "severity": "Major",
  "title": "Missing moeb tool: <external_tool_name>",
  "description": "Used <external_tool_name> to perform <operation>. No moeb equivalent exists.",
  "proposed_resolution": "Run moeb spec to introduce a moeb equivalent covering <operation>.",
  "gating_condition": null
}
```

## Signal Resolution Mode

Inspect `{{signal_id}}`:

**If `{{signal_id}}` is non-empty:**
1. Read `.moeb/signals/index.md`.
2. Find the table row whose `Signal ID` column equals `{{signal_id}}`.
3. Extract the file path from the `File` column of that row.
4. Read the signal record from that path.
5. Skip Phase 1 (signal scan) and Phase 2 (signal selection) below.
6. Proceed directly to Phase 3 (mark as picked up) with the loaded signal record.

**If `{{signal_id}}` is empty:**
Proceed with Phase 1 and Phase 2 as defined below.

## Phase 1 — Scan

Call `enter_phase` with `phase_id: "phase-1"` as the first action in this phase.

Call `list_directory` on `.moeb/signals/catalogue/`. For each entry ending with
`.signal.json`, call `read_file` on that path. Parse the JSON object. Collect every
signal object where `picked_up_at` is absent or null and `status` is `"open"` or
`"reopened"`. Record each signal alongside its file path.

If no `.signal.json` files exist in the catalogue or all signals have `picked_up_at`
set, skip to Phase 12 with the message: `"No unresolved signals found."`. If no unresolved signal is found, exit without writing a completion event.

## Phase 2 — Select

Call `enter_phase` with `phase_id: "phase-2"` as the first action in this phase.

Assign each unresolved signal a numeric severity rank: `"Critical"` → 2, `"Major"` → 1,
any other value → 0. Sort by severity rank descending, then by `timestamp` ascending
(oldest first among equal severity). Select the first signal after sorting. Record its
`signal_id`, `severity`, `title`, `proposed_resolution`, `description`, and source file
path.

## Phase 3 — Mark

Call `enter_phase` with `phase_id: "phase-3"` as the first action in this phase.

Compute `fix_branch` before patching:
1. Derive `domain`: if `category` is `"Error"`, use `"fix"`; otherwise use `"moeb"`.
2. Derive `slug`: lowercase `title`, replace non-`[a-z0-9-]` chars with hyphens,
   collapse consecutive hyphens, strip leading/trailing hyphens, truncate to 50 chars.
3. `fix_branch` = `"feat/<domain>-<slug>"`.

Call `read_file` on the source file path. Locate the closing `}` of the chosen signal
object. Call `patch_file` with `old_string` set to the closing `}` of the signal object
and `new_string` set to the same closing `}` preceded by the two new fields:

  `,\n  "picked_up_at": "<current ISO 8601 timestamp>",\n  "fix_branch": "<fix_branch>"\n}`

If `patch_file` returns an error, add the two fields in working memory and call
`write_file` to overwrite the source file with the complete updated content.

5. Parse the signal JSON in memory. Set `"status"` to `"in_progress"`. Write the updated signal back to `.moeb/signals/catalogue/<signal_id>.signal.json` using `write_file`.
6. Run the **Index Update sub-procedure**: read `.moeb/signals/index.md`, find the row whose `Signal ID` cell matches the selected signal's `signal_id`, update its `Status` cell to `in_progress`, write the complete updated `index.md` using `write_file`. If no matching row exists, append a new row with the current signal's field values.

### Per-Step Review Sub-Loop (signal pickup)

0. **Error preflight.** If the preceding write returned an error (result contains
   "failed", "error", or "could not"): Record StepMetric with `step_id: "signal-pickup"`,
   `iteration_count = 0`, `acceptance_rate = 1.0`, `delta_scores = []`. Do NOT call
   `complete_review`. Proceed to Phase 4. Skip steps 1–8.

1. **Inline Reviewer.** Without calling any tool, adopt the Reviewer Persona. Evaluate
   the signal pickup artifact: `picked_up_at` and `fix_branch` must be present and the
   `signal_id` must match the selected signal. Produce a unified diff if improvements
   are needed, or an empty string if correct.

2. If the diff is empty or whitespace: terminate sub-loop. Record StepMetric with
   `step_id: "signal-pickup"`, `iteration_count = 0`, `acceptance_rate = 1.0`,
   `delta_scores = []`.

3. **Inline Moderator.** Without calling any tool, adopt the Moderator Persona. Produce
   JSON: `{ "accepted": bool, "delta_score": float, "rationale": string }`.

4. Parse Moderator JSON. If `[PARSE_WARNING]`: treat as
   `{ "accepted": false, "delta_score": 0.0, "rationale": "Moderator parse failure" }`.

5. If `accepted = true` and `delta_score >= 0.01` and `iteration_count < 2`:
   - Apply the patch: call `patch_file` with `old_string` set to the content being
     replaced and `new_string` set to the updated content. If `patch_file` returns an
     error, call `write_file` with the complete updated artifact content immediately —
     do not retry `patch_file`.
   - If the write tool returns an error result, terminate the sub-loop immediately —
     append `{ "delta_score": 0.0, "accepted": false }` to history; proceed to step 6.
   - Otherwise: append `{ "delta_score": <score>, "accepted": true }` to history;
     return to step 1.

6. Otherwise: terminate sub-loop.

7. Record StepMetric with `step_id: "signal-pickup"`.

8. Call `complete_review` with the signal file path.

Record a StepMetric for `step_id: "mark-signal"`.

## Phase 4 — Branch

Call `enter_phase` with `phase_id: "phase-4"` as the first action in this phase.

Call `create_branch` with `domain` and `slug` derived in Phase 3.
Record a StepMetric for `step_id: "create-branch"`.

## Phase 5 — Emit Event

Call `enter_phase` with `phase_id: "phase-5"` as the first action in this phase.

Generate a UUID v4 `event_id` for this event.

Derive `repo` by calling `read_file` on `.git/config`. Search for the `url =` line
under a `[remote "origin"]` stanza and extract the value after `= ` (trimmed). If
`.git/config` cannot be read or no origin URL is found, use the basename of the working
directory as the fallback value for `repo`.

Construct the event object and call `write_file` with path
`.moeb/events/<signal_id>.event.json`, pretty-printed with 2-space indentation:

```json
{
  "type": "signal.accepted",
  "event_id": "<uuid-v4>",
  "signal_id": "<signal_id from Phase 2>",
  "repo": "<repo URL or directory basename>",
  "branch": "<fix_branch from Phase 3>",
  "timestamp": "<current ISO 8601 timestamp>"
}
```

Record a StepMetric for `step_id: "emit-event"`.

### Per-Step Review Sub-Loop (event artifact)

0. **Error preflight.** If the preceding write returned an error (result contains
   "failed", "error", or "could not"): Record StepMetric with
   `step_id: "event-artifact"`, `iteration_count = 0`, `acceptance_rate = 1.0`,
   `delta_scores = []`. Do NOT call `complete_review`. Proceed to Phase 5b. Skip steps 1–8.

1. **Inline Reviewer.** Without calling any tool, adopt the Reviewer Persona. Evaluate
   the event artifact: the JSON must contain `type`, `event_id`, `signal_id`, `repo`,
   `branch`, and `timestamp` fields with correct values. Produce a unified diff if any
   required field is missing or malformed, or an empty string if correct.

2. If the diff is empty or whitespace: terminate sub-loop. Record StepMetric with
   `step_id: "event-artifact"`, `iteration_count = 0`, `acceptance_rate = 1.0`,
   `delta_scores = []`.

3. **Inline Moderator.** Without calling any tool, adopt the Moderator Persona. Produce
   JSON: `{ "accepted": bool, "delta_score": float, "rationale": string }`.

4. Parse Moderator JSON. If `[PARSE_WARNING]`: treat as
   `{ "accepted": false, "delta_score": 0.0, "rationale": "Moderator parse failure" }`.

5. If `accepted = true` and `delta_score >= 0.01` and `iteration_count < 2`:
   - Apply the patch: call `patch_file` with `old_string` set to the content being
     replaced and `new_string` set to the updated content. If `patch_file` returns an
     error, call `write_file` with the complete updated artifact content immediately —
     do not retry `patch_file`.
   - If the write tool returns an error result, terminate the sub-loop immediately —
     append `{ "delta_score": 0.0, "accepted": false }` to history; proceed to step 6.
   - Otherwise: append `{ "delta_score": <score>, "accepted": true }` to history;
     return to step 1.

6. Otherwise: terminate sub-loop.

7. Record StepMetric with `step_id: "event-artifact"`.

8. Call `complete_review` with `.moeb/events/<signal_id>.event.json`.

## Phase 5b — Budget Breach Check

Call `get_run_status` and inspect the `budget breaches:` section of the output.

For each breach listed, apply the granularity rule:
- If any breach has `level=tool`: emit one tool-level signal per unique (phase, turn) combination.
- Else if any breach has `level=phase`: emit one phase-level signal per unique phase.
- Else if any breach has `level=run`: emit one run-level signal.

Use the same signal templates defined in run.skill.md's Budget Breach Check section.
Write each signal via the Canonical Signal Dedup-and-Write Procedure in Phase 7.

## Phase 6 — Verify

Call `enter_phase` with `phase_id: "phase-6"` as the first action in this phase.

Evaluate the fix_signal workflow for rubric compliance. Supply verdicts for all criteria
from the injected rubric baseline and the specification's `## Rubric / ### Structured`
table. Do not supply verdicts for kernel-authoritative criteria (`tool-errors`,
`rubric-qualification`). Call `verify_rubrics` with the complete verdict list.

## Phase 7 — End-of-Skill Review

Call `enter_phase` with `phase_id: "phase-7"` as the first action in this phase.

Without calling any tool, adopt the **QA Architect Persona** pre-loaded in your context.
Evaluate the fix_signal workflow outputs (signal marking patch, branch creation, event
artifact, accumulated StepMetrics). Produce a `ReviewSignalReport` JSON matching the
schema defined in the QA Architect Persona. Hold the result in working memory.

Parse the returned `ReviewSignalReport` JSON:

1. Assign each signal:
   - `signal_id`: a fresh UUID v4
   - `run_id`: the current run identifier (`{{run_id}}`)
   - `timestamp`: ISO 8601 current time

Write each signal via the **Canonical Signal Dedup-and-Write Procedure** defined below.

## Canonical Signal Dedup-and-Write Procedure

# Identity key: "<category>-<title-slug>"  (slug = lowercase, [^a-z0-9]+ → '-', strip edges, max 80 chars)
# Canonical path: .moeb/signals/catalogue/<signal_id>.signal.json
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
       Write canonical record to `.moeb/signals/catalogue/<signal_id>.signal.json`.
  4. If found:
       Parse existing record E.
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

    `| <signal_id> | <title> | <category> | <severity> | <status> | <occurrence_count> | <last_seen> | [catalogue/<signal_id>.signal.json](catalogue/<signal_id>.signal.json) |`

2. Do not fail or abort the skill if critical signals are present. Continue to the
   Metrics Recording phase.

## Phase 7a — Push Thinking Blocks

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

After `push_thinking_blocks` returns, proceed to Phase 7b — Reasoning Review.

## Phase 7b — Reasoning Review

> **MCP/inline mode note:** Do not call `query_agent` in this phase. When running
> inline (via `start_spec` or `start_run` MCP tools), the agent IS the model and must
> use only inline persona reasoning. The Reasoning Reviewer Persona is pre-loaded in
> your context for exactly this purpose.

This phase runs when `{{no_review}}` is `"false"` (the default). Skip ONLY if
`{{no_review}}` is the exact string `"true"`.

This review is advisory — its findings are emitted as signals to the improvement
queue but do not affect the rubric verdict or run pass/fail.

1. If `RunState.thinking_blocks` is empty, skip this phase entirely and proceed to
   Phase 8.

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

Continue to Phase 8 — Metrics Recording regardless of signal count.

## Phase 8 — Metrics Recording

Call `enter_phase` with `phase_id: "phase-8"` as the first action in this phase.

Assemble `RunMetrics`:
- `run_id`: `{{run_id}}`
- `timestamp`: ISO 8601 run-start time
- `step_metrics`: StepMetric records from all steps
- `wall_time_ms`: elapsed milliseconds since fix_signal run start
- `rubric_score` and `end_review_error_count`: written by kernel — do NOT compute
- `tools_used`: sorted array of tool names from `get_run_status`.
- `token_usage`: object with fields `input_tokens`, `output_tokens`, `cache_read_tokens`, `cache_creation_tokens`, `total_tokens` — read from the `token usage:` line in `get_run_status` output.

Write to `.moeb/metrics/{{run_id}}.metrics.json`.

Write the run file to `{{run_file_path}}`:
```json
{
  "run_id": "{{run_id}}",
  "timestamp": "<ISO 8601 start time>",
  "command": "fix_signal",
  "event_path": ".moeb/events/<signal_id>.event.json",
  "metrics_path": ".moeb/metrics/{{run_id}}.metrics.json",
  "emittedSignals": [
    { "id": "<signal_id_1>", "description": "<title_1>" }
  ],
  "rubric_score": "<from verify_rubrics output>",
  "end_review_error_count": "<count of Critical signals>",
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

## Commit

Call `enter_phase` with `phase_id: "phase-commit"` as the first action in this phase.

Call `git_commit` with:
- `kind`: `"run"`
- `domain`: `"moeb"`
- `slug`: `"fix-signal"`

This commit is unconditional. It executes regardless of whether QA produced errors,
capturing all changes made during the fix_signal run.

## Tag

Call `enter_phase` with `phase_id: "phase-tag"` as the first action in this phase.

Call `tag_run` with:
- `run_id`: `{{run_id}}`
- `domain`: `"moeb"`
- `slug`: `"fix-signal"`

This tag is created unconditionally — even when `end_review_error_count` is non-zero —
providing a durable git trace for every fix_signal invocation including failed runs.

## Signal Tag

Call `enter_phase` with `phase_id: "phase-signal-tag"` as the first action in this phase.

Call `tag_signal` with:
- `signal_id`: the ID of the signal selected during Phase 2
- `event_id`: the `event_id` field from the emitted event artifact
  (`.moeb/events/<signal_id>.event.json`)

This creates an annotated git tag `signal/<signal_id>` with annotation body
`fix_signal selected <signal_id> event:<event_id>` on the isolation branch,
making the signal selection decision and its corresponding event traceable in git history.

## Complete

Call `enter_phase` with `phase_id: "phase-complete"` as the first action in this phase.

Derive the compact ISO timestamp as `yyyyMMddTHHmmssZ` from the current time (e.g.
`20260604T132045Z`). Construct the path `.moeb/events/{compact_timestamp}_{signal_id}.event.json`.
Write using `write_file`:

```json
{
  "event_type": "signal_fixed",
  "signal_id": "<signal_id from run context>",
  "fix_branch": "<branch name from the branch-creation phase>",
  "timestamp": "<current ISO 8601 timestamp>"
}
```

Respond with a concise summary: which signal was selected (signal_id, title, severity),
which branch was created (Phase 4), which event was emitted (path and type), and whether
the run completed successfully.
