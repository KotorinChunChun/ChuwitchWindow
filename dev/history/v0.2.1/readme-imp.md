# README整備と撮影用CLI（2026-11-08）

## P1: 安全な撮影の入口
- [x] 設定・除外のフロントエンド10件、Rust設定10件合格。
- [x] 実OS操作を持たないCargo exampleを追加。既存フォルダー拒否・相対パス拒否・設定書戻しの試験1件合格。
- [x] フロントエンドとexampleのビルド成功。
- [x] 既存のsrc-tauri/src/window.rs変更は保持し、コミットに含めない。コンパイルは現在の作業ツリーで実施。

## P2: 撮影とユーザー説明
- [x] 入口・設定・モニター選択・G1保存結果・除外ルールの5枚を確認してREADMEへ掲載。
- [x] ローカル参照・5画像・差分を検査。期限付きの旧画像URLを除き、設定項目の所在も現画面へ合わせた。

### 実測と境界

- 撮影用exampleだけでTaskDialogIndirectの入口不足が発生したため、build.rsでexample向けCommon Controls v6マニフェストを追加。再ビルド後の実画面起動に成功。
- SampleDisplay1をG1へ変更し、隔離config.jsonのmonitor_groups.SampleDisplay1=1を読戻し。run_on_startup/run_as_admin/middle_drag_enabledはfalse。
- 撮影プロセス32724をAlt+F4で正常終了し、対象ウィンドウの消滅を確認。
- 実OSの配置変更・ホットキー・スタートアップ・管理者操作は今回未検証。普段の設定と履歴は操作していない。
- 改善候補：1200px幅では除外ルールが横スクロールになる。項目を見失いにくい折返し配置は今後のUI検討とし、今回は現状の操作説明を付けた。
