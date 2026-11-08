# README撮影用の隔離画面

通常のデスクトップを動かさず、実フロントエンドを架空の2モニターとSampleViewer.exeで確認するCargo exampleです。設定だけを指定した新規フォルダーへ保存します。通常アプリの起動・ログ初期化・フック・IPC・OS予約・整列・URL起動は実行しません。未登録コマンドは成功を装わずエラーになります。

```powershell
cd C:\develop\KotorinToys\tools\ChuwitchWindow
npx tsc
npx vite build
cargo test --manifest-path src-tauri/Cargo.toml --example manual-preview --features custom-protocol --locked
cargo build --manifest-path src-tauri/Cargo.toml --example manual-preview --features custom-protocol --locked
.\src-tauri\target\debug\examples\manual-preview.exe C:\develop\KotorinToys\artifacts\readme-samples\ChuwitchWindow-new
```

最後のパスは親フォルダーが存在する、新しい絶対パスを指定します。既存フォルダーを指定すると起動を拒否します。実移動とUndoは `dev/scripts/test-debug-ChuwitchWindow-all.ps1` のMonitorMovement/Arrange試験で専用ウィンドウを使って確認します。撮影用画面の保存結果だけで実モニター操作を確認済みにしません。
