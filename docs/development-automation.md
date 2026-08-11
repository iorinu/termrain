# Development automation

termrain は、決定的な品質検査を GitHub Actions で実行し、設計的なレビューは各開発者がローカルの Codex CLI で行う。

## GitHub Actions

- `ci.yml`: format、Clippy、release build、tests を Linux / macOS / Windows で実行する。
- `security.yml`: `cargo audit` と GitHub Actions workflow の `actionlint` を実行する。毎週月曜日 03:17 UTC にも実行する。
- `.github/dependabot.yml`: Cargo と GitHub Actions の更新 PR を毎週作成する。

これらの workflow は OpenAI などの API key を使わない。

## ローカル Codex レビュー

Codex CLI に ChatGPT/Codex アカウントでログイン済みであれば、次で `origin/main..HEAD` のコミット差分をレビューできる。

```sh
scripts/review-with-codex.sh
```

比較する commit range を明示する場合:

```sh
scripts/review-with-codex.sh origin/release-branch HEAD
```

レビュー規約は `.github/codex-review-rules.md` にある。termrain 固有の非同期・TUI・外部 API の観点に加え、Readable Code / Clean Code の考え方に基づく命名、責務分割、条件分岐、重複、コメントの質も確認する。Codex は read-only sandbox でレビュー専用に使い、コード変更、commit、push、PR コメント投稿を行わせない。Codex CLI は repository の `AGENTS.md` を読み込むため、信頼できない third-party branch や fork のレビューには使わない。

## opt-in pre-push hook

各開発者は、次のコマンドで個人用 hook を有効にできる。

```sh
scripts/install-pre-push-codex-review-hook.sh
```

既存の `.git/hooks/pre-push` は上書きしない。hook 有効後は push 前に Codex review が成功する必要がある。ネットワーク障害など緊急時だけ、理由を記録したうえで次のように明示的に bypass できる。

```sh
TERMRAIN_SKIP_CODEX_REVIEW=1 git push
```

## 日常の流れ

```text
変更 → cargo fmt / clippy / test → scripts/review-with-codex.sh → commit / push → GitHub CI
```

Codex のレビューは助言であり、CI・人間のレビュー・テストの代わりにはならない。
