# Relay npm launcher

Install and start Relay with one command:

```bash
npx --yes @relay-ai/relay web
```

Supported hosts:

- Apple Silicon macOS
- Windows 10/11 through an Ubuntu WSL2 terminal

Docker Desktop must already be installed and running. The launcher downloads the matching GitHub Release, verifies its SHA256 checksum, installs application files under `~/.relay/app`, and keeps persistent company, Agent, project, PostgreSQL and Harness state outside the application directory.

```bash
npx --yes @relay-ai/relay status
npx --yes @relay-ai/relay install
npx --yes @relay-ai/relay logs
npx --yes @relay-ai/relay restart
npx --yes @relay-ai/relay stop
```
