# Expert TUI

Standalone Windows x64 terminal assistant. Requires Node.js 20+.

Install an explicitly built local TGZ with `npm install -g <file.tgz>` and run `expert-tui`.
The package identity is `@wweiyi/expert-tui`; this extraction does not publish it.

Portable ZIP users can run `expert.cmd`. User state is stored separately under
`%LOCALAPPDATA%/expert-tui/state`; UI configuration is under `%APPDATA%/expert-tui`.

`expert-tui --version`, `expert-tui update --check`, and `expert-tui --snapshot welcome`
are supported. Update uses this package's own npm identity.

The bundle contains the Rust TUI, Dart Host, file tools, and media worker.
Audio/video extraction additionally requires ffmpeg/ffprobe.

License: see LICENSE.
