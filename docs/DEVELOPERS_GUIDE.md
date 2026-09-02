# ChuwitchWindow 開発者ガイド

ビルド・構成・リリース。方針の原本は `dev/templates/PROJECT_RULE.md`。

利用者向けは [README.md](../README.md) と [USERS_GUIDE.md](USERS_GUIDE.md)。

## 開発手法

スタートアップ常駐のため Rust を採用し、トレイ時のバイナリと消費メモリを 10MB 級に抑えています。設定 UI は React です。設定画面を開くと WebView2 が載り約 250MB になります。スタートアップでは設定画面を出さず、閉じたら破棄して回避しています。

出荷済み仕様: [v0.1.0-req.md](../dev/history/v0.1.0-req.md)、[v0.2.1-req.md](../dev/history/v0.2.1-req.md)。  
現行版（0.2.2）: [v0.2.2-req.md](../dev/v0.2.2-req.md) / [v0.2.2-imp.md](../dev/v0.2.2-imp.md) / [v0.2.2-task.md](../dev/v0.2.2-task.md)。

## ビルド方法

前提: Node.js v18 以上、Rust、Tauri CLI の前提（Visual Studio Build Tools 等）。

```powershell
npm install
npm run tauri dev
npm run tauri build
```

成果物: `src-tauri/target/release/chuwitchwindow.exe`。インストーラは `src-tauri/target/release/bundle/nsis/`。

テスト:

```powershell
npm test
cargo test --manifest-path src-tauri/Cargo.toml
```

フロントは Vitest + jsdom（`src/**/*.test.ts(x)`）。Rust は `#[cfg(test)]` の単体テスト。どちらも実 HWND や `SetWindowPos` は呼ばない。

## プロジェクト構成

```
├── README.md
├── LICENSE
├── docs/                      # 利用者向け
│   ├── USERS_GUIDE.md
│   ├── DEVELOPERS_GUIDE.md    # 本ファイル
│   ├── FAQ.md
│   └── *.png
├── dev/                       # 開発者・AI（Git 追跡。*.ps1 と scripts/ は除く）
│   ├── templates/
│   ├── history/               # 完了した版の要求
│   ├── PLAN_IDEAS.md
│   ├── PLAN_REJECT.md
│   ├── v0.2.2-req.md
│   ├── v0.2.2-imp.md
│   ├── v0.2.2-task.md
│   └── v0.2.2-*.md            # 現行版の設計・テスト・UX（D01〜D09 等）
├── src/                       # フロント（React + TypeScript）
├── src-tauri/                 # バックエンド（Rust / Tauri 2）
└── release/                   # Git 対象外。成果物のローカル控え
```

`src-tauri/src/` の主なモジュール: `lib.rs`, `commands.rs`, `config.rs`, `monitor.rs`, `window.rs`, `logic.rs`, `hotkey.rs`, `tray.rs`, `hotplug.rs`, `history.rs`, `admin.rs`, `logger.rs`, `pin.rs`, `rule.rs`, `arrange.rs`。

## バージョン管理

次の3箇所（および `getVersion()` が読む `tauri.conf.json`）を揃える。

1. `src-tauri/Cargo.toml`
2. `src-tauri/tauri.conf.json`
3. `package.json`

一括更新スクリプトがあれば **ローカル** `dev/scripts/update_version.ps1`（Git 対象外）。リポジトリには置かない（決定 Q14-B）。

```powershell
pwsh -File .\dev\scripts\update_version.ps1 -NewVersion "0.2.2"
```

## リリース手順（本リポジトリ固有）

汎用テンプレは [dev/templates/RELEASE_RULE.md](../dev/templates/RELEASE_RULE.md)。署名は MSI X 側（Q12）。自動化はローカルのみ。

すべて **pwsh**。

```powershell
cd C:\develop\ChuwitchWindow
npm test
cargo test --manifest-path src-tauri/Cargo.toml
# バージョンを 0.2.2 に揃える
npm run tauri build
```

- exe: `src-tauri/target/release/chuwitchwindow.exe`
- NSIS: `src-tauri/target/release/bundle/nsis/`
- zip には exe と README.md
- タグ例: `v0.2.2`
- 公開控えはローカル `release/Publish/`（Git 対象外）

品質ゲート: [v0.2.2-quality-gates.md](../dev/v0.2.2-quality-gates.md)。

## 技術スタック

| レイヤー       | 技術                                                         |
| -------------- | ------------------------------------------------------------ |
| フレームワーク | [Tauri 2](https://tauri.app/)                                |
| フロントエンド | React 18 + TypeScript                                        |
| スタイリング   | [Tailwind CSS v4](https://tailwindcss.com/)                  |
| アニメーション | [Framer Motion](https://motion.dev/)                         |
| アイコン       | [Lucide React](https://lucide.dev/)                          |
| バックエンド   | Rust + [windows-rs](https://github.com/microsoft/windows-rs) |
| ビルドツール   | Vite 7                                                       |

## ライセンス

MIT。ルートの `LICENSE`。
