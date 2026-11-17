import { useState, useEffect } from "react";
import { AppConfig } from "../types";
import { DialogKind } from "./CustomDialog";
import { ShieldAlert, Keyboard } from "lucide-react";
import { cn } from "../lib/utils";
import { invoke } from "@tauri-apps/api/core";
import { save, open } from "@tauri-apps/plugin-dialog";

export function SettingsPanel({
    config,
    isAdmin,
    onChange,
    onResetConfig,
    onShowDialog,
}: {
    config: AppConfig;
    isAdmin: boolean;
    onChange: (c: AppConfig) => void;
    onResetConfig: () => void;
    onShowDialog: (title: string, message: string, kind?: DialogKind) => Promise<boolean>;
}) {
    const [isPathRegistered, setIsPathRegistered] = useState(false);
    const [autostartConflictPath, setAutostartConflictPath] = useState<string | null>(null);

    const checkPathAndAutostart = async (currentConfig: AppConfig) => {
        try {
            const isPathReg = await invoke<boolean>("check_path_registered_cmd");
            setIsPathRegistered(isPathReg);

            // Check actual autostart registration
            const autoStatus = await invoke<{
                is_registered: boolean;
                is_same_path: boolean;
                registered_path: string | null;
            }>("check_autostart_registered_cmd");

            let needUpdate = false;
            let newRunOnStartup = currentConfig.run_on_startup;

            if (autoStatus.is_registered && !autoStatus.is_same_path) {
                setAutostartConflictPath(autoStatus.registered_path);
                if (currentConfig.run_on_startup) {
                    newRunOnStartup = false;
                    needUpdate = true;
                }
            } else {
                setAutostartConflictPath(null);
                if (!autoStatus.is_registered && currentConfig.run_on_startup) {
                    newRunOnStartup = false;
                    needUpdate = true;
                } else if (autoStatus.is_registered && autoStatus.is_same_path && !currentConfig.run_on_startup) {
                    newRunOnStartup = true;
                    needUpdate = true;
                }
            }

            if (needUpdate) {
                onChange({ ...currentConfig, run_on_startup: newRunOnStartup });
            }
            return { autoStatus, wasUpdated: needUpdate, newRunOnStartup };
        } catch (e) {
            console.error(e);
            return null;
        }
    };

    useEffect(() => {
        checkPathAndAutostart(config);
        // Only run on mount or when admin status changes.
        // We handle explicit toggle via onChange elsewhere.
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [isAdmin]);

    const handleRunOnStartupToggle = async (newValue: boolean) => {
        try {
            // まずタスクスケジューラの登録/解除を試みる
            await invoke("sync_admin_startup", { enable: newValue });

            // 成功した場合のみ設定を更新
            const nextConfig = { ...config, run_on_startup: newValue };
            onChange(nextConfig);

            // 登録完了後に実際の状態を再確認
            await checkPathAndAutostart(nextConfig);

            if (newValue) {
                await onShowDialog("完了", "自動起動を登録しました。", "success");
            } else {
                await onShowDialog("完了", "自動起動を解除しました。", "success");
            }
        } catch (e) {
            // 失敗した場合でも実際の状態を確認して同期する
            console.error("自動起動の設定に失敗:", e);
            await checkPathAndAutostart(config);
            await onShowDialog("エラー", `自動起動の${newValue ? "登録" : "解除"}に失敗しました。\n\n${e}`, "error");
        }
    };

    const updateConfig = (updates: Partial<AppConfig>) => {
        onChange({ ...config, ...updates });
    };

    return (
        <div className="flex-1 rounded-2xl bg-slate-900/40 border border-slate-800 backdrop-blur-sm p-6 overflow-y-auto">
            <div className="flex flex-col lg:flex-row gap-8">
                {/* Left Column: Basic settings and maintenance */}
                <div className="flex-1 space-y-8 min-w-0">
                    {/* General Section */}
                    <section>
                        <h3 className="text-xl font-medium text-slate-200 mb-4 border-b border-slate-800 pb-2">一般設定</h3>
                        <div className="space-y-4">
                            <ToggleOption
                                label="PC起動時に自動実行 (スタートアップ)"
                                description="Schtasks（タスクスケジューラ）を用いてログイン時に自動実行します"
                                checked={config.run_on_startup}
                                disabled={!isAdmin}
                                onChange={handleRunOnStartupToggle}
                            />
                            
                            {autostartConflictPath && (
                                <div className="flex items-start p-3 bg-red-500/10 border border-red-500/20 rounded-lg text-red-200 text-sm mt-2">
                                    <ShieldAlert className="w-5 h-5 mr-3 flex-shrink-0 text-red-400" />
                                    <p>ChuwitchWindow_AutoStart は別のパスのEXEが登録済みです。<br/>パス：{autostartConflictPath}</p>
                                </div>
                            )}

                            {!isAdmin && config.run_on_startup && (
                                <div className="flex items-start p-3 bg-amber-500/10 border border-amber-500/20 rounded-lg text-amber-200 text-base">
                                    <ShieldAlert className="w-6 h-6 mr-3 flex-shrink-0 text-amber-500" />
                                    <p>スタートアップの登録・解除には管理者権限が必要です。下のボタンから再起動してください。</p>
                                </div>
                            )}

                            {!isAdmin && (
                                <button
                                    onClick={() => invoke("restart_as_admin")}
                                    className="mt-4 flex items-center px-4 py-2 bg-slate-800 hover:bg-slate-700 rounded-lg text-base text-slate-300 transition-colors border border-slate-700 cursor-pointer"
                                >
                                    <ShieldAlert className="w-5 h-5 mr-2" />
                                    管理者として再起動する
                                </button>
                            )}
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
                                label="フルスクリーンウィンドウを移動対象から除外"
                                description="ゲームなどのフルスクリーンウィンドウは移動対象から外します"
                                checked={config.ignore_fullscreen}
                                onChange={(v) => updateConfig({ ignore_fullscreen: v })}
                            />
                            <ToggleOption
                                label="最小化ウィンドウを移動対象から除外"
                                description="最小化されているウィンドウを移動対象から外すことで、パフォーマンスが向上します"
                                checked={config.exclude_minimized}
                                onChange={(v) => updateConfig({ exclude_minimized: v })}
                            />
                        </div>
                    </section>

                    {/* Maintenance Section */}
                    <section>
                        <h3 className="text-xl font-medium text-slate-200 mb-4 border-b border-slate-800 pb-2">メンテナンス</h3>
                        <div className="space-y-4">
                            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                                <button
                                    onClick={async () => {
                                        try {
                                            const path = await save({
                                                filters: [{ name: "JSON", extensions: ["json"] }],
                                                defaultPath: "chuwitch_config.json",
                                            });
                                            if (path) {
                                                await invoke("export_config_cmd", { path });
                                                await onShowDialog("完了", "設定を書き出しました", "success");
                                            }
                                        } catch (e) {
                                            console.error(e);
                                            await onShowDialog("エラー", "書き出しに失敗しました", "error");
                                        }
                                    }}
                                    className="flex items-center justify-center px-4 py-3 bg-slate-800 hover:bg-slate-700 rounded-xl text-slate-200 transition-colors border border-slate-700 cursor-pointer"
                                >
                                    設定をファイルに保存 (Export)
                                </button>
                                <button
                                    onClick={async () => {
                                        try {
                                            const path = await open({
                                                filters: [{ name: "JSON", extensions: ["json"] }],
                                                multiple: false,
                                            });
                                            if (path && typeof path === "string") {
                                                const newConfig = await invoke<AppConfig>("import_config_cmd", { path });
                                                onChange(newConfig);
                                                await onShowDialog("完了", "設定を読み込みました", "success");
                                            }
                                        } catch (e) {
                                            console.error(e);
                                            await onShowDialog("エラー", "読み込みに失敗しました。ファイル形式を確認してください。", "error");
                                        }
                                    }}
                                    className="flex items-center justify-center px-4 py-3 bg-slate-800 hover:bg-slate-700 rounded-xl text-slate-200 transition-colors border border-slate-700 cursor-pointer"
                                >
                                    ファイルから設定を復元 (Import)
                                </button>
                            </div>

                            <div className="p-4 bg-blue-500/5 border border-blue-500/20 rounded-xl">
                                <div className="flex items-center justify-between">
                                    <div className="pr-4">
                                        <div className="text-base font-medium text-slate-200">PATH 環境変数にアプリを追加</div>
                                        <div className="text-sm text-slate-500 mt-1">
                                            コマンドプロンプト等で "chuwitchwindow" と打つだけで実行できるようになります。
                                        </div>
                                    </div>
                                    <div className="flex items-center gap-2">
                                        {isPathRegistered ? (
                                            <button
                                                onClick={async () => {
                                                    try {
                                                        await invoke("unregister_from_path_cmd");
                                                        setIsPathRegistered(false);
                                                        await onShowDialog("完了", "PATH 環境変数から削除しました。", "success");
                                                    } catch (e) {
                                                        await onShowDialog("エラー", "削除に失敗しました: " + e, "error");
                                                    }
                                                }}
                                                className="px-4 py-2 bg-red-900/30 hover:bg-red-800/50 text-red-400 rounded-lg text-sm transition-colors border border-red-800/50 cursor-pointer whitespace-nowrap"
                                            >
                                                削除
                                            </button>
                                        ) : (
                                            <button
                                                onClick={async () => {
                                                    try {
                                                        await invoke("register_to_path_cmd");
                                                        setIsPathRegistered(true);
                                                        await onShowDialog("完了", "PATH 環境変数に登録しました。反映にはターミナルの再起動が必要です。", "success");
                                                    } catch (e) {
                                                        await onShowDialog("エラー", "登録に失敗しました: " + e, "error");
                                                    }
                                                }}
                                                className="px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white rounded-lg text-sm transition-colors cursor-pointer whitespace-nowrap"
                                            >
                                                登録を追加
                                            </button>
                                        )}
                                    </div>
                                </div>
                            </div>

                            <div className="pt-4 border-t border-slate-800 flex justify-end">
                                <button
                                    onClick={(e) => {
                                        e.preventDefault();
                                        e.stopPropagation();
                                        onResetConfig();
                                    }}
                                    className="px-4 py-2 bg-red-950/30 hover:bg-red-900/50 text-red-400 border border-red-900/50 rounded-lg text-sm transition-colors cursor-pointer"
                                >
                                    設定を完全に消去して初期化
                                </button>
                            </div>
                        </div>
                    </section>
                </div>

                {/* Right Column: Hotkeys */}
                <div className="flex-1 space-y-8 min-w-0">
                    <section>
                        <h3 className="text-xl font-medium text-slate-200 mb-4 border-b border-slate-800 pb-2">ショートカットキー</h3>
                        <p className="text-base text-slate-500 mb-6">
                            入力欄をクリックし、キーの組み合わせを押してください。
                        </p>
                        <div className="space-y-4">
                            <HotkeyInput
                                label="増加方向へシフト"
                                tooltip="アクティブなウィンドウなどを増加方向（右や下）のモニタへ比率を維持したまま移動・入れ替えします"
                                value={config.rotate_cw_hotkey}
                                onChange={(v) => updateConfig({ rotate_cw_hotkey: v })}
                            />
                            <HotkeyInput
                                label="減少方向へシフト"
                                tooltip="アクティブなウィンドウなどを減少方向（左や上）のモニタへ比率を維持したまま移動・入れ替えします"
                                value={config.rotate_ccw_hotkey}
                                onChange={(v) => updateConfig({ rotate_ccw_hotkey: v })}
                            />
                            <HotkeyInput
                                label="1回前の状態に戻す (Undo)"
                                tooltip="直前に行ったウィンドウの入れ替えや移動操作を元に戻します"
                                value={config.undo_hotkey}
                                onChange={(v) => updateConfig({ undo_hotkey: v })}
                            />
                            <HotkeyInput
                                label="指定画面と入れ替え (修飾キー)"
                                tooltip="プライマリモニタと、入力した数字(2~9)のモニタのウィンドウを瞬時に入れ替えます"
                                value={config.swap_target_modifiers ? `${config.swap_target_modifiers}+2` : ""}
                                onChange={(v) => {
                                    const parts = v.split("+");
                                    if (parts.length > 0) parts.pop();
                                    updateConfig({ swap_target_modifiers: parts.join("+") });
                                }}
                                displayFormat={(v) => {
                                    if (!v) return "";
                                    const parts = v.split("+");
                                    parts.pop();
                                    return parts.length > 0 ? `${parts.join("+")} + 2~9` : "2~9";
                                }}
                            />

                            <div className="pt-4 border-t border-slate-800/50"></div>

                            <HotkeyInput
                                label="ウィンドウのピン留め切替"
                                tooltip="アクティブなウィンドウを現在のモニターにピン留めし、移動・入れ替えの対象から除外します"
                                value={config.pin_hotkey}
                                onChange={(v) => updateConfig({ pin_hotkey: v })}
                            />
                            <HotkeyInput
                                label="一斉退避（エスケープ）"
                                tooltip="現在のモニターの全ウィンドウを、他のモニターへ一斉に退避させます"
                                value={config.escape_hotkey}
                                onChange={(v) => updateConfig({ escape_hotkey: v })}
                            />
                            <HotkeyInput
                                label="一極集中（ギャザー）"
                                tooltip="全モニターのウィンドウを、現在のモニターへ一極集中（集約）させます"
                                value={config.gather_hotkey}
                                onChange={(v) => updateConfig({ gather_hotkey: v })}
                            />
                            <HotkeyInput
                                label="自動整列"
                                tooltip="ウィンドウの自動整列（並べて表示、重ねて表示など）メニューを表示します"
                                value={config.arrange_hotkey}
                                onChange={(v) => updateConfig({ arrange_hotkey: v })}
                            />
                        </div>
                    </section>
                </div>
            </div>
        </div>
    );
}

function ToggleOption({ label, description, checked, disabled, onChange }: { label: string; description?: string; checked: boolean; disabled?: boolean; onChange: (v: boolean) => void }) {
    return (
        <div className={cn("flex items-center justify-between p-4 bg-slate-900/50 rounded-xl border border-slate-800", disabled && "opacity-50 grayscale")}>
            <div className="pr-4">
                <div className="text-base font-medium text-slate-200">{label}</div>
                {description && <div className="text-sm text-slate-500 mt-1">{description}</div>}
            </div>
            <button
                disabled={disabled}
                onClick={() => !disabled && onChange(!checked)}
                className={cn(
                    "relative inline-flex h-6 w-11 items-center rounded-full transition-colors focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 focus:ring-offset-slate-900",
                    disabled ? "cursor-not-allowed" : "cursor-pointer",
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
    tooltip,
    value,
    onChange,
    displayFormat
}: {
    label: string;
    tooltip?: string;
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
    }, [value, isRecording]);

    // 録画中はバックエンドのショートカット機能を一時無効化する。
    // 録画終了時だけでなく、録画中にタブ切り替え等でアンマウントされた場合も解除する。
    useEffect(() => {
        if (!isRecording) return;
        invoke("set_recording_state_cmd", { isRecording: true }).catch(console.error);
        return () => {
            invoke("set_recording_state_cmd", { isRecording: false }).catch(console.error);
        };
    }, [isRecording]);

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
                <div className="flex items-center" title={tooltip}>
                    <Keyboard className="w-6 h-6 text-slate-500 mr-3" />
                    <span className="text-base font-medium text-slate-200 cursor-help border-b border-dotted border-slate-600">{label}</span>
                </div>
                <button
                    onClick={(e) => {
                        e.stopPropagation();
                        setIsRecording(true);
                        setTempValue("入力待機中...");
                    }}
                    className={cn(
                        "px-4 py-3 rounded-md border text-sm font-mono tracking-wider transition-all min-w-[200px] text-center cursor-pointer",
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
