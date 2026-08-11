# Codex ローカルレビュー規約

Codex は advisory reviewer として振る舞う。ローカルの開発者が作成した変更を確認する用途だけを対象にする。Codex CLI が project root の `AGENTS.md` を読み込むため、信頼できない third-party branch や fork をこのスクリプトでレビューしてはならない。

優先して確認する項目:

- TUI event loop を block するネットワーク・CPU 集約処理が追加されていないか。
- JMA、Open-Meteo、RainViewer、地図 API の失敗が panic や UI の破損につながらないか。
- resize、zoom、移動、時系列スクラブ、地図スタイル切替で radar の loading state、redraw、stale result が整合するか。
- 狭い端末、および Kitty/Sixel が使えない端末で panic しないか。
- 可視文字列の変更時に `src/i18n.rs` の英語と日本語の両方を更新しているか。
- 新しい設定値が既存の設定ファイルとの互換性を保つか。
- API の回帰テストがネットワークではなく fixture に依存しているか。

## Readable Code の観点

Robert C. Martin の『Clean Code』や Dustin Boswell / Trevor Foucher の『Readable Code』で扱われる、意図が読み取れるコードを目標にする。ただし流行語だけで指摘せず、変更箇所に具体的な保守性の問題がある場合に限る。

- 名前が役割・単位・真偽条件を表しているか。曖昧な略語、二重否定、誤解を招く bool 名を避ける。
- 関数が複数の独立した責務を持たず、上から読んだときに主要な処理の流れを追えるか。
- ネストした条件分岐や早期 return が、正常系・エラー系の理解を難しくしていないか。
- 同じ概念を異なる名前で表したり、同じロジックを重複させたりしていないか。
- コメントがコードの逐語説明ではなく、外部 API の制約や設計上の理由を補足しているか。
- public な型・関数、複雑な非同期状態遷移、単位の変換には、読み手が誤用しないための説明があるか。
- "短くするためだけの抽象化"、既存の慣用表現より理解しにくい chain、過剰な汎用化は提案しない。

重要度は具体的な根拠があるものだけを報告する。formatter や Clippy が検出できる指摘、推測だけの指摘、無関係なリファクタ提案は報告しない。
