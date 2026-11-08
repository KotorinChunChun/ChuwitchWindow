# ChuwitchWindow 開発者ガイド

ビルド・構成・テストの公開開発者向けガイド。必須ルールと資料目録の正本は [AGENTS.md](../AGENTS.md)、配布・公開手順の正本は [dev/RELEASE_RULE.md](../dev/RELEASE_RULE.md)。

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

フロントは Vitest + jsdom（`src/**/*.test.ts(x)`）。Rust は `#[cfg(test)]` の単体テスト。タイトルバーの判定は非表示の試験ウィンドウで確認する。

### CLIからの自動判定

KotorinToysに収録された開発版では、ChuwitchWindowのディレクトリから `pwsh` で次のように実行できます。`$runLog` には対象版の `dev/history/vX.Y.Z/logs/<実行識別子>/` を指定します。

```powershell
./dev/scripts/test-debug-ChuwitchWindow-all.ps1 -List
./dev/scripts/test-debug-ChuwitchWindow-all.ps1 -Feature Drag -LogDirectory $runLog
./dev/scripts/test-debug-ChuwitchWindow-all.ps1 -Feature All -LogDirectory $runLog
./dev/scripts/test-debug-ChuwitchWindow-all.ps1 -Feature Runtime -RunningProcessId $appProcessId -LogDirectory $runLog
```

`Drag` はタイトルバー、グループ選定と前面化、吸着、モニター間のXY補正を検証します。`MonitorMovement` と `Arrange` では、仮想の作業領域を使い、実際の試験窓を動かして座標・サイズ・元に戻す処理を検証します。`Runtime` は `--startup` で起動済みの対象PIDについて、管理窓口の応答と設定画面の非表示を確認します。`All` に `-RunningProcessId` を指定すると、実行中のアプリの検証も追加できます。

結果はJSONで返し、選んだ試験がすべて通過すると終了コードは `0`、失敗または試験が0件なら `1` です。`All` は登録済みの自動試験をすべて実行します。物理入力、実モニターの着脱、通知領域、Windowsへの設定登録など、自動判定の対象外はJSONの `manualChecks` に含まれます。

## プロジェクト構成

```
├── AGENTS.md                  # 必須ルール・配置・資料目録
├── README.md
├── LICENSE
├── docs/                      # 利用者向け
│   ├── USERS_GUIDE.md
│   ├── DEVELOPERS_GUIDE.md    # 本ファイル
│   ├── FAQ.md
│   └── *.png
├── dev/                       # 開発者本人の計画・判断・記録（正式資料は Git 追跡）
│   ├── README.md              # 開発資料の入口
│   ├── RELEASE_RULE.md        # 配布・公開手順の正本
│   ├── scripts/               # 実在する運用スクリプト（Git 追跡）
│   ├── private/               # 非公開資料（Git 対象外）
│   ├── temp/                  # プロジェクト外の一時制作物（Git 対象外）
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

次の補助スクリプトで上記 3 ファイルと `package-lock.json` を更新できる。実行後は差分を照合する。

```powershell
pwsh -File .\dev\scripts\update-ChuwitchWindow-version.ps1 -NewVersion "0.2.2"
```

## リリース手順（本リポジトリ固有）

プロジェクト固有の手順は [dev/RELEASE_RULE.md](../dev/RELEASE_RULE.md)。署名は MSI X 側の別工程（Q12）であり、本リポジトリでは行わない。外部公開は明示依頼時だけ行う。

すべて **pwsh**。

```powershell
cd C:\develop\ChuwitchWindow
npm test
cargo test --manifest-path src-tauri/Cargo.toml
# バージョンを対象版へ揃える
npm run tauri build
```

- exe: `src-tauri/target/release/chuwitchwindow.exe`
- NSIS: `src-tauri/target/release/bundle/nsis/`
- zip には exe と README.md
- タグ例: `v0.2.2`
- 公開控えはローカル `release/`（Git 対象外）

品質ゲート: [v0.2.2-quality-gates.md](../dev/v0.2.2-quality-gates.md)。

GitHub 公開を伴うスクリプトは `dev/scripts/publish-release-ChuwitchWindow-all.ps1`。公開の明示依頼がある場合だけ使用し、通常のビルド確認では実行しない。

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
