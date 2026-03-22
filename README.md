# SessionDock

Claude Codeの全セッションを一画面で監視。終わったら音で教えてくれる。

**[Website](https://adachic.github.io/sessiondock/)** · **[Download](https://github.com/adachic/sessiondock/releases/download/v0.1.0/SessionDock_0.1.0_aarch64.dmg)** · **[@norio2026](https://x.com/norio2026)**

## Features

- **一覧表示** — 複数ターミナルで走るClaude Codeのセッションをリアルタイム表示
- **通知音** — 入力待ちになったら3秒後に音でお知らせ。完了時もmacOS通知
- **コスト表示** — 各セッションのAPI費用・コンテキスト残量を常時表示
- **コンパクト/詳細** — 表示モードを切り替え可能
- **多言語対応** — English / 日本語 / 中文 / 한국어（システム言語を自動検出）
- **完全ローカル** — `~/.claude/` を読むだけ。外部通信なし。APIキー不要

## Install

### Homebrew (推奨)
```bash
brew install --cask adachic/tap/sessiondock
```

### Direct Download
[GitHub Releases](https://github.com/adachic/sessiondock/releases) から `.dmg` をダウンロード（Apple Silicon）

## How it works

Claude Codeは `~/.claude/sessions/` にセッション情報を、`~/.claude/projects/` に会話ログをJSONLで保存しています。SessionDockはこれらのファイルをリアルタイムで監視し、各セッションの状態（実行中/入力待ち/完了）、トークン消費量、API費用を表示します。

## Specs

| | |
|---|---|
| App Size | 3.6 MB |
| Cost | Free / Open Source |
| Network | Zero (fully local) |
| Platform | macOS 13+ / Apple Silicon |
| Languages | EN / JP / CN / KR |
| Built with | Tauri v2 (Rust + React) |

## License

MIT
