# Issue #21: CARTO Voyager のAPIキー対応調査と実装方針

## 調査ステータス

- 調査日: 2026-09-12 UTC
- 対象Issue: [Issue #21](https://github.com/iorinu/termrain/issues/21) [1]
- この文書の範囲: 解決方法、無料利用の可否、実装方向、受入条件の整理
- 実装: この文書では行わない。実装は方針確定後の別タスクで行う

## 結論

Issue #21は、CARTO Voyagerをデフォルトに戻す変更ではなく、利用者が自分で取得したCARTO APIキーを設定した場合だけ、既存の`carto_voyager`選択肢を正常に使えるようにする機能として解決する。

推奨する方向は次のとおりである。

1. OpenFreeMap Libertyを引き続きデフォルトにする。
2. `carto_voyager`は明示的に選択した場合だけ使う。
3. APIキーの主な入力経路は`~/.config/termrain/config.toml`の`[radar].carto_api_key`にする。
4. CARTO Voyagerのリクエストにだけ、CARTO指定の`key`クエリパラメータを付ける。
5. APIキーがない場合はCARTOへ未認証リクエストを送らず、透かし画像ではなく、キーが必要だと分かる警告を表示する。OpenFreeMapへ暗黙に切り替えない。
6. APIキーはtermrainの共有物にせず、利用者ごとのキーを利用する。

この対応でIssue #21の短期的な要求は満たせる。ただし、CARTO自身がラスタbasemapを廃止方向としており、新規アプリにはベクタbasemapを推奨しているため、長期的なデフォルト候補にはしない。[2][5]

Issue #17の原因となったCARTOのbasemap-styles PR #48も、ラスタbasemapのAPIキー必須化と廃止方向をREADMEへ追記する内容だった。[6]

## 無料で利用できるか

### 判定

個人利用や小規模な開発・検証であれば、CARTO Voyagerを無料で利用できる見込みが高い。CARTOのAPIキー申請ページは、キーが無料で、申請に承認待ちがなく、CARTOアカウントも不要だと案内している。[2]

CARTOのbasemap案内ページも、APIキーがあれば月500万タイルリクエストまで無料で利用できると説明している。[3]

ただし、無料であることは無制限利用を意味しない。無料枠は次の条件付きである。

- 5,000,000タイルリクエスト / 暦月
- ラスタとベクタの両サービスを合算
- 同一Customerが持つ全APIキーを合算
- CARTOとOpenStreetMapの帰属表示を維持
- 自分で発行したAPIキーだけを使い、他人と共有しない
- CARTO Basemap Termsに従う

5,000,000という上限は、CARTOのAPIキー申請ページと利用規約の両方に記載されている。[2][4] 利用規約上、無料サービスは取消可能で、上限超過時にはレート制限・停止・ブロックが行われる可能性がある。[4]

APIキー申請ページは無料枠を「非商用利用向け」と説明しつつ、商用プロジェクトにもキーを発行し、上限超過時には商用契約への移行を相談すると説明している。[2] したがって、商用利用を無料で無条件に継続できるとは判断しない。公開情報だけでは、超過時の具体的な価格や契約条件は確定できない。

### termrainのリクエスト量との比較

現在のtermrainは、レーダー取得1回あたり、背景地図を最大5列×3行、つまり最大15タイル取得する構造になっている。地図タイルはプロセス内メモリに`style + z + x + y`でキャッシュされるため、同じプロセス内の再取得ではキャッシュヒットが発生する。

キャッシュが一度も効かず、毎回15タイルを取得すると仮定しても、既定の600秒間隔を30日間続けた単純計算は次のとおりである。

```text
30日間の取得回数:       4,320回
4,320回 × 15タイル:    64,800タイル
無料枠5,000,000に対する割合: 1.296%
```

これはtermrain単体の一つの利用パターンに対する試算であり、CARTOが利用を保証する値ではない。地図スタイルの切替、移動、ズーム、複数プロセスの起動、別アプリで同じキーを使う場合は利用量が増える。APIキー対応を理由に無制限リトライや不要な先読みを追加してはならない。

## 現在の実装と不足している箇所

Issue #21の解決に必要な範囲では、次の実装を変更する。

| 層 | 現在の実装 | 不足しているもの |
|---|---|---|
| 設定 | `RadarConfig`に地図スタイルと道路幅がある | CARTOキーのフィールドがない（`src/config.rs:145`） |
| 設定反映 | `app::run`がスタイル等をproviderへ渡す（`src/app/mod.rs:51-62`） | キーをproviderへ渡す経路がない |
| URL | `MapStyle::tile_url`がキーなしURLを作る（`src/config.rs:112-132`） | CARTOだけに`key`を付ける処理がない |
| 日本の取得 | `Jma::fetch_map_image`がラスタタイルを取得する（`src/api/jma.rs:134-161`） | APIキー付きCARTOリクエストがない |
| 海外の取得 | `OpenMeteo::fetch_map_image`がラスタタイルを取得する（`src/api/open_meteo.rs:71-97`） | APIキー付きCARTOリクエストがない |
| UI | 地図スタイル名をレーダー枠のタイトルに表示する（`src/ui/radar.rs:72-98`） | CARTOに必要なOpenStreetMap/CARTO帰属と、キー未設定警告の表示が不足 |
| キャッシュ | スタイル、ズーム、タイル座標だけをメモリキーにする | 現状はキーをキャッシュへ保存していない。この性質を維持する必要がある |

設定ファイルにAPIキーを書くだけでは解決しない。現在のコードはキーを読み込まず、CARTO URLにも付加しないためである。

## 無料利用を前提にした推奨仕様

### 機能要件

#### FR-001: 設定ファイルでキーを指定できる

`[radar].carto_api_key`に`Option<String>`相当の任意設定を追加する。`serde(default)`を付け、既存の設定ファイルにこの項目がなくても読み込めるようにする。

設定例には実キーを置かず、次のプレースホルダーだけを使う。

```toml
[radar]
map_style = "carto_voyager"
carto_api_key = "YOUR_KEY"
```

初回実装では`CARTO_API_KEY`環境変数を必須にしない。設定ファイルを主経路にすることで、ユーザーが毎回`export`する必要をなくす。将来環境変数による一時的な上書きを追加する場合は、設定ファイルとの優先順位を別途仕様化する。

#### FR-002: CARTO Voyagerにだけキーを付ける

`MapStyle::CartoVoyager`を取得するときだけ、次の形式のURLへURLエンコードした`key`クエリパラメータを追加する。これはCARTO公式の案内と一致する。[2][5]

```text
https://basemaps.cartocdn.com/rastertiles/voyager/{z}/{x}/{y}.png?key=YOUR_KEY
```

OpenFreeMap、OpenStreetMap、国土地理院のリクエストにはCARTOキーを付けない。

#### FR-003: デフォルトと明示選択を分ける

`carto_api_key`が未設定でも、OpenFreeMap Libertyをデフォルトとして起動できる。`carto_voyager`は既存の5種類のサイクルに残すが、設定または`m`キーで明示的に選択したときだけ利用する。

#### FR-004: キー未設定時に未認証リクエストを送らない

`carto_voyager`が選択され、キーが未設定または空文字列の場合は、CARTOへリクエストを送らない。TUIを終了させず、次のようなキー不足の警告を表示する。

```text
CARTO VoyagerにはAPIキーが必要です。設定ファイルの[radar].carto_api_keyを確認してください。
```

OpenFreeMapへの暗黙のフォールバックは採用しない。ユーザーがCARTOを選択した事実と、実際に表示している地図が一致しなくなるためである。スタイルを変える場合はユーザーが`m`キーで選ぶ。

#### FR-005: APIキーを出力しない

実キーは次の場所へ出力しない。

- ログ
- TUIのエラー表示、エラー文字列
- HTTP URLを含むログや診断情報
- キャッシュキー、キャッシュパス、ファイル名
- README、設定例、テストfixture、コミット、PR、Issue

HTTPSリクエストのURLにキーが含まれることはCARTOの仕様上必要である。しかし、HTTPクライアントのエラーコンテキストにはURL全体を入れず、`CARTO Voyagerタイル取得失敗`のようなキーを含まない文言を使う。

#### FR-006: 帰属表示を維持する

CARTOの利用規約は、CARTO basemapを表示する際にOpenStreetMapとCARTOの両方を帰属表示し、表示を見た人にとって目立つ状態にすることを求めている。[4][7]

現在のレーダー枠タイトルは`Map: <style label>`を表示しているため、CARTO選択時のラベルを少なくとも次の情報を含む形へ変更する。

```text
CARTO Voyager (© OpenStreetMap contributors, © CARTO)
```

英語・日本語の表示幅を確認し、狭い端末でも帰属が欠落しない表示方法を決める。

### 非機能要件

#### NFR-001: ローカル設定を保護する

APIキーを保存する設定ファイルはユーザーのローカル専用とする。Unix系ではキーを保存するファイルを作成・更新するときに`0600`相当の権限を設定し、既存ファイルの権限が広すぎる場合は警告または安全な権限へ変更する。WindowsではユーザープロファイルのACLを前提にし、その扱いをドキュメントへ記載する。

APIキーはCARTOのAPIキー申請ページでも「共有せず、無関係なプロジェクトで再利用しない」よう案内されている。[2] リポジトリへ共有キーを追加して全利用者で使う方式は採用しない。

#### NFR-002: キャッシュに資格情報を含めない

現在の`style + z + x + y`というメモリキャッシュキーを維持する。URL文字列やAPIキーをキャッシュ識別子にしない。設定ファイルを次回起動時に変更した場合も、キャッシュはプロセス終了で破棄されるため、キー付き・キーなしのタイルが永続的に混ざらない。

#### NFR-003: 利用量を増やさない

APIキー対応のためにタイルの先読み、無制限リトライ、既存キャッシュを無視した再取得を追加しない。HTTPエラー時は既存のTUIの非クラッシュ方針に従い、欠落タイルとキー不足を安全なメッセージへ変換する。

#### NFR-004: ラスタ依存を隔離する

CARTOの公式資料は、ラスタ（PNG）basemapを古いサービスとして扱い、廃止方向であること、新規アプリにはベクタbasemapを推奨している。[2][5] 今回の実装はIssue #21を解決する短期の互換・オプトイン対応とし、CARTO Voyagerをデフォルトへ戻す理由にしない。

## 実装の進め方

### 推奨する変更順

1. `RadarConfig`に`#[serde(default)] carto_api_key: Option<String>`を追加する。
2. `WeatherProvider`にCARTOキーを渡す設定メソッドを追加し、`src/app/mod.rs`で設定を反映する。既存の`set_map_style`などと同じ起動時設定の流れに置く。
3. JMAとOpen-Meteoの両方のproviderが同じ仕様でキーを保持する。
4. CARTO URLの生成を共通化し、CARTOの場合だけクエリパラメータを追加する。キーのURLエンコードを手作業で行わず、既存依存の`urlencoding`またはURLのquery APIを使う。
5. キーを含むURLをエラーコンテキストやログへ渡さない。HTTPレスポンスのステータス、画像デコードエラー、キー不足をキーなしのメッセージへ変換する。
6. キー未設定時は送信前にエラー化し、レーダー全体をクラッシュさせずにTUIへ警告を伝える。現在、背景地図タイルエラーをログだけで処理している経路があるため、ユーザーに見える警告の伝達経路を確認する。
7. CARTOの帰属をレーダー枠タイトルへ追加する。
8. 設定ファイル保存処理のUnix権限を確認し、キーを保存する場合の権限を`0600`相当にする。
9. 英語・日本語READMEと設定例へ、キーの取得先、無料枠、ラスタ廃止方向、秘密情報の注意を追加する。実装前のこの設計文書だけでは、利用者向け設定が実装済みだと誤解させない。

### 変更しないもの

- OpenFreeMap Libertyのデフォルト設定
- OpenStreetMap、国土地理院、OpenFreeMapのURLと認証方式
- CARTO APIキーをGitHub Actions、リポジトリ設定、リリース成果物へ埋め込むこと
- CARTOの契約や利用量をtermrain側で管理すること
- CARTOのベクタbasemapを今回のIssueで同時実装すること

## テスト計画

### 単体テスト

- `carto_voyager`にキーを指定すると、URLのクエリに`key`が1つだけ追加される。
- 空白や`&`などを含むテスト用文字列がURLエンコードされ、別のクエリパラメータとして解釈されない。
- OpenFreeMap、OpenStreetMap、GSIのURLにCARTOキーが含まれない。
- `carto_api_key`のない旧形式TOMLを読み込むと`None`になり、デフォルトのOpenFreeMapが変わらない。
- キーを含むエラー文字列、ログ用文字列、キャッシュキーが生成されない。
- `carto_voyager`選択時のキー未設定で、HTTPクライアントが呼ばれず、TUI継続用のエラーになる。
- `RadarConfig`のTOML往復で、テスト用プレースホルダーが変化しない。

### 実サービスを使う手動確認

有効なキーを使った確認は、リポジトリ外の一時的なユーザー設定で行う。キーをコマンドライン引数、シェル履歴、ログ、スクリーンショット、Issueへ出さない。

確認項目は次のとおりである。

1. CARTOのキー申請ページから利用者自身のキーを取得する。[2]
2. `carto_voyager`とキーを一時設定へ入れてtermrainを起動する。
3. Voyagerタイルに`API KEY REQUIRED`透かしがなく、通常の地図が表示されることを確認する。
4. OpenFreeMapへ切り替え、CARTOキーなしのリクエストが正常に動くことを確認する。
5. キー不足時に未認証CARTOリクエストを送らず、TUIが終了しないことを確認する。
6. 実キーを含む一時設定を検証後に削除し、ログ・キャッシュ・リポジトリへ残っていないことを確認する。

## 受入条件

- [ ] FR-001: `[radar].carto_api_key`を設定ファイルで指定でき、キーなしの既存TOMLも読める。
- [ ] FR-002: 有効なキーを設定した`carto_voyager`で、CARTO公式の`key`クエリパラメータを使ってタイルを取得できる。
- [ ] FR-003: キーがなくてもOpenFreeMapをデフォルトとして起動でき、他の地図スタイルが変わらない。
- [ ] FR-004: キー未設定のCARTO選択で未認証リクエストを送らず、TUIをクラッシュさせずに警告を表示する。
- [ ] FR-005/NFR-002: 実キーがログ、エラー、キャッシュ、ドキュメント、テストfixture、Gitへ出ない。
- [ ] FR-006: CARTOとOpenStreetMapの帰属表示がレーダー画面で確認できる。
- [ ] NFR-001: キーを保存するUnix系設定ファイルが`0600`相当で作成・更新される。
- [ ] NFR-003: 不要な先読みや無制限リトライを追加せず、HTTPエラーでもTUIが継続する。
- [ ] NFR-004: CARTO Voyagerをデフォルトへ戻さず、ラスタ廃止方向をREADMEまたは設定ドキュメントへ記載する。
- [ ] `cargo fmt --all -- --check`、`cargo clippy --all-targets`、`cargo test --release`、`cargo build --release`が成功する。

## 未決事項

実装開始前に、次の項目だけ確認する。

- CARTOキー未設定時の警告をレーダー枠タイトル、`last_error`、専用メッセージのどの経路で表示するか。
- Windowsで設定ファイルのACLをどこまで検査・変更するか。Unixの`0600`と同じ動作を要求できない場合の案内文。
- 環境変数による一時上書きを将来追加するか。初回実装では設定ファイルのみを推奨する。
- 有効キーでの手動検証を誰の環境で行うか。キーの値は共有せず、結果だけを確認する。

## 関連Issue・PR

- [Issue #17: 背景地図で`API KEY REQUIRED`が表示される問題](https://github.com/iorinu/termrain/issues/17)
- [Issue #21: support authenticated CARTO Voyager basemap tiles](https://github.com/iorinu/termrain/issues/21)
- [PR #20: restore API-key-free map defaults](https://github.com/iorinu/termrain/pull/20)

## Sources

[1] https://github.com/iorinu/termrain/issues/21
[2] https://carto.com/basemaps/apikey
[3] https://carto.com/basemaps
[4] https://carto.com/legal/basemap-terms
[5] https://github.com/CartoDB/basemap-styles/blob/master/README.md
[6] https://github.com/CartoDB/basemap-styles/pull/48
[7] https://carto.com/attributions
