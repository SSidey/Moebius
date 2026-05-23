---
review: true
---
The harness README, specification schema, and rubrics catalogue are already in your
context. Do not re-read any of them.

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
   - Apply the diff: `patch_file` with the proposed diff on the spec file path.
   - Append `{ "delta_score": <score>, "accepted": true }` to iteration history.
   - Return to step 1.

6. Otherwise: terminate sub-loop.

7. Record StepMetric for `step_id: "spec-file"`.

8. Call `complete_review` with the spec file path (`.moeb/specifications/<domain>/<domain>.<slug>.md`).
   This is a no-op at the kernel level (path starts with `.moeb/`) but makes the review
   obligation explicit and maintains symmetry with the run skill.

## Phase 5 — Branch

After the file is written, call `create_branch` with `domain` and `slug` extracted from
the frontmatter. The tool creates the `feat/<domain>-<slug>` branch per Conventional
Branch 1.0.0.

## Phase 6 — Link README

Read `.moeb/README.md` using `read_file`. Locate the `### <domain>` section (create it if
absent). Append a new table row for this specification using `patch_file` on `.moeb/README.md`:

```
| <Title> | <one-sentence description> | [specifications/<domain>/<domain>.<slug>.md](specifications/<domain>/<domain>.<slug>.md) | active |
```

Use `patch_file` with a minimal unified diff targeting only the insertion point. The
table row must include the Status column with value `active`.

### Per-Step Review Sub-Loop (README patch)

Skip this sub-loop entirely if `{{no_review}}` is `"true"`.

After patching `.moeb/README.md`:

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

4. Parse Moderator JSON. Apply diff via `patch_file` if `accepted = true` and
   `delta_score >= 0.01` and `iteration_count < 2`. Record StepMetric for
   `step_id: "readme-link"`.

5. Call `complete_review` with `.moeb/README.md`.
   This is a no-op at the kernel level (path starts with `.moeb/`) but makes the review
   obligation explicit and maintains symmetry with the run skill.

## Phase — End-of-Skill Review

Skip this phase entirely if `{{no_review}}` is `"true"`.

Without calling any tool, adopt the **QA Architect Persona** pre-loaded in your context.
Evaluate the spec file, README.md, their contents, and the accumulated StepMetrics as
JSON. Produce a ReviewSignalReport JSON matching the schema defined in the QA Architect
Persona. Hold the result in working memory and continue with parsing and signal processing
below.

If `[PARSE_WARNING]` prefix is present in the response, treat it as a Critical error
signal: append a signal with title "QA Architect response parse failure" and description
containing the raw response, then continue.

Parse the returned `ReviewSignalReport` JSON:

1. For each signal with `severity = "Critical"`:
   a. If `auto_generated` is set in the run context, skip `start_spec` calls entirely
      (recursion guard — resolution specs do not generate further resolution specs).
   b. Otherwise call `start_spec` with `proposed_resolution` as the requirement and
      record the resulting spec path in the signal as `auto_spec_path`.

2. Assign `signal_id`, `run_id`, `timestamp` to each signal.

3. Write signals to `.moeb/signals/<run_id>.signals.json`.

4. Continue to Metrics Recording regardless of critical signal presence.

## Phase — Metrics Recording

1. Assemble `RunMetrics`:
   - `run_id`: the current run identifier
   - `timestamp`: ISO 8601 run-start time
   - `rubric_score`: 1.0 (spec runs succeed or are retried; no rubric scoring)
   - `step_metrics`: all StepMetric records from spec-file and readme-link steps
   - `end_review_error_count`: count of Critical signals (0 when `{{no_review}}` is `"true"`)
   - `wall_time_ms`: elapsed milliseconds since run start

2. Write to `.moeb/metrics/<run_id>.metrics.json`.

3. Load last `{{metrics_window}}` metrics files. If fewer than 2 exist, skip regression detection.

4. If `rubric_score < rolling_avg * (1 - {{metrics_degradation_margin}})`: append a
   DegradationSignal to the signals file.

5. Emit `MetricsEvent { metrics: <RunMetrics> }` to the trace.

## Phase 7 — Commit

Call `git_commit` with:
- `spec_path`: `.moeb/specifications/<domain>/<domain>.<slug>.md`
- `readme_path`: `.moeb/README.md`
- `domain` and `slug` from frontmatter.
