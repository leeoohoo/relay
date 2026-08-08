# Relay installation packages

Choose the package for your computer:

- `relay-macos-apple-silicon.tar.gz`: Apple Silicon Macs only.
- `relay-windows-wsl2-x86_64.zip`: Windows 10/11 with WSL2 and Docker Desktop.

Both packages contain the web console and a prebuilt host Agent Trigger, so users do not need Node.js, pnpm, Rust, or Cargo. Docker remains required because Relay Server, PostgreSQL, and Harness run in containers.

Each archive contains an `INSTALL.md` with the exact startup command. Verify downloads with `SHA256SUMS` when needed.

The macOS archive is currently usable without an Apple certificate, but unsigned downloads may require one Gatekeeper approval. The workflow is ready for Developer ID signing and notarization when certificate secrets are added.
