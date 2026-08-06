# Source File Size Refactor Tracker

## Objective

- Keep every hand-written source file at or below 1,000 lines.
- Split code by domain responsibility instead of hiding large files behind `include!` or generated fragments.
- Remove duplicate validation, mapping, request parsing, repository delegation, and UI state code while splitting.
- Preserve public APIs and current behavior unless a change is explicitly documented.
- Keep every batch buildable and testable.

## Baseline (2026-08-06)

| Status | Lines | File | Planned ownership split |
| --- | ---: | --- | --- |
| [x] | 16,634 | `crates/application/src/service.rs` | split into contracts, platform services, validation, pagination and tests; facade is now 991 lines |
| [ ] | 6,725 | `crates/infrastructure/src/postgres.rs` | connection, transactions, domain repositories, row mapping |
| [ ] | 5,236 | `apps/web/src/pages/App.tsx` | application shell, feature pages, dialogs, hooks and view models |
| [ ] | 4,655 | `apps/server/src/main.rs` | bootstrap, router, state, errors, DTOs and domain handlers |
| [ ] | 3,870 | `crates/domain/src/company.rs` | company models, profession catalog, project types, skills, tests |
| [ ] | 3,849 | `crates/mcp/src/lib.rs` | gateway, schemas, tool definitions, dispatch, audit and inputs |
| [ ] | 3,608 | `crates/application/src/memory.rs` | state, persistence and domain repository implementations |
| [ ] | 3,365 | `crates/infrastructure/src/codex_trigger.rs` | runner, command, approval, progress, configuration and tests |
| [ ] | 2,565 | `apps/agent-trigger/src/main.rs` | bootstrap, service loop, execution, plugins and installer |
| [ ] | 2,488 | `apps/web/src/styles.css` | tokens, base, layout and feature-specific stylesheets |
| [x] | 2,239 | `crates/infrastructure/src/lib.rs` | PostgreSQL-only adapter construction; redundant enum delegation removed |
| [ ] | 2,034 | `crates/infrastructure/src/codex_control.rs` | runtime, auth profiles, MCP, CLI settings and storage |
| [ ] | 1,574 | `crates/infrastructure/src/git_workspace.rs` | manager, validation, Git commands, worktrees and credentials |

The baseline excludes generated output and dependency/build directories such as `target`, `node_modules`, and `dist`.

## Work batches

### 1. Application boundary

- [x] Move public request/view/bundle types from `service.rs` into domain-oriented `contracts/` modules and re-export them without breaking callers.
- [ ] Split the monolithic `PlatformRepository` port into auth, company, chat, project, task, memory and Codex ports; keep only a compatibility composition trait if required.
- [x] Move `PlatformApp` methods into domain-oriented platform modules.
- [x] Move shared validation into focused `validation/` modules and remove duplicated normalizers.
- [x] Split application unit tests by domain.
- [ ] Split `MemoryPlatformRepository` into state/persistence plus domain-specific implementations.

### 2. Infrastructure adapters

- [ ] Split PostgreSQL persistence by repository domain, with shared connection/transaction/mapping modules.
- [x] Remove duplicated `RepositoryAdapter` delegation by standardizing runtime storage on PostgreSQL.
- [ ] Split Codex trigger execution into runner, command, approval, progress and configuration modules.
- [ ] Split Codex control into runtime, profiles, MCP, CLI settings and persistence modules.
- [ ] Split Git workspace behavior into validation, command execution, worktree and credential modules.

### 3. Runtime applications

- [ ] Reduce server `main.rs` to bootstrap only; move routes and handlers into domain modules.
- [ ] Reduce agent-trigger `main.rs` to bootstrap only; move polling, execution, plugin and installer logic into services.
- [ ] Keep DTO conversion and HTTP error mapping centralized instead of repeating them in handlers.

### 4. Web application

- [ ] Reduce `App.tsx` to the application shell and navigation composition.
- [ ] Move organization, skills, projects, Codex console and chat into independent feature pages/components.
- [ ] Move API calls to `api/`, remote-state behavior to hooks, and shared types to `types/`.
- [ ] Split the global stylesheet into tokens/base/layout plus feature styles; remove repeated card/form/list rules.
- [ ] Preserve pagination, SSE, approval and Codex management behavior during extraction.

### 5. Domain and MCP

- [ ] Split company entities from profession/project-type catalogs and skill/rule content.
- [ ] Split MCP tool schemas, definitions, dispatch, audit and gateway code.
- [ ] Keep catalogs data-driven so Chinese/English content does not duplicate control flow.

### 6. Permanent enforcement

- [ ] Add `scripts/check_source_file_sizes.sh` with a default 1,000-line limit.
- [ ] Run the size check in CI and fail on newly oversized hand-written source files.
- [ ] Document narrow exclusions for generated/vendor files only; no source-file allowlist.

## Batch acceptance checks

Run the relevant focused checks after every extraction, then the full suite before completion:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
pnpm --dir apps/web test
pnpm --dir apps/web build
scripts/check_source_file_sizes.sh
git diff --check
```

## Definition of done

- [ ] No hand-written Rust, TypeScript, TSX, JavaScript, JSX or CSS source file exceeds 1,000 lines.
- [ ] No compatibility `include!` fragments or meaningless numbered files are used to bypass the limit.
- [ ] Public behavior and persisted data compatibility are verified.
- [ ] Repeated code found during the split is consolidated behind domain-level abstractions.
- [ ] CI enforces the limit for future changes.
