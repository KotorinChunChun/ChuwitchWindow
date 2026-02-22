import { useState, useEffect } from "react";
import { AppConfig } from "../types";
import { ShieldAlert, Keyboard } from "lucide-react";
import { cn } from "../lib/utils";
import { invoke } from "@tauri-apps/api/core";

export function SettingsPanel({
    config,
    isAdmin,
    onChange,
}: {
    config: AppConfig;
    isAdmin: boolean;
    onChange: (c: AppConfig) => void;
}) {
    const updateConfig = (updates: Partial<AppConfig>) => {
        onChange({ ...config, ...updates });
    };

    return (
        <div className="flex-1 rounded-2xl bg-slate-900/40 border border-slate-800 backdrop-blur-sm p-6 overflow-y-auto">
            <div className="space-y-8 max-w-2xl">

                {/* Registration Section */}
                <section>
                    <h3 className="text-xl font-medium text-slate-200 mb-4 border-b border-slate-800 pb-2">一般設定</h3>

                    <div className="space-y-4">
                        <ToggleOption
                            label="PC起動時に自動実行 (スタートアップ)"
                            description="Schtasks（タスクスケジューラ）を用いてログイン時に自動実行します"
                            checked={config.run_on_startup}
                            onChange={(v) => updateConfig({ run_on_startup: v })}
                        />

                        {!isAdmin && config.run_on_startup && (
                            <div className="flex items-start p-3 bg-amber-500/10 border border-amber-500/20 rounded-lg text-amber-200 text-base">
                                <ShieldAlert className="w-6 h-6 mr-3 flex-shrink-0 text-amber-500" />
                                <p>スタートアップの登録・解除には管理者権限が必要です。下のボタンから再起動してください。</p>
                            </div>
                        )}

                        {!isAdmin && (
                            <button
                                onClick={() => invoke("restart_as_admin")}
                                className="mt-4 flex items-center px-4 py-2 bg-slate-800 hover:bg-slate-700 rounded-lg text-base text-slate-300 transition-colors border border-slate-700"
                            >
                                <ShieldAlert className="w-5 h-5 mr-2" />
                                管理者として再起動する
                            </button>
                        )}
                    </div>
                </section>

                {/* Hotkeys Section */}
                <section>
                    <h3 className="text-xl font-medium text-slate-200 mb-4 border-b border-slate-800 pb-2">ショートカットキー</h3>
                    <p className="text-sm text-slate-500 mb-4">
                        入力欄をクリックし、登録したいキーの組み合わせを押してください。
                    </p>
                    <div className="space-y-4">
                        <HotkeyInput
                            label="増加方向へシフト"
                            value={config.rotate_cw_hotkey}
                            onChange={(v) => updateConfig({ rotate_cw_hotkey: v })}
                        />
                        <HotkeyInput
                            label="減少方向へシフト"
                            value={config.rotate_ccw_hotkey}
                            onChange={(v) => updateConfig({ rotate_ccw_hotkey: v })}
                        />
                        <HotkeyInput
                            label="1回前の状態に戻す (Undo)"
                            value={config.undo_hotkey}
                            onChange={(v) => updateConfig({ undo_hotkey: v })}
                        />
                        <HotkeyInput
                            label="指定画面をプライマリデスクトップと入れ替え"
                            value={config.swap_target_modifiers ? `${config.swap_target_modifiers}+2` : ""}
                            onChange={(v) => {
                                // Extract modifiers only, discarding the last key
                                const parts = v.split("+");
                                if (parts.length > 0) {
                                    parts.pop(); // remove the target key (e.g. 2)
                                }
                                updateConfig({ swap_target_modifiers: parts.join("+") });
                            }}
                            displayFormat={(v) => {
                                if (!v) return "";
                                const parts = v.split("+");
                                parts.pop();
                                return parts.length > 0 ? `${parts.join("+")} + 2~9` : "2~9";
                            }}
                        />
                    </div>
                </section>

                {/* Behavior Section */}
                <section>
                    <h3 className="text-xl font-medium text-slate-200 mb-4 border-b border-slate-800 pb-2">動作設定</h3>
                    <div className="space-y-4">
                        <ToggleOption
                            label="グループ内で入れ替えを行う"
                            description="ONの場合、増加・減少方向へのシフト時に同じグループのモニタ間のみで入れ替えを行います"
                            checked={config.swap_within_groups}
                            onChange={(v) => updateConfig({ swap_within_groups: v })}
                        />
                        <ToggleOption
                            label="フルスクリーンウィンドウを除外"
                            description="ゲームなどのフルスクリーンウィンドウは移動対象から外します"
                            checked={config.ignore_fullscreen}
                            onChange={(v) => updateConfig({ ignore_fullscreen: v })}
                        />
                    </div>
                </section>

            </div>
        </div>
    );
}

