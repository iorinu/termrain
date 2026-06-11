# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `--completion <SHELL>` flag to emit shell completion scripts
  (bash / zsh / fish / powershell / elvish).
- Homebrew formula auto-installs shell completion via
  `generate_completions_from_executable` on next release.

### Changed
- `--help` output is now in English (CLI doc comments translated).

## [0.3.0] - 2026-06-11

### Changed
- **Foreign radar source switched from Open-Meteo to RainViewer.**
  The previous 32×16 multi-point precipitation query hit free-tier rate
  limits (429) after a few pans, and was replaced with RainViewer's tile
  API (free, world coverage, no API key).
- Provider name shown in the header is now `Open-Meteo + RainViewer` for
  non-Japan locations to reflect both data sources.
- Radar playback range is now provider-specific via the new
  `WeatherProvider::radar_offset_range()` trait method. JMA stays at
  `-6..=+12`; Open-Meteo + RainViewer uses `-12..=0` (past 2 h only,
  matching RainViewer's free tier).

### Added
- Skip "max X mm/h" in the radar panel title when no numeric data is
  available (image-based providers like RainViewer).

### Fixed
- Radar timestamp was previously stuck at the latest past frame during
  playback for foreign locations (the play loop iterated offsets the
  provider didn't actually support).
- Open-Meteo radar tiles are only fetched within the visible view
  range (typically 1, up to 4 tiles) instead of a blind 5×3 grid.

## [0.2.2] - 2026-06-11

### Fixed
- Foreign radar (Open-Meteo) failed to load with HTTP 414 because the
  GET URL exceeded ~8000 chars for the 512-point precipitation query.
  Switched to POST with a JSON body.
- `observed_at` was interpreted in UTC+0 for foreign locations, making
  the timestamp off by the local UTC offset.

### Changed
- Parallelized Open-Meteo precipitation POST and CARTO map tile fetches
  during radar rendering.
- Extracted the Open-Meteo base URL into an `API_BASE` constant.

## [0.2.1] - 2026-06-11

### Fixed
- Radar panel title in the canvas fallback renderer was hardcoded to
  Japanese; now uses the i18n strings so `--lang english` shows `Radar`.

## [0.2.0] - 2026-06-11

### Added
- Wide composite radar images that fill wide terminals via aspect-aware
  rendering (radar panel now uses the full available width on widescreen
  setups).

### Changed
- UI polish: panel temperature gradients, colored rain bars in the
  hourly list, and a now-highlight on the current hour.

## [0.1.0] - 2026-06-10

Initial public release.

### Added
- TUI layout: current weather, weekly forecast, rain radar, hourly chart,
  Yahoo-style hourly list, header and status footer.
- Rain radar rendering through Kitty graphics protocol, with map and rain
  alpha-blended into a single image (verified on wezterm).
- Map tile sources: CARTO Voyager (worldwide), GSI Standard and GSI Aerial
  (Japan only). Cycled with the `m` key.
- 14-step rain color gradient with a legend bar baked into the image.
- Radar time scrubbing from -30 min to +60 min (`,` / `.`) and auto-play
  with the `p` key, using JMA `targetTimes_N1.json` (past) and
  `targetTimes_N2.json` (forecast).
- Help modal triggered by `?`, splash screen during initial fetch, and a
  spinner shown while radar requests are in flight.
- Automatic provider selection: JMA Nowcast inside Japan, Open-Meteo
  elsewhere. JMA `current()` and `daily()` are enriched with Open-Meteo
  data (real-time temperature/humidity/wind, missing daily values).
- Hourly forecast is always served by Open-Meteo because JMA does not
  publish hourly granularity.
- Bilingual UI (English default, Japanese available via config or
  `--lang`). All major strings live in `src/i18n.rs`.
- CLI options: `--city`, `--lat` / `--lon`, `--lang`, `--save`,
  `--list-city`, `--force-jma`, `--dump`.
- XDG-compliant paths: `~/.config/termrain/config.toml` (auto-created on
  first launch) and `~/.cache/termrain/` for logs and downloaded GeoJSON.
- Persistent in-memory tile cache for both rain and base map tiles.
- GitHub Actions: `ci.yml` (fmt / clippy / build / test on Linux, macOS,
  Windows) and `release.yml` (binaries for aarch64-apple-darwin,
  x86_64-apple-darwin, x86_64-unknown-linux-gnu, x86_64-pc-windows-msvc).
- Homebrew formula stub under `docs/homebrew/termrain.rb`, ready to be
  copied into the `iorinu/homebrew-tap` repository after release.
- Bilingual README (`README.md` / `README.ja.md`), MIT LICENSE, and a
  main-screen screenshot in `docs/screenshots/main.png`.

[Unreleased]: https://github.com/iorinu/termrain/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/iorinu/termrain/releases/tag/v0.1.0
