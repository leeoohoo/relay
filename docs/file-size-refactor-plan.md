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
| [x] | 5,236 | `apps/web/src/pages/App.tsx` | application shell is now 400 lines; feature pages, dialogs and view models were extracted |
| [x] | 4,655 | `apps/server/src/main.rs` | bootstrap/router is now 653 lines; account, company, chat, project, Codex, Agent, auth/error, DTO and tests are focused modules |
| [x] | 3,870 | `crates/domain/src/company.rs` | entity/governance facade is now 791 lines; profession catalogs/playbooks, project-type catalog/inference/rules and tests are focused modules |
| [x] | 3,849 | `crates/mcp/src/lib.rs` | facade/input DTOs are now 797 lines; gateway/audit, Handler, schemas and domain dispatchers are focused modules |
| [ ] | 3,608 | `crates/application/src/memory.rs` | state, persistence and domain repository implementations |
| [x] | 3,365 | `crates/infrastructure/src/codex_trigger.rs` | facade/types are now 222 lines; configuration/discovery, runtime, app-server RPC, JSONL events, model catalog, validation and tests are focused modules |
| [x] | 2,565 | `apps/agent-trigger/src/main.rs` | bootstrap/poll loop is now 589 lines; Codex control/install/plugins, execution and Relay Skill materialization are focused modules |
| [ ] | 2,488 | `apps/web/src/styles.css` | tokens, base, layout and feature-specific stylesheets |
| [x] | 2,239 | `crates/infrastructure/src/lib.rs` | PostgreSQL-only adapter construction; redundant enum delegation removed |
| [x] | 2,034 | `crates/infrastructure/src/codex_control.rs` | facade/types are now 386 lines; store, requests/runtime, MCP, persistence, validation and tests are focused modules |
| [x] | 1,574 | `crates/infrastructure/src/git_workspace.rs` | manager, validation and Git command modules; facade/workflow is now 941 lines |

The baseline excludes generated output and dependency/build directories such as `target`, `node_modules`, and `dist`.

## Structure audit (2026-08-06)

The next extractions are ordered by responsibility and coupling, not only by line count.

| Priority | Area | Current problem | Target modules | Duplication to remove while splitting |
| --- | --- | --- | --- | --- |
| P0 | `apps/web/src/pages/App.tsx` | Application shell, five Codex surfaces, projects, tasks, project rules/assets/Git, and dialogs share one file | `app-shell`, `codex/`, `projects/`, `tasks/` feature modules | repeated Codex environment fetch/poll state; repeated busy/error wrappers; repeated project permission derivation and grant flow |
| P0 | `apps/server/src/main.rs` | Bootstrap, router assembly, auth extraction, DTOs, error mapping and all HTTP handlers are coupled | `bootstrap`, `http/router`, `http/error`, domain route modules | repeated company/session authorization, JSON response construction and query parsing |
| P0 | `crates/infrastructure/src/postgres.rs` | Every PostgreSQL repository and most row mapping live behind one adapter file | `postgres/connection`, `transaction`, domain repositories, shared row mapping | repeated transaction begin/commit/error conversion; repeated optional row decoding and pagination queries |
| P1 | `crates/domain/src/company.rs` | Core entities are mixed with profession catalogs, project-type rules, bilingual Skill text and tests | `company/entities`, `governance`, `professions`, `project_types`, focused tests | Chinese/English catalog construction and repeated permission/rule metadata |
| P1 | `crates/mcp/src/lib.rs` | Protocol gateway, tool schema, dispatch, audit and inputs change together | `gateway`, `tools/schema`, `tools/dispatch`, `audit`, `inputs` | repeated argument validation, company/agent context resolution and tool result/error envelopes |
| P1 | `crates/application/src/memory.rs` | In-memory state, persistence and all repository implementations are coupled | `memory/state`, `persistence`, domain repository adapters | repeated lock acquisition, entity lookup and not-found/conflict conversion |
| P1 | `crates/infrastructure/src/codex_trigger.rs` | Command construction, process lifecycle, approvals, progress events and tests are interleaved | `trigger/config`, `command`, `runner`, `approval`, `progress`, tests | repeated process-output normalization and event/status persistence |
| P2 | `apps/agent-trigger/src/main.rs` | CLI bootstrap, polling loop, installation, plugin discovery and task execution share globals | `config`, `poller`, `executor`, `installer`, `plugins` | repeated environment parsing, retry/backoff and Trigger API calls |
| P2 | `crates/infrastructure/src/codex_control.rs` | Runtime discovery, auth profiles, MCP and CLI settings share storage/command helpers | `control/runtime`, `profiles`, `mcp`, `settings`, `store` | repeated target-selector resolution, command execution and snapshot persistence |
| P2 | `crates/infrastructure/src/git_workspace.rs` | Validation, Git command execution, worktree lifecycle and credentials are coupled | `git/validation`, `command`, `workspace`, `credentials` | repeated command error formatting, path safety checks and remote URL normalization |

### Web extraction sequence

- [x] Extract organization dialogs, Agent management, memory/approval views and shared platform types.
- [x] Finish and verify the current Skill center and chat center extraction.
- [x] Extract Codex control-center navigation, MCP, plugins, runners, auth/environment, profiles and Trigger into focused files.
- [ ] Introduce one shared Codex environment hook for loading, pending-operation refresh intervals and stale-request cancellation.
- [x] Extract project list/create/detail shell separately from project rule, assets and Git panels.
- [x] Extract task list, task dialog, status/priority presentation and task filters.
- [x] Reduce `App.tsx` to session/company orchestration, navigation and top-level dialogs.

Current web-shell result: `App.tsx` is 400 lines. Project, task, rule, asset, Git, Codex, chat, Skill, Agent and organization views now live in focused feature modules.

### Refactor rules discovered during audit

- A feature module owns its request state and view; the application shell passes identity, token and refresh callbacks only.
- Shared hooks are introduced only when at least two extracted features have the same lifecycle, not merely similar names.
- API request bodies and permission mutation logic must have one typed helper rather than being rebuilt in multiple components.
- Catalog data and bilingual content stay data-driven; UI language selection must not duplicate business flow.
- Tests move with the behavior they cover, and every extraction batch must compile before the next file is touched.

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
- [x] Split Codex trigger execution into configuration/discovery, runtime, app-server RPC, progress/JSONL, model catalog and validation modules.
- [x] Split Codex control into store/profile settings, request/runtime, MCP, persistence, validation and focused test modules.
- [x] Split Git workspace behavior into manager, validation and command execution modules while retaining focused worktree lifecycle code in the facade.

### 3. Runtime applications

- [x] Reduce server `main.rs` to bootstrap, shared state and route composition; move handlers into domain modules.
- [x] Reduce agent-trigger `main.rs` to bootstrap, handlers and polling; move execution, plugin/installer control and Skill materialization into services.
- [ ] Keep DTO conversion and HTTP error mapping centralized instead of repeating them in handlers.

### 4. Web application

- [x] Reduce `App.tsx` to the application shell and navigation composition.
- [x] Move organization, skills, projects, Codex console and chat into independent feature pages/components.
- [ ] Move API calls to `api/`, remote-state behavior to hooks, and shared types to `types/`.
- [x] Split the global stylesheet into ordered foundation, feature, theme, communication and responsive workbench stylesheets while preserving cascade behavior.
- [ ] Preserve pagination, SSE, approval and Codex management behavior during extraction.

### 5. Domain and MCP

- [x] Split company entities from profession/project-type catalogs and skill/rule content.
- [x] Split MCP tool schemas, definitions, domain dispatch, audit and gateway code.
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
