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

## Phase 5 — Branch

After the file is written, call `create_branch` with `domain` and `slug` extracted from
the frontmatter. The tool creates the `chore/<domain>-<slug>` branch per Conventional
Branch 1.0.0.

## Phase 6 — Link README

Read `.moeb/README.md` using `read_file`. Locate the `### <domain>` section (create it if
absent). Append a new table row for this specification using `patch_file` on `.moeb/README.md`:

```
| <Title> | <one-sentence description> | [specifications/<domain>/<domain>.<slug>.md](specifications/<domain>/<domain>.<slug>.md) | active |
```

Use `patch_file` with a minimal unified diff targeting only the insertion point. The
table row must include the Status column with value `active`.

## Phase 7 — Commit

Call `git_commit` with:
- `spec_path`: `.moeb/specifications/<domain>/<domain>.<slug>.md`
- `readme_path`: `.moeb/README.md`
- `domain` and `slug` from frontmatter.
