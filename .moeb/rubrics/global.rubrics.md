## Project global rubric criteria

The following criteria apply to every `moeb` command executed in this project. Include them
in your `verify_rubrics` call along with any criteria in the specification's own `## Rubric`
section.

| Name | Description | Threshold | Pass Condition |
|------|-------------|-----------|----------------|
| `kernel-thin-and-parity` | No domain-specific or workflow-specific coordination logic may be placed in kernel Rust code when it can live in skill markdown or role files. Every tool registered in the CLI tool registry must also be registered in the MCP stdio server tool list. | Zero violations | Spec review: no proposed step places review/coordination logic in Rust; Run review: grep confirms no skill-specific logic in kernel tools; tool lists in tools/mod.rs and the MCP server match exactly |
