# ADR-0001: Layer ownership and dependency direction

- Status: Accepted
- Date: 2026-08-09

## Decision

Relay uses inward dependencies:

1. `domain` owns business entities and invariants and depends only on shared primitives.
2. `application` owns repository ports and use-case orchestration and may depend on `domain` and `shared`.
3. `infrastructure` implements application ports and owns PostgreSQL, Harness, Git and Codex integration.
4. `apps/server`, `apps/agent-trigger`, `apps/mcp-server` and `apps/web` are delivery surfaces.

The production application entry remains `PlatformApp`. A parallel `services/` facade or a second repository abstraction is not allowed.

## Enforcement

CI runs `scripts/check_dependency_boundaries.sh` and `scripts/check_architecture_boundaries.sh`. Pull requests must declare the affected ownership layer and justify new navigation or aggregate growth.
