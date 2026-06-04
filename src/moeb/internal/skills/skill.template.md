---
review: true
---
# SKILL TEMPLATE
#
# Copy this file and fill in the command-specific phases below.
# Mandatory phases (marked MANDATORY) must appear in every skill with these exact
# heading names — the skill-mandatory-phases rubric criterion checks for them by
# searching the loaded skill context for: "Verify", "End-of-Skill Review",
# "Metrics Recording", "Commit", "Tag", "Complete".
#
# Skill-specific VCS phases (e.g. Branch for spec, Version and Tag for run) go in
# the [SKILL-SPECIFIC VCS] slot between Tag and Complete.
#
# Delete all comment lines (lines beginning with #) before deploying as a real skill.

## [Command-Specific Phases]

# Replace this section with the phases that perform the command's actual work.
# Example for a two-phase implementation command:
#
# ## Phase 1 — Scope
#   (list_directory, search_files, grep_files, read_file_range)
#
# ## Phase 2 — Implement
#   (write_file / patch_file + Per-Step Review Sub-Loop)

## Phase N — Verify                                            <!-- MANDATORY -->

Collect all rubric criteria from two sources:
1. The injected `{{command_rubrics}}` section — every row is mandatory.
2. The specification's `## Rubric / ### Structured` table — every row is mandatory.

Evaluate pass, fail, or na for each criterion. Call `verify_rubrics` with the complete
list. Do not call with a partial list.

Note: `tool-errors` and `rubric-qualification` are kernel-authoritative — do not
supply verdicts for these; any agent-supplied values are overridden.

## Phase N+1 — End-of-Skill Review                            <!-- MANDATORY -->

Without calling any tool, adopt the **QA Architect Persona** pre-loaded in your context.
Evaluate all artifacts produced in this run, accumulated StepMetrics, and the
specification's full `## Rubric` section. Produce a ReviewSignalReport JSON matching
the schema defined in the QA Architect Persona. Hold the result in working memory.

If `[PARSE_WARNING]` prefix is present, treat as Critical error signal with title
"QA Architect response parse failure" and continue.

Parse the ReviewSignalReport JSON:
1. Assign `signal_id` (UUID v4), `run_id`, `timestamp` to each signal.
2. Write the complete array to `.moeb/signals/{{run_id}}.signals.json`.
3. Continue regardless of critical signal presence.

## Phase N+2 — Metrics Recording                              <!-- MANDATORY -->

### Part A — Record

1. Assemble RunMetrics:
   - `run_id`: current run identifier
   - `timestamp`: ISO 8601 run-start time
   - `rubric_score`: kernel-written from RunState — do NOT compute or write this field
   - `step_metrics`: all StepMetric records accumulated across steps
   - `end_review_error_count`: kernel-written from RunState — do NOT compute or write
   - `wall_time_ms`: elapsed milliseconds since run start

2. Write to `.moeb/metrics/{{run_id}}.metrics.json`.

3. Write run file to `{{run_file_path}}`:
   ```json
   {
     "run_id": "{{run_id}}",
     "timestamp": "<ISO 8601 start time>",
     "command": "<command name>",
     "spec_path": "<path of the spec file>",
     "signals_path": ".moeb/signals/{{run_id}}.signals.json",
     "metrics_path": ".moeb/metrics/{{run_id}}.metrics.json",
     "rubric_score": <from verify_rubrics output>,
     "end_review_error_count": <count of Critical signals>
   }
   ```

### Part B — Regression Detection

4. Load last `{{metrics_window}}` `.metrics.json` files from `.moeb/metrics/` ordered
   by `timestamp` ascending. If fewer than 2 exist, skip regression detection.

5. Compute `rolling_avg = mean(rubric_score for each loaded file)`.

6. If `rubric_score < rolling_avg * (1 - {{metrics_degradation_margin}})`:
   Append a DegradationSignal to `.moeb/signals/{{run_id}}.signals.json`:
   ```json
   {
     "signal_id": "<UUID v4>",
     "run_id": "{{run_id}}",
     "timestamp": "<ISO 8601>",
     "category": "Error",
     "severity": "Critical",
     "title": "Rubric score degraded below rolling baseline",
     "description": "rubric_score is more than {{metrics_degradation_margin}}% below the rolling average.",
     "proposed_resolution": "Revert to the git tag preceding this run, diagnose the regression cause, and branch from the pre-regression version.",
     "gating_condition": null
   }
   ```

7. Emit `MetricsEvent { metrics: <RunMetrics> }` to the trace.

## Phase N+3 — Commit                                         <!-- MANDATORY -->

Call `git_commit` with the appropriate parameters for this command (kind, domain, slug,
and any additional fields required by the command). Stages all working-tree changes and
commits with the conventional message for this command type.

## Phase N+4 — Tag                                            <!-- MANDATORY -->

Call `tag_run` with:
- `run_id`: `{{run_id}}`
- `domain`: value of `domain` from the spec frontmatter
- `slug`: value of `slug` from the spec frontmatter

The tool creates annotated git tag `run/{{run_id}}` with annotation
`moeb <domain>/<slug> @ <ISO 8601 timestamp>`.

## [Skill-Specific VCS Phases]                                <!-- OPTIONAL SLOT -->

# Insert any command-specific VCS phases here, between Tag and Complete.
# Examples:
#   - Branch (create_branch) — for spec command
#   - Version and Tag (bump_version + create_candidate_tag) — for run command
# Omit this section if the command has no skill-specific VCS steps.

## Phase N+5 — Complete                                       <!-- MANDATORY -->

Respond with a concise summary of every file created or updated during this run.
