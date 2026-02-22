# <img width="32" height="32" alt="image" src="https://github.com/user-attachments/assets/b1280f86-85a9-4fdd-95b4-dade568e92c3" /> ChuwitchWindow - ウィンドウ配置入れ替えツール

本ツールは、マルチモニター環境向けの **ウィンドウ配置入れ替えツール** です。  
ショートカットキー一発で、開いているウィンドウ群を別のディスプレイへ一斉に移動・交換・巡回できます。

言い換えれば、マルチモニタ環境におけるデスクトップの秩序を維持するためのツールです。

## プロジェクト概要

本プロジェクトは、AIと協力して以下の工程で開発を行いました。

1. 開発に至った背景 : [BACK.md](docs/BACK.md)
2. 要件定義仕様書 : [SPEC.md](docs/SPEC.md)
3. AIコーディング : [DEV.md](docs/DEV.md)

本書では、基本的な操作説明のみを行います。

## ✨ 主な機能

<img width="2752" alt="ChuwitchWindow概要" src="https://github.com/user-attachments/assets/af15de34-ed7a-493b-a53b-c9f78a6c2f4a" />

| 機能 | 説明 | デフォルトキー |
|------|------|---------------|
| **巡回シフト（増加方向）** | 全ウィンドウを次のモニターへ移動（1→2→3→1） | `Win+Ctrl+Alt+→` |
| **巡回シフト（減少方向）** | 全ウィンドウを前のモニターへ移動（1→3→2→1） | `Win+Ctrl+Alt+←` |
| **指定モニターとの入れ替え** | プライマリモニターと指定番号のモニター間でウィンドウを交換 | `Win+Ctrl+Alt+2〜9` |
| **Undo（元に戻す）** | 直前の入れ替え操作を取り消し | `Win+Ctrl+Alt+Z` |
| **モニターグループ** | 同じグループ内のモニター間のみで入れ替えを実行 | — |

## 🖥️ 対応環境

- **OS**: Windows 10 / 11
- **必要要件**: 特になし（単体 `.exe` で動作）

## 📦 インストール

アプリの一覧へインストールしたくない方／できない方のために、EXEのZIP版とインストール版を用意しています。

