import { describe, expect, it } from "vitest";
import { matchNode, wildcardMatch } from "./ruleUtils";
import { createWindow } from "../test/fixtures";

describe("wildcardMatch", () => {
  it("matches mstsc*.exe case-insensitively", () => {
    expect(wildcardMatch("MSTSC.EXE", "mstsc*.exe")).toBe(true);
  });
  it("rejects extra chars for ?", () => {
    expect(wildcardMatch("app12.exe", "app?.exe")).toBe(false);
  });
  it("matches a single char for ?", () => {
    expect(wildcardMatch("app1.exe", "app?.exe")).toBe(true);
  });
  it("matches empty prefix for *", () => {
    expect(wildcardMatch(".exe", "*.exe")).toBe(true);
  });
});

describe("matchNode", () => {
  it("matches AND group when all children match", () => {
    const win = createWindow({ process_name: "notepad.exe", class_name: "Notepad" });
    expect(
      matchNode(win, {
        type: "group",
        match_type: "AND",
        children: [
          { type: "rule", conditions: [{ field: "process_name", operator: "equals", value: "notepad.exe" }] },
          { type: "rule", conditions: [{ field: "class_name", operator: "contains", value: "Note" }] },
        ],
      }),
    ).toBe(true);
  });

  it("rejects unknown fields instead of treating them as true", () => {
    const win = createWindow();
    expect(
      matchNode(win, {
        type: "rule",
        conditions: [{ field: "unknown", operator: "equals", value: "x" }],
      }),
    ).toBe(false);
  });

  it("matches fullscreen except mstsc via not_wildcard", () => {
    const rdp = createWindow({ process_name: "mstsc.exe", is_fullscreen: true });
    const game = createWindow({ process_name: "game.exe", is_fullscreen: true });
    const rule = {
      type: "rule" as const,
      conditions: [
        { field: "is_fullscreen", operator: "equals", value: "true" },
        { field: "process_name", operator: "not_wildcard", value: "mstsc*.exe" },
      ],
    };
    expect(matchNode(rdp, rule)).toBe(false);
    expect(matchNode(game, rule)).toBe(true);
  });
});
