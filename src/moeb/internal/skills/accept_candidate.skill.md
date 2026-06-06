---
review: true
---
IMPORTANT — DO NOT narrate, plan, or summarise before calling tools. Your FIRST action
must be a tool call.

## Tool Origin Policy

Prefer moeb tools for every operation. Available tools: read_file, read_files,
read_file_range, write_file, patch_file, grep_files, list_directory, search_files,
create_task_list, update_task, verify_rubrics, complete_review, query_agent, git_commit,
tag_run, create_branch, bump_version, get_version.
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

## Phase 1 — Read Signal

Call `read_file` on `.moeb/signals/catalogue/{{signal_id}}.signal.json`.

If the call returns an error (file not found or unreadable):
1. Assign a fresh UUID v4 as the signal_id for this error signal.
2. Write a Critical signal record to `.moeb/signals/catalogue/<fresh_uuid>.signal.json`:
   ```json
   {
     "signal_id": "<fresh_uuid>",
     "category": "Error",
     "severity": "Critical",
     "title": "accept_candidate: signal not found — {{signal_id}}",
     "description": "accept_candidate was invoked with signal_id={{signal_id}} but no file exists at .moeb/signals/catalogue/{{signal_id}}.signal.json.",
     "proposed_resolution": "Verify the signal_id passed to accept_candidate. Check .moeb/signals/index.md for the correct UUID.",
     "gating_condition": null,
     "status": "open",
     "occurrence_count": 1,
     "first_seen": "<current ISO 8601 timestamp>",
     "last_seen": "<current ISO 8601 timestamp>",
     "occurrences": [{ "timestamp": "<current ISO 8601 timestamp>", "run_id": "{{run_id}}" }]
   }
   ```
3. Update `.moeb/signals/index.md` (append a new row for this error signal).
4. Proceed directly to Phase 9 — Complete with a failure summary. Do not proceed to Phase 2.

## Phase 2 — Resolve

Parse the signal JSON from Phase 1. Apply the following changes in memory:
- Set `"status"` to `"resolved"`.
- Set `"resolved_at"` to the current ISO 8601 timestamp.
- If `{{resolution_summary}}` is non-empty, set `"resolution_summary"` to `{{resolution_summary}}`.

Do not alter any other field (do not overwrite `last_seen`, `occurrences`, or `picked_up_at`).

Write the updated signal back to `.moeb/signals/catalogue/{{signal_id}}.signal.json` using `write_file`.

Run the **Index Update sub-procedure**: read `.moeb/signals/index.md`, find the row whose
`Signal ID` cell matches `{{signal_id}}`, update its `Status` cell to `resolved`, then
write the complete updated `index.md` using `write_file`. If no matching row is found,
append a new row with the current signal's field values.

Record a StepMetric with `step_id: "resolve-signal"`, `iteration_count: 0`,
`acceptance_rate: 1.0`, `delta_scores: []`.

## Phase 3 — Emit Resolved Event

Construct the following JSON object:
```json
{
  "type": "signal.resolved",
  "signal_id": "{{signal_id}}",
  "resolved_at": "<resolved_at timestamp from Phase 2>",
  "timestamp": "<current ISO 8601 timestamp>"
}
```

Call `write_file` with path `.moeb/events/{{signal_id}}.resolved.event.json` and the
pretty-printed (2-space indent) JSON as content.

Record a StepMetric with `step_id: "emit-resolved-event"`, `iteration_count: 0`,
`acceptance_rate: 1.0`, `delta_scores: []`.

## Phase 4 — Verify

Evaluate all rubric criteria from the injected rubric baseline. Supply verdicts for all
mandatory criteria. Do not supply verdicts for kernel-authoritative criteria
(`tool-errors`, `rubric-qualification`). Call `verify_rubrics` with the complete list
of verdicts.

## Phase 5 — End-of-Skill Review

Without calling any tool, adopt the **QA Architect Persona** pre-loaded in your context.
Evaluate the signal status update (Phase 2), the event emission (Phase 3), and the
accumulated StepMetrics. Produce a `ReviewSignalReport` JSON matching the schema defined
in the QA Architect Persona. Hold the result in working memory.

Parse the returned `ReviewSignalReport` JSON. Assign each signal:
- `signal_id`: a fresh UUID v4
- `run_id`: `{{run_id}}`
- `timestamp`: ISO 8601 current time

Process signals via the **Canonical Signal Dedup-and-Write Procedure** defined below.
Then run the Index Update sub-procedure.

Set working memory `qa_passed = true` if no signal has `"severity": "Critical"`;
otherwise `qa_passed = false`.

## Canonical Signal Dedup-and-Write Procedure

# Identity key: "<category>-<title-slug>"  (slug = lowercase, [^a-z0-9]+ → '-', strip edges, max 80 chars)
# Canonical path: .moeb/signals/catalogue/<signal_id>.signal.json
# Severity order: Info < Minor < Major < Critical
# current_run_id: {{run_id}}
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

After all catalogue writes complete, add each emitted signal as { "id": signal_id, "description": title } to the emittedSignals array in the run file (written in Phase 6 — Metrics Recording).

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
  - If found: replace that row with updated values for `Severity`, `Status`,
    `Occurrences`, and `Last Seen`, then write the complete updated file using `write_file`.
  - If not found: append a new row, then write the complete updated file using `write_file`:

    `| <signal_id> | <title> | <category> | <severity> | <status> | <occurrence_count> | <last_seen> | [catalogue/<signal_id>.signal.json](catalogue/<signal_id>.signal.json) |`

## Phase 6 — Metrics Recording

Call `get_run_status` immediately before writing to capture `tools_used` and `token_usage`.

Write the `RunMetrics` object to `.moeb/metrics/{{run_id}}.metrics.json`:
```json
{
  "run_id": "{{run_id}}",
  "timestamp": "<ISO 8601 run-start time>",
  "step_metrics": [<StepMetric records from Phases 2 and 3>],
  "wall_time_ms": <elapsed milliseconds since run start>,
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

Write the run file to `{{run_file_path}}`:
```json
{
  "run_id": "{{run_id}}",
  "timestamp": "<ISO 8601 run-start time>",
  "command": "accept_candidate",
  "signal_id": "{{signal_id}}",
  "event_path": ".moeb/events/{{signal_id}}.resolved.event.json",
  "metrics_path": ".moeb/metrics/{{run_id}}.metrics.json",
  "emittedSignals": [
    { "id": "<signal_id_1>", "description": "<title_1>" }
  ],
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

## Phase 7 — Commit

No implementation source files are modified; all changes are harness artefacts in
`.moeb/`. No `git_commit` call is made in this phase.

## Phase 8 — Tag

Call `tag_run` with:
- `run_id`: `{{run_id}}`
- `domain`: `"signal"`
- `slug`: `"accept"`

The tool creates an annotated git tag `run/{{run_id}}` with annotation
`moeb signal/accept @ <timestamp>`.

## Phase 9 — Complete

Respond with a concise summary:
- Which signal was accepted: `signal_id`, `title`, and previous status.
- The `resolved_at` timestamp set in Phase 2.
- The path of the emitted resolved event (`.moeb/events/{{signal_id}}.resolved.event.json`).
- Whether the run completed successfully or exited early due to a missing signal file.