### 軽く試してみたい方
1. [Releases](https://github.com/KotorinChunChun/ChuwitchWindow/releases) のAssetsから `*.zip` をダウンロード
2. `*.zip` を解凍して `ChuwitchWindow.exe` ファイルを直接実行

### 本格的に利用したい方（スタートアップに登録したい方はこちらを推奨）
1. [Releases](https://github.com/KotorinChunChun/ChuwitchWindow/releases) のAssetsから `*.setup.exe` をダウンロード
2. `*.setup.exe` を実行してインストール
    - スタートメニューやデスクトップにショートカットが作成されます。
    - Windowsの【設定】の【アプリ】の【インストールされているアプリ】からアンインストールが可能です。

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

![](https://private-user-images.githubusercontent.com/55196383/553078983-6410993e-9b87-4cf7-861e-b31b97f33570.png?jwt=eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJpc3MiOiJnaXRodWIuY29tIiwiYXVkIjoicmF3LmdpdGh1YnVzZXJjb250ZW50LmNvbSIsImtleSI6ImtleTUiLCJleHAiOjE3NzE3NjMzODAsIm5iZiI6MTc3MTc2MzA4MCwicGF0aCI6Ii81NTE5NjM4My81NTMwNzg5ODMtNjQxMDk5M2UtOWI4Ny00Y2Y3LTg2MWUtYjMxYjk3ZjMzNTcwLnBuZz9YLUFtei1BbGdvcml0aG09QVdTNC1ITUFDLVNIQTI1NiZYLUFtei1DcmVkZW50aWFsPUFLSUFWQ09EWUxTQTUzUFFLNFpBJTJGMjAyNjAyMjIlMkZ1cy1lYXN0LTElMkZzMyUyRmF3czRfcmVxdWVzdCZYLUFtei1EYXRlPTIwMjYwMjIyVDEyMjQ0MFomWC1BbXotRXhwaXJlcz0zMDAmWC1BbXotU2lnbmF0dXJlPThiZTAwNmM3ZGM2M2IzZTUyYzZlYjBkNjM2MjdjZWRmMDI2NzQyN2I0YTQyNWExOWFmN2FhNDM3YThkMjc0NGMmWC1BbXotU2lnbmVkSGVhZGVycz1ob3N0In0.xE86VVABk3cmajwNMk26Cd-zJnEVcYJlmEfPCW4y2qI)

### ウィンドウの動作

- **閉じる（×ボタン）** → タスクトレイに格納（アプリは終了しません）
- **最小化** → タスクトレイに格納

### 設定画面

設定画面は3つのタブで構成されています。

- **モニター構成**: モニターの物理配置をグラフィカルに表示。クリックでグループ色を変更し、同じ色のモニター間でウィンドウを一斉交換できます。
- **設定・常駐**: ショートカットキーの変更、スタートアップ登録、動作設定を行います。
- **アプリについて**: バージョン情報、GitHubリンク、設定のリセット機能があります。

設定は `%APPDATA%\com.kotorichun\chuwitchwindow\config.json` に自動保存されます。

### モニタ構成

- プライマリモニターを選択できます。（プライマリモニタは、任意のモニタと交換できるようになります）
- モニタにグループ色を設定できます。（同じ色のモニター間でウィンドウを一斉交換できるようになります）
- 順位を設定できます。（巡回シフトの順番を変更できます）

![](https://private-user-images.githubusercontent.com/55196383/553030517-cbf31a7a-6e69-4069-9ece-7535cc373c4b.png?jwt=eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJpc3MiOiJnaXRodWIuY29tIiwiYXVkIjoicmF3LmdpdGh1YnVzZXJjb250ZW50LmNvbSIsImtleSI6ImtleTUiLCJleHAiOjE3NzE3NjMzODAsIm5iZiI6MTc3MTc2MzA4MCwicGF0aCI6Ii81NTE5NjM4My81NTMwMzA1MTctY2JmMzFhN2EtNmU2OS00MDY5LTllY2UtNzUzNWNjMzczYzRiLnBuZz9YLUFtei1BbGdvcml0aG09QVdTNC1ITUFDLVNIQTI1NiZYLUFtei1DcmVkZW50aWFsPUFLSUFWQ09EWUxTQTUzUFFLNFpBJTJGMjAyNjAyMjIlMkZ1cy1lYXN0LTElMkZzMyUyRmF3czRfcmVxdWVzdCZYLUFtei1EYXRlPTIwMjYwMjIyVDEyMjQ0MFomWC1BbXotRXhwaXJlcz0zMDAmWC1BbXotU2lnbmF0dXJlPTllODkxMzcyM2JmZjgwNjExZTliMzZiM2VmYmJlM2E2YmFjMGJlYThkYmRjZDFjMTIyMzFjMGRlMmIxZTA1OWImWC1BbXotU2lnbmVkSGVhZGVycz1ob3N0In0.hFUvWM8MP2AJQiPupVOlqtLIn2K970LOSHQ7cL5m-po)

### 設定・常駐

- 自動起動の登録ができます。
- ショートカットキーの設定ができます。
- グループ内での入れ替えを行うかモード切替ができます。
- フルクリーンアプリの扱いを変更できます。

![](https://private-user-images.githubusercontent.com/55196383/553030503-3193eb73-6f15-4cf4-8a7e-34a94d00571a.png?jwt=eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJpc3MiOiJnaXRodWIuY29tIiwiYXVkIjoicmF3LmdpdGh1YnVzZXJjb250ZW50LmNvbSIsImtleSI6ImtleTUiLCJleHAiOjE3NzE3NjMzODAsIm5iZiI6MTc3MTc2MzA4MCwicGF0aCI6Ii81NTE5NjM4My81NTMwMzA1MDMtMzE5M2ViNzMtNmYxNS00Y2Y0LThhN2UtMzRhOTRkMDA1NzFhLnBuZz9YLUFtei1BbGdvcml0aG09QVdTNC1ITUFDLVNIQTI1NiZYLUFtei1DcmVkZW50aWFsPUFLSUFWQ09EWUxTQTUzUFFLNFpBJTJGMjAyNjAyMjIlMkZ1cy1lYXN0LTElMkZzMyUyRmF3czRfcmVxdWVzdCZYLUFtei1EYXRlPTIwMjYwMjIyVDEyMjQ0MFomWC1BbXotRXhwaXJlcz0zMDAmWC1BbXotU2lnbmF0dXJlPWFlMjQ1M2E4NzU0ZjkxMjAwMDE2OGM1ZDMwZDU2NmM1YjBlNWNhZDg2NjZkMTZlMDRiYWNmMGZmYTY5ZDRkMGEmWC1BbXotU2lnbmVkSGVhZGVycz1ob3N0In0.c3UaAfmygZkI9oXZvHI-M5OhtbiElO72KS3M3mrNlXA)

### 🛡️ 管理者権限について

- 通常は一般ユーザー権限で動作しますが、その場合は管理者権限で実行されているアプリを移動させることができません。
- 管理者権限ウィンドウも操作対象にしたい場合は、設定画面から **「管理者として再起動する」** を実行してください。  
- スタートアップの登録・解除にも管理者権限が必要です。

## 利用条件

- 本ツールはフリーウェアです。無制限にご利用頂けます。
- 本ツールを使用したことにより発生したいかなる損害も作者は責任を負いません。
- Rust未経験者がAIを頼りに書いているため、コード品質については期待しないでください。
- バグや追加して欲しい機能などがあれば、GitHubのIssuesやTwitterなどでお声掛けください。

## 👤 作者

**ことりちゅん** ([@KotorinChunChun](https://github.com/KotorinChunChun))

![](https://avatars.githubusercontent.com/u/55196383?s=400&u=1ae1d910e5b368dac3cf74f0471411ab003f7852&v=4)
