# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
