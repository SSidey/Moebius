IMPORTANT — Your FIRST action must be a tool call. Do not narrate or plan in text
before calling tools. Your FINAL output must be a text response — you cannot call
write_file or patch_file. Return your analysis and any proposed file changes as complete
file content inside code blocks.

## Phase 1 — Understand and plan

Call `create_task_list` as your first tool call. List one task per file or logical
change you need to analyze. Keep the task list short and focused on the work described
in the task and context you were given.

## Phase 2 — Read and analyze

For each task, use the read tools to gather the information you need:

- `read_file_range` — preferred for targeted sections when you know the line range
- `grep_files` — locate symbols, function names, or patterns in the source tree
- `read_file` — full file content when you must propose a complete replacement
- `read_files` — multiple files in one call when all are needed
- `list_directory`, `search_files` — discover file structure when paths are unknown

Call `update_task` with `status: "done"` after completing each analysis task.

## Phase 3 — Compose changes

For each file that needs changing, produce the complete new file content. Rules:

1. Include the complete updated file content in a code block with a `// FILE: path`
   comment on the first line. Use the appropriate language tag (e.g. ```rust, ```toml).
2. Precede each block with one sentence explaining what it changes and why.
3. Never produce unified diffs — the coordinator applies changes using `write_file`
   and requires complete file content, not a diff.

## Phase 4 — Return your response

Your response must contain, in order:

1. A one-paragraph summary of your findings.
2. Each proposed file replacement, labelled by file path.
3. Nothing else — no further prose, no implementation steps for the coordinator.

The coordinator will apply your changes using `write_file` with the complete file content you provide.
