# Issue #21: CARTO Voyager のAPIキー対応

## 概要

termrain 0.4.0では、OpenFreeMap Libertyをデフォルトにし、CARTO Voyagerを5種類の地図スタイルの一つとして残している。

CARTO Voyagerのラスタタイルは、未認証リクエストに`API KEY REQUIRED`の透かしを返す。CARTOの公式資料では、APIキーをタイルURLの`key`クエリパラメータで渡す方式が案内されている。[1][2]

この資料は、Issue #21で扱う「有効なAPIキーを設定した利用者がCARTO Voyagerを使えるようにする」機能の設計メモである。現時点では実装しない。

## CARTOの利用条件

2026-09-08時点で確認できる公式情報:

- APIキーの発行は無料で、APIキー申請にCARTOアカウントは不要と案内されている。[3]
- CARTO basemapは月500万タイルリクエストまでがfair-use limitと案内されている。[4]
- 上限を超える場合、非商用プロジェクトは個別に上限引き上げを相談できる場合がある。[4]
- 商用利用で上限を超える場合はEnterpriseライセンスの相談になる。[4]
- Enterpriseの具体的な価格や超過時の固定料金は公開されていない。[4]
- ラスタbasemapは廃止方向で、新規アプリにはベクタbasemapが推奨されている。[1][2]

500万リクエストは起動回数ではなく、タイル画像の取得回数で数えられる。termrainでは、表示範囲、ズーム、更新頻度によって1回の再取得で複数タイルを取得するため、利用量は利用方法によって変わる。

## 目的

- 有効なCARTO APIキーを持つ利用者が、`carto_voyager`を選択してVoyagerラスタ地図を表示できるようにする。
- APIキー未設定時も、OpenFreeMapをデフォルトとして利用できる現在の動作を壊さない。
- APIキーをGit、ログ、キャッシュ、エラー表示へ漏らさない。
- APIキー設定を毎回の`export`に依存させず、termrainの設定ファイルで指定できるようにする。

## 対象外

- OpenFreeMap Libertyをデフォルトから変更すること
- CARTOのベクタスタイルを新しく実装すること
- CARTOの料金契約やEnterprise契約をtermrain側で管理すること
- APIキーをリポジトリの設定例やfixtureへ記載すること

## 利用イメージ

```toml
[radar]
map_style = "carto_voyager"
carto_api_key = "YOUR_KEY"
```

`YOUR_KEY`は説明用のプレースホルダーであり、実際のキーをリポジトリへ保存してはならない。実際の設定ファイルがdotfilesリポジトリで管理されている場合も、APIキーを含む状態ではコミットしない。

APIキーが設定されていない場合は、次のどちらかを実装時に選ぶ。

1. 現在と同じくCARTOから透かし画像を返させ、CARTOが認証必須であることを表示する。
2. CARTO選択時に明確な警告を表示し、OpenFreeMapへフォールバックする。

この選択は、利用者が`m`キーでCARTOを比較用に選びたいか、失敗表示を避けたいかを確認してから確定する。

## 機能要件

### FR-001: 設定ファイルでAPIキーを指定できる

`[radar] carto_api_key`でCARTO Voyager用APIキーを指定できる。APIキーを環境変数だけに限定しない。

### FR-002: CARTOリクエストだけにキーを付ける

`map_style = "carto_voyager"`のタイルリクエストだけに、URLエンコードした`key`クエリパラメータを付ける。

```text
https://basemaps.cartocdn.com/rastertiles/voyager/{z}/{x}/{y}.png?key=...
```

OpenFreeMap、OpenStreetMap、国土地理院のリクエストにはCARTOキーを付けない。

### FR-003: デフォルト経路を変更しない

`carto_api_key`が未設定でも、OpenFreeMap Libertyをデフォルトとして起動できる。APIキー対応によって、他の4種類の地図スタイルの取得方式を変更しない。

### FR-004: APIキーを出力しない

APIキーは次の場所に出してはならない。

