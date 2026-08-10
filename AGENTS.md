# termrain Development Guide

This file contains repository-specific guidance for developers and coding agents modifying termrain.

## Project Overview

`termrain` is a Rust TUI application that displays weather forecasts and rain radar imagery in the terminal.

- It uses Rust edition 2024 and requires Rust 1.95.0 or later.
- The UI is built with `ratatui` and `crossterm`.
- Tokio and reqwest handle asynchronous HTTP communication.
- JMA supplies Japanese data; Open-Meteo and RainViewer supply international data.
- On compatible terminals, the app uses Kitty graphics or Sixel; otherwise it falls back to text rendering.

See `README.md` and `README.ja.md` for user-facing behavior and controls.

## Source Layout

- `src/main.rs`: Entry point. It only parses CLI arguments, initializes logging, and starts the application.
- `src/cli.rs`: CLI argument definitions using `clap`.
- `src/config.rs`: Reads and writes `~/.config/termrain/config.toml` and determines cache paths.
- `src/api/`: Weather and map data access layer.
  - `mod.rs`: Defines the `WeatherProvider` trait and selects JMA or Open-Meteo.
  - `jma.rs`: Handles JMA weather data and radar for Japan.
  - `open_meteo.rs`: Handles Open-Meteo forecasts and RainViewer radar outside Japan.
  - `geocoding.rs`: Resolves city names to coordinates.
- `src/app/`: Application state and event loop.
  - `state.rs`: Defines `AppState` and `Msg` values sent from asynchronous tasks.
  - `fetch.rs`: Fetches weather, radar, and map data concurrently in the background.
  - `input.rs`: Handles keyboard input and terminal resize events.
  - `startup.rs`: Applies CLI overrides and handles `--list-city` and `--dump`.
  - `mod.rs`: Initializes the TUI and owns the Tokio event loop.
- `src/ui/`: Rendering-only components. Do not add state mutation or HTTP calls here.
- `src/map.rs`: Handles GeoJSON map data such as coastlines and administrative boundaries.
- `src/i18n.rs`: Owns English and Japanese UI strings.
- `.github/workflows/ci.yml`: Runs format, clippy, release build, and tests on Linux, macOS, and Windows.

## Implementation Guidelines

### Keep responsibilities separate

Keep changes within the appropriate layer whenever possible.

- Put API fetching, JSON conversion, and image composition in `src/api/`.
- Put background-task spawning and `Msg` application to `AppState` in `src/app/fetch.rs`.
- Put key bindings and state changes caused by input in `src/app/input.rs`.
- Put layout and rendering details in `src/ui/`.
- When adding or changing visible strings, update both English and Japanese entries in `src/i18n.rs`.

### Adding or changing providers

- Keep the UI independent of a concrete API implementation. Define required shared behavior on `WeatherProvider`.
- Convert `current`, `hourly`, `daily`, and `radar` responses into the shared data structures expected by the UI.
- Represent provider-specific radar time ranges with `radar_offset_range()`.
- Return external API failures to callers through `anyhow::Result`; do not let them crash the TUI.
- Never add API keys, tokens, or personal data to source code, logs, or test fixtures.

### Asynchronous work

- Do not block the TUI event loop with networking or expensive work.
- Spawn background work with `tokio::spawn` and return results to the main loop as `mpsc` `Msg` values.
- Run independent data fetches concurrently where practical.
- If changing a radar refresh path (resize, zoom, movement, time scrub, or map-style change), ensure all entry points keep loading state and redraw behavior consistent.

### UI and terminal compatibility

- Layout must not panic on narrow or short terminals. Use tools such as `saturating_*` and `Constraint::Min`.
- Preserve rendering when no image protocol is detected.
- Normal logging to stdout or stderr corrupts a running TUI. Follow the existing `tracing` setup and write logs to files.
- When changing text containing Japanese or emoji, account for display width. Follow existing code using `unicode-width` when necessary.
- For UI or radar changes, verify the screen on a compatible terminal when possible and attach before/after screenshots to the PR.

### Configuration and caching

- Access settings through `Config` and preserve the existing XDG-style path conventions.
- When adding configuration fields, consider `serde(default)` or another suitable default to preserve compatibility with existing config files.
- Reuse caching for downloadable map data and tiles; do not add unnecessary network requests.

## Coding Conventions

- Follow the output of `cargo fmt`; do not introduce manual formatting rules.
- Prefer existing naming, module boundaries, and error-handling patterns.
- Add comments not only for what code does, but also for design rationale and external API constraints where useful.
- Keep user-facing CLI `--help` text in English and maintain both English and Japanese TUI support.
- Do not mix unrelated refactors or dependency upgrades into a focused change.

## Verification

After changes, run the following commands as appropriate for the affected area:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets
cargo build --release
cargo test --release
```

CI runs the same commands on Linux, macOS, and Windows. If a command cannot be run locally, state which command was skipped and why.

For CLI changes, verify at least the following:

```sh
cargo run -- --help
cargo run -- --list-city Tokyo
cargo run -- --dump --city Tokyo
```

These commands access external APIs. If the network is unavailable, report the failure instead of hiding it.

## Change Checklist

- [ ] The changed code belongs to the appropriate module.
- [ ] Visible strings were updated in both English and Japanese where needed.
- [ ] External network failures do not crash the TUI.
- [ ] Small-terminal and image-protocol fallback behavior remains intact.
- [ ] `cargo fmt --all -- --check` passed.
- [ ] Clippy, build, tests, and manual CLI checks were run as appropriate.
- [ ] `README` and `CHANGELOG` were updated when user-facing behavior changed.

## Release Notes

- Pushing a `v*` tag triggers the GitHub Actions release workflow.
- The release workflow builds binaries for macOS (Apple Silicon and Intel), Linux x86_64, and Windows x86_64.
- Updating the Homebrew formula requires a CI secret. Never expose secret values in the repository, logs, issues, or pull requests.
