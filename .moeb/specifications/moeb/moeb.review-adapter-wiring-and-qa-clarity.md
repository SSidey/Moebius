---
domain: moeb
slug: review-adapter-wiring-and-qa-clarity
status: active
---

# Review Sub-Agent Adapter Wiring and QA Review Clarity

## Raw Requirement

Fix two observed failures in the review system:

1. Step Review (query_agent) uses the wrong adapter in MCP/serve mode. When moeb runs
   as an MCP server (`moeb serve`), the `QueryAgentTool` is registered with
   `adapter: None`, causing it to fall back to `DefaultAdapterFactory.build()` on every
   call. If the moeb config has `active_adapter = "openai"` but the user is running
   Claude Code as the external coordinator expecting Anthropic, sub-agent calls
   (reviewer, moderator, qa-architect) go to OpenAI and fail with quota errors. The fix:
   in `serve.rs`, build the configured adapter eagerly at MCP server startup (using
   `DefaultAdapterFactory` with a noop trace, consistent with the existing
   `resolve_adapter` fallback) and pass it to the MCP executor so `QueryAgentTool` gets
   `adapter: Some(...)` rather than `None`.

2. The End-of-Skill QA review (qa-architect phase in `run.skill.md`) is ambiguous: the
   condition "Skip this phase entirely if `{{no_review}}` is `true`" — after substitution
   with `"false"` — becomes "Skip if `false` is `true`", a double-negative that can
   confuse the executing LLM agent. Additionally, `start_run.rs` (MCP path) hardcodes
   `{{no_review}} = "false"` without consulting the skill file's `review:` frontmatter
   flag, while `domain/run.rs` (CLI path) correctly uses `load_skill_review_flag`. The
   fix: rephrase the review skip conditions in `run.skill.md` to lead with an affirmative
   rule, and align `start_run.rs` to use `load_skill_review_flag` identically to the CLI
   path.

## Description

This specification fixes two related failures in the review sub-system introduced by
`moeb.composable-review-wrapper.md` and `moeb.query-agent-tool.md`.

**Failure 1 — MCP adapter gap.** `RealToolExecutor::new_mcp` creates a `QueryAgentTool`
with `adapter: None`. The `resolve_adapter` fallback calls `DefaultAdapterFactory.build()`
at each invocation, reading `active_adapter` from the moeb config. In the MCP serve
path (`moeb serve`), the external coordinator (Claude Code) is typically Anthropic, but
moeb's `query_agent` sub-agents use whatever adapter moeb has configured locally. If
that adapter is `openai` and the user has exceeded their OpenAI quota, every reviewer,
moderator, and qa-architect call fails with a 429 error. The fix introduces
`RealToolExecutor::new_mcp_with_adapter` and `ToolRegistry::mcp_with_adapter` — built on
the existing `with_query_agent` path — and updates `serve.rs` to build the configured
adapter eagerly at startup, propagating any configuration error before the server
accepts connections.

**Failure 2 — Review default ambiguity.** Two sub-failures:

- *Double-negative skip condition.* `run.skill.md`'s Per-Step Review Sub-Loop and
  End-of-Skill Review phases both read "Skip entirely if `{{no_review}}` is `'true'`".
  After template substitution with `"false"`, the rendered instruction is "Skip if
  `false` is `true`". An LLM may interpret `no_review = false` as "the review flag is
  false", leading to confusion about whether to execute the phase. The fix rephrases
  both conditions to lead with an affirmative: "This phase runs when `{{no_review}}` is
  `'false'` (the default). Skip ONLY if `{{no_review}}` is the exact string `'true'`."

- *MCP/CLI parity gap in `start_run.rs`.* The `StartRunTool` hardcodes
  `.replace("{{no_review}}", "false")` without reading the skill file's `review:`
  frontmatter flag. `domain/run.rs` (CLI path) computes `effective_no_review =
  no_review || !skill_review_enabled` using `load_skill_review_flag`. If a skill has
  `review: false` in its frontmatter, the MCP path ignores it and always injects
  `"false"`, diverging from CLI behavior. The fix makes `start_run.rs` use
  `load_skill_review_flag` to compute the effective value before injection.

## Backlinks

### Parents

| Label | Path | Purpose |
|-------|------|---------|
| Harness README | README.md | Root index |
| Query Agent Tool | specifications/moeb/moeb.query-agent-tool.md | Introduces QueryAgentTool with adapter: Option wiring |
| Composable Review Wrapper | specifications/moeb/moeb.composable-review-wrapper.md | Defines the review phases and {{no_review}} template variable |
| MCP Stdio Server | specifications/moeb/moeb.mcp-stdio-server.md | Defines the serve.rs RealToolExecutor::new_mcp construction |
| MCP Serve / CLI Parity | specifications/moeb/moeb.serve-cli-parity.md | Defines start_run.rs and the {{no_review}} hardcoded value |
| Agent Skills | specifications/moeb/moeb.agent-skills.md | Defines the skill file frontmatter review: field and load_skill_review_flag |