- ログ
- TUIのエラー表示
- エラーメッセージ
- キャッシュパスやキャッシュキー
- README、テストfixture、コミット、PR、Issue

HTTPリクエストの送信先URLにキーが含まれること自体はCARTOの仕様上必要だが、URLをログへ記録しない。

### FR-005: 未設定時の動作を明示する

APIキー未設定時のCARTO選択時には、透かし画像を許容するかOpenFreeMapへフォールバックするかを、実装前に決める。どちらの場合も、TUI全体をクラッシュさせない。

## 非機能要件

### NFR-001: 設定ファイルの扱い

APIキーを保存する設定ファイルは、利用者のローカル環境だけで管理する。ドキュメントではプレースホルダーだけを使い、Git管理対象のdotfilesへ実キーが入らないように案内する。

### NFR-002: リクエスト量

CARTOの月500万タイルリクエストというfair-use limitを前提に、APIキー対応を理由に不要な再取得や無制限リトライを追加しない。[4]

### NFR-003: 既存キャッシュとの互換性

キー付き・キーなしのURL文字列をキャッシュ識別子として保存しない。キャッシュは地図スタイルとタイル座標など、キーを含まない情報で識別する。

## 実装案

1. `RadarConfig`に`Option<String>`の`carto_api_key`を追加する。
2. `MapStyle::CartoVoyager`のタイルURL生成またはHTTPリクエスト直前に、`Url`のquery APIで`key`を追加する。
3. APIキーを含むURLをログ出力しない。
4. キー未設定時のCARTO選択に対する動作を決定する。
5. URL生成、設定のTOML往復、キー漏えい防止、未設定時のエラー処理をテストする。
6. 英語・日本語README、CHANGELOG、設定例を更新する。

環境変数による上書きを追加する場合でも、設定ファイルを主な設定方法とし、優先順位とログ上の扱いを明記する。環境変数を必須にしない。

## 受入条件

- [ ] 有効なAPIキーを設定した`carto_voyager`で、APIキー要求の透かしなしにタイルを表示できる。
- [ ] `carto_api_key`未設定でもOpenFreeMapをデフォルトとして起動できる。
- [ ] CARTOキーがCARTO以外のリクエストに付かない。
- [ ] ログ、TUIエラー、キャッシュ識別子、テストfixture、ドキュメント例に実キーが出ない。
- [ ] APIキーのURLエンコードをテストしている。
- [ ] APIキー未設定やHTTPエラーでTUIがクラッシュしない。
- [ ] 月500万タイルリクエストのfair-use limit、非商用/商用時の扱い、ラスタ廃止方向をREADMEまたは設定ドキュメントに記載している。[4]
- [ ] `cargo fmt`、`cargo clippy`、`cargo test`、`cargo build --release`が成功する。

## 未決事項

- APIキー未設定時に、透かし画像を表示するかOpenFreeMapへフォールバックするか。
- `CARTO_API_KEY`環境変数を設定ファイルの補助として実装するか。
- APIキーの設定ファイルが存在する場合に、Unix系でパーミッション`0600`を要求または警告するか。
- 有効なAPIキーを使った実タイル取得を、秘密情報を残さずどのように手動検証するか。

## 関連Issue・PR

- [Issue #17: 背景地図で`API KEY REQUIRED`が表示される問題](https://github.com/iorinu/termrain/issues/17)
- [Issue #21: support authenticated CARTO Voyager basemap tiles](https://github.com/iorinu/termrain/issues/21)
- [PR #20: restore API-key-free map defaults](https://github.com/iorinu/termrain/pull/20)

## 参照資料

1. [CARTO basemap styles README](https://github.com/CartoDB/basemap-styles/blob/master/README.md)
2. [CARTO raster basemap API-key requirement PR](https://github.com/CartoDB/basemap-styles/pull/48)
3. [CARTO basemap API key request](https://carto.com/basemaps/apikey)
4. [CARTO basemaps usage and fair-use limit](https://carto.com/basemaps)
