# 開発自動化と AI 駆動開発

termrain は、機械的に判定できる品質を GitHub Actions で検査し、設計・実装・レビューの補助をローカルの AI エージェントで行う。この分担により、AI の有用性を取り込みつつ、API 利用料・秘密情報・自動変更のリスクを抑える。

## 目的と非目的

### 目的

- push 前に、Codex でプロジェクト固有の観点を含む差分レビューを行う。
- pull request と `main` に対して、format・lint・build・test・依存関係監査を自動実行する。
- Cargo と GitHub Actions の依存更新を Dependabot で追跡する。
- AI の提案を、人間が理解・検証・承認できる小さな変更単位に保つ。

### 非目的

- AI に `main` への push、PR の merge、release、外部 API の更新を任せない。
- AI レビューを CI の必須品質ゲートや人間のレビューの代わりにしない。
- ChatGPT/Codex のサブスクリプション認証や秘密情報を GitHub Actions に渡さない。

## 役割分担

| 担当 | 責務 |
|---|---|
| 人間 | 要件、優先順位、設計判断、公開・merge・release の承認 |
| Codex | 差分の探索、具体的な不具合・読みやすさ・テスト不足の指摘 |
| GitHub Actions | format、Clippy、build、test、依存関係監査、workflow 構文検査 |
| Dependabot | 依存更新 PR の作成 |

## GitHub Actions

| Workflow | 起動条件 | 内容 |
|---|---|---|
| `ci.yml` | `main` への push、PR | Linux / macOS / Windows で format、Clippy、release build、tests |
| `security.yml` | `main` への push、PR、毎週月曜 03:17 UTC、手動実行 | `cargo audit` と `actionlint` |
| `release.yml` | `v*` tag の push、手動実行 | 配布バイナリの作成と GitHub Release の公開 |

`security.yml` と Dependabot は API key を使わない。`cargo audit` は既知の脆弱性を検出し、`actionlint` は GitHub Actions の構文・式の不整合を検出する。

## Dependabot

`.github/dependabot.yml` は Cargo と GitHub Actions の更新 PR を週次で作る。

- Cargo 更新は 1 PR にまとめる。
- GitHub Actions は更新単位ごとに PR を作るため、複数 PR が同時に届くことがある。
- メジャーバージョン更新は、変更内容と CI を確認してから merge する。
- Dependabot PR も通常の PR と同じく、CI・レビュー・人間の承認を通す。

## ローカル Codex レビュー

Codex CLI に ChatGPT/Codex アカウントでログイン済みなら、次で `origin/main..HEAD` のコミット差分をレビューできる。

```sh
scripts/review-with-codex.sh
```

比較する commit range を明示する場合:

```sh
scripts/review-with-codex.sh origin/release-branch HEAD
```

レビュー規約は `.github/codex-review-rules.md` にある。termrain 固有の非同期・TUI・外部 API の観点に加え、Readable Code / Clean Code の考え方に基づく命名、責務分割、条件分岐、重複、コメントの質を確認する。

### 安全上の境界

- スクリプトは `codex exec --sandbox read-only` を使うため、Codex はワークスペースを編集できない。
- Codex は diff を stdin で受け取り、レビュー対象として扱う。
- Codex CLI は repository の `AGENTS.md` を読み込む。そのため、このスクリプトは自分または信頼できる共同開発者のブランチにだけ使い、third-party fork や信頼できないブランチには使わない。
- コード変更を依頼する Codex セッションは、レビュー用スクリプトとは分ける。変更後は必ず diff、テスト、レビューを行う。

## Readable Code のレビュー基準

AI には、単に「短いコード」ではなく、将来の自分や共同開発者が安全に変更できるコードを確認させる。主な観点は以下。

- 名前が役割、単位、真偽条件を伝えるか。
- 関数が複数の独立した責務を混在させず、処理の流れを上から追えるか。
- 条件分岐・ネスト・early return が、正常系とエラー系の理解を妨げていないか。
- 同じ概念の別名やロジックの重複がないか。
- コメントがコードの言い換えではなく、外部 API の制約や設計上の理由を説明しているか。
- 複雑な非同期状態遷移、単位変換、public API が誤用されにくいか。

これらは一律のスタイル規則ではない。具体的な保守性の問題がある場合だけ、コード例と最小の改善案を伴って指摘する。

## opt-in pre-push hook

各開発者は、次のコマンドで個人用 hook を有効にできる。

```sh
scripts/install-pre-push-codex-review-hook.sh
```

既存の `.git/hooks/pre-push` は上書きしない。hook は Git が渡す local / remote SHA を読み、作業ツリーではなく実際に push する commit range をレビューする。

ネットワーク障害など緊急時だけ、理由を記録したうえで明示的に bypass できる。

```sh
TERMRAIN_SKIP_CODEX_REVIEW=1 git push
```

## 日常の開発フロー

```text
1. Issue / 要件から完了条件と制約を決める
2. AI と関連コード・既存テスト・影響範囲を調査する
3. 変更を小さく実装し、必要なテストを追加する
4. cargo fmt / clippy / build / test を実行する
5. commit した差分を scripts/review-with-codex.sh でレビューする
6. 指摘を確認して修正し、必要なら再レビューする
7. PR を作成し、GitHub Actions と人間のレビューを通す
8. 承認後に merge する
```

## PR 作成前チェックリスト

- [ ] 変更の目的と完了条件を PR 本文で説明できる。
- [ ] 新しい挙動には、正常系・失敗系・境界条件のテストがある。
- [ ] 外部 API のテストは live network ではなく fixture を使う。
- [ ] UI 変更では狭い端末と画像プロトコル非対応時を確認した。
- [ ] 可視文字列の変更時、英語・日本語の両方を更新した。
- [ ] `cargo fmt --all -- --check`、`cargo clippy --all-targets`、`cargo build --release`、`cargo test --release` が成功した。
- [ ] Codex のレビュー指摘を確認し、対応または対応しない理由を記録した。

## トラブルシューティング

| 状況 | 対応 |
|---|---|
| `codex` が見つからない | Codex CLI をインストールし、ログイン状態を確認する。 |
| `origin/main` が見つからない | `git fetch origin main` を実行する。 |
| hook が既に存在する | installer は上書きしない。既存 hook と統合する前に内容を確認する。 |
| Codex が利用上限・ネットワーク障害で失敗する | 原因を確認し、緊急時だけ bypass を使う。常用しない。 |
| Dependabot PR が多い | Cargo はグループ化済み。Actions 更新をまとめたい場合は `.github/dependabot.yml` の group を検討する。 |

Codex のレビューは助言であり、CI・人間のレビュー・テストの代わりにはならない。

## 設計資料から AI に実装を依頼する

責務分離などの中長期設計は、[AI 実行タスク](ai-tasks.md) の `ready` task に分けてから依頼する。これにより「docs を読んで次の task を進めて」のような短い依頼でも、AI は task ID、許可範囲、非対象、完了条件を確認してから作業できる。

`blocked` または `proposed` の task は設計判断が不足している状態であり、AI は実装を始めない。必要な判断や先行 task を報告する。