## Steps

### Step 1 — Add `ToolRegistry::mcp_with_adapter` and `RealToolExecutor::new_mcp_with_adapter`

File: `src/moeb/src/tools/mod.rs`

**1a. Add `ToolRegistry::mcp_with_adapter`.**

Add the following method to `ToolRegistry`, immediately after the existing `mcp` method:

```rust
/// Register the MCP tools (standard tools + start_run + start_spec + get_run_status)
/// with an eagerly wired query_agent adapter.
pub fn mcp_with_adapter(
    state: SharedRunState,
    adapter: std::sync::Arc<dyn crate::ports::AiPort>,
    read_paths: Arc<Mutex<HashSet<String>>>,
) -> Self {
    let mut r = Self::with_query_agent(
        std::sync::Arc::clone(&state),
        std::sync::Arc::clone(&adapter),
        Arc::clone(&read_paths),
    );
    r.register(Box::new(start_run::StartRunTool));
    r.register(Box::new(start_spec::StartSpecTool));
    r.register(Box::new(get_run_status::GetRunStatusTool {
        state: std::sync::Arc::clone(&state),
    }));
    r
}
```

**1b. Add `RealToolExecutor::new_mcp_with_adapter`.**

Add the following method to `RealToolExecutor`, immediately after the existing `new_mcp`
method:

```rust
pub fn new_mcp_with_adapter(
    state: SharedRunState,
    adapter: std::sync::Arc<dyn crate::ports::AiPort>,
) -> Self {
    let read_paths = Arc::new(Mutex::new(HashSet::new()));
    Self {
        registry: ToolRegistry::mcp_with_adapter(
            std::sync::Arc::clone(&state),
            adapter,
            Arc::clone(&read_paths),
        ),
        cache: Mutex::new(HashMap::new()),
        read_paths,
        state,
    }
}
```

### Step 2 — Update `serve.rs` to build adapter eagerly at startup

File: `src/moeb/src/commands/serve.rs`

In the `Transport::Stdio` arm of the `serve` function, replace:

```rust
let executor = RealToolExecutor::new_mcp(state);
```

with adapter-eager construction:

```rust
let adapter = {
    use crate::ports::AdapterFactoryPort;
    use crate::trace::{FileContentMode, TraceCommand, TraceConfig, TraceContext};
    let noop_trace = Arc::new(TraceContext::new(TraceConfig {
        command: TraceCommand::Run,
        spec: String::new(),
        adapter: String::new(),
        model: String::new(),
        retention: 0,
        file_content_mode: FileContentMode::Embed,
    }));
    crate::adapters::DefaultAdapterFactory.build(noop_trace)?
};
let executor = RealToolExecutor::new_mcp_with_adapter(state, adapter);
```

The `?` propagates a configuration error (e.g., missing or unrecognised `active_adapter`)
before the server enters its stdio read loop, giving the user an immediate diagnostic
rather than a failure on the first `query_agent` call.

The HTTP arm is unchanged — it manages its own adapter lifecycle per session.

### Step 3 — Align `start_run.rs` review flag with `domain/run.rs`

File: `src/moeb/src/tools/start_run.rs`

In `StartRunTool::execute`, the skill is already loaded:

```rust
let skill_name = crate::skills::extract_skill_name(&spec_content)
    .unwrap_or_else(|| "run".to_string());
let skill_content = crate::skills::load_skill(&moeb_dir, &skill_name);
```

After these two lines, add the review flag computation that mirrors `domain/run.rs`:

```rust
let skill_review_enabled =
    crate::skills::load_skill_review_flag(&moeb_dir, &skill_name);
let effective_no_review = !skill_review_enabled;
let no_review_value = if effective_no_review { "true" } else { "false" };
```

Then replace the hardcoded substitution:

```rust
.replace("{{no_review}}", "false")
```

with:

```rust
.replace("{{no_review}}", no_review_value)
```

Note: `start_run.rs` has no CLI `--no-review` flag (the tool is always invoked by the
external agent, not the user). The only signal available is the skill frontmatter. The
CLI path in `domain/run.rs` combines both (`no_review || !skill_review_enabled`); here
only the skill flag applies.

### Step 4 — Rephrase review skip conditions in `run.skill.md`

File: `src/moeb/internal/skills/run.skill.md`

There are two skip conditions that use the double-negative pattern. Replace both with
affirmative-first phrasing.

**4a. Per-Step Review Sub-Loop header.**

Replace:

```
Skip this sub-loop entirely if `{{no_review}}` is `"true"`.
```

with:

```
This sub-loop runs when `{{no_review}}` is `"false"` (the default). Skip ONLY if
`{{no_review}}` is the exact string `"true"`.
```

**4b. End-of-Skill Review phase header.**

Replace:

```
Skip this phase entirely if `{{no_review}}` is `"true"`.
```

