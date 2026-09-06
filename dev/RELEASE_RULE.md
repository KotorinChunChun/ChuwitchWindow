# ChuwitchWindow 配布・公開手順

本書は ChuwitchWindow の配布手順の正本である。ビルド、配布物生成、外部公開、署名を別工程として扱う。外部公開、Git タグ、マージ、署名はユーザーの明示指示がある場合だけ実施する。

## 前提

- Windows 10 / 11
- Node.js 18 以上と npm
- Rust と Cargo
- Tauri 2 の Windows 向け前提（Visual Studio Build Tools、WebView2 等）
- PowerShell 7 以降（`pwsh`）
- GitHub 公開を行う場合だけ GitHub CLI と認証済みアカウント

## バージョン更新

次の 3 ファイルのバージョンを同じ値へ更新する。

1. `package.json`
2. `src-tauri/Cargo.toml`
3. `src-tauri/tauri.conf.json`

次の補助スクリプトで更新し、差分を照合する。

```powershell
pwsh -File .\dev\scripts\update-ChuwitchWindow-version.ps1 -NewVersion "0.2.2"
```

このスクリプトは 3 ファイルを更新した後、`npm install --package-lock-only --ignore-scripts` で `package-lock.json` を整合させる。

## 検証と Release ビルド

プロジェクトルートで実行する。

```powershell
pwsh -NoProfile -Command "npm ci"
pwsh -NoProfile -Command "npm test"
pwsh -NoProfile -Command "cargo test --manifest-path src-tauri/Cargo.toml"
pwsh -NoProfile -Command "npm run tauri build"
```

主な成果物:

- 実行ファイル: `src-tauri/target/release/chuwitchwindow.exe`
- NSIS インストーラ: `src-tauri/target/release/bundle/nsis/`

成果物のローカル控えは `release/` に置く。`release/` は Git 対象外である。

## ZIP 版の生成

Release ビルド済みの `chuwitchwindow.exe` と `README.md` を一時ディレクトリへコピーし、`release/ChuwitchWindow_vX.Y.Z.zip` として圧縮する。生成だけを自動化する場合、スクリプト名は `dev/scripts/package-ChuwitchWindow-zip.ps1` とし、外部公開処理を含めない。

## 外部公開

ユーザーが公開を明示した場合だけ、検証済みの ZIP と NSIS インストーラを GitHub Releases へ登録する。タグは `vMAJOR.MINOR.PATCH` とする。生成またはアップロードの一部が失敗した場合、全体を成功扱いにしない。

公開用スクリプト `dev/scripts/publish-release-ChuwitchWindow-all.ps1` は、Release ビルド、ZIP / NSIS 配布物生成、GitHub 公開の順に実行する。公開前の確認に `y` と回答した場合だけ続行し、成果物生成または公開に失敗した場合は非 0 で終了する。この適用確認では実行しない。

```powershell
pwsh -File .\dev\scripts\publish-release-ChuwitchWindow-all.ps1
```

既存成果物だけを公開する入口は現時点では存在しない。追加する場合は `publish-exe-ChuwitchWindow-all.ps1` とし、成果物がなければ失敗させ、暗黙にビルドへ切り替えない。

## 署名

本リポジトリの工程では Authenticode 署名を行わない。コード署名は MSI X 側の別工程で扱う。証明書や秘密鍵を本リポジトリへ保存せず、`*.pfx` と `*.p12` は Git 対象外とする。

## リリース後の資料

リリースした事実だけでは版別資料を完成扱いにしない。要求・実装・検証の完了条件を満たした後、`dev/vX.Y.Z-req.md`、`imp.md`、必要な `task.md` と関連資料を `dev/history/vX.Y.Z/` へ移す。未完了事項を切り離す場合は、元資料と引き継ぎ先の双方へ出典とリンクを残す。
