import { useEffect, useState, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { MonitorMap } from "@/components/MonitorMap";
import { SettingsPanel } from "@/components/SettingsPanel";
import { ExclusionPanel } from "@/components/ExclusionPanel";
import { ArrangePopup } from "@/components/ArrangePopup";
import { AboutTab } from "@/components/AboutTab";
import { Sidebar, type TabId } from "@/components/Sidebar";
import { CustomDialog, DialogKind } from "@/components/CustomDialog";
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
  const [activeTab, setActiveTab] = useState<TabId>("map");
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [monitors, setMonitors] = useState<MonitorInfo[]>([]);
  const [isAdmin, setIsAdmin] = useState(false);

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

  const loadData = async () => {
    try {
      const loadedConfig: AppConfig = await invoke("get_config");
      setConfig(loadedConfig);
      const loadedMonitors: MonitorInfo[] = await invoke("get_all_monitors_cmd");
      setMonitors(loadedMonitors);
      const admin: boolean = await invoke("is_user_an_admin");
      setIsAdmin(admin);
    } catch (e) {
      console.error("Failed to load data", e);
    }
  };

  useEffect(() => {
    loadData();

    const unlistenPromise = listen("display-changed", async () => {
      try {
        const loadedMonitors: MonitorInfo[] = await invoke("get_all_monitors_cmd");
        setMonitors(loadedMonitors);
      } catch (error) {
        console.error("Failed to reload monitors:", error);
      }
    });

    return () => {
      unlistenPromise.then((f) => f());
    };
  }, []);

  // Ctrl+Tab / Ctrl+Shift+Tab によるタブ切り替え
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.key === "Tab") {
        e.preventDefault();
        const tabs: TabId[] = ["map", "exclusion", "settings", "about"];
        const currentIndex = tabs.indexOf(activeTab);
        if (e.shiftKey) {
          setActiveTab(tabs[(currentIndex - 1 + tabs.length) % tabs.length]);
        } else {
          setActiveTab(tabs[(currentIndex + 1) % tabs.length]);
        }
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [activeTab]);

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

  if (!config) {
    return (
      <div className="flex items-center justify-center h-screen bg-slate-950 text-white">
        <div className="animate-pulse">Loading...</div>
      </div>
    );
  }

  return (
    <div className="flex h-screen bg-slate-950 text-slate-100 font-sans overflow-hidden">
      <Sidebar activeTab={activeTab} onTabChange={setActiveTab} isAdmin={isAdmin} />

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
            <AboutTab onShowDialog={showDialog} />
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
