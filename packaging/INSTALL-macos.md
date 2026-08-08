# Relay for Apple Silicon macOS

Prerequisite: Docker Desktop for Mac.

1. Extract this archive.
2. Start Docker Desktop and wait until it reports that Docker is running.
3. Open Terminal in the extracted Relay directory and run:

   ```bash
   ./install.sh
   ```

The Release already includes the web console and the Apple Silicon Agent Trigger. Node.js, pnpm, Rust, and Cargo are not required on the host.

If macOS still blocks the unsigned download, run `xattr -dr com.apple.quarantine .` in the extracted directory and retry. Future signed and notarized packages can remove this fallback.
