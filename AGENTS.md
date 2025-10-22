# Repository Guidelines

## Project Structure & Module Organization

`src/main.rs` boots the Actix Web server and wires Tokio runtime chores. All reusable logic flows through `src/lib.rs`, which exposes the public API used by the binary. Command-line parsing lives in `src/cli.rs`, while runtime configuration (env overrides, JSON parsing) sits in `src/config.rs`. Hot-reload housekeeping and workspace watching code is grouped into `src/internal_scope.rs` and `src/startup.rs`. Front-end assets delivered by the dev server belong under `src/js`. Build artifacts land in `target/`; keep it out of version control.

## Build, Test, and Development Commands

- `cargo check` — fast type-check before opening a PR.
- `cargo build --release` — optimized binaries for benchmarks or deployment.
- `cargo run -- --help` — inspect supported CLI flags; add new options here.
- `cargo run -- --watch ./src/js` — typical dev loop: runs the server and watches asset changes.

## Coding Style & Naming Conventions

Use `cargo fmt` (Rust 2024 defaults) before pushing. Favor `snake_case` for functions/modules, `CamelCase` for types, and `SHOUTING_SNAKE_CASE` for constants. Keep modules focused; prefer private helpers in the same file unless they are reused broadly. When touching JavaScript helpers under `src/js`, follow the existing ES module structure and lint with `node ./src/js/lint.mjs` if the script is present; otherwise mirror the Rust naming rules.

## Testing Guidelines

Write unit tests alongside code in `#[cfg(test)]` modules. Integration suites should go under a future `tests/` directory with filename mirroring the feature under test. Run `cargo test` locally before any branch review; use `cargo test -p web-dev-server -- --nocapture` when debugging verbose output. Keep coverage high on request/response handlers and filesystem watchers, as regressions there surface slowly.

## Commit & Pull Request Guidelines

Adopt Conventional Commits (`feat`, `fix`, `chore`, etc.) to keep the history searchable, for example `feat: add websocket reload channel`. Squash commits per PR unless reviewers request otherwise. Open PRs with a short summary, bullet list of changes, mention of affected configs (ports, TLS), and instructions to reproduce the manual test plan. Attach screenshots or terminal captures when behaviour is visible in the browser.

## Configuration & Secrets

Environment defaults resolve via `src/config.rs`; when introducing new vars, create or update an `.env.example` so others can mirror the setup. Never commit real credentials—use placeholder values and confirm `git status` before pushing.
