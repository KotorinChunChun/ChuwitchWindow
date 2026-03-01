import { useState, useEffect, useCallback } from "react";
import { AppConfig, WindowUIData } from "../types";
import { DialogKind } from "./CustomDialog";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { RefreshCw, Plus, Trash2, Edit } from "lucide-react";
import { cn } from "../lib/utils";
import {
    type RuleNode,
    parseRules,
    isExcluded,
    updateNodeByPath,
    getNodeByPath,
} from "../lib/ruleUtils";

function RuleTreeEditor({ 
  node, 
  onChange, 
  onDelete, 
  isRoot = false,
  path = [],
  selectedPath = [],
  onSelect,
  onShowDialog
}: { 
  node: RuleNode, 
  onChange: (newNode: RuleNode) => void,
  onDelete?: () => void,
  isRoot?: boolean,
  path?: number[],
  selectedPath?: number[],
  onSelect?: (path: number[], node: RuleNode) => void,
  onShowDialog: (title: string, message: string, kind?: DialogKind) => Promise<boolean>,
}) {
  const isSelected = path.length === selectedPath.length && path.every((v, i) => v === selectedPath[i]);
  
  if (node.type === "group") {
    return (
      <div 
        className={cn("rounded-lg border transition-colors cursor-pointer", 
          isRoot ? "border-transparent" : "mt-2 p-2 shadow-sm", // 字下げと左マージン削除
          isSelected ? "bg-blue-900/40 border-blue-500/50 ring-1 ring-blue-500/50" : "bg-slate-800/20 border-slate-600/50 hover:border-slate-500/50"
        )}
        onClick={(e) => {
            e.stopPropagation();
            if (onSelect) onSelect(path, node);
        }}
      >
        <div className="flex items-center space-x-2 mb-2 cursor-pointer">
          <select 
            value={node.match_type} 
            onChange={e => onChange({ ...node, match_type: e.target.value })}
            onClick={(e) => e.stopPropagation()}
            className="bg-slate-900 border border-slate-600 rounded px-2 py-0.5 text-slate-200 text-xs focus:border-blue-500 outline-none w-auto font-medium cursor-pointer"
          >
            <option value="OR">OR (いずれか)</option>
            <option value="AND">AND (すべて)</option>
          </select>
          <span className="text-xs text-slate-400">で除外</span>
          
          {!isRoot && onDelete && (
             <button 
                onClick={async (e) => { 
                    e.stopPropagation(); 
                    if (await onShowDialog("確認", "このグループを削除してもよろしいですか？", "ask")) {
                        onDelete(); 
                    }
                }} 
                className="text-slate-500 hover:text-red-400 text-xs flex items-center px-1.5 py-0.5 rounded transition-colors ml-auto cursor-pointer"
                title="グループを削除"
             >
               <Trash2 className="w-3.5 h-3.5" />
             </button>
          )}
        </div>
        
        {/* 左の縦線だけ残しつつ余白を最小限に */}
        <div className="space-y-1.5 border-l border-slate-700/50 py-1 pl-1 ml-1">
          {(!node.children || node.children.length === 0) && (
              <div className="text-slate-500 text-xs italic py-1">ルールなし</div>
          )}
          {node.children && node.children.map((child, idx) => (
             <RuleTreeEditor 
               key={idx} 
               node={child} 
               path={[...path, idx]}
               selectedPath={selectedPath}
               onSelect={onSelect}
               onShowDialog={onShowDialog}
               onChange={newChild => {
                 const newChildren = [...node.children];
                 newChildren[idx] = newChild;
                 onChange({ ...node, children: newChildren });
               }} 
               onDelete={() => {
                 const newChildren = node.children.filter((_, i) => i !== idx);
                 onChange({ ...node, children: newChildren });
               }} 
             />
          ))}
        </div>
      </div>
    );
  } else {
    // rule
    return (
      <div 
        className={cn(
          "flex items-center justify-between p-2 rounded border cursor-pointer transition-colors shadow-sm",
          isSelected ? "bg-blue-900/40 border-blue-500/50 ring-1 ring-blue-500/50" : "bg-slate-900 border-slate-700 hover:border-slate-500/50"
        )}
        onClick={(e) => {
            e.stopPropagation();
            if (onSelect) onSelect(path, node);
        }}
      >
        <div className="flex-1 space-y-1 min-w-0 cursor-pointer">
          {node.conditions && node.conditions.map((cond, i) => (
            <div key={i} className="text-xs text-slate-300 flex items-center space-x-1.5 truncate">
              <span className="font-mono bg-slate-800 px-1 py-0.5 rounded text-[10px] text-blue-300 min-w-[4.5rem] text-center">{cond.field}</span>
              <span className="text-slate-500 text-[10px] w-8 text-center">{cond.operator === "equals" ? "=" : cond.operator === "contains" ? "含む" : cond.operator === "starts_with" ? "始" : "終"}</span>
              <span className="font-mono text-emerald-300 text-[11px] bg-slate-950 px-1 py-0.5 rounded truncate">"{cond.value}"</span>
            </div>
          ))}
        </div>
        {onDelete && (
          <button 
             onClick={async (e) => { 
                 e.stopPropagation(); 
                 if (await onShowDialog("確認", "このルールを削除してもよろしいですか？", "ask")) {
                     onDelete(); 
                 }
             }} 
             className="ml-2 p-1 text-slate-500 hover:text-red-400 hover:bg-slate-800 rounded transition-colors flex-shrink-0 cursor-pointer"
             title="このルールを削除"
          >
            <Trash2 className="w-3.5 h-3.5" />
          </button>
        )}
      </div>
    );
  }
}

