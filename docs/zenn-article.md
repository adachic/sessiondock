---
title: "Claude Codeのセッションをまとめて管理できるアプリ「SessionDock」を作ったぞい"
emoji: "📊"
type: "tech"
topics: ["claudecode", "ai", "rust", "tauri", "macOS"]
published: true
---

## 何を作ったか

**SessionDock** — Claude Codeで複数セッションを並行して走らせてる時に、全部まとめて一画面で監視できるmacOSアプリです。

https://adachic.github.io/sessiondock/

GitHub: https://github.com/adachic/sessiondock

## なんで作ったか

Claude Codeをガッツリ使ってると、気づいたらこうなってませんか。

```
ターミナルタブ1: Claude Code（フロントエンド実装中）
ターミナルタブ2: Claude Code（API設計中）
ターミナルタブ3: Claude Code（テスト書いてる）
ターミナルタブ4: Claude Code（ドキュメント生成中）
ターミナルタブ5: Claude Code（バグ修正中）
```

で、こうなる。

**「どのタブが動いてるか分からない」** — タブのタイトルが全部「claude」で区別つかない。1個ずつ切り替えて確認するの、地味にだるい。

**「終わったのに気づかず放置」** — 処理終わってるのに30分気づかなかった、とかザラにある。時間もったいない。

**「何を頼んだか忘れる」** — 3つ以上並行すると「あのタブ何やってたっけ」が頻発。タブ開いてスクロールして思い出す作業が発生。

**「コストが見えない」** — 5セッション並行で合計いくらトークン使ったか分からない。月末に請求見て青ざめるやつ。

全部自分が困ってたことなので、作りました。

## 機能

### 全セッション一覧表示

メニューバーに常駐して、クリックするとダッシュボードが開きます。

各セッションに表示される情報:
- **状態**: 実行中 / 入力待ち / 完了
- **プロジェクト名**: どのディレクトリで動いてるか
- **テーマ**: 最初にユーザーが送ったリクエスト（何の作業か一目で分かる）
- **現在のタスク**: 今何をやっているか
- **経過時間**: その状態になってから何秒/何分経ったか

コンパクト表示と詳細表示を切り替えられます。

### 入力待ちになったら3秒後に音で通知

これが一番欲しかった機能。

Claude Codeが処理を終えてユーザー入力待ちになると、3秒後に `Glass.aiff` が鳴ります。3秒待つのは、ツール実行の一時停止（すぐRunningに戻る）で鳴らないようにするため。

本当に入力待ちの時だけ鳴る。これだけでQOLが爆上がりする。

### API費用 & コンテキスト残量

各セッションごとに:
- **トークン消費量**: 入力/出力の累計
- **推定API費用**: モデル別の単価で自動計算（Opus $5/$25, Sonnet $3/$15, Haiku $1/$5 per 1M tokens）
- **コンテキスト残量**: 直近リクエストのコンテキスト使用量 / ウィンドウ上限の%表示

「今日合計でいくら使ったか」もフッターに表示。

## 技術的な話

### どうやってセッション情報を取ってるか

Claude Codeは `~/.claude/` 配下にセッション情報を保存してます。SessionDockはこれを読んでるだけ。

```
~/.claude/
├── sessions/           ← セッション一覧（PID, 作業ディレクトリ, 開始時刻）
│   ├── 4286.json
│   └── 50840.json
└── projects/           ← 会話ログ（JSONL形式でリアルタイム追記）
    └── -Users-you-myproject/
        └── {sessionId}.jsonl
```

#### sessions/*.json の中身

```json
{
  "pid": 4286,
  "sessionId": "bf321dd5-d32b-4df1-8ee5-2c837d9ed978",
  "cwd": "/Users/you/mypro/myapp",
  "startedAt": 1774145027279
}
```

PID、セッションID、作業ディレクトリ、開始時刻。これでセッション一覧が作れる。

#### JSONL会話ログの中身

各行がJSON。`type` フィールドで種類が分かる:

```json
{"type": "user", "message": {"content": "テストを書いて"}, "timestamp": "..."}
{"type": "assistant", "message": {"model": "claude-opus-4-6", "stop_reason": "end_turn", "usage": {"input_tokens": 8000, "output_tokens": 500}}, ...}
{"type": "system", "subtype": "turn_duration", "durationMs": 42000, ...}
```

### 状態判定ロジック

ここが一番ハマったところ。シンプルに見えて罠が多い。

```
状態判定:
1. PIDが生きてない → 完了
2. PIDが生きてるがプロセス名がclaude/nodeじゃない → 完了（PID再利用）
3. 最後のassistantエントリの stop_reason が:
   - "end_turn" or null → 入力待ち
   - "tool_use" → 実行中（ツール使用してまだ続く）
4. 最後のエントリが "user" type で:
   - content に "text" がある → 実行中（ユーザーが送信、Claudeが処理開始）
   - content に "tool_result" しかない → 状態変更なし（ツール結果の返却であってユーザー入力ではない）
5. isSidechain: true のエントリ → 無視（サブエージェントのログ）
```

特に4番目が厄介で、Claude Codeの `user` type エントリの大半は実は**ツール実行結果の返却**（`tool_result`）。これを「ユーザーが入力した」と判定すると、入力待ちなのに実行中に戻ってしまう。

あと5番目。Claude Codeがサブエージェント（Agent tool）を使うと、`isSidechain: true` のエントリが同じJSONLに書き込まれる。これを拾うとメインの会話の状態がめちゃくちゃになる。

### 技術スタック

| レイヤー | 技術 |
|---------|------|
| フレームワーク | **Tauri v2** |
| バックエンド | **Rust** |
| フロントエンド | **React + TypeScript** |
| ファイル監視 | notify (Rust crate) |
| プロセス監視 | sysinfo (Rust crate) |
| 通知 | tauri-plugin-notification |
| 通知音 | afplay (macOS標準) |

**なぜTauriか**: Electronだと150MB超えるアプリが、Tauriなら**3.6MB**。メモリ消費も20-50MBで済む。メニューバー常駐アプリとしてはTauri一択。

## 開発にかかった時間

企画から公開まで**1日**。

といっても自分（人間）がやったのは「こういうの欲しい」と言っただけで、Claude Code（このアプリで監視される側）が全部書いてくれた。マッチポンプ感ある。

## インストール

### Homebrew（推奨）
```bash
brew install --cask adachic/tap/sessiondock
```

### Direct Download
[GitHub Releases](https://github.com/adachic/sessiondock/releases) から `.dmg` をダウンロード（3.6MB, Apple Silicon）

## スペック

| | |
|---|---|
| アプリサイズ | 3.6MB |
| 費用 | 無料 / OSS |
| 外部通信 | なし（完全ローカル） |
| 対応 | macOS 13+ / Apple Silicon |

## まとめ

Claude Codeを複数走らせてる人は試してみてください。特に「終わったら音が鳴る」だけで体験がだいぶ変わります。

フィードバック・PR歓迎です。

- LP: https://adachic.github.io/sessiondock/
- GitHub: https://github.com/adachic/sessiondock
- X: [@norio2026](https://x.com/norio2026)
