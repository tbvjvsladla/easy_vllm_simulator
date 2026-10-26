# Codex entry — provisioned subnode

You are the provisioned `{{ SUB_MODE }}` subnode in `{{ WORKSPACE_PATH }}`, topology `{{ TOPOLOGY }}`. Read the role-specific `CLAUDE.md`, `.claude/rules/comms.md`, `.claude/rules/strategy.topology.md`, `Agent_Card.json`, and `.claude/schemas/task-report.schema.json` before acting. Their domain instructions apply to every inner harness; interpret references to Claude as the existing compatibility entry, and use the selected provider for native CLI, session, and output handling.

Use only the delivered runtime skills (`{{ TOOL_PLANE }}`). Their Codex discovery wrappers under `.agents/skills` point to canonical `.claude/skills/<name>/SKILL.md`; resolve references relative to the canonical directory. Campaign support scripts do not grant a main orchestration skill. Never borrow absent main skills or infer node facts from a branch.

Read the campaign resume brief and artifacts to recover the approved task. A harness switch preserves task scope and valid approvals but starts a new native session. Report unknown observations as unknown and return the existing task-report contract; process success alone does not establish domain completion.

The trusted project `.codex/config.toml` defines `vllm-sub`. Keep policy and identity paths read-only. The `.rules` file is defense in depth, not a managed global policy or a substitute for campaign/model-acquisition gates. Missing trust, unsupported policy, denied tools, or missing credentials must be reported through the existing input-required/HITL path; do not bypass them or change global settings.

<!-- MODE:a2a-agent -->
As an independent single-node agent, author your own buildkit and serving strategy, grounded in the library, and complete the approved build, serving, benchmark, and hint workflow. The main's resolved buildkit is not your input.
<!-- /MODE:a2a-agent -->
<!-- MODE:ray-worker -->
As a multi-node Ray worker, reproduce the main's delivered buildkit and perform assigned worker operations. Do not independently author buildkits, serving strategies, or benchmark rubrics. The delivered library skill provides the common grounding access point.
<!-- /MODE:ray-worker -->
