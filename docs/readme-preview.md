# README撮影用の隔離画面

このCargo exampleは、実際のフロントエンドを架空の2モニターとSampleViewer.exeで表示します。通常のデスクトップは操作せず、指定した新規フォルダーには設定だけを保存します。通常アプリの起動・ログ初期化・フック・IPC・OS予約・整列・URL起動は実行しません。未登録のコマンドにはエラーを返します。

```powershell
cd C:\develop\KotorinToys\tools\ChuwitchWindow
npx tsc
npx vite build
cargo test --manifest-path src-tauri/Cargo.toml --example manual-preview --features custom-protocol --locked
cargo build --manifest-path src-tauri/Cargo.toml --example manual-preview --features custom-protocol --locked
.\src-tauri\target\debug\examples\manual-preview.exe C:\develop\KotorinToys\artifacts\readme-samples\ChuwitchWindow-new
```

最後の引数には、親フォルダーが存在する未作成のフォルダーを絶対パスで指定します。既存のフォルダーを指定すると起動を拒否します。実際の移動とUndoは `dev/scripts/test-debug-ChuwitchWindow-all.ps1` のMonitorMovement/Arrange試験で専用ウィンドウを使って確認します。撮影用画面で設定を保存した結果だけでは、実モニターの操作を確認したことにはなりません。
