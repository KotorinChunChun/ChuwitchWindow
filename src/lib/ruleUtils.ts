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

/**
 * `*` は0文字以上、`?` は任意の1文字に一致する。
 * Windows上の名前を扱いやすくするため、大文字・小文字は区別しない。
 */
export function wildcardMatch(value: string, pattern: string): boolean {
    const valueChars = Array.from(value.toLocaleLowerCase());
    const patternChars = Array.from(pattern.toLocaleLowerCase());
    let previous = new Array<boolean>(valueChars.length + 1).fill(false);
    previous[0] = true;

    for (const token of patternChars) {
        const current = new Array<boolean>(valueChars.length + 1).fill(false);
        if (token === "*") current[0] = previous[0];

        for (let index = 1; index <= valueChars.length; index++) {
            if (token === "*") {
                current[index] = previous[index] || current[index - 1];
            } else if (token === "?") {
                current[index] = previous[index - 1];
            } else {
                current[index] = previous[index - 1] && valueChars[index - 1] === token;
            }
        }
        previous = current;
    }

    return previous[valueChars.length];
}

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
            let fieldVal: string | null = null;
            if (cond.field === "process_name") fieldVal = win.process_name;
            else if (cond.field === "class_name") fieldVal = win.class_name;
            else if (cond.field === "title") fieldVal = win.title;
            else if (cond.field === "style") fieldVal = win.style.toString(16).toUpperCase().padStart(8, '0');
            else if (cond.field === "ex_style") fieldVal = win.ex_style.toString(16).toUpperCase().padStart(8, '0');
            else if (cond.field === "is_fullscreen") fieldVal = String(win.is_fullscreen);
            else if (cond.field === "is_minimized") fieldVal = String(win.is_minimized);
            else if (cond.field === "is_maximized") fieldVal = String(win.is_maximized);

            // 未知のフィールドが否定演算子で真にならないようにする。
            if (fieldVal === null) return false;

            let matched = false;
            if (cond.operator === "equals") matched = fieldVal === cond.value;
            else if (cond.operator === "not_equals") matched = fieldVal !== cond.value;
            else if (cond.operator === "contains") matched = fieldVal.includes(cond.value);
            else if (cond.operator === "not_contains") matched = !fieldVal.includes(cond.value);
            else if (cond.operator === "starts_with") matched = fieldVal.startsWith(cond.value);
            else if (cond.operator === "ends_with") matched = fieldVal.endsWith(cond.value);
            else if (cond.operator === "wildcard") matched = wildcardMatch(fieldVal, cond.value);
            else if (cond.operator === "not_wildcard") matched = !wildcardMatch(fieldVal, cond.value);

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
