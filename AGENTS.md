# ChuwitchWindow 開発ルール

このファイルを、プロジェクト固有の必須ルール・配置・資料目録の正本とする。利用方法は `README.md`、公開開発者向け手順は `docs/`、開発者本人の計画・判断・記録は `dev/` に置く。

## 必須ルール

- PowerShell の実行には `pwsh` を使う。文書とコミットメッセージは日本語で記録する。
- 作業開始時に `git status` を確認し、既存の未コミット変更と固有ルールを保持する。
- 必要な資料だけを目的別目録から読む。`dev/private/` と `dev/temp/` は、ユーザーが明示した資料以外を通常の調査・検索・静的検査の対象にしない。
- 大量の実装はフェーズに分け、各フェーズの完了時に該当するテストまたはビルドを通してからコミットする。
- 要求は `dev/vX.Y.Z-req.md`、AI の実装計画・報告は `dev/vX.Y.Z-imp.md`、追加の進捗管理が必要な場合は `dev/vX.Y.Z-task.md` に記録する。
- 実装予定・実装中の版別資料は `dev/` 直下に置く。完成した一式だけを `dev/history/vX.Y.Z/` へ移し、未完了事項は出典と引き継ぎ先を元資料と引き継ぎ先の双方へ記録する。
- 過去資料の未チェック欄だけで未実装や再試験が必要と判断せず、後続の完了記録とユーザー確認を照合する。ユーザー確認による工程終了と、新しい測定証跡の取得は区別する。
- Conventional Commits の簡略形式（`feat:`、`fix:`、`docs:`、`refactor:`、`chore:` など）を使い、要約は日本語にする。

## 構成と配置

| 配置 | 役割 |
| --- | --- |
| `README.md` | 利用者向けの概要、導入、基本操作 |
| `docs/` | 利用者およびビルド・改造を行う第三者向け資料 |
| `dev/` | 開発計画、判断、版別資料、作業記録 |
| `dev/scripts/` | 実在するビルド・起動・検証・配布用スクリプト。Git 追跡対象 |
| `dev/private/` | 秘密情報・個人情報を含む非公開資料。Git・公開成果物・通常調査から除外 |
| `dev/temp/` | プロジェクト外の一時制作物。Git・目録・通常調査から除外 |
| `src/` | React / TypeScript フロントエンドと固有テスト |
| `src-tauri/` | Rust / Tauri バックエンド、権限、NSIS 設定、製品同梱リソース |
| `tests/` | 複数機能で共有する試験、fixtures、期待結果を置く場合に使用 |
| `release/` | ローカルの配布成果物。Git 対象外 |

Tauri の規約上必要な `src-tauri/`、`src-tauri/nsis/`、`src-tauri/capabilities/`、`src-tauri/permissions/` は上記構成の例外として保持する。不要な空フォルダや空資料は作らない。取り込み用 `templates/` は恒久構成に含めない。

開発参考例は `dev/examples/`、継続更新する検証記録は `dev/testing/`、将来計画は `dev/roadmap/` を必要になった時だけ作る。製品同梱素材と試験素材を兼用する場合は各用途へコピーし、共通更新時に同一内容を検証する。

## 資料を読む条件と目録

| 作業・目的 | 読む資料 |
| --- | --- |
| 概要・基本操作 | `README.md`、必要に応じて `docs/USERS_GUIDE.md`、`docs/FAQ.md` |
| ビルド・改造・テスト | `docs/DEVELOPERS_GUIDE.md` |
| 要求の実装 | 対象の `dev/vX.Y.Z-req.md` と対応する `imp`・`task`、リンクされた設計資料 |
| 現行 v0.2.2 の品質判定 | `dev/v0.2.2-quality-gates.md`、`dev/v0.2.2-test-plan.md`、必要なテストケース |
| 将来機能の検討 | `dev/PLAN_IDEAS.md`、`dev/PLAN_REJECT.md`、必要な設計資料 |
| 配布・公開 | `dev/RELEASE_RULE.md` |
| 過去の判断・結果 | `dev/history/` の該当版と、後続の完了記録 |

## 運用スクリプト

製品操作用スクリプトは原則 `dev/scripts/` に置き、`動詞-モード-ChuwitchWindow-機能名[-検証データ].ps1` とする。

- 動詞は `build`、`run`、`test`、`publish`。ローカルで配布物を作るだけなら `package` を使う。
- モードは `debug`、`release`、または既存成果物だけを使う `exe`。`build-exe` は作らない。
- アプリ全体の機能名は `all`。自動判定する試験だけを `test` とし、失敗時は非 0 を返す。
- 補助スクリプトはモードを無理に付けず、`動詞-ChuwitchWindow-目的.ps1` のように用途を明示する。
- 改名時は呼び出し元、README、開発者ガイド、設定、テストを更新する。旧名を残す場合は正規名への転送とし、理由を本書へ記録する。

現存する運用スクリプト:

- `dev/scripts/update-ChuwitchWindow-version.ps1`: バージョン設定と lockfile を更新する補助処理。
- `dev/scripts/test-debug-ChuwitchWindow-all.ps1`: 機能別の自動試験を実行し、JSONと終了コードで結果を返す。
- `dev/scripts/publish-release-ChuwitchWindow-all.ps1`: Release ビルド、ZIP / NSIS 配布物生成、GitHub 公開を行う。明示的な公開作業でのみ実行する。

## 配布と秘密情報

- ビルド、配布物生成、外部公開、署名を別工程として扱う。詳細は `dev/RELEASE_RULE.md` を参照する。
- 外部公開、Git タグ、マージ、署名はユーザーの明示指示なしに実行しない。本リポジトリでは Authenticode 署名を行わない。
- `dev/private/`、`dev/temp/`、`release/`、`*.pfx`、`*.p12` を Git 対象外にする。API キー等をソースへ直書きしない。
- 追跡済みの秘密情報を発見した場合は値を転載せず、追跡解除だけでは履歴から消えないことと、失効・再発行等の必要対応を報告する。
