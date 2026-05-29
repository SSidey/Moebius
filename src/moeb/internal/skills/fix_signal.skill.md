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

Call `list_directory` on `.moeb/signals/`. For each entry ending with `.signals.json`,
call `read_file` on that path. Parse the JSON array. Collect every signal object where
`picked_up_at` is absent or null. Record each signal alongside its source file path.

If no `.signals.json` files exist or all signals have `picked_up_at` set, skip to
Phase 12 with the message: `"No unresolved signals found."`.

## Phase 2 — Select

Assign each unresolved signal a numeric severity rank: `"Critical"` → 2, `"Major"` → 1,
any other value → 0. Sort by severity rank descending, then by `timestamp` ascending
(oldest first among equal severity). Select the first signal after sorting. Record its
`signal_id`, `severity`, `title`, `proposed_resolution`, `description`, and source file
path.

## Phase 3 — Mark

Compute `fix_branch` before patching:
1. Derive `domain`: if `category` is `"Error"`, use `"fix"`; otherwise use `"moeb"`.
2. Derive `slug`: lowercase `title`, replace non-`[a-z0-9-]` chars with hyphens,
   collapse consecutive hyphens, strip leading/trailing hyphens, truncate to 50 chars.
3. `fix_branch` = `"feat/<domain>-<slug>"`.

Call `read_file` on the source file path. Construct a minimal unified diff adding
`"picked_up_at": "<current ISO 8601 timestamp>"` and `"fix_branch": "<fix_branch>"` as
new fields after the last existing field of the chosen signal object, before its closing
`}`. Call `patch_file` with this diff.

If `patch_file` returns an error, add the two fields in memory and call `write_file` to
overwrite the source file.

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
   - Call `patch_file` on the signal file path with the proposed diff.
   - On error: terminate; append `{ "delta_score": 0.0, "accepted": false }` to history;
     proceed to step 6.
   - Otherwise: append `{ "delta_score": <score>, "accepted": true }` to history;
     return to step 1.

6. Otherwise: terminate sub-loop.

7. Record StepMetric with `step_id: "signal-pickup"`.

8. Call `complete_review` with the signal file path.

Record a StepMetric for `step_id: "mark-signal"`.

## Phase 4 — Branch

Call `create_branch` with `domain` and `slug` derived in Phase 3.
Record a StepMetric for `step_id: "create-branch"`.

## Phase 5 — Emit Event

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
   `delta_scores = []`. Do NOT call `complete_review`. Proceed to Phase 6. Skip steps 1–8.

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
   - Call `patch_file` on `.moeb/events/<signal_id>.event.json` with the proposed diff.
   - On error: terminate; append `{ "delta_score": 0.0, "accepted": false }` to history;
     proceed to step 6.
   - Otherwise: append `{ "delta_score": <score>, "accepted": true }` to history;
     return to step 1.

6. Otherwise: terminate sub-loop.

7. Record StepMetric with `step_id: "event-artifact"`.

8. Call `complete_review` with `.moeb/events/<signal_id>.event.json`.

## Phase 6 — Verify

Evaluate the fix_signal workflow for rubric compliance. Supply verdicts for all criteria
from the injected rubric baseline and the specification's `## Rubric / ### Structured`
table. Do not supply verdicts for kernel-authoritative criteria (`tool-errors`,
`rubric-qualification`). Call `verify_rubrics` with the complete verdict list.

## Phase 7 — End-of-Skill Review

Act as a QA Architect: evaluate the fix_signal workflow outputs (signal marking patch,
branch creation, event artifact, accumulated StepMetrics). Produce a
`ReviewSignalReport` JSON with four categories (`"Error"`, `"SkillImprovement"`,
`"ToolImprovement"`, `"NewCapability"`) and a `"summary"` paragraph. Each signal must
carry `signal_id` (UUID v4), `run_id`, `timestamp`, `category`, `severity`, `title`,
`description`, `proposed_resolution`, and `gating_condition`. Write the complete array
to `.moeb/signals/{{run_id}}.signals.json`.

## Phase 8 — Metrics Recording

Assemble `RunMetrics`:
- `run_id`: `{{run_id}}`
- `timestamp`: ISO 8601 run-start time
- `step_metrics`: StepMetric records from all steps
- `wall_time_ms`: elapsed milliseconds since fix_signal run start
- `rubric_score` and `end_review_error_count`: written by kernel — do NOT compute

Write to `.moeb/metrics/{{run_id}}.metrics.json`.

Write the run file to `{{run_file_path}}`:
```json
{
  "run_id": "{{run_id}}",
  "timestamp": "<ISO 8601 start time>",
  "command": "fix_signal",
  "event_path": ".moeb/events/<signal_id>.event.json",
  "signals_path": ".moeb/signals/{{run_id}}.signals.json",
  "metrics_path": ".moeb/metrics/{{run_id}}.metrics.json",
  "rubric_score": "<from verify_rubrics output>",
  "end_review_error_count": "<count of Critical signals>"
}
```

## Commit

Call `git_commit` with:
- `kind`: `"run"`
- `domain`: `"moeb"`
- `slug`: `"fix-signal"`

This commit is unconditional. It executes regardless of whether QA produced errors,
capturing all changes made during the fix_signal run.

## Tag

Call `tag_run` with:
- `run_id`: `{{run_id}}`
- `domain`: `"moeb"`
- `slug`: `"fix-signal"`

This tag is created unconditionally — even when `end_review_error_count` is non-zero —
providing a durable git trace for every fix_signal invocation including failed runs.

## Signal Tag

Call `tag_signal` with:
- `signal_id`: the ID of the signal selected during Phase 2
- `event_id`: the `event_id` field from the emitted event artifact
  (`.moeb/events/<signal_id>.event.json`)

This creates an annotated git tag `signal/<signal_id>` with annotation body
`fix_signal selected <signal_id> event:<event_id>` on the isolation branch,
making the signal selection decision and its corresponding event traceable in git history.

## Complete

Respond with a concise summary: which signal was selected (signal_id, title, severity),
which branch was created (Phase 4), which event was emitted (path and type), and whether
the run completed successfully.
