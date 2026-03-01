import { WindowUIData } from "../types";

// --- 型定義 ---

export type RuleCondition = {
    field: string;
    operator: string;
    value: string;
};

export type RuleNode =
  | { type: "group"; match_type: string; children: RuleNode[] }
  | { type: "rule"; conditions: RuleCondition[] };

// --- ルールマッチング ---

/** ウィンドウがルールノードの条件に一致するかを再帰的に判定する */
export function matchNode(win: WindowUIData, node: RuleNode): boolean {
    if (node.type === "group") {
        if (!node.children || node.children.length === 0) return false;
        const isAnd = node.match_type?.toUpperCase() === "AND";
        for (const child of node.children) {
            const matched = matchNode(win, child);
            if (isAnd && !matched) return false;
            if (!isAnd && matched) return true;
        }
        return isAnd;
    } else if (node.type === "rule") {
        if (!node.conditions || node.conditions.length === 0) return false;
        for (const cond of node.conditions) {
            let fieldVal = "";
            if (cond.field === "process_name") fieldVal = win.process_name;
            else if (cond.field === "class_name") fieldVal = win.class_name;
            else if (cond.field === "title") fieldVal = win.title;
            else if (cond.field === "style") fieldVal = win.style.toString(16).toUpperCase().padStart(8, '0');
            else if (cond.field === "ex_style") fieldVal = win.ex_style.toString(16).toUpperCase().padStart(8, '0');

            let matched = false;
            if (cond.operator === "equals") matched = fieldVal === cond.value;
            else if (cond.operator === "contains") matched = fieldVal.includes(cond.value);
            else if (cond.operator === "starts_with") matched = fieldVal.startsWith(cond.value);
            else if (cond.operator === "ends_with") matched = fieldVal.endsWith(cond.value);

            if (!matched) return false;
        }
        return true;
    }
    return false;
}

// --- ルールパース ---

/** JSON文字列からルールツリーをパースする。レガシー配列形式もサポート。 */
export function parseRules(jsonStr: string): RuleNode {
    if (!jsonStr || jsonStr.trim() === "") {
        return { type: "group", match_type: "OR", children: [] };
    }
    try {
        const parsed = JSON.parse(jsonStr);
        if (Array.isArray(parsed)) {
            const children: RuleNode[] = [];
            for (const rule of parsed) {
                if (rule.conditions && rule.conditions.length > 1) {
                    if (rule.match_type?.toUpperCase() === "OR") {
                        const subChildren: RuleNode[] = [];
                        for (const cond of rule.conditions) {
                            subChildren.push({ type: "rule", conditions: [cond] });
                        }
                        children.push({ type: "group", match_type: "OR", children: subChildren });
                    } else {
                        children.push({ type: "rule", conditions: rule.conditions });
                    }
                } else if (rule.conditions && rule.conditions.length > 0) {
                    children.push({ type: "rule", conditions: rule.conditions });
                }
            }
            return { type: "group", match_type: "OR", children };
        } else if (parsed.type === "group" || parsed.type === "rule") {
            return parsed as RuleNode;
        }
    } catch {
        // JSON構文エラー
    }
    return { type: "group", match_type: "OR", children: [] };
}

/** ウィンドウが除外ルールに一致するかを判定する */
export function isExcluded(win: WindowUIData, jsonStr: string): boolean {
    const root = parseRules(jsonStr);
    return matchNode(win, root);
}

// --- ツリー操作 ---

/** パスで指定されたノードを新しいノードに置き換える（null で削除） */
export function updateNodeByPath(root: RuleNode, path: number[], newNode: RuleNode | null): RuleNode {
    if (path.length === 0) return newNode as RuleNode;

    const clonedRoot = JSON.parse(JSON.stringify(root));
    let curr = clonedRoot;
    for (let i = 0; i < path.length - 1; i++) {
        curr = curr.children[path[i]];
    }
    const lastIdx = path[path.length - 1];
    if (newNode === null) {
        curr.children.splice(lastIdx, 1);
    } else {
        curr.children[lastIdx] = newNode;
    }
    return clonedRoot;
}

/** パスで指定されたノードを取得する */
export function getNodeByPath(root: RuleNode, path: number[]): RuleNode | null {
    let curr = root;
    for (const idx of path) {
        if (curr.type === "group" && curr.children && curr.children[idx]) {
            curr = curr.children[idx];
        } else {
            return null;
        }
    }
    return curr;
}
