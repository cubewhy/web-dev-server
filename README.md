# web-dev-server

<!--toc:start-->

- [web-dev-server](#web-dev-server)
  - [Features](#features)
  - [Getting Started](#getting-started)
  - [CLI Flags](#cli-flags)
  - [Live Reload Workflow](#live-reload-workflow)
  - [Project Layout](#project-layout)
  - [Development Tasks](#development-tasks)
  - [Contributing](#contributing)
  <!--toc:end-->

Minimal Actix-based development server for static sites with smart live reloads. It watches a workspace folder, serves HTML/CSS/JS, and keeps your browser in sync using lightweight websocket events.

Warning: This is a self-use solution and is not suitable for production.

## Developed with AI

This project was almost developed using AI assistance.

## Features

- Instant boot with Tokio + Actix-Web.
- File watcher that broadcasts reload or diff events via `/ _live/ws`.
- Optional “diff mode” that hot-swaps HTML `<body>` and linked CSS without full refreshes.
- Auto-opens the default browser on startup (toggle with `--no-open-browser`).

## Getting Started

1. Ensure you have a Rust toolchain that supports the 2024 edition (`rustup update stable`).
2. Clone the repo and enter the workspace.
3. Start the dev server against your web assets:
   ```bash
   cargo run -- ./examples/site --port 4100
   ```
4. The server prints a startup summary with the URLs it serves. Leave the process running to keep live reload active.

## CLI Flags

- `<path>`: Directory containing `index.html` and assets (defaults to the repo root).
- `--port <u16>`: TCP port (defaults to `3000`; if in use, the server auto-increments until it finds a free slot). The server binds to `127.0.0.1`.
- `--diff-mode`: Switch to partial refreshes; HTML updates keep state intact when paths line up.
- `--no-open-browser`: Disable automatic browser launch for remote/CI runs.

## Live Reload Workflow

The watcher (via `notify`) broadcasts JSON events to the injected client script at `/_live/script.js`. When diff mode is off—or when a change cannot be classified—the client performs a full reload. HTML/CSS changes in diff mode trigger precise updates while preserving runtime state.

## Project Layout

- `src/main.rs`: CLI entry that parses flags and runs the server.
- `src/startup.rs`: Actix app assembly, watcher loop, and live reload messaging.
- `src/internal_scope.rs`: Internal `/ _live` scope (health, websocket, injected script).
- `src/js/script.js`: Browser-side live reload client.

## Development Tasks

- `cargo check`: Fast compile-time validation before committing.
- `cargo fmt`: Enforce the Rust style guide.
- `cargo test`: Run unit tests (see `src/startup.rs` for examples).
- `cargo build --release`: Produce an optimized binary when packaging the tool.

## Contributing

Follow the guidance in `AGENTS.md` for coding standards, commit expectations, and pull-request checklists. Open issues with reproduction steps to help triage watcher or websocket behaviour quickly.

## License

Licensed under GPL-3.0.

You're allowed to use, modify, and distribute this software under the terms of
the GNU General Public License version 3.0 as published by the Free Software Foundation.
See the `LICENSE` file for details.
