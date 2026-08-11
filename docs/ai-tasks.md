# AI 実行タスク

この文書は、設計資料を AI が安全に実行できる小さな作業単位へ変換するための task manifest である。

利用者は task ID を知らなくても、たとえば次のように指示できる。

```text
- docs を読んで次の ready task を進めて
- 責務分離ロードマップの最初の実装可能な部分をやって
- ARC-001 を実施して
- この節に対応する task を実施して
```

AI は [開発者向けドキュメント](README.md)、関連する設計資料、この manifest、`AGENTS.md` を読んでから作業する。「next」または「最初の実装可能な部分」が指定された場合は、`Status: ready` のうち最も高い Priority の task を選ぶ。複数の task が同じ Priority で ready の場合だけ、AI は実装せず候補を示して確認する。

## 用語と識別子

- **ADR (Architecture Decision Record)** は、設計上の選択とその理由を残す記録である。コード変更を直接指示するものではない。詳しくは [責務分離のロードマップ: 今後の設計判断の記録](architecture-roadmap.md#今後の設計判断の記録) を参照する。
- **task** は、1 つの PR で実装・検証できる作業単位である。task ID は実装対象を指す。一方、ADR は task を `ready` にする前提となる判断を残すことがある。
- 以下の prefix は termrain 内だけで使うローカルな命名規則であり、一般的な標準用語ではない。

| Prefix | 展開 | 用途 |
|---|---|---|
| `ARC` | Architecture Refactoring Change | module の責務分離や依存方向を改善する task |
| `RAD` | Radar behavior | radar の状態遷移、取得世代、表示整合性に関する task |
| `MAP` | Map data | 地図データ、cache、検索に関する task |

## AI の実行プロトコル

各 task を実施する AI は、必ず次の順に進める。

1. task の ID、目的、依存関係、許可範囲、非対象を読み、作業対象を短く宣言する。
2. task が参照する source と既存 test を調査する。実装や API を推測しない。
3. 許可範囲だけを変更する。scope 外の改善候補は実装せず、最後に報告する。
4. task の完了条件にある検証を実行する。失敗時は根本原因を調べ、勝手に scope を広げない。
5. 完了時は、変更ファイル、実行した検証、残るリスク、次に unblock される task を報告する。
6. commit、push、PR、merge、release は利用者が明示的に依頼した場合だけ行う。

task が `blocked`、`proposed`、または依存 task 未完了なら、AI は実装を開始しない。なぜ開始できないかと、必要な前提を報告する。

## Status の意味

| Status | 意味 | AI の動作 |
|---|---|---|
| `ready` | 依存関係と完了条件が明確で、単独の PR にできる | 明示依頼または「次の ready task」で実施する |
| `blocked` | 先行 task や設計判断が必要 | 実装しない。blocker を説明する |
| `proposed` | 方向性だけが決まり、scope が未確定 | 実装しない。必要な設計判断を示す |
| `done` | 完了済み | 実装しない。成果物を参照する |

## task の書式

新しい task は、次の項目をすべて埋める。ID は上表の領域 prefix と連番を使う。例: `ARC-001`、`RAD-001`、`MAP-001`。新しい prefix を導入する場合は、この表に展開と用途を追加する。

```text
## TASK-ID — short title

- Status: ready | blocked | proposed | done
- Priority: high | medium | low
- Depends on: task ID または none
- Source: 関連する設計文書の見出し
- Goal: 完了時に実現すること
- Allowed paths: 変更してよい path
- Do not change: この task で変えてはいけない振る舞い・領域

### Investigation
実装前に確認する source、型、既存 test。

### Implementation
順番を守る必要がある小さな作業。

### Acceptance criteria
検証可能な完了条件。必ず test / command と観測すべき振る舞いを含める。

### Handoff
次に `ready` または unblock される task、残る判断。
```

## 現在の task

### ARC-001 — Characterize shared radar utility behavior

- Status: `ready`
- Priority: high
- Depends on: none
- Source: [責務分離のロードマップ: 優先する最初の境界](architecture-roadmap.md#優先する最初の境界-provider-共通処理)
- Goal: JMA と Open-Meteo が共有しているレーダー helper の現在の振る舞いを、ネットワーク不要の test で固定する。
- Allowed paths: `src/api/jma.rs`, `src/api/open_meteo.rs`, `src/api/mod.rs`, `src/api/` 配下の test 用 module、`docs/ai-tasks.md` の ARC-001 / ARC-002 status と handoff
- Do not change: 外部 API URL、timeout、cache key、公開 CLI、`RadarGrid` の意味、画像 protocol の選択、タイル helper の配置

### Investigation

- `src/api/open_meteo.rs` が `src/api/jma.rs` から import している `blend`、`draw_cross`、`draw_legend_bar`、`lonlat_to_tile`、`sample_bilinear`、`tile_to_lonlat` を確認する。
- 現在ある test と、各 helper の入力・出力・境界条件を確認する。
- テスト対象は pure な計算・画像処理だけにし、実際の JMA、RainViewer、地図タイルへの HTTP 要求を行わない。

### Implementation

1. タイル座標変換の代表地点・境界値について、現在の結果を固定する test を追加する。
2. 補間、alpha blend、legend、中心 cross のうち、将来 `tiles` module に移す helper を test する。
3. 期待値は「現状の仕様」を説明する名前にし、単に実装詳細をなぞらない。
4. 既存 production code の module 移動は行わない。

### Acceptance criteria

- [ ] 追加 test は外部ネットワークなしで実行できる。
- [ ] `cargo test --release` が成功する。
- [ ] `cargo fmt --all -- --check` と `cargo clippy --all-targets` が成功する。
- [ ] JMA と Open-Meteo の production import / module 構造は変わらない。
- [ ] PR 本文で、ARC-002 が依存できるようになった test を説明できる。

### Handoff

すべての完了条件を満たした場合、ARC-001 を `done`、ARC-002 を `ready` に変更する。helper の出力が意図と異なる場合は、ARC-001 を `done` にせず、移動せず別の bug-fix task として切り出す。

### ARC-002 — Extract provider-independent radar utilities

- Status: `blocked`
- Priority: high
- Depends on: ARC-001
- Source: [責務分離のロードマップ: 優先する最初の境界](architecture-roadmap.md#優先する最初の境界-provider-共通処理)
- Goal: provider 非依存のタイル・画像 helper を private な共通 module に置き、Open-Meteo が JMA implementation helper に依存しないようにする。
- Allowed paths: `src/api/mod.rs`, `src/api/jma.rs`, `src/api/open_meteo.rs`, `src/api/tiles.rs`, 対応する test
- Do not change: API URL、外部 API の time semantics、cache の key / lifetime、レーダーの表示結果、provider 選択規則

### Investigation

- ARC-001 の test が移動対象を十分に覆っていることを確認する。
- `pub(crate)` が必要な範囲だけになるよう、Rust module privacy を設計する。
- helper が実際に provider 非依存か確認する。JMA の色変換・時刻・cache policy は移動対象に含めない。

### Implementation

1. `src/api/tiles.rs` を private module として追加する。
2. provider 非依存 helper を、関数シグネチャと振る舞いを変えずに移動する。
3. JMA と Open-Meteo の import を新しい module に向ける。
4. 無関係な整形・rename・最適化をしない。

### Acceptance criteria

- [ ] Open-Meteo が `super::jma` の helper を import しない。
- [ ] ARC-001 の test が移動後も成功する。
- [ ] `cargo fmt --all -- --check`、`cargo clippy --all-targets`、`cargo build --release`、`cargo test --release` が成功する。
- [ ] 同一の入力に対する tile 座標、画像サイズ、`RadarGrid` bounds が変わらないことを test または既存観測で説明できる。

### Handoff

JMA provider の分割 task を具体化できる。JMA 固有の forecast / radar / cache / decode はまだ分けない。

### RAD-001 — Correlate radar failures with request generations

- Status: `proposed`
- Priority: high
- Depends on: none
- Source: [責務分離のロードマップ: 失敗状態を設計に含める](architecture-roadmap.md#失敗状態を設計に含める)
- Goal: 最新のレーダー取得が失敗した場合に loading state を安全に解消し、古い失敗結果が新しい操作の UI を上書きしないようにする。
- Allowed paths: `src/app/state.rs`, `src/app/fetch.rs`, `src/app/input.rs`, 対応する test
- Do not change: provider の HTTP 実装、UI layout、外部 API のエラー文言

### Blocker

`Msg::Error(String)` を request id と取得種別を持つ型へどこまで拡張するか、state 遷移を Action / reducer へ進める前に決める必要がある。まず ADR または小さな設計 task で、error message の型と「既存の表示を残す」ポリシーを確定する。

### Handoff

設計決定後に `RAD-001` を `ready` へ変更する。実装 task は「最新 request の成功・失敗」「古い request の成功・失敗」を独立して test する。
