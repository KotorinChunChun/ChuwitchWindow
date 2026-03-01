import { CheckCircle2, XCircle, AlertTriangle, Info } from "lucide-react";
import { cn } from "../lib/utils";
import { motion, AnimatePresence } from "framer-motion";

export type DialogKind = "info" | "success" | "warning" | "error" | "ask";

interface CustomDialogProps {
  isOpen: boolean;
  title: string;
  message: string;
  kind?: DialogKind;
  onConfirm: () => void;
  onCancel?: () => void;
}

export function CustomDialog({
  isOpen,
  title,
  message,
  kind = "info",
  onConfirm,
  onCancel,
}: CustomDialogProps) {
  const isAsk = kind === "ask";

  const getIcon = () => {
    switch (kind) {
      case "success":
        return <CheckCircle2 className="w-12 h-12 text-emerald-500" />;
      case "error":
        return <XCircle className="w-12 h-12 text-red-500" />;
      case "warning":
        return <AlertTriangle className="w-12 h-12 text-amber-500" />;
      case "ask":
        return <AlertTriangle className="w-12 h-12 text-blue-500" />;
      default:
        return <Info className="w-12 h-12 text-blue-500" />;
    }
  };

  return (
    <AnimatePresence>
      {isOpen && (
        <div className="fixed inset-0 z-[9999] flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm">
          <motion.div
            initial={{ opacity: 0, scale: 0.95, y: 20 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.95, y: 20 }}
            className="w-full max-w-md bg-slate-900 border border-slate-800 rounded-2xl shadow-2xl overflow-hidden"
          >
            <div className="p-6">
              <div className="flex flex-col items-center text-center space-y-4">
                {getIcon()}
                <div className="space-y-2">
                  <h3 className="text-xl font-bold text-slate-100">{title}</h3>
                  <p className="text-slate-400 text-sm whitespace-pre-wrap">{message}</p>
                </div>
              </div>
            </div>

            <div className="p-4 bg-slate-950/50 border-t border-slate-800 flex flex-row gap-3">
              {isAsk && onCancel && (
                <button
                  onClick={onCancel}
                  className="flex-1 px-4 py-2.5 bg-slate-800 hover:bg-slate-700 text-slate-300 rounded-xl text-sm font-medium transition-colors border border-slate-700 cursor-pointer"
                >
                  キャンセル
                </button>
              )}
              <button
                onClick={onConfirm}
                className={cn(
                  "flex-1 px-4 py-2.5 rounded-xl text-sm font-medium transition-colors cursor-pointer text-white",
                  isAsk ? "bg-blue-600 hover:bg-blue-500" : 
                  kind === "error" ? "bg-red-600 hover:bg-red-500" :
                  kind === "success" ? "bg-emerald-600 hover:bg-emerald-500" :
                  "bg-blue-600 hover:bg-blue-500"
                )}
              >
                {isAsk ? "実行する" : "閉じる"}
              </button>
            </div>
          </motion.div>
        </div>
      )}
    </AnimatePresence>
  );
}
