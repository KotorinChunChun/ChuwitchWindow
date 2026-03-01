import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AppWindow, LayoutGrid, Rows, Columns, Layers, X } from "lucide-react";
import { cn } from "@/lib/utils";

export function ArrangePopup() {
  const handleArrange = async (type: "Grid" | "Vertical" | "Horizontal" | "Cascade") => {
    try {
      // 動作確認済みのため debug ログは削除し、本来のロジックに戻す
      await invoke("exec_arrange_cmd", { arrangeType: type });
    } catch (e) {
      console.error("Failed to execute arrange:", e);
    }
  };

  const handleClose = async () => {
    try {
      await invoke("hide_arrange_window_cmd");
    } catch (e) {
      console.error("Failed to hide window:", e);
    }
  };

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "1") handleArrange("Grid");
      if (e.key === "2") handleArrange("Vertical");
      if (e.key === "3") handleArrange("Horizontal");
      if (e.key === "4") handleArrange("Cascade");
      if (e.key === "Escape") handleClose();
    };

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, []);

  const items = [
    { id: "Grid", label: "並べて表示", icon: LayoutGrid, key: "1" },
    { id: "Vertical", label: "上下に並べる", icon: Rows, key: "2" },
    { id: "Horizontal", label: "左右に並べる", icon: Columns, key: "3" },
    { id: "Cascade", label: "重ねて表示", icon: Layers, key: "4" },
  ];

  return (
    <div className="h-screen w-screen flex items-center justify-center bg-transparent overflow-hidden select-none p-4">
      <div className="w-full h-full bg-slate-900/95 backdrop-blur-2xl border border-slate-700/50 rounded-2xl shadow-2xl flex flex-col items-stretch overflow-hidden">
        <div className="flex items-center justify-between px-6 py-3 border-b border-slate-800/50 flex-none leading-none">
          <div className="flex items-center text-sm font-bold text-slate-400 tracking-widest uppercase">
            <AppWindow className="w-5 h-5 mr-3 text-blue-400 flex-shrink-0" />
            ウィンドウの自動整列
          </div>
          <button 
            onClick={handleClose}
            className="p-2 hover:bg-slate-800 rounded-lg transition-colors text-slate-500 hover:text-slate-200 cursor-pointer"
            title="閉じる (Esc)"
          >
            <X className="w-6 h-6" />
          </button>
        </div>

        <div className="flex-1 flex items-center justify-center p-8 gap-8 min-h-0 overflow-hidden">
          {items.map((item) => (
            <div key={item.id} className="h-full flex items-center justify-center">
              <button
                onClick={() => handleArrange(item.id as any)}
                className={cn(
                  "group flex flex-col items-center justify-center rounded-[2rem] transition-all border-2 cursor-pointer h-full aspect-square p-4 sm:p-8",
                  "bg-slate-800/40 border-slate-700/30 hover:bg-blue-600/20 hover:border-blue-500/50 hover:scale-[1.02] active:scale-[0.98] shadow-2xl relative"
                )}
              >
                <item.icon className="w-20 h-20 mb-6 text-slate-200 group-hover:text-blue-400 transition-all duration-300 transform group-hover:scale-110 flex-shrink-0" />
                <div className="text-2xl font-bold tracking-tight text-slate-300 group-hover:text-white transition-colors whitespace-nowrap">
                  {item.label}
                </div>
                <div className="mt-6 px-6 py-2 bg-slate-950/80 rounded-2xl text-xl font-black text-slate-500 group-hover:text-blue-400 border border-slate-700/50 group-hover:border-blue-500/50 shadow-inner leading-none">
                  {item.key}
                </div>
              </button>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
