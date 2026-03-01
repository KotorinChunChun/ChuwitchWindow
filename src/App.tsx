import { useEffect, useState, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { MonitorMap } from "@/components/MonitorMap";
import { SettingsPanel } from "@/components/SettingsPanel";
import { ExclusionPanel } from "@/components/ExclusionPanel";
import { ArrangePopup } from "@/components/ArrangePopup";
import { CustomDialog, DialogKind } from "@/components/CustomDialog";
import { LayoutGrid, Settings, Info, Ban } from "lucide-react";
import { cn } from "@/lib/utils";
import { AppConfig, MonitorInfo } from "@/types";

function App() {
  const [route, setRoute] = useState(window.location.hash);

  useEffect(() => {
    const handleHashChange = () => setRoute(window.location.hash);
    window.addEventListener("hashchange", handleHashChange);
    return () => window.removeEventListener("hashchange", handleHashChange);
  }, []);

  if (route === "#arrange") {
    return <ArrangePopup />;
  }

  return <MainApp />;
}

function MainApp() {
  const [activeTab, setActiveTab] = useState<"map" | "settings" | "about" | "exclusion">(
    "map",
  );
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [monitors, setMonitors] = useState<MonitorInfo[]>([]);
  const [isAdmin, setIsAdmin] = useState(false);
  const [appLogs, setAppLogs] = useState<string>("Loading logs...");
  const logTailRef = useRef<HTMLTextAreaElement>(null);

  // ダイアログ管理用の状態
  const [dialogState, setDialogState] = useState<{
    isOpen: boolean;
    title: string;
    message: string;
    kind: DialogKind;
    resolve: (v: boolean) => void;
  }>({
    isOpen: false,
    title: "",
    message: "",
    kind: "info",
    resolve: () => {},
  });

  const showDialog = (title: string, message: string, kind: DialogKind = "info"): Promise<boolean> => {
    return new Promise((resolve) => {
      setDialogState({
        isOpen: true,
        title,
        message,
        kind,
        resolve,
      });
    });
  };

  const closeDialog = (result: boolean) => {
    dialogState.resolve(result);
    setDialogState((prev) => ({ ...prev, isOpen: false }));
  };

  // 定期更新用のタイマーID保持
  const logTimerRef = useRef<number | null>(null);

  const loadData = async () => {
    try {
      const loadedConfig: AppConfig = await invoke("get_config");
      setConfig(loadedConfig);
      const loadedMonitors: MonitorInfo[] = await invoke(
        "get_all_monitors_cmd",
      );
      setMonitors(loadedMonitors);
      const admin: boolean = await invoke("is_user_an_admin");
      setIsAdmin(admin);
      fetchLogs();
    } catch (e) {
      console.error("Failed to load data", e);
    }
  };

  const fetchLogs = async () => {
    try {
      const logs: string = await invoke("get_app_logs_cmd");
      setAppLogs(logs);
    } catch (e) {
      setAppLogs(`Failed to load logs: ${e}`);
    }
  };

  useEffect(() => {
    loadData();

    const unlistenPromise = listen("display-changed", async () => {
      try {
        const loadedMonitors: MonitorInfo[] = await invoke(
          "get_all_monitors_cmd",
        );
        setMonitors(loadedMonitors);
      } catch (error) {
        console.error("Failed to reload monitors:", error);
      }
    });

    return () => {
      unlistenPromise.then((f) => f());
      if (logTimerRef.current) window.clearInterval(logTimerRef.current);
    };
  }, []);

  // aboutタブが表示されているときのみ5秒おきにログ更新
  useEffect(() => {
    if (activeTab === "about") {
      fetchLogs();
      logTimerRef.current = window.setInterval(fetchLogs, 5000);
    } else {
      if (logTimerRef.current) {
        window.clearInterval(logTimerRef.current);
        logTimerRef.current = null;
      }
    }
    return () => {
      if (logTimerRef.current) window.clearInterval(logTimerRef.current);
    };
  }, [activeTab]);

  // ログが更新されたら最下部へスクロール
  useEffect(() => {
    if (logTailRef.current) {
      logTailRef.current.scrollTop = logTailRef.current.scrollHeight;
    }
  }, [appLogs]);

  // Ctrl+Tab / Ctrl+Shift+Tab によるタブ切り替え
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.key === "Tab") {
        e.preventDefault();
        const tabs: ("map" | "exclusion" | "settings" | "about")[] = [
          "map",
          "exclusion",
          "settings",
          "about",
        ];
        const currentIndex = tabs.indexOf(activeTab);
        if (e.shiftKey) {
          setActiveTab(tabs[(currentIndex - 1 + tabs.length) % tabs.length] as any);
        } else {
          setActiveTab(tabs[(currentIndex + 1) % tabs.length] as any);
        }
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [activeTab]);

  const handleClearLogs = async () => {
    if (await showDialog("確認", "実行ログを完全に消去しますか？", "ask")) {
      try {
        await invoke("clear_app_logs_cmd");
        fetchLogs();
        await showDialog("完了", "ログを消去しました", "success");
      } catch (e) {
        await showDialog("エラー", `ログの消去に失敗しました: ${e}`, "error");
      }
    }
  };

  const handleConfigChange = async (newConfig: AppConfig) => {
    setConfig(newConfig);
    try {
      await invoke("save_config", { newConfig });
    } catch (e) {
      console.error(e);
      await showDialog("エラー", `設定の保存に失敗しました。\n\n${e}`, "error");
    }
  };

  const isResettingRef = useRef(false);
  const handleResetConfig = async () => {
    if (isResettingRef.current) return;
    isResettingRef.current = true;

    try {
      const confirmed = await showDialog(
        "設定の初期化",
        "設定を初期状態にリセットしますか？この操作は取り消せません。",
        "ask"
      );
      if (confirmed) {
        const defaultConfig: AppConfig = await invoke("reset_config_cmd");
        setConfig(defaultConfig);
        await showDialog("リセット完了", "設定を初期状態にリセットしました。", "success");
      }
    } catch (error) {
      console.error("Failed to reset config:", error);
      await showDialog("エラー", "リセットに失敗しました。", "error");
    } finally {
      isResettingRef.current = false;
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

  if (!config) {
    return (
      <div className="flex items-center justify-center h-screen bg-slate-950 text-white">
        <div className="animate-pulse">Loading...</div>
      </div>
    );
  }

  return (
    <div className="flex h-screen bg-slate-950 text-slate-100 font-sans overflow-hidden">
      {/* Sidebar */}
      <div className="w-64 flex flex-col bg-slate-900/50 backdrop-blur-xl border-r border-slate-800">
        <div className="p-6">
          <h1 className="text-xl font-bold bg-gradient-to-r from-blue-400 to-cyan-300 bg-clip-text text-transparent">
            ChuwitchWindow
          </h1>
          <p className="text-xs text-slate-400 mt-1">Multi-Monitor Wizard</p>
        </div>

        <nav className="flex-1 px-4 space-y-2 mt-4">
          <button
            onClick={() => setActiveTab("map")}
            className={cn(
              "flex items-center w-full px-4 py-3 rounded-lg text-base font-medium transition-colors cursor-pointer",
              activeTab === "map"
                ? "bg-blue-500/10 text-blue-400 border border-blue-500/20"
                : "text-slate-400 hover:bg-slate-800/50 hover:text-slate-200",
            )}
          >
            <LayoutGrid className="w-5 h-5 mr-3" /> モニター構成
          </button>
          <button
            onClick={() => setActiveTab("exclusion")}
            className={cn(
              "flex items-center w-full px-4 py-3 rounded-lg text-base font-medium transition-colors cursor-pointer",
              activeTab === "exclusion"
                ? "bg-blue-500/10 text-blue-400 border border-blue-500/20"
                : "text-slate-400 hover:bg-slate-800/50 hover:text-slate-200",
            )}
          >
            <Ban className="w-5 h-5 mr-3" /> 除外リスト
          </button>
          <button
            onClick={() => setActiveTab("settings")}
            className={cn(
              "flex items-center w-full px-4 py-3 rounded-lg text-base font-medium transition-colors cursor-pointer",
              activeTab === "settings"
                ? "bg-blue-500/10 text-blue-400 border border-blue-500/20"
                : "text-slate-400 hover:bg-slate-800/50 hover:text-slate-200",
            )}
          >
            <Settings className="w-5 h-5 mr-3" /> 設定・常駐
          </button>
          <button
            onClick={() => setActiveTab("about")}
            className={cn(
              "flex items-center w-full px-4 py-3 rounded-lg text-base font-medium transition-colors cursor-pointer",
              activeTab === "about"
                ? "bg-blue-500/10 text-blue-400 border border-blue-500/20"
                : "text-slate-400 hover:bg-slate-800/50 hover:text-slate-200",
            )}
          >
            <Info className="w-5 h-5 mr-3" /> アプリについて
          </button>
        </nav>

        <div className="p-4 border-t border-slate-800/50 text-xs text-slate-500">
          {isAdmin ? (
            <span className="text-green-400">Admin Mode</span>
          ) : (
            <span className="text-amber-400"></span>
          )}
        </div>
      </div>

      {/* Main Content */}
      <div
        className={cn(
          "flex-1 flex flex-col relative",
          activeTab === "map" ? "overflow-hidden" : "overflow-y-auto",
        )}
      >
        <div className="absolute inset-0 bg-[radial-gradient(ellipse_at_top,_var(--tw-gradient-stops))] from-slate-900 via-slate-950 to-slate-950 -z-10"></div>
        <main className="flex-1 p-8 flex flex-col min-h-0">
          {activeTab === "map" && (
            <div className="flex-1 flex flex-col space-y-6 min-h-0">
              <div className="flex-none">
                <h2 className="text-2xl font-semibold text-white">
                  モニターマップ
                </h2>
                <p className="text-slate-400 mt-1">
                  モニタをクリックすると所属グループ（色）を変更でき、同じ色のモニタ間でウィンドウを一斉交換できます。
                </p>
              </div>
              <div className="flex-1 rounded-2xl bg-slate-900/40 border border-slate-800 backdrop-blur-sm p-6 flex flex-col min-h-0 relative">
                <MonitorMap
                  monitors={monitors}
                  config={config}
                  onChange={handleConfigChange}
                />
              </div>
            </div>
          )}
          {activeTab === "exclusion" && config && (
            <div className="h-full flex flex-col space-y-6">
              <div>
                <h2 className="text-2xl font-semibold text-white">除外リスト</h2>
                <p className="text-slate-400 mt-1">
                  ウィンドウ移動の対象から除外するアプリケーションを設定します。
                </p>
              </div>
              <ExclusionPanel
                config={config}
                onChange={handleConfigChange}
                onShowDialog={showDialog}
              />
            </div>
          )}
          {activeTab === "settings" && (
            <div className="h-full flex flex-col space-y-6">
              <div>
                <h2 className="text-2xl font-semibold text-white">設定</h2>
                <p className="text-slate-400 mt-1">
                  ショートカットと振る舞いの設定
                </p>
              </div>
              <SettingsPanel
                config={config}
                isAdmin={isAdmin}
                onChange={handleConfigChange}
                onResetConfig={handleResetConfig}
                onShowDialog={showDialog}
              />
            </div>
          )}
          {activeTab === "about" && (
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
          )}
        </main>
      </div>
      <CustomDialog
        isOpen={dialogState.isOpen}
        title={dialogState.title}
        message={dialogState.message}
        kind={dialogState.kind}
        onConfirm={() => closeDialog(true)}
        onCancel={() => closeDialog(false)}
      />
    </div>
  );
}

export default App;
