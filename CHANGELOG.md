# Changelog

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
- Transient Codex app-server startup failures and unsupported managed profile arguments.
- Shallow Harness imports that could not publish complete repository history.
- Duplicate or stale Codex work-session entries and opaque MCP tool failure messages.
