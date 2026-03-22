# SessionDock

Claude Codeの全セッションを一画面で監視。終わったら音で教えてくれる。

## Features

- **一覧表示** — 複数ターミナルで走るClaude Codeのセッションをリアルタイム表示
- **通知音** — 入力待ちになったら3秒後に音でお知らせ。完了時もmacOS通知
- **コスト表示** — 各セッションのAPI費用・コンテキスト残量を常時表示
- **完全ローカル** — `~/.claude/` を読むだけ。外部通信なし。APIキー不要

## Install

### Homebrew (推奨)
```bash
brew install --cask adachic/tap/sessiondock
```

### Direct Download
[GitHub Releases](https://github.com/adachic/sessiondock/releases) から `.dmg` をダウンロード

## How it works

Claude Codeは `~/.claude/sessions/` にセッション情報を、`~/.claude/projects/` に会話ログをJSONLで保存しています。SessionDockはこれらのファイルをリアルタイムで監視し、各セッションの状態（実行中/入力待ち/完了）、トークン消費量、API費用を表示します。

## Specs

| | |
|---|---|
| App Size | 3.6 MB |
| Cost | Free / Open Source |
| Network | Zero (fully local) |
| Built with | Tauri (Rust + React) |

## License

MIT