with:

```
This phase runs when `{{no_review}}` is `"false"` (the default). Skip ONLY if
`{{no_review}}` is the exact string `"true"`.
```

Both replacement strings must appear verbatim — the agent must not paraphrase.

## Decisions

### Decision 1 — Build adapter eagerly in `serve.rs`, not lazily in `resolve_adapter`

**Rationale:** The existing `resolve_adapter` fallback in `QueryAgentTool` already calls
`DefaultAdapterFactory.build()` when `adapter = None`. Keeping that fallback but calling
it once at server startup (in `serve.rs`) rather than on every `query_agent` invocation
makes configuration errors visible immediately, avoids re-reading config per call, and
makes the MCP path structurally consistent with the CLI coordinator path
(`new_coordinator` in `domain/run.rs`).

**Rejected alternative — patch `resolve_adapter` to log the adapter name:** Would
improve observability but not fix the per-call fallback or the parity gap with CLI.

**Rejected alternative — add a `--adapter` flag to `moeb serve`:** Unnecessary;
`DefaultAdapterFactory` already reads the configured adapter from the same config file
used by `moeb run`. A new flag would duplicate the configuration surface.

**Consequences:** If no adapter is configured (`active_adapter` is empty), `moeb serve
--transport stdio` will now fail at startup with "No adapter configured. Run `moeb use
<adapter>` first." This is a breaking change for users who run `moeb serve` without
configuring an adapter and never call `query_agent`. However, the existing tool set
(write_file, patch_file, etc.) remains functional through the standard registry; only
`query_agent` required the adapter. Users who need the old behaviour can still call
`moeb serve` after configuring an adapter.

### Decision 2 — `start_run.rs` uses only the skill `review:` flag; no `--no-review` passthrough

**Rationale:** The `StartRunTool` is a kernel MCP tool called by an external agent, not
by the user directly. There is no ambient `--no-review` flag to thread through. The only
per-skill source of truth is the frontmatter flag. If the external agent needs to
suppress review, it must call a different tool or the skill author must set `review:
false` in the frontmatter.

**Rejected alternative — add a `no_review` input parameter to `start_run`:** Would
allow the external agent to pass the flag, but breaks the "tool inputs are minimal"
principle and requires every caller to know about the flag. Deferred to a future spec if
the use case is validated.

**Consequences:** The MCP path now honours `review: false` in skill frontmatter,
matching CLI behaviour. Users who want per-invocation review suppression via MCP must
use a skill that opts out.

### Decision 3 — Affirmative-first phrasing in `run.skill.md` without renaming `{{no_review}}`

**Rationale:** Renaming `{{no_review}}` to `{{review_enabled}}` would require changes
across the template substitution in `domain/run.rs`, `tools/start_run.rs`, and the
spec.skill.md, creating a wide diff for a naming preference. The confusion is not the
variable name but the single-negative condition that the LLM must evaluate. Prepending
the affirmative case ("This phase runs when `{{no_review}}` is `'false'`") gives the
model the expected execution path before the exception, resolving ambiguity without
renaming.

**Rejected alternative — rename to `{{review_enabled}}`:** Correct semantics but a
larger surface change with no functional benefit beyond naming clarity. Deferred to a
future spec if the ambiguity persists after the phrasing fix.

**Consequences:** The existing `{{no_review}}` token and all substitution logic are
unchanged. Only the prose instructions in `run.skill.md` are updated.

## Rubric

### Structured

| Name | Description | Threshold | Pass Condition |
|------|-------------|-----------|----------------|
| `no-drift` | No decision in this spec contradicts a decision in any linked parent spec | Zero contradictions | Manual review: adapter-eager construction extends the existing `with_query_agent` pattern from `moeb.query-agent-tool.md`; `load_skill_review_flag` is the existing function from `moeb.agent-skills.md`; phrasing change is additive only |
| `spec-schema-compliance` | All required frontmatter fields and body sections are present | 100% | Schema validation exits 0 |
| `mcp-cli-parity` | After this spec, `start_run.rs` and `domain/run.rs` derive `{{no_review}}` from the same source (skill frontmatter flag) | Identical logic | Code review: both paths call `load_skill_review_flag`; the CLI path additionally ORs the `--no-review` CLI flag |
| `adapter-eager-wiring` | `serve.rs` (stdio arm) builds the configured adapter before entering the read loop | Adapter built before first request | Code review: `DefaultAdapterFactory.build(noop_trace)?` appears before `McpServer::new` in the stdio arm |
| `no-http-arm-change` | The HTTP transport arm in `serve.rs` is not modified | Zero changes to the HTTP arm | Diff review: only the stdio arm changes |
| `skill-condition-clarity` | Both review skip conditions in `run.skill.md` use affirmative-first phrasing | Both occurrences updated | Text search: "runs when `{{no_review}}` is `\"false\"`" appears twice in the file |
