---
review: true
---
IMPORTANT — DO NOT narrate, plan, or summarise before calling tools. Your FIRST action
must be a tool call.

## Tool Origin Policy

Prefer moeb tools for every operation. Available tools: read_file, read_files,
read_file_range, write_file, patch_file, grep_files, list_directory, search_files,
create_task_list, update_task, verify_rubrics, complete_review, query_agent, git_commit,
tag_run, create_branch, bump_version, get_version, start_spec, start_run.
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

Record a StepMetric for `step_id: "mark-signal"`.

## Phase 4 — Branch

Call `create_branch` with `domain` and `slug` derived in Phase 3.
Record a StepMetric for `step_id: "create-branch"`.

## Phase 5 — Spec

Construct the `requirement` string:
- If `proposed_resolution` is non-null and non-empty: use it as-is.
- Otherwise: concatenate `title + ". " + description`.

Call `start_spec` with this requirement string. **Follow all returned skill phases
completely** — including Author, Write, Per-Step Review sub-loops, Link README, Verify,
End-of-Skill Review, Metrics Recording, Commit, Tag, Branch, and Complete.

After the embedded spec workflow concludes, record the spec file path written
(`.moeb/specifications/<domain>/<domain>.<slug>.md` derived from the spec frontmatter).

## Phase 6 — Run

Call `start_run` with the `spec_path` recorded in Phase 5. **Follow all returned skill
phases completely** — including discovery, implementation, Per-Step Review sub-loops,
Verify, End-of-Skill Review, Metrics Recording, Commit, Version Bump, Tag, and Complete.

## Phase 7 — Verify

Evaluate the fix_signal workflow for rubric compliance. Supply verdicts for all criteria
from the injected rubric baseline and the specification's `## Rubric / ### Structured`
table. Do not supply verdicts for kernel-authoritative criteria (`tool-errors`,
`rubric-qualification`). Call `verify_rubrics` with the complete verdict list.

## Phase 8 — End-of-Skill Review

Act as a QA Architect: evaluate the fix_signal workflow outputs (signal marking patch,
branch creation, embedded spec and run outcomes, accumulated StepMetrics). Produce a
`ReviewSignalReport` JSON with four categories (`"Error"`, `"SkillImprovement"`,
`"ToolImprovement"`, `"NewCapability"`) and a `"summary"` paragraph. Each signal must
carry `signal_id` (UUID v4), `run_id`, `timestamp`, `category`, `severity`, `title`,
`description`, `proposed_resolution`, and `gating_condition`. Write the complete array
to `.moeb/signals/{{run_id}}.signals.json`.

## Phase 9 — Metrics Recording

Assemble `RunMetrics`:
- `run_id`: `{{run_id}}`
- `timestamp`: ISO 8601 run-start time
- `step_metrics`: StepMetric records from mark-signal and create-branch steps
- `wall_time_ms`: elapsed milliseconds since fix_signal run start
- `rubric_score` and `end_review_error_count`: written by kernel — do NOT compute

Write to `.moeb/metrics/{{run_id}}.metrics.json`.

Write the run file to `{{run_file_path}}`:
```json
{
  "run_id": "{{run_id}}",
  "timestamp": "<ISO 8601 start time>",
  "command": "fix_signal",
  "spec_path": "<spec path from Phase 5>",
  "signals_path": ".moeb/signals/{{run_id}}.signals.json",
  "metrics_path": ".moeb/metrics/{{run_id}}.metrics.json",
  "rubric_score": "<from verify_rubrics output>",
  "end_review_error_count": "<count of Critical signals>"
}
```

## Phase 10 — Commit

The fix_signal workflow does not produce implementation artifacts requiring a dedicated
commit. All spec and code commits are issued by the embedded spec and run skill phases.
No `git_commit` call is made in this phase.

## Phase 11 — Tag

Call `tag_run` with:
- `run_id`: `{{run_id}}`
- `domain`: domain derived in Phase 3
- `slug`: slug derived in Phase 3

## Phase 12 — Complete

Respond with a concise summary: which signal was selected (signal_id, title, severity),
which branch was created (Phase 4), which spec was authored (path and domain/slug), and
whether the run completed successfully.
