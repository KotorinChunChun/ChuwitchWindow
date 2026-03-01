import { LayoutGrid, Settings, Info, Ban } from "lucide-react";
import { cn } from "@/lib/utils";

/** タブの定義 */
const TABS = [
  { id: "map" as const, label: "モニター構成", icon: LayoutGrid },
  { id: "exclusion" as const, label: "除外リスト", icon: Ban },
  { id: "settings" as const, label: "設定・常駐", icon: Settings },
  { id: "about" as const, label: "アプリについて", icon: Info },
] as const;

export type TabId = (typeof TABS)[number]["id"];

/**
 * サイドバーコンポーネント
 * アプリ名、ナビゲーションタブ、管理者モード表示を持つ。
 */
export function Sidebar({
  activeTab,
  onTabChange,
  isAdmin,
}: {
  activeTab: TabId;
  onTabChange: (tab: TabId) => void;
  isAdmin: boolean;
}) {
  return (
    <div className="w-64 flex flex-col bg-slate-900/50 backdrop-blur-xl border-r border-slate-800">
      <div className="p-6">
        <h1 className="text-xl font-bold bg-gradient-to-r from-blue-400 to-cyan-300 bg-clip-text text-transparent">
          ChuwitchWindow
        </h1>
        <p className="text-xs text-slate-400 mt-1">Multi-Monitor Wizard</p>
      </div>

      <nav className="flex-1 px-4 space-y-2 mt-4">
        {TABS.map((tab) => (
          <button
            key={tab.id}
            onClick={() => onTabChange(tab.id)}
            className={cn(
              "flex items-center w-full px-4 py-3 rounded-lg text-base font-medium transition-colors cursor-pointer",
              activeTab === tab.id
                ? "bg-blue-500/10 text-blue-400 border border-blue-500/20"
                : "text-slate-400 hover:bg-slate-800/50 hover:text-slate-200",
            )}
          >
            <tab.icon className="w-5 h-5 mr-3" />
            {tab.label}
          </button>
        ))}
      </nav>

      <div className="p-4 border-t border-slate-800/50 text-xs text-slate-500">
        {isAdmin ? (
          <span className="text-green-400">Admin Mode</span>
        ) : (
          <span className="text-amber-400"></span>
        )}
      </div>
    </div>
  );
}
