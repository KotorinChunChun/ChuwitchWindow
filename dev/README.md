# ChuwitchWindow 開発資料

このフォルダには、開発者本人の計画・判断・版別資料・作業記録を置く。利用者向け情報は [`README.md`](../README.md)、公開開発者向けの手順は [`docs/DEVELOPERS_GUIDE.md`](../docs/DEVELOPERS_GUIDE.md)、必須ルールと資料目録は [`AGENTS.md`](../AGENTS.md) を参照する。

## 現行版 v0.2.2

- 要求: [`v0.2.2-req.md`](v0.2.2-req.md)
- 実装計画・報告: [`v0.2.2-imp.md`](v0.2.2-imp.md)
- 進捗: [`v0.2.2-task.md`](v0.2.2-task.md)
- 品質基準: [`v0.2.2-quality-gates.md`](v0.2.2-quality-gates.md)
- テスト計画・ケース: [`v0.2.2-test-plan.md`](v0.2.2-test-plan.md) / [`v0.2.2-test-cases.md`](v0.2.2-test-cases.md)
- 詳細設計: `v0.2.2-D01-*.md` から `v0.2.2-D09-*.md`、[`v0.2.2-ux.md`](v0.2.2-ux.md)

現行版は実装中のため `dev/` 直下に置く。完成条件を満たした資料一式だけを `history/v0.2.2/` へ移す。

## 継続資料

- 将来候補: [`PLAN_IDEAS.md`](PLAN_IDEAS.md)
- 不採用・保留判断: [`PLAN_REJECT.md`](PLAN_REJECT.md)
- 配布手順: [`RELEASE_RULE.md`](RELEASE_RULE.md)
- 運用スクリプト: [`scripts/`](scripts/)
- 完了済み・歴史的資料: [`history/`](history/)

## 非公開・一時領域

- `private/`: 秘密情報・個人情報を含む非公開資料。Git・公開成果物・通常調査から除外する。
- `temp/`: プロジェクト外の一時制作物。Git・目録・通常調査から除外する。

どちらも、ユーザーが明示した資料だけを読む。開発経緯や通常の判断記録は非公開扱いにせず、原則としてこの `dev/` 配下で追跡する。