function ToggleOption({ label, description, checked, onChange }: { label: string; description?: string; checked: boolean; onChange: (v: boolean) => void }) {
    return (
        <div className="flex items-center justify-between p-4 bg-slate-900/50 rounded-xl border border-slate-800">
            <div className="pr-4">
                <div className="text-base font-medium text-slate-200">{label}</div>
                {description && <div className="text-sm text-slate-500 mt-1">{description}</div>}
            </div>
            <button
                onClick={() => onChange(!checked)}
                className={cn(
                    "relative inline-flex h-6 w-11 items-center rounded-full transition-colors focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 focus:ring-offset-slate-900",
                    checked ? "bg-blue-500" : "bg-slate-700"
                )}
            >
                <span
                    className={cn(
                        "inline-block h-4 w-4 transform rounded-full bg-white transition-transform",
                        checked ? "translate-x-6" : "translate-x-1"
                    )}
                />
            </button>
        </div>
    );
}

function HotkeyInput({
    label,
    value,
    onChange,
    displayFormat
}: {
    label: string;
    value: string;
    onChange: (v: string) => void;
    displayFormat?: (v: string) => string;
}) {
    const [isRecording, setIsRecording] = useState(false);
    const [tempValue, setTempValue] = useState(value);
    const [conflictError, setConflictError] = useState(false);

    const mapCode = (code: string) => {
        if (code.startsWith("Key")) return code.substring(3); // e.g., KeyA -> A
        if (code.startsWith("Digit")) return code.substring(5); // e.g., Digit1 -> 1
        if (code === "ArrowRight") return "Right";
        if (code === "ArrowLeft") return "Left";
        if (code === "ArrowUp") return "Up";
        if (code === "ArrowDown") return "Down";
        return code;
    };

    useEffect(() => {
        if (!isRecording) {
            setTempValue(value);
            checkConflict(value);
        }

        // バックエンドにも録画状態を伝え、ショートカット機能を一時無効化する
        invoke("set_recording_state_cmd", { isRecording }).catch(console.error);
    }, [value, isRecording]);

    const checkConflict = async (hotkeyStr: string) => {
        if (!hotkeyStr) {
            setConflictError(false);
            return;
        }
        try {
            const isConflict = await invoke<boolean>("check_hotkey_conflict_cmd", { hotkeyStr });
            setConflictError(isConflict);
        } catch (e) {
            console.error(e);
            setConflictError(true);
        }
    };

    useEffect(() => {
        if (!isRecording) {
            return;
        }

        const handleKeyDown = (e: KeyboardEvent) => {
            e.preventDefault();

            const isModifierOnly = ["Control", "Shift", "Alt", "Meta", "OS"].includes(e.key);

            const keys = [];
            if (e.metaKey) keys.push("Win");
            if (e.ctrlKey) keys.push("Ctrl");
            if (e.altKey) keys.push("Alt");
            if (e.shiftKey) keys.push("Shift");

            if (!isModifierOnly) {
                keys.push(mapCode(e.code));
                const combined = keys.join("+");
                setTempValue(combined);
                onChange(combined);
                setIsRecording(false);
            } else {
                // Live preview for modifiers
                setTempValue(keys.join("+") + "+...");
            }
        };

        const handleKeyUp = (e: KeyboardEvent) => {
            const keys = [];
            if (e.metaKey) keys.push("Win");
            if (e.ctrlKey) keys.push("Ctrl");
            if (e.altKey) keys.push("Alt");
            if (e.shiftKey) keys.push("Shift");

            if (isRecording) {
                if (keys.length === 0) {
                    setTempValue("入力待機中...");
                } else {
                    setTempValue(keys.join("+") + "+...");
                }
            }
        };

        const handleClickOutside = () => {
            setIsRecording(false);
            setTempValue(value);
            checkConflict(value);
        };

        window.addEventListener("keydown", handleKeyDown);
        window.addEventListener("keyup", handleKeyUp);

        // Small delay to prevent current click from cancelling recording
        setTimeout(() => {
            window.addEventListener("click", handleClickOutside);
        }, 10);

        return () => {
            window.removeEventListener("keydown", handleKeyDown);
            window.removeEventListener("keyup", handleKeyUp);
            window.removeEventListener("click", handleClickOutside);
        };
    }, [isRecording, onChange, value]);

    const displayVal = displayFormat && !isRecording && tempValue === value ? displayFormat(tempValue) : tempValue;

    return (
        <div className="flex flex-col space-y-2 p-4 bg-slate-900/50 rounded-xl border border-slate-800">
            <div className="flex items-center justify-between">
                <div className="flex items-center">
                    <Keyboard className="w-6 h-6 text-slate-500 mr-3" />
                    <span className="text-base font-medium text-slate-200">{label}</span>
                </div>
                <button
                    onClick={(e) => {
                        e.stopPropagation();
                        setIsRecording(true);
                        setTempValue("入力待機中...");
                    }}
                    className={cn(
                        "px-4 py-3 rounded-md border text-sm font-mono tracking-wider transition-all min-w-[200px] text-center",
                        isRecording
                            ? "bg-blue-500/20 border-blue-500 text-blue-300 ring-2 ring-blue-500/50 animate-pulse"
                            : conflictError
                                ? "bg-red-950 border-red-500 text-red-400"
                                : "bg-slate-950 border-slate-700 hover:border-slate-500 text-slate-300"
                    )}
                >
                    {displayVal || "未設定"}
                </button>
            </div>
            {!isRecording && conflictError && (
                <div className="text-sm text-red-400 font-medium text-right mt-1">
                    ⚠️ 何らかのプロセスで使用中です
                </div>
            )}
        </div>
    );
}
