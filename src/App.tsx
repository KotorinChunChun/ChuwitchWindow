import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { motion, AnimatePresence } from "framer-motion";
import { MonitorMap } from "@/components/MonitorMap";
import { SettingsPanel } from "@/components/SettingsPanel";
import { LayoutGrid, Settings, Info, MonitorPlay } from "lucide-react";
import { cn } from "@/lib/utils";
import { AppConfig, MonitorInfo } from "@/types";

function App() {
    const [windowLabel, setWindowLabel] = useState<string | null>(null);

    useEffect(() => {
        const appWindow = getCurrentWindow();
        setWindowLabel(appWindow.label);

        if (appWindow.label === "osd") {
            appWindow.setIgnoreCursorEvents(true).catch(console.error);
        }
    }, []);

    if (windowLabel === "osd") {
        return <OsdApp />;
    }

    if (windowLabel === "main") {
        return <MainApp />;
    }

    return null;
}

function OsdApp() {
    const [message, setMessage] = useState<string | null>(null);

    useEffect(() => {
        let timeout: NodeJS.Timeout;
        const unlisten = listen<string>("osd-notify", (event) => {
            setMessage(event.payload);
            const appWindow = getCurrentWindow();
            appWindow.show().catch(console.error);

            clearTimeout(timeout);
            timeout = setTimeout(() => {
                setMessage(null);
                setTimeout(() => appWindow.hide().catch(console.error), 500);
            }, 2000);
        });

        return () => {
            unlisten.then(f => f());
            clearTimeout(timeout);
        };
    }, []);

    return (
        <div className="w-full h-full flex items-center justify-center p-4 bg-transparent overflow-hidden">
            <AnimatePresence>
                {message && (
                    <motion.div
                        initial={{ opacity: 0, scale: 0.9, y: 20 }}
                        animate={{ opacity: 1, scale: 1, y: 0 }}
                        exit={{ opacity: 0, scale: 0.95, y: -10 }}
                        transition={{ type: "spring", stiffness: 300, damping: 25 }}
                        className="flex items-center px-8 py-5 bg-slate-900/80 backdrop-blur-xl border border-slate-700/50 shadow-2xl rounded-2xl"
                    >
                        <MonitorPlay className="w-8 h-8 text-blue-400 mr-4" />
                        <span className="text-2xl font-bold bg-gradient-to-r from-blue-300 to-cyan-300 bg-clip-text text-transparent tracking-widest drop-shadow-sm">
                            {message}
                        </span>
                    </motion.div>
                )}
            </AnimatePresence>
        </div>
    );
}

