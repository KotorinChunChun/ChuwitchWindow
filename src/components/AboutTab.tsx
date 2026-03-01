import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { DialogKind } from "./CustomDialog";

/**
 * 「アプリについて」タブ
 * バージョン情報、開発者情報、実行ログの表示・操作を提供する。
 */
export function AboutTab({
  onShowDialog,
}: {
  onShowDialog: (title: string, message: string, kind?: DialogKind) => Promise<boolean>;
}) {
  const [appLogs, setAppLogs] = useState<string>("Loading logs...");
  const logTailRef = useRef<HTMLTextAreaElement>(null);
  const logTimerRef = useRef<number | null>(null);

  const fetchLogs = async () => {
    try {
      const logs: string = await invoke("get_app_logs_cmd");
      setAppLogs(logs);
    } catch (e) {
      setAppLogs(`Failed to load logs: ${e}`);
    }
  };

  // マウント時にログ取得開始、5秒ごとに自動更新
  useEffect(() => {
    fetchLogs();
    logTimerRef.current = window.setInterval(fetchLogs, 5000);
    return () => {
      if (logTimerRef.current) window.clearInterval(logTimerRef.current);
    };
  }, []);

  // ログ更新時に最下部へスクロール
  useEffect(() => {
    if (logTailRef.current) {
      logTailRef.current.scrollTop = logTailRef.current.scrollHeight;
    }
  }, [appLogs]);

  const handleClearLogs = async () => {
    if (await onShowDialog("確認", "実行ログを完全に消去しますか？", "ask")) {
      try {
        await invoke("clear_app_logs_cmd");
        fetchLogs();
        await onShowDialog("完了", "ログを消去しました", "success");
      } catch (e) {
        await onShowDialog("エラー", `ログの消去に失敗しました: ${e}`, "error");
      }
    }
  };

  const handleOpenGithub = async (e: React.MouseEvent) => {
    e.preventDefault();
    try {
      await invoke("open_url", {
        url: "https://github.com/KotorinChunChun/ChuwitchWindow",
      });
    } catch (error) {
      console.error("Failed to open URL:", error);
    }
  };

  return (
    <div className="h-full flex flex-col space-y-6">
      <div>
        <h2 className="text-2xl font-semibold text-white">
          ChuwitchWindow
        </h2>
        <p className="text-slate-400 mt-1">
          Version 0.2.0 (2026.03.01)
        </p>
      </div>
      <div className="flex-1 rounded-2xl bg-slate-900/40 border border-slate-800 backdrop-blur-sm p-6 text-sm text-slate-300 space-y-4 flex flex-col min-h-0">
        <div className="flex-none">
          <div className="text-sm font-medium text-slate-200">
            マルチモニタ環境向け ウィンドウ管理ユーティリティ
          </div>
          <div className="text-xs text-slate-500 mt-2">
            ウィンドウの配置をキーボードショートカットで「増加方向/減少方向」へシフトさせたり、特定の画面（1番や2番など）へ瞬時に送る・入れ替えるなど、柔軟なウィンドウ管理機能を提供するユーティリティツールです。
            <br />
            特定のモニタグループ内でのみ入れ替えを行ったり、配置を直前の状態に戻すUndo機能も備えています。
          </div>
        </div>
        <div className="pt-2 border-t border-slate-800 flex flex-col space-y-2 flex-none">
          <div className="flex justify-between items-center text-sm">
            <span className="text-slate-400">開発者</span>
            <span className="text-slate-200 font-medium">
              ことりちゅん
            </span>
          </div>
          <div className="flex justify-between items-center text-sm">
            <span className="text-slate-400">GitHub</span>
            <a
              href="https://github.com/KotorinChunChun/ChuwitchWindow"
              onClick={handleOpenGithub}
              className="text-blue-400 hover:text-blue-300 transition-colors"
            >
              https://github.com/KotorinChunChun/ChuwitchWindow
            </a>
          </div>
        </div>

        <div className="pt-6 border-t border-slate-800 flex-1 flex flex-col min-h-0">
          <div className="flex justify-between items-center mb-2 flex-none">
            <span className="text-sm font-medium text-slate-200">実行ログ</span>
            <div className="flex gap-2">
                <button
                  onClick={handleClearLogs}
                  className="px-2 py-1 bg-red-900/30 hover:bg-red-800/50 text-red-400 rounded text-xs transition-colors border border-red-800/50 cursor-pointer"
                >
                  全消去
                </button>
                <button
                  onClick={fetchLogs}
                  className="px-2 py-1 bg-slate-800 hover:bg-slate-700 text-slate-300 rounded text-xs transition-colors border border-slate-700 cursor-pointer"
                >
                  更新
                </button>
            </div>
          </div>
          <textarea
            ref={logTailRef}
            value={appLogs}
            readOnly
            className="flex-1 w-full bg-slate-950 border border-slate-800 rounded p-2 text-xs font-mono text-slate-400 focus:outline-none resize-none"
            spellCheck={false}
          />
        </div>
      </div>
    </div>
  );
}
