# Expert TUI client

Rust + Ratatui terminal interface for the standalone Dart task Host.

Build and package instructions: [project README](../../README.md).
Interaction guide: [terminal controls](../../docs/TUI-CONVENIENCE-2026-10-30.md).
Runtime extraction and compatibility: [migration notes](../../docs/EXTRACTION.md).

Offline preview:

```powershell
cargo run --manifest-path next/Cargo.toml -p expert-tui -- --snapshot welcome
```

For live tasks use the complete bundle produced by `python tool/build_native.py`.
A bare client build does not include the Host or media worker.

Default configuration is `%APPDATA%/expert-tui/tui.json`.
Default Host discovery is `%LOCALAPPDATA%/expert-tui/state/host/discovery/bridge.json`.
The original explicit environment overrides remain available.

GUI-only actions require a separately compatible GUI client. The standalone
bundle does not ship one.