function MainApp() {
    const [activeTab, setActiveTab] = useState<"map" | "settings" | "about">("map");
    const [config, setConfig] = useState<AppConfig | null>(null);
    const [monitors, setMonitors] = useState<MonitorInfo[]>([]);
    const [isAdmin, setIsAdmin] = useState(false);

    useEffect(() => {
        async function loadData() {
            try {
                const loadedConfig: AppConfig = await invoke("get_config");
                setConfig(loadedConfig);

                const loadedMonitors: MonitorInfo[] = await invoke("get_all_monitors_cmd");
                setMonitors(loadedMonitors);

                const adminStatus: boolean = await invoke("is_user_an_admin");
                setIsAdmin(adminStatus);

                getCurrentWindow().show();
            } catch (error) {
                console.error("Failed to load init data:", error);
            }
        }
        loadData();
    }, []);

    const handleConfigChange = async (newConfig: AppConfig) => {
        setConfig(newConfig);
        try {
            await invoke("save_config", { newConfig });
            if (newConfig.run_on_startup !== config?.run_on_startup) {
                await invoke("sync_admin_startup", { enable: newConfig.run_on_startup });
            }
        } catch (e) {
            console.error(e);
        }
    };

    const handleResetConfig = async () => {
        if (confirm("設定を初期状態にリセットしますか？この操作は取り消せません。")) {
            try {
                const defaultConfig: AppConfig = await invoke("reset_config_cmd");
                setConfig(defaultConfig);
                alert("設定をリセットしました。");
            } catch (error) {
                console.error("Failed to reset config:", error);
                alert("リセットに失敗しました。");
            }
        }
    };

    const handleOpenGithub = async (e: React.MouseEvent) => {
        e.preventDefault();
        try {
            await invoke("open_url", { url: "https://github.com/kotorinChunChun/ChuwitchWindow" });
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
                    <button onClick={() => setActiveTab("map")} className={cn("flex items-center w-full px-4 py-3 rounded-lg text-base font-medium transition-colors", activeTab === "map" ? "bg-blue-500/10 text-blue-400 border border-blue-500/20" : "text-slate-400 hover:bg-slate-800/50 hover:text-slate-200")}>
                        <LayoutGrid className="w-5 h-5 mr-3" /> モニター構成
                    </button>
                    <button onClick={() => setActiveTab("settings")} className={cn("flex items-center w-full px-4 py-3 rounded-lg text-base font-medium transition-colors", activeTab === "settings" ? "bg-blue-500/10 text-blue-400 border border-blue-500/20" : "text-slate-400 hover:bg-slate-800/50 hover:text-slate-200")}>
                        <Settings className="w-5 h-5 mr-3" /> 設定・常駐
                    </button>
                    <button onClick={() => setActiveTab("about")} className={cn("flex items-center w-full px-4 py-3 rounded-lg text-base font-medium transition-colors", activeTab === "about" ? "bg-blue-500/10 text-blue-400 border border-blue-500/20" : "text-slate-400 hover:bg-slate-800/50 hover:text-slate-200")}>
                        <Info className="w-5 h-5 mr-3" /> アプリについて
                    </button>
                </nav>

                <div className="p-4 border-t border-slate-800/50 text-xs text-slate-500">
                    {isAdmin ? <span className="text-green-400">Admin Mode</span> : <span className="text-amber-400"></span>}
                </div>
            </div>

            {/* Main Content */}
            <div className={cn("flex-1 flex flex-col relative", activeTab === "map" ? "overflow-hidden" : "overflow-y-auto")}>
                <div className="absolute inset-0 bg-[radial-gradient(ellipse_at_top,_var(--tw-gradient-stops))] from-slate-900 via-slate-950 to-slate-950 -z-10"></div>
                <main className="flex-1 p-8 flex flex-col min-h-0">
                    {activeTab === "map" && (
                        <div className="flex-1 flex flex-col space-y-6 min-h-0">
                            <div className="flex-none"><h2 className="text-2xl font-semibold text-white">モニターマップ</h2><p className="text-slate-400 mt-1">モニタをクリックすると所属グループ（色）を変更でき、同じ色のモニタ間でウィンドウを一斉交換できます。</p></div>
                            <div className="flex-1 rounded-2xl bg-slate-900/40 border border-slate-800 backdrop-blur-sm p-6 flex flex-col min-h-0 relative"><MonitorMap monitors={monitors} config={config} onChange={handleConfigChange} /></div>
                        </div>
                    )}
                    {activeTab === "settings" && (
                        <div className="h-full flex flex-col space-y-6">
                            <div><h2 className="text-2xl font-semibold text-white">設定</h2><p className="text-slate-400 mt-1">ショートカットと振る舞いの設定</p></div>
                            <SettingsPanel config={config} isAdmin={isAdmin} onChange={handleConfigChange} />
                        </div>
                    )}
                    {activeTab === "about" && (
                        <div className="h-full flex flex-col space-y-6">
                            <div>
                                <h2 className="text-2xl font-semibold text-white">ChuwitchWindow</h2>
                                <p className="text-slate-400 mt-1">Version 0.1.0 (2026.02.21)</p>
                            </div>
                            <div className="rounded-2xl bg-slate-900/40 border border-slate-800 backdrop-blur-sm p-6 text-sm text-slate-300 space-y-4">
                                <div>
                                    <div className="text-sm font-medium text-slate-200">マルチモニタ環境向け ウィンドウ管理ユーティリティ</div>
                                    <div className="text-xs text-slate-500 mt-2">
                                        ウィンドウの配置をキーボードショートカットで「増加方向/減少方向」へシフトさせたり、特定の画面（1番や2番など）へ瞬時に送る・入れ替えるなど、柔軟なウィンドウ管理機能を提供するユーティリティツールです。<br />
                                        特定のモニタグループ内でのみ入れ替えを行ったり、配置を直前の状態に戻すUndo機能も備えています。
                                    </div>
                                </div>
                                <div className="pt-2 border-t border-slate-800 flex flex-col space-y-2">
                                    <div className="flex justify-between items-center text-sm">
                                        <span className="text-slate-400">開発者</span>
                                        <span className="text-slate-200 font-medium">ことりちゅん</span>
                                    </div>
                                    <div className="flex justify-between items-center text-sm">
                                        <span className="text-slate-400">GitHub</span>
                                        <a
                                            href="https://github.com/kotorinChunChun/ChuwitchWindow"
                                            onClick={handleOpenGithub}
                                            className="text-blue-400 hover:text-blue-300 transition-colors"
                                        >
                                            https://github.com/kotorinChunChun/ChuwitchWindow
                                        </a>
                                    </div>
                                </div>

                                <div className="pt-6 border-t border-slate-800">
                                    <button
                                        onClick={handleResetConfig}
                                        className="px-4 py-2 bg-red-950/30 hover:bg-red-900/50 text-red-400 border border-red-900/50 rounded-lg text-xs transition-colors"
                                    >
                                        設定を完全に消去して初期化
                                    </button>
                                </div>
                            </div>
                        </div>
                    )}
                </main>
            </div>
        </div>
    );
}

export default App;
