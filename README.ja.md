# termrain

[English](./README.md) | **日本語**

ターミナルで天気予報と雨雲レーダーを表示する Rust 製の TUI アプリ。

Kitty graphics protocol を活用してカラー地図に雨雲を重ね、Yahoo 天気の雨雲レーダー風の表示をターミナル内で実現します。世界中の主要都市に対応し、日本国内は気象庁ナウキャストの 1km / 5分粒度データを利用します。

![termrain main screen](docs/screenshots/main.png)

<!-- 追加スクショは下記の形式で。ファイルを docs/screenshots/ に置けば表示されます。
![Help modal](docs/screenshots/help.png)
![Future radar scrub](docs/screenshots/radar-future.png)
-->


## 主な機能

- **現在の天気**: 気温・湿度・風速・天気アイコン
- **時間別予報グラフ**: 気温折れ線 + 降水量バー (1時間刻みで 48 時間先まで)
- **時間別予報リスト**: Yahoo 天気風の縦並び (時刻 / アイコン / 気温 / 降水確率 or 降水量)
- **週間予報**: 7 日分のアイコン・最高/最低気温・降水確率
- **雨雲レーダー** (画像表示、地図 + 雨雲 alpha blend)
  - 14 段階のカラーグラデーション + 凡例カラーバー焼き込み
  - 時系列スクラブ (過去 30 分 〜 未来 60 分)
  - `p` キーで自動アニメーション再生
  - 地図スタイル切替 (CARTO Voyager / 国土地理院 標準 / 航空写真)
- **多言語対応**: 英語 / 日本語 (デフォルト英語、設定 or `--lang` で切替)
- **自動プロバイダー切替**: 日本国内は気象庁 (JMA)、それ以外は Open-Meteo
- **タイルキャッシュ**: 地図・雨雲タイルをメモリに保持し、移動・ズームを高速化
- **ヘルプモーダル**: `?` キーで操作一覧と凡例を表示


## 必要要件

- Rust 1.95.0+ (edition 2024)
- Kitty graphics protocol に対応したターミナル (動作確認: **wezterm**)


## インストール

### A. Homebrew (macOS / Linux)

```sh
brew tap iorinu/tap
brew install termrain
```

### B. プレビルドバイナリをダウンロード

