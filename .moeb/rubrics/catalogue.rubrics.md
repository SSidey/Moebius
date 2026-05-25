# Rubrics Catalogue

A trait-keyed catalogue of conditionally applicable rubric criteria. Entries are selected
by the spec agent based on traits detected in the raw requirement and description of the
specification being authored. They are not auto-injected — selection is agent-driven.

Each entry carries:
- **Domain** — the harness domain this criterion belongs to.
  Used as a coarse pre-filter before trait matching; agents skip entries whose domain
  is unrelated to the specification being authored.
- **Traits** — comma-separated tags that trigger selection within the domain (e.g. `ai-adapter`).
- **Applies At** — comma-separated list of command names (e.g. `spec`, `run`, `spec, run`)
  or the special value `All` (every moeb command). Governs how the entry is applied per
  command: `spec` governs spec authoring quality for this invocation; any other command
  name causes the row to be copied into the spec's `## Rubric` section for that command's
  agent to verify during implementation.

To retire a criterion, set its `status` to `superseded` and add a note identifying the
replacement. Do not delete rows.

## Criteria

| id | Name | Description | Threshold | Pass Condition | Domain | Traits | Applies At | Status |
|----|------|-------------|-----------|----------------|--------|--------|------------|--------|
| `adapter-structural-parity` | Adapter implementations are structurally identical | `AnthropicAdapter::send` and `OpenAiAdapter::send` follow the same retry loop skeleton; only API-specific serialisation differs | Identical structure | Code review of both adapter files side-by-side finds no structural asymmetry | `moeb` | `ai-adapter` | `run` | active |
| `thin-kernel` | New Rust code in tools/, commands/, or domain/ must be either (a) a primitive operation wrapper (filesystem, git, or HTTP with no domain logic) or (b) a thin dispatcher that calls into skills, tools, or agents and returns the result unchanged. Workflow logic — sequencing, conditionals, review loops, retry strategy, branching decisions — must live in skill markdown or role files, not in compiled Rust. If the same capability can be achieved by updating a skill file, that path is mandatory. | Zero workflow-logic functions in new Rust | Spec review: no Step proposes placing sequencing or conditional workflow logic in Rust when a skill-file change would suffice; Run review: every new function in tools/ or commands/ either wraps a single OS/VCS/HTTP call or contains no domain-specific branching | `moeb` | `kernel-change` | `spec, run` | active |
| `mcp-cli-behavioural-parity` | For any operation exposed through both the CLI agent loop and the MCP server, the observable outcome must be identical: same file mutations, same tool result string format, same rendered prompt template. No MCP-only or CLI-only code paths may alter the final output. Any tool registered in the standard registry must be registered in the MCP registry with an identical input schema and result contract. | Zero divergent outcomes | Run review: for every tool added or modified, both ToolRegistry::standard() and the MCP server carry identical registrations; start_run and domain/run.rs render from the same run.prompt template with the same rubric layers; start_spec and domain/spec.rs render from the same spec.prompt template | `moeb` | `mcp-surface` | `run` | active |
| `new-skill-mandatory-phases` | New Skill Mandatory Phases | Any `*.skill.md` file written during this run contains all mandatory phase heading strings: "Verify", "End-of-Skill Review", "Metrics Recording", "Commit", "Tag", "Complete" | All six strings present in the written skill file | moeb | skill-authoring | run | active |
