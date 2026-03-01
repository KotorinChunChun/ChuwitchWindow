# <img width="32" height="32" alt="image" src="https://github.com/user-attachments/assets/b1280f86-85a9-4fdd-95b4-dade568e92c3" /> ChuwitchWindow - ウィンドウ配置入れ替えツール 開発環境

### 🔧 開発手法

スタートアップに登録して常駐するプログラムということで、消費メモリが少なく実行速度が早いことに定評があるRustを開発言語として採用しました。バイナリと消費メモリをいずれも10MBに抑えることができています。

設定画面のUIについては、AIが得意なReactを採用しました。
その結果、設定画面を開いたときにWebView2が起動するため起動が遅く、消費メモリが250MBほどに増大してしまいました。
しかし、スタートアップ起動時に設定画面を出さないようにしたり、閉じたら解放されるようにしたりすることで、この問題は回避しています。

詳しい仕様は [SPECv0.1.md](SPECv0.1.md) および [SPECv0.2.md](SPECv0.2.md) を参照してください。

## 🔧 ビルド方法

### 前提条件

- [Node.js](https://nodejs.org/) (v18以上)
- [Rust](https://www.rust-lang.org/tools/install)
- [Tauri CLI](https://tauri.app/) の前提条件（Visual Studio Build Tools 等）

### ビルド手順

```powershell
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
├── docs/                   # プロジェクトのドキュメント類
│   ├── BACK.md             # 開発に至った背景
│   ├── SPECv0.1.md           # ツールの基本仕様書
│   ├── SPECv0.2.md           # 次期アップデート(v0.2)仕様書
│   ├── DEV.md              # 開発者向けドキュメント
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
└── tsconfig.json
```

## � バージョン管理

バージョンアップ時には、以下の4箇所のバージョン表記を揃える必要があります。

1.  **`src-tauri/Cargo.toml`**: Rustバイナリのバージョン。
2.  **`src-tauri/tauri.conf.json`**: アプリケーション設定およびインストーラー用。
3.  **`package.json`**: Node.jsプロジェクトとしてのバージョン。
4.  **`src/App.tsx`**: 設定画面（アプリについてタブ）に表示されるバージョンと更新日。

### 自動更新スクリプト

`scripts/update_version.ps1` を使用して一括更新が可能です。

```powershell
# 使用例 (v0.2.1 に更新する場合)
./scripts/update_version.ps1 -NewVersion "0.2.1"
```

このスクリプトは、設定画面の日付も実行時の日付（YYYY.MM.DD）に自動更新します。

## �📝 技術スタック

| レイヤー       | 技術                                                         |
| -------------- | ------------------------------------------------------------ |
| フレームワーク | [Tauri 2](https://tauri.app/)                                |
| フロントエンド | React 18 + TypeScript                                        |
| スタイリング   | [Tailwind CSS v4](https://tailwindcss.com/)                  |
| アニメーション | [Framer Motion](https://motion.dev/)                         |
| アイコン       | [Lucide React](https://lucide.dev/)                          |
| バックエンド   | Rust + [windows-rs](https://github.com/microsoft/windows-rs) |
| ビルドツール   | Vite 7                                                       |

## 📄 ライセンス

MIT License

---

[README.md](../README.md)