[Releases](https://github.com/iorinu/termrain/releases) ページからプラットフォームごとのアーカイブを取得できます:

- `termrain-vX.Y.Z-aarch64-apple-darwin.tar.gz` (Apple Silicon Mac)
- `termrain-vX.Y.Z-x86_64-apple-darwin.tar.gz` (Intel Mac)
- `termrain-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz` (Linux x86_64)
- `termrain-vX.Y.Z-x86_64-pc-windows-msvc.zip` (Windows、TUI 描画は未検証)

```sh
# 例: Apple Silicon Mac、最新リリース
curl -L https://github.com/iorinu/termrain/releases/latest/download/termrain-v0.1.0-aarch64-apple-darwin.tar.gz | tar xz
sudo install -m 755 termrain /usr/local/bin/
```

### C. ソースからビルド (`cargo install`)

```sh
cargo install --git https://github.com/iorinu/termrain
```

`~/.cargo/bin/termrain` にバイナリが入ります (`PATH` 通っていれば `termrain` で起動可)。

### D. リポジトリをクローンしてビルド

```sh
git clone https://github.com/iorinu/termrain
cd termrain
cargo build --release
# 直接実行
./target/release/termrain --city Tokyo
# システムに入れる (任意)
install -m 755 target/release/termrain /usr/local/bin/
```


## クイックスタート

```sh
# まずは引数なしで起動 (初回は ~/.config/termrain/config.toml を自動作成、東京表示)
termrain

# 都市名で起動
termrain --city Tokyo
termrain --city Osaka
termrain --city Paris

# 緯度経度直接指定
termrain --lat 34.8265 --lon 135.4717   # 箕面市

# 日本語UI + 大阪をデフォルトに保存
termrain --city Osaka --lang ja --save

# 以降は素のコマンドで前回の設定で起動
termrain
```


## キー操作

| キー | 動作 |
|---|---|
| `q` / `Esc` | 終了 |
| `?` | ヘルプモーダルを開く / 閉じる |
| `r` | 現在時刻に戻して再取得 |
| `+` / `-` | ズーム (6 〜 13、デフォルト 11 ≒ 16km 四方) |
| `h` `j` `k` `l` | 地点移動 (約 2km / キー) |
| `,` / `.` | 雨雲を時系列で 前 / 後 にスクラブ |
| `p` | 雨雲アニメーション 再生 / 停止 |
| `m` | 地図スタイル切替 (CARTO Voyager → 国土地理院 標準 → 航空写真) |


## CLI 引数一覧

| 引数 | 説明 |
|---|---|
| `--city <NAME>` | 都市名で地点指定 (Open-Meteo Geocoding で解決) |
| `--lat <LAT>` | 緯度 (`--lon` と組で指定) |
| `--lon <LON>` | 経度 (`--lat` と組で指定) |
| `--lang <en\|ja>` | 表示言語を上書き (`en` / `english` / `ja` / `japanese`) |
| `--save` | 起動時の設定 (都市 / 言語 / 緯度経度) を `~/.config/termrain/config.toml` に保存 |
| `--list-city <QUERY>` | 同名都市の候補を最大 10 件表示して終了 (起動はしない) |
| `--force-jma` | 緯度経度が日本国外でも JMA を使う (実験用) |
| `--dump` | TUI を起動せず、現在の天気を標準出力に JSON 風で出して終了 (デバッグ用) |
| `--completion <SHELL>` | 指定したシェル用の補完スクリプトを標準出力に出して終了 |
| `-h` / `--help` | ヘルプ表示 |
| `-V` / `--version` | バージョン表示 |

### シェル補完

`bash` / `zsh` / `fish` / `powershell` / `elvish` 用の補完スクリプトを生成できます:

```sh
# zsh (初回のみ)
mkdir -p ~/.zsh/completions
termrain --completion zsh > ~/.zsh/completions/_termrain
# ~/.zshrc に以下を追加 (まだ無ければ):
#   fpath=(~/.zsh/completions $fpath)
#   autoload -U compinit && compinit

# bash
termrain --completion bash > ~/.local/share/bash-completion/completions/termrain

# fish
termrain --completion fish > ~/.config/fish/completions/termrain.fish
```

Homebrew でインストールしている場合は補完が自動で有効になります (v0.3.1 以降)。

同名都市のあいまい解消例:

```sh
$ termrain --list-city Ueno
Candidates for "Ueno":
   1. Uwano   Mie, Japan        lat= 33.8500  lon=135.9833
   2. Ueno    Niigata, Japan    lat= 37.1820  lon=138.7457
   3. Ueno    Kyoto, Japan      lat= 35.1167  lon=135.8333
   ...
# 欲しい候補の緯度経度をコピペして起動
$ termrain --lat 37.1820 --lon 138.7457 --save
```


## 設定ファイル

初回起動時に `~/.config/termrain/config.toml` が自動生成されます (`XDG_CONFIG_HOME` が設定されていればそちらを優先)。

```toml
[location]
name = "Tokyo"
latitude = 35.6812
longitude = 139.7671
country = "JP"           # "JP" なら気象庁 (JMA)、それ以外は Open-Meteo

[ui]
unit = "metric"          # metric / imperial (現状は metric のみ)
refresh_interval = 600   # 自動再取得の間隔 (秒)、0 で無効
language = "english"     # english / japanese

[radar]
zoom = 11                # 6 (広域 ≒ 130km) 〜 13 (狭域 ≒ 4km)
map_style = "carto_voyager"  # carto_voyager / gsi_std / gsi_photo
```

CLI 引数 `--save` で現在の起動引数をこのファイルに書き込めます。


## キャッシュ

- ログ: `~/.cache/termrain/termrain.log.*`
- 地図・行政界データ: `~/.cache/termrain/*.geojson` および `*.json`
- 雨雲タイルはメモリキャッシュ (プロセス終了で破棄)

ディスクキャッシュを消すなら:

```sh
rm -rf ~/.cache/termrain
```


## データ出典

- **気象庁ナウキャスト** (雨雲レーダー、日本): <https://www.jma.go.jp/>
- **国土地理院** (地図タイル、日本): <https://maps.gsi.go.jp/>
- **CARTO Basemaps** (地図タイル、世界): © OpenStreetMap contributors, © CARTO
- **Open-Meteo** (海外の天気予報、Geocoding): <https://open-meteo.com/>
- **RainViewer** (海外の雨雲レーダータイル): <https://www.rainviewer.com/>
- **Natural Earth** (海岸線・国境): public domain
- **GADM 4.1** (日本市町村界): <https://gadm.org/>


## ライセンス

[MIT License](./LICENSE) © 2026 iorinu
