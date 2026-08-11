# 開発者向けドキュメント

termrain を開発・保守するための資料です。利用方法はルートの [README](../README.md) と [日本語 README](../README.ja.md) を参照してください。

| ドキュメント | 内容 |
|---|---|
| [内部設計](architecture.md) | 層ごとの責務、起動・データフロー、非同期レーダーの整合性、変更の置き場所 |
| [責務分離のロードマップ](architecture-roadmap.md) | 現状の分離候補、目標 module 構成、段階的な移行順序、ADR の運用 |
| [AI 実行タスク](ai-tasks.md) | 曖昧な依頼でも AI が安全に実行できる task ID、scope、完了条件、依存関係 |
| [開発自動化と AI 駆動開発](development-automation.md) | ローカル Codex レビュー、GitHub Actions、Dependabot、セキュリティ上の境界、日常の開発フロー |
| [スクリーンショット](screenshots/README.md) | README / PR 用のスクリーンショット管理 |
| [Homebrew Formula](homebrew/termrain.rb) | tap で配布する Formula の参照コピー |

## 原則

- 決定的に検査できることは CI で検査する。
- AI は設計・レビューの補助に使い、テスト・人間の判断・リリース承認を置き換えない。
- API key、トークン、個人情報をリポジトリ・ログ・fixture に含めない。
- 利用者に見える振る舞いを変える場合は、英語・日本語の README と CHANGELOG の更新要否を確認する。
- ADR は設計判断の記録、task は実装・検証可能な作業単位である。task ID prefix の意味は [AI 実行タスク](ai-tasks.md#用語と識別子) に定義する。
