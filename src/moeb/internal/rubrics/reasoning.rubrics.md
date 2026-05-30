# Reasoning Quality Taxonomy

Criteria used by the Reasoning Reviewer to evaluate the quality of an LLM agent's internal
monologue (thinking blocks) during a run. Each criterion defines the ideal state; any deviation
from the ideal is a candidate signal for the improvement queue.

This file is the canonical, expandable reference for reasoning quality criteria. Add new
criteria here atomically when new friction patterns are identified.

## Criteria

| Name | Description | Ideal | Signal when |
|------|-------------|-------|-------------|
| `decision-linearity` | The agent reaches a conclusion once and commits to it without revisiting closed decisions unless genuinely new information warrants it. | Single decision path per question, no reversals. | Agent states conclusion A, then reverses to B, then reconsiders A — or reopens a decision that was already resolved in earlier thinking. |
| `spec-citation-discipline` | The agent cites the relevant spec instruction once and applies it directly, without re-reading or re-deriving the same constraint. | Constraint identified once, applied immediately. | Agent re-reads, re-quotes, or re-derives the same spec rule two or more times within a single operation's thinking. |
| `tool-selection-directness` | The agent picks the appropriate tool with a single, brief rationale and proceeds. | One tool chosen, one rationale, no debate. | Agent debates two equivalent tools, changes its tool choice mid-reasoning, or re-opens the tool selection question after an initial choice. |
| `scope-containment` | The agent's reasoning stays within the boundaries of the current operation and does not plan or consider out-of-scope work. | Reasoning scoped to the current step only. | Agent considers doing work beyond the current operation, then walks it back, or spends tokens on hypothetical future steps not required by the spec. |
| `context-retention` | The agent uses state and information already established earlier in the run without re-reading or re-deriving it. | Prior context is used directly, not reconstructed. | Agent re-reads a file it already has in context, re-derives a value it computed earlier, or re-checks a condition it already verified. |
| `confidence-calibration` | The agent uses hedging language only when genuinely uncertain about new information, not for questions the spec answers definitively. | Confident language for spec-covered decisions; hedging only for genuinely novel uncertainty. | Agent uses "I think", "probably", "I believe", or similar hedges when deciding something the active spec instruction answers without ambiguity. |
| `proportionality` | Token spend on a decision is proportional to the complexity and novelty of that decision. Trivial or spec-mandated choices are decided in one or two sentences. | Brief reasoning for simple choices; deeper reasoning only for genuinely complex trade-offs. | Agent spends more than two or three sentences justifying a choice that is either trivial or directly mandated by the spec. |
