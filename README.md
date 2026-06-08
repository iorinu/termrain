# termrain

ターミナルで天気予報と雨雲レーダーを表示する TUI アプリ (Rust)。

Kitty graphics protocol を活用して、カラー地図の上に雨雲を重ねる Yahoo 雨雲レーダー風の表示を端末内で実現する。

## 主な機能

- **現在の天気** ／ **時間別予報** (24-48h) ／ **週間予報** (7日)
- **雨雲レーダー** (画像表示、地図 + 雨雲 alpha blend)
  - 14段階の Yahoo 風カラーグラデーション
  - 凡例カラーバーを画像に焼き込み
  - 時系列スクラブ (過去30分 〜 未来60分)
  - 自動アニメーション再生
  - 地図スタイル切替 (CARTO Voyager / 国土地理院 標準 / 航空写真)
- 日本国内は **気象庁ナウキャスト**、海外は **Open-Meteo** を自動選択
- タイルのメモリキャッシュで滑らかな移動・ズーム

## 必要要件

- Rust 1.95.0+ (edition 2024)
- Kitty graphics / iTerm2 inline image / Sixel に対応した端末
  - 推奨: **wezterm**, **kitty**, **iTerm2**
  - 非対応端末ではフォールバックなし（要追加実装）

## ビルド & 実行

```sh
# 箕面市 (緯度経度直接指定)
cargo run --release -- --lat 34.8265 --lon 135.4717

# 都市名で指定 (Open-Meteo geocoding)
cargo run --release -- --city "Osaka"
cargo run --release -- --city "Paris"

# JSON ダンプ (動作確認用、TUI 起動なし)
cargo run --release -- --dump --lat 35.68 --lon 139.77
```

## キー操作

| キー | 動作 |
|---|---|
| `q` / `Esc` | 終了 |
| `r` | 再取得（現在時刻に戻す） |
| `+` / `-` | ズーム (6〜13) |
| `h` / `j` / `k` / `l` | 地点移動 (0.02° ≒ 2km) |
| `,` / `.` | 時系列スクラブ (前 / 後) |
| `p` | アニメーション再生 toggle |
| `m` | 地図スタイル切替 |

## 設定ファイル

`~/.config/termrain/config.toml`:

```toml
[location]
name = "Osaka"
latitude = 34.6937
longitude = 135.5023
country = "JP"

[ui]
unit = "metric"
refresh_interval = 600

[radar]
zoom = 11
map_style = "carto_voyager"  # carto_voyager | gsi_std | gsi_photo
```

## データ出典

- **気象庁ナウキャスト** (雨雲レーダー): https://www.jma.go.jp/
- **国土地理院** (地図タイル): https://maps.gsi.go.jp/
- **CARTO Basemaps** (地図タイル): © OpenStreetMap contributors, © CARTO
- **Open-Meteo** (海外天気): https://open-meteo.com/
- **Natural Earth** (海岸線・国境): public domain
- **GADM 4.1** (日本市町村界): https://gadm.org/

## ライセンス

未指定（個人開発、利用規約に従う）。
