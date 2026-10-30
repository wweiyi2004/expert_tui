# Expert TUI

Standalone Windows x64 terminal assistant. Requires Node.js 20+.

The package retains the existing `@wweiyi/expert` identity and `expert` command.
Install with `npm install -g @wweiyi/expert` or install an explicitly built local TGZ.
The registry's existing 0.2.0 release predates the repository split; this workflow
setup does not publish a replacement release.

Portable ZIP users can run `expert.cmd`. User state is stored separately under
`%LOCALAPPDATA%/expert-tui/state`; UI configuration is under `%APPDATA%/expert-tui`.

`expert --version`, `expert update --check`, and `expert --snapshot welcome`
are supported. Update uses this package's own npm identity.

The bundle contains the Rust TUI, Dart Host, file tools, and media worker.
Audio/video extraction additionally requires ffmpeg/ffprobe.

License: see LICENSE.

Source: https://github.com/wweiyi2004/expert_tui
