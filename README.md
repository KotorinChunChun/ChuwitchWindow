# ChuwitchWindow

マルチモニター環境向けの **ウィンドウ配置入れ替えツール** です。  
ショートカットキー一発で、開いているウィンドウ群を別のディスプレイへ一斉移動・交換できます。

## ✨ 主な機能

| 機能 | 説明 | デフォルトキー |
|------|------|---------------|
| **巡回シフト（増加方向）** | 全ウィンドウを次のモニターへ移動（1→2→3→1） | `Win+Ctrl+Alt+→` |
| **巡回シフト（減少方向）** | 全ウィンドウを前のモニターへ移動（1→3→2→1） | `Win+Ctrl+Alt+←` |
| **指定モニターとの入れ替え** | プライマリモニターと指定番号のモニター間でウィンドウを交換 | `Win+Ctrl+Alt+2〜9` |
| **Undo（元に戻す）** | 直前の入れ替え操作を取り消し | `Win+Ctrl+Alt+Z` |
| **モニターグループ** | 同じグループ内のモニター間のみで入れ替えを実行 | — |
| **OSD通知** | 切り替え後に移動先をモニター中央に数秒間表示 | — |

## 🖥️ 対応環境

- **OS**: Windows 10 / 11
- **必要要件**: 特になし（単体 `.exe` で動作）

## 📦 インストール

1. [Releases](https://github.com/KotorinChunChun/ChuwitchWindow/releases) のAssetsから最新の `.zip` をダウンロード
2. 任意のフォルダに配置して解凍して `setup.exe` を実行

設定は `%APPDATA%\com.kotorichun\chuwitchwindow\config.json` に自動保存されます。

プログラムはインストールされるため、設定からアンインストールが可能です。

## 🚀 使い方

### 基本操作

1. アプリを起動するとタスクトレイに常駐します
2. ショートカットキーでウィンドウの入れ替えを実行できます
3. 設定画面では、ショートカットキーのカスタマイズやモニターグループの設定が行えます

### トレイアイコン

| 操作 | 動作 |
|------|------|
| ダブルクリック | 設定画面を表示 |
| 右クリック → 設定 | 設定画面を表示 |
| 右クリック → 再起動 | アプリを再起動 |
| 右クリック → 終了 | アプリを終了 |

### ウィンドウの動作

- **閉じる（×ボタン）** → タスクトレイに格納（アプリは終了しません）
- **最小化** → タスクトレイに格納

### 設定画面

設定画面は3つのタブで構成されています。

- **モニター構成**: モニターの物理配置をグラフィカルに表示。クリックでグループ色を変更し、同じ色のモニター間でウィンドウを一斉交換できます。
- **設定・常駐**: ショートカットキーの変更、スタートアップ登録、動作設定を行います。
- **アプリについて**: バージョン情報、GitHubリンク、設定のリセット機能があります。

## 🔧 ビルド方法

### 前提条件

- [Node.js](https://nodejs.org/) (v18以上)
- [Rust](https://www.rust-lang.org/tools/install)
- [Tauri CLI](https://tauri.app/) の前提条件（Visual Studio Build Tools 等）

### ビルド手順

```bash
# 依存関係のインストール
npm install

# 開発サーバーの起動
npm run tauri dev

# リリースビルド
npm run tauri build
```

ビルド成果物は `src-tauri/target/release/chuwitchwindow.exe` に出力されます。

## 📁 プロジェクト構成

```
├── src/                    # フロントエンド (React + TypeScript)
│   ├── App.tsx             # メインアプリケーション
│   ├── components/
│   │   ├── MonitorMap.tsx   # モニター配置マップ
│   │   └── SettingsPanel.tsx # 設定パネル
│   ├── lib/
│   │   └── utils.ts        # ユーティリティ (cn 関数等)
│   └── types/
│       └── index.ts        # 型定義
├── src-tauri/              # バックエンド (Rust / Tauri 2)
│   └── src/
│       ├── lib.rs          # エントリポイント・コマンド定義
│       ├── config.rs       # 設定の読み書き
│       ├── monitor.rs      # モニター情報の取得
│       ├── window.rs       # ウィンドウ操作
│       ├── logic.rs        # 入れ替えロジック
│       ├── hotkey.rs       # グローバルホットキー管理
│       ├── tray.rs         # システムトレイ
│       ├── hotplug.rs      # モニターの接続/切断検知
│       ├── history.rs      # Undo履歴管理
│       ├── admin.rs        # 管理者権限・スタートアップ
│       └── logger.rs       # ログ出力
├── package.json
├── vite.config.ts
├── tsconfig.json
└── 要求仕様書.md            # 詳細な要求仕様
```

## 🛡️ 管理者権限について

通常は一般ユーザー権限で動作します。  
タスクマネージャー等の管理者権限ウィンドウも操作対象にしたい場合は、設定画面から **「管理者として再起動する」** を実行してください。  
スタートアップの登録・解除にも管理者権限が必要です。

## 📝 技術スタック

| レイヤー | 技術 |
|---------|------|
| フレームワーク | [Tauri 2](https://tauri.app/) |
| フロントエンド | React 18 + TypeScript |
| スタイリング | [Tailwind CSS v4](https://tailwindcss.com/) |
| アニメーション | [Framer Motion](https://motion.dev/) |
| アイコン | [Lucide React](https://lucide.dev/) |
| バックエンド | Rust + [windows-rs](https://github.com/microsoft/windows-rs) |
| ビルドツール | Vite 7 |

## 📄 ライセンス

MIT License

## 👤 作者

**ことりちゅん** ([@KotorinChunChun](https://github.com/KotorinChunChun))
