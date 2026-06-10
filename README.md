# termrain

**English** | [日本語](./README.ja.md)

Terminal weather forecast and rain radar TUI, written in Rust.

termrain uses the Kitty graphics protocol to overlay rain clouds on a color map, bringing a Yahoo-style rain radar inside your terminal. It works for cities around the world and uses the JMA Nowcast (1 km / 5-minute resolution) for locations in Japan.

![termrain main screen](docs/screenshots/main.png)

<!-- Add more screenshots by dropping files into docs/screenshots/ and uncommenting:
![Help modal](docs/screenshots/help.png)
![Future radar scrub](docs/screenshots/radar-future.png)
-->


## Features

- **Current weather**: temperature, humidity, wind, weather icon
- **Hourly chart**: temperature line + precipitation bar, hourly for up to 48 hours
- **Hourly list (Yahoo style)**: vertical list with time / icon / temperature / precipitation amount or probability
- **Weekly forecast**: 7 days with icon, high / low temperature, precipitation probability
- **Rain radar** (raster image: map + rain alpha blend)
  - 14-step color gradient with a legend bar baked into the image
  - Time scrub from -30 minutes (past) to +60 minutes (forecast)
  - Auto play loop with the `p` key
  - Map style switch (CARTO Voyager / GSI Standard / GSI Aerial)
- **Localized UI**: English / Japanese (English is the default, switch via config or `--lang`)
- **Automatic provider selection**: JMA in Japan, Open-Meteo everywhere else
- **Tile cache**: map and radar tiles are cached in memory for smooth panning and zooming
- **Help modal**: press `?` to see key bindings and the rain legend


## Requirements

- Rust 1.95.0+ (edition 2024)
- A terminal with Kitty graphics protocol support (verified on **wezterm**)


## Installation

### A. Homebrew (macOS / Linux)

```sh
brew tap iorinu/tap
brew install termrain
```

### B. Prebuilt binaries

Archives for each platform are attached to every release on the
[Releases](https://github.com/iorinu/termrain/releases) page:

- `termrain-vX.Y.Z-aarch64-apple-darwin.tar.gz` (Apple Silicon Mac)
- `termrain-vX.Y.Z-x86_64-apple-darwin.tar.gz` (Intel Mac)
- `termrain-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz` (Linux x86_64)
- `termrain-vX.Y.Z-x86_64-pc-windows-msvc.zip` (Windows; TUI rendering is unverified)

```sh
# Example: Apple Silicon Mac, latest release
curl -L https://github.com/iorinu/termrain/releases/latest/download/termrain-v0.1.0-aarch64-apple-darwin.tar.gz | tar xz
sudo install -m 755 termrain /usr/local/bin/
```

### C. Build from source (`cargo install`)

```sh
cargo install --git https://github.com/iorinu/termrain
```

The binary is placed at `~/.cargo/bin/termrain` (run as `termrain` if that directory is on your `PATH`).

### D. Clone the repository and build

```sh
git clone https://github.com/iorinu/termrain
cd termrain
cargo build --release
# Run directly
./target/release/termrain --city Tokyo
# Install system-wide (optional)
install -m 755 target/release/termrain /usr/local/bin/
```


## Quick start

```sh
# Just run it: the first launch creates ~/.config/termrain/config.toml and shows Tokyo
termrain

# Run with a city name
termrain --city Tokyo
termrain --city Osaka
termrain --city Paris

# Run with explicit coordinates
termrain --lat 34.8265 --lon 135.4717   # Minoh, Japan

# Switch to Japanese UI with Osaka as the default
termrain --city Osaka --lang ja --save

# After that, just run termrain to launch with the saved defaults
termrain
```


## Key bindings

| Key | Action |
|---|---|
| `q` / `Esc` | Quit |
| `?` | Toggle help modal |
| `r` | Reload at current time |
| `+` / `-` | Zoom (6 to 13, default 11 ≒ 16 km wide) |
| `h` `j` `k` `l` | Pan the radar location (~2 km per keypress) |
| `,` / `.` | Scrub radar time backward / forward |
| `p` | Toggle radar animation playback |
| `m` | Cycle map style (CARTO Voyager → GSI Standard → GSI Aerial) |


## CLI options

| Option | Description |
|---|---|
| `--city <NAME>` | Look up a city by name (resolved by Open-Meteo Geocoding) |
| `--lat <LAT>` | Latitude (combine with `--lon`) |
| `--lon <LON>` | Longitude (combine with `--lat`) |
| `--lang <en\|ja>` | Override UI language (`en` / `english` / `ja` / `japanese`) |
| `--save` | Persist the launch settings (city / language / coordinates) to `~/.config/termrain/config.toml` |
| `--list-city <QUERY>` | Print up to 10 disambiguation candidates and exit (no TUI) |
| `--force-jma` | Force JMA even when the coordinates fall outside Japan (experimental) |
| `--dump` | Skip the TUI and dump the current weather to stdout (debug) |
| `-h` / `--help` | Print help |
| `-V` / `--version` | Print version |

Disambiguating a name that resolves to multiple cities:

```sh
$ termrain --list-city Ueno
Candidates for "Ueno":
   1. Uwano   Mie, Japan        lat= 33.8500  lon=135.9833
   2. Ueno    Niigata, Japan    lat= 37.1820  lon=138.7457
   3. Ueno    Kyoto, Japan      lat= 35.1167  lon=135.8333
   ...
# Copy the lat/lon you want and launch with it
$ termrain --lat 37.1820 --lon 138.7457 --save
```


## Configuration file

The first launch creates `~/.config/termrain/config.toml` (or `$XDG_CONFIG_HOME/termrain/config.toml` if `XDG_CONFIG_HOME` is set).

```toml
[location]
name = "Tokyo"
latitude = 35.6812
longitude = 139.7671
country = "JP"           # "JP" picks JMA; anything else picks Open-Meteo

[ui]
unit = "metric"          # metric / imperial (only metric for now)
refresh_interval = 600   # auto refetch interval in seconds; 0 disables it
language = "english"     # english / japanese

[radar]
zoom = 11                # 6 (wide ≒ 130 km) to 13 (narrow ≒ 4 km)
map_style = "carto_voyager"  # carto_voyager / gsi_std / gsi_photo
```

`--save` rewrites this file with the current launch arguments.


## Caches

- Logs: `~/.cache/termrain/termrain.log.*`
- Map and admin boundary data: `~/.cache/termrain/*.geojson` and `*.json`
- Radar tiles are kept in memory only (cleared when the process exits)

To wipe the on-disk caches:

```sh
rm -rf ~/.cache/termrain
```


## Data sources

- **JMA Nowcast** (rain radar, Japan): <https://www.jma.go.jp/>
- **GSI (Geospatial Information Authority of Japan)** (map tiles, Japan): <https://maps.gsi.go.jp/>
- **CARTO Basemaps** (map tiles, worldwide): © OpenStreetMap contributors, © CARTO
- **Open-Meteo** (weather & rain outside Japan, geocoding): <https://open-meteo.com/>
- **Natural Earth** (coastline, country borders): public domain
- **GADM 4.1** (Japanese municipal boundaries): <https://gadm.org/>


## License

[MIT License](./LICENSE) © 2026 iorinu
