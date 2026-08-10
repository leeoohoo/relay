# Changelog

## v1.0.3

### Added

- Explicit replacement of damaged or stale project work sessions while preserving the project, branch, task, generation, and latest checkpoint context.
- Task dependency conditions for success, completion, and failure paths, including rejected-review-to-rework workflows.
- Project-scoped approval for non-privileged localhost preview ports, alongside exact-origin website approval.
- Local project import by a Trigger-host absolute path in addition to the native directory picker.

### Changed

- Project Agent sessions now combine Trigger activity, current task state, session generation, and previous-turn summaries instead of presenting stale thread state as the current run.
- Managed project workspaces expose a standard `.git` marker while Relay keeps private Git metadata separate; Codex child processes no longer inherit `GIT_DIR` or `GIT_WORK_TREE`.
- Runner profiles retain the last successful response during rate limits and clearly distinguish run approval from website approval.
- Authentication refresh failures preserve the active user unless the server explicitly returns `401 Unauthorized`.
- Runtime Skills instruct repository-wide tools to exclude Relay-managed read-only Skill directories.

### Fixed

- Trigger wake reasons rejected by the PostgreSQL constraint during ready-task handoff, project resume, and intent recovery.
- Pausing an Agent or project allowing an already claimed Trigger or pending Intent to start another Codex session; paused work now remains pending until explicitly resumed.
- Browser reload, back, forward, and blank-page operations being incorrectly routed through website approval despite having no new target URL; one-time website approval now covers the same origin for the current Agent work session.
- Website approval runs entering a waiting state before the approval request was successfully persisted.
- Project work-session creation delays appearing as an idle or reusable historical session.
- Historical session summaries exposing host usernames and absolute Relay workspace paths.
- Stale import errors remaining visible after a project was successfully created.
- Login forms shipping with development credentials and rate limits incorrectly returning users to another account.

## v1.0.2

### Added

- Relay-managed Chrome DevTools MCP for project browser automation and Web acceptance testing.
- Relay website approval flow with project- and origin-scoped persistent approval.
- Harness API repository browser with branch switching, pagination, syntax highlighting, and SVG preview.
- Dedicated browser artifact storage at `.relay/browser-artifacts/`.

### Changed

- Separated Agent control sessions from project work sessions and tightened runtime Skill isolation.
- Limited the plugin catalog to capabilities supported by Codex CLI.
- Improved project creation, Agent identity presentation, task handoff, and live execution progress.
- Release publishing now refreshes the existing GitHub Release title and notes when a tag is rebuilt.

### Fixed

- Browser screenshot and snapshot persistence while retaining a read-only project mount.
- Browser approval routing, approval continuation, inherited approval overrides, and profile recovery after restart.
- Transient Codex app-server startup failures, silent session-resume stalls, and unsupported managed profile arguments.
- Shallow Harness imports that could not publish complete repository history.
- Duplicate or stale Codex work-session entries and opaque MCP tool failure messages.