export function ExclusionPanel({
    config,
    onChange,
    onShowDialog,
}: {
    config: AppConfig;
    onChange: (c: AppConfig) => void;
    onShowDialog: (title: string, message: string, kind?: DialogKind) => Promise<boolean>;
}) {
    const [windows, setWindows] = useState<WindowUIData[]>([]);
    
    // 履歴管理
    const [history, setHistory] = useState<string[]>([config.exclusion_rules]);
    const [historyIndex, setHistoryIndex] = useState<number>(0);
    
    const [jsonInput, setJsonInput] = useState(config.exclusion_rules);
    const [jsonError, setJsonError] = useState("");
    const [selectedWindowHwnd, setSelectedWindowHwnd] = useState<string>("");
    
    // ツリー選択・編集管理
    const [selectedPath, setSelectedPath] = useState<number[]>([]);
    const [formMode, setFormMode] = useState<"ADD" | "EDIT" | "ADD_GROUP">("ADD");

    // フォーム入力の状態管理
    const [formConditions, setFormConditions] = useState<Record<string, {enabled: boolean, operator: string, value: string}>>({
        process_name: { enabled: true, operator: "equals", value: "" },
        class_name: { enabled: false, operator: "equals", value: "" },
        title: { enabled: false, operator: "contains", value: "" },
        style: { enabled: false, operator: "equals", value: "" },
        ex_style: { enabled: false, operator: "equals", value: "" },
    });

    const rootNode = parseRules(jsonInput);

    // 新しい状態を追加し履歴を保存
    const commitChange = useCallback((newJson: string) => {
        setJsonInput(newJson);
        const newHistory = [...history.slice(0, historyIndex + 1), newJson];
        setHistory(newHistory);
        setHistoryIndex(newHistory.length - 1);
        setJsonError("");
        onChange({ ...config, exclusion_rules: newJson });
    }, [history, historyIndex, config, onChange]);

    const undo = useCallback((e?: KeyboardEvent) => {
        if (historyIndex > 0) {
            if (e) e.preventDefault();
            const newIndex = historyIndex - 1;
            setHistoryIndex(newIndex);
            const pastJson = history[newIndex];
            setJsonInput(pastJson);
            setJsonError("");
            onChange({ ...config, exclusion_rules: pastJson });
        }
    }, [historyIndex, history, config, onChange]);

    const redo = useCallback((e?: KeyboardEvent) => {
        if (historyIndex < history.length - 1) {
            if (e) e.preventDefault();
            const newIndex = historyIndex + 1;
            setHistoryIndex(newIndex);
            const futureJson = history[newIndex];
            setJsonInput(futureJson);
            setJsonError("");
            onChange({ ...config, exclusion_rules: futureJson });
        }
    }, [historyIndex, history, config, onChange]);

    // Undo/Redo のショートカット (Ctrl+Z, Ctrl+Y / Ctrl+Shift+Z)
    useEffect(() => {
        const handleKeyDown = (e: KeyboardEvent) => {
            // テキスト入力中の場合はブラウザ標準の Undo を優先するためスキップ
            if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLSelectElement) {
                return;
            }
            if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'z') {
                if (e.shiftKey) {
                    redo(e);
                } else {
                    undo(e);
                }
            }
            if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'y') {
                redo(e);
            }
        };

        window.addEventListener('keydown', handleKeyDown);
        return () => window.removeEventListener('keydown', handleKeyDown);
    }, [undo, redo]);

    const fetchWindows = async () => {
        try {
            const wins = await invoke<WindowUIData[]>("get_window_list_cmd");
            setWindows(wins);
        } catch (e) {
            console.error("Failed to fetch windows", e);
        }
    };

    useEffect(() => {
        fetchWindows();

        const unlisten = listen("pin-toggled", () => {
            fetchWindows();
        });

        return () => {
            unlisten.then(f => f());
        };
    }, []);

    useEffect(() => {
        if (jsonError === "" && jsonInput !== config.exclusion_rules) {
            // 外部変更などがあれば同期 (基本的には UI操作による変更がメイン)
            // config が新しくなった際、それが history の最新と違えば追加する
            if (history[historyIndex] !== config.exclusion_rules) {
                setJsonInput(config.exclusion_rules);
                setHistory(prev => [...prev.slice(0, historyIndex + 1), config.exclusion_rules]);
                setHistoryIndex(prev => prev + 1);
            }
        }
    }, [config.exclusion_rules]);

    const handleJsonChange = (value: string) => {
        setJsonInput(value);
        try {
            JSON.parse(value);
            setJsonError("");
            // JSON手動入力が成功したらコミット
            commitChange(value);
        } catch (e: any) {
            setJsonError("JSONフォーマットエラー");
        }
    };

    const handleWindowSelect = (hwndStr: string) => {
        setSelectedWindowHwnd(hwndStr);
        if (!hwndStr) return;
        
        const win = windows.find(w => w.hwnd.toString() === hwndStr);
        if (win) {
            setFormConditions(prev => ({
                process_name: { ...prev.process_name, value: win.process_name, enabled: true },
                class_name: { ...prev.class_name, value: win.class_name, enabled: false },
                title: { ...prev.title, value: win.title, enabled: false },
                style: { ...prev.style, value: win.style.toString(16).toUpperCase().padStart(8, '0'), enabled: false },
                ex_style: { ...prev.ex_style, value: win.ex_style.toString(16).toUpperCase().padStart(8, '0'), enabled: false },
            }));
            if (formMode !== "EDIT") {
                setFormMode("ADD");
            }
        }
    };

    const handleSelectNode = (path: number[], node: RuleNode) => {
        setSelectedPath(path);
        if (node.type === "rule") {
            setFormMode("EDIT");
            // Populate form
            const newForm = {
                process_name: { enabled: false, operator: "equals", value: "" },
                class_name: { enabled: false, operator: "equals", value: "" },
                title: { enabled: false, operator: "contains", value: "" },
                style: { enabled: false, operator: "equals", value: "" },
                ex_style: { enabled: false, operator: "equals", value: "" },
            };
            node.conditions.forEach(cond => {
                if (newForm[cond.field as keyof typeof newForm]) {
                    newForm[cond.field as keyof typeof newForm] = {
                        enabled: true,
                        operator: cond.operator,
                        value: cond.value
                    };
                }
            });
            setFormConditions(newForm);
        } else {
            setFormMode("ADD");
        }
    };

    const handleAddOrUpdate = () => {
        const activeConditions = Object.entries(formConditions)
            .filter(([_, c]) => c.enabled && c.value.trim() !== "")
            .map(([field, c]) => ({
                field,
                operator: c.operator,
                value: c.value
            }));
        
        if (activeConditions.length === 0) {
            onShowDialog("注意", "有効な条件が選択されていません。", "warning");
            return;
        }

        const newRuleNode: RuleNode = { type: "rule", conditions: activeConditions };

        if (formMode === "EDIT") {
            // Update selected rule
            const newRoot = updateNodeByPath(rootNode, selectedPath, newRuleNode);
            const newJson = JSON.stringify(newRoot, null, 2);
            commitChange(newJson);
            
            // Switch back to ADD mode on parent group
            setSelectedPath(selectedPath.slice(0, -1));
            setFormMode("ADD");
        } else {
            // Add to selected group or root
            const targetPath = selectedPath;
            const targetNode = getNodeByPath(rootNode, targetPath);
            
            if (!targetNode || targetNode.type !== "group") {
                onShowDialog("エラー", "追加先のグループが選択されていません。", "error");
                return;
            }

            // 重複チェック
            const isDuplicate = targetNode.children.some(child => 
                JSON.stringify(child) === JSON.stringify(newRuleNode)
            );
            if (isDuplicate) {
                onShowDialog("注意", "このグループには既に全く同じ条件が存在します。", "warning");
                return;
            }

            const currentGroup = JSON.parse(JSON.stringify(targetNode));
            currentGroup.children.push(newRuleNode);
            
            const newRoot = updateNodeByPath(rootNode, targetPath, currentGroup);
            const newJson = JSON.stringify(newRoot, null, 2);
            commitChange(newJson);
        }
    };

    const handleAddGroup = () => {
        const targetPath = selectedPath;
        const targetNode = getNodeByPath(rootNode, targetPath);
        
        if (!targetNode || targetNode.type !== "group") {
            onShowDialog("エラー", "追加先のグループが選択されていません。", "error");
            return;
        }

        const newGroupNode: RuleNode = { type: "group", match_type: "AND", children: [] };
        
        const isDuplicate = targetNode.children.some(child => 
            JSON.stringify(child) === JSON.stringify(newGroupNode)
        );
        if (isDuplicate) {
            onShowDialog("注意", "空のグループが既に存在します。", "warning");
            return;
        }

        const currentGroup = JSON.parse(JSON.stringify(targetNode));
        currentGroup.children.push(newGroupNode);
        
        const newRoot = updateNodeByPath(rootNode, targetPath, currentGroup);
        const newJson = JSON.stringify(newRoot, null, 2);
        commitChange(newJson);
    };

    const handleUpdateRootTree = (newRoot: RuleNode) => {
        const newJson = JSON.stringify(newRoot, null, 2);
        commitChange(newJson);
    };

    let targetGroupText = "ルートグループ";
    const selectedNode = getNodeByPath(rootNode, selectedPath);
    if (selectedNode?.type === "group" && selectedPath.length > 0) {
        targetGroupText = "選択中グループ";
    } else if (selectedNode?.type === "rule") {
        targetGroupText = "選択ルールを編集";
    }

    return (
        <div className="flex-1 flex flex-col min-h-0 rounded-2xl bg-slate-900/40 border border-slate-800 backdrop-blur-sm p-4">
            <h3 className="text-xl font-medium text-slate-200 mb-4 border-b border-slate-800 pb-2 flex-none">除外ルールの設定</h3>
            
            {/* 横並び4列レイアウト */}
            <div className="flex-1 flex flex-row gap-4 min-h-0 overflow-x-auto pb-2">
                
                {/* 1. 条件式ツリー 追加位置・編集対象を選択 */}
                <div className="flex-1 min-w-[280px] flex flex-col min-h-0 bg-slate-950/50 rounded-lg border border-slate-800 p-3">
                    <div className="flex items-center justify-between mb-2 flex-none">
                        <h4 className="text-sm font-medium text-slate-300">条件式ツリー (追加先・編集対象)</h4>
                        <div className="flex space-x-1">
                            <button 
                                onClick={() => undo()} 
                                disabled={historyIndex === 0}
                                className="px-1.5 py-0.5 text-xs text-slate-400 disabled:opacity-30 hover:bg-slate-800 rounded transition-colors"
                                title="元に戻す (Ctrl+Z)"
                            >↩</button>
                            <button 
                                onClick={() => redo()} 
                                disabled={historyIndex === history.length - 1}
                                className="px-1.5 py-0.5 text-xs text-slate-400 disabled:opacity-30 hover:bg-slate-800 rounded transition-colors"
                                title="やり直し (Ctrl+Y)"
                            >↪</button>
                        </div>
                    </div>
                    <div className="flex-1 overflow-y-auto pr-1 cursor-text" onClick={() => {
                        // 領域外クリックでルートグループを選択状態に
                        if (rootNode.type === "group") handleSelectNode([], rootNode);
                    }}>
                        {jsonError ? (
                            <div className="p-3 text-center text-red-400 text-xs bg-red-500/10 rounded border border-red-500/20">
                                JSONエラーのため表示できません
                            </div>
                        ) : (
                            <RuleTreeEditor 
                                node={rootNode} 
                                onChange={handleUpdateRootTree} 
                                isRoot={true}
                                path={[]}
                                selectedPath={selectedPath}
                                onSelect={handleSelectNode}
                                onShowDialog={onShowDialog}
                            />
                        )}
                    </div>
                    {formMode !== "EDIT" && (
                        <div className="flex-none pt-3 mt-2 border-t border-slate-800">
                            <button
                                onClick={handleAddGroup}
                                className="w-full flex items-center justify-center px-3 py-1.5 bg-cyan-700 hover:bg-cyan-600 border border-cyan-600/50 rounded-lg text-xs font-medium transition-colors text-white shadow-sm cursor-pointer"
                            >
                                <Plus className="w-3 h-3 mr-1.5" />
                                AND/ORグループ作成
                            </button>
                        </div>
                    )}
                </div>

                {/* 2. 条件を編集 - ウィンドウから情報を取得 / 条件を設定 */}
                <div className="flex-1 min-w-[320px] flex flex-col min-h-0 bg-slate-950/40 rounded-lg border border-slate-800 p-4 overflow-y-auto">
                    
                    <h4 className="text-sm font-medium text-slate-300 mb-3 border-b border-slate-800 pb-1 flex-none">① ウィンドウから情報を取得</h4>
                    
                    {/* 通常のウィンドウ一覧 */}
                    <div className="flex space-x-2 mb-6 flex-none">
                        <select 
                            className="flex-1 bg-slate-950 border border-slate-700 rounded-lg px-2 py-1.5 text-slate-200 focus:outline-none focus:border-blue-500 text-xs min-w-0 cursor-pointer"
                            value={selectedWindowHwnd}
                            onChange={(e) => handleWindowSelect(e.target.value)}
                        >
                            <option value="">-- 対象ウィンドウを選択 --</option>
                            {windows.map(w => (
                                <option key={w.hwnd} value={w.hwnd.toString()}>
                                    {w.is_pinned ? "📌 " : ""}{w.process_name} - {w.title.substring(0, 30)}{w.title.length > 30 ? "..." : ""}
                                </option>
                            ))}
                        </select>
                        <button 
                            onClick={fetchWindows}
                            className="flex-none p-1.5 bg-slate-800 hover:bg-slate-700 rounded-lg text-slate-300 transition-colors border border-slate-700 cursor-pointer"
                            title="ウィンドウ一覧を更新"
                        >
                            <RefreshCw className="w-4 h-4" />
                        </button>
                    </div>

                    <h4 className="text-sm font-medium text-slate-300 mb-3 border-b border-slate-800 pb-1 flex-none">
                        ② 条件を設定 <span className="text-xs text-blue-400 font-normal ml-2">{formMode === "EDIT" ? "(編集中)" : "(新規追加)"}</span>
                    </h4>
                    
                    <div className="space-y-2 mb-4 bg-slate-900/30 p-3 rounded-lg border border-slate-800 flex-none">
                        {[
                            { key: "process_name", label: "プロセス名" },
                            { key: "class_name", label: "クラス名" },
                            { key: "title", label: "タイトル" },
                            { key: "style", label: "スタイル" },
                            { key: "ex_style", label: "拡張スタイル" },
                        ].map(f => (
                            <div key={f.key} className="flex items-center space-x-2">
                                <input 
                                    type="checkbox" 
                                    checked={formConditions[f.key].enabled} 
                                    onChange={e => setFormConditions(p => ({...p, [f.key]: {...p[f.key], enabled: e.target.checked}}))}
                                    className="flex-none w-3.5 h-3.5 bg-slate-900 border-slate-700 rounded focus:ring-blue-500 cursor-pointer"
                                />
                                <label 
                                    className="flex-none w-16 text-xs text-slate-300 font-medium truncate cursor-pointer" 
                                    title={f.label}
                                    onClick={() => setFormConditions(p => ({...p, [f.key]: {...p[f.key], enabled: !p[f.key].enabled}}))}
                                >{f.label}</label>
                                <select 
                                    className="flex-none bg-slate-950 border border-slate-700 rounded px-1.5 py-1 text-slate-200 focus:outline-none focus:border-blue-500 text-[11px] w-20 disabled:opacity-50 cursor-pointer"
                                    value={formConditions[f.key].operator}
                                    onChange={e => setFormConditions(p => ({...p, [f.key]: {...p[f.key], operator: e.target.value}}))}
                                    disabled={!formConditions[f.key].enabled}
                                >
                                    <option value="equals">一致 (=)</option>
                                    <option value="contains">含む (*)</option>
                                    <option value="starts_with">始まる</option>
                                    <option value="ends_with">終わる</option>
                                </select>
                                <input 
                                    type="text" 
                                    value={formConditions[f.key].value} 
                                    onChange={e => setFormConditions(p => ({...p, [f.key]: {...p[f.key], value: e.target.value}}))}
                                    className="flex-1 min-w-0 bg-slate-950 border border-slate-700 rounded px-2 py-1 text-slate-200 focus:outline-none focus:border-blue-500 text-xs disabled:opacity-50"
                                    placeholder="値を入力"
                                    disabled={!formConditions[f.key].enabled}
                                    spellCheck={false}
                                />
                            </div>
                        ))}
                    </div>

                    <div className="flex flex-col space-y-2 flex-none">
                        <button
                            onClick={handleAddOrUpdate}
                            className={cn(
                                "w-full flex items-center justify-center px-3 py-2 rounded-lg text-sm font-medium transition-colors cursor-pointer",
                                formMode === "EDIT" ? "bg-emerald-600 hover:bg-emerald-500" : "bg-blue-600 hover:bg-blue-500"
                            )}
                        >
                            {formMode === "EDIT" ? <><Edit className="w-4 h-4 mr-1.5" />ルールを更新して保存</> : <><Plus className="w-4 h-4 mr-1.5" />「{targetGroupText}」に追加</>}
                        </button>

                        {formMode === "EDIT" && (
                            <button
                                onClick={() => {
                                    setFormMode("ADD");
                                    setSelectedPath([]);
                                    setFormConditions({
                                        process_name: { enabled: true, operator: "equals", value: "" },
                                        class_name: { enabled: false, operator: "equals", value: "" },
                                        title: { enabled: false, operator: "contains", value: "" },
                                        style: { enabled: false, operator: "equals", value: "" },
                                        ex_style: { enabled: false, operator: "equals", value: "" },
                                    });
                                }}
                                className="w-full px-3 py-1.5 bg-slate-800 hover:bg-slate-700 border border-slate-700 rounded-lg text-xs font-medium transition-colors text-slate-400 cursor-pointer"
                            >
                                編集をキャンセル (新規追加に戻る)
                            </button>
                        )}
                    </div>
                </div>

                {/* 3. JSON直接編集 */}
                <div className="flex-1 min-w-[240px] flex flex-col min-h-0 bg-slate-950/40 rounded-lg border border-slate-800 p-3">
                    <h4 className="flex-none text-sm font-medium text-slate-300 mb-2">JSON直接編集</h4>
                    <textarea
                        value={jsonInput}
                        onChange={(e) => handleJsonChange(e.target.value)}
                        className={cn(
                            "flex-1 w-full bg-slate-950 border rounded p-2 text-xs font-mono text-slate-400 focus:outline-none focus:border-blue-500 transition-colors resize-none",
                            jsonError ? "border-red-500/50" : "border-slate-800"
                        )}
                        spellCheck={false}
                    />
                </div>

                {/* 4. 対象ウィンドウの判定結果 */}
                <div className="flex-1 min-w-[260px] flex flex-col min-h-0 bg-slate-950/50 rounded-lg border border-slate-800">
                    <div className="flex-none p-2 bg-slate-900 border-b border-slate-800 flex justify-between items-center">
                        <span className="text-xs font-medium text-slate-300">対象ウィンドウの判定結果</span>
                        <button onClick={fetchWindows} className="text-slate-500 hover:text-slate-300 transition-colors cursor-pointer">
                            <RefreshCw className="w-3.5 h-3.5" />
                        </button>
                    </div>
                    <div className="flex-1 overflow-y-auto p-1.5 space-y-1">
                        {jsonError ? (
                            <div className="p-3 text-center text-red-400 text-xs">JSONエラー</div>
                        ) : (
                            <>
                                {windows.map(w => {
                                    const excluded = isExcluded(w, jsonInput);
                                    return (
                                        <div key={w.hwnd} className={cn(
                                            "p-1.5 rounded text-xs flex items-center justify-between border",
                                            excluded 
                                                ? "bg-red-500/10 border-red-500/20 text-slate-400 opacity-70"
                                                : "bg-green-500/10 border-green-500/20 text-slate-300"
                                        )}>
                                            <div className="truncate pr-2 flex-1 min-w-0">
                                                <div className="font-medium truncate">{w.process_name} <span className="text-[10px] text-slate-500 ml-1">{w.title}</span></div>
                                                <div className="flex gap-1 mt-0.5 truncate overflow-hidden">
                                                    <span className="text-[9px] bg-slate-800 text-slate-400 px-1 rounded truncate flex-shrink min-w-0">{w.class_name}</span>
                                                </div>
                                            </div>
                                            <div className="flex-shrink-0 text-[10px] font-medium px-1.5 py-0.5 rounded">
                                                {excluded ? <span className="text-red-400">除外</span> : <span className="text-green-400">移動</span>}
                                            </div>
                                        </div>
                                    );
                                })}
                                {windows.length === 0 && (
                                    <div className="p-4 text-center text-slate-500 text-xs">ウィンドウなし</div>
                                )}
                            </>
                        )}
                    </div>
                </div>

            </div>
        </div>
    );
}
