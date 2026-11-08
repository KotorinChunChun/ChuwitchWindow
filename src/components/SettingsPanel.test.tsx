import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { expect, it, vi } from "vitest";
import { createConfig } from "../test/fixtures";
import { SettingsPanel } from "./SettingsPanel";

vi.mock("@tauri-apps/api/core", () => ({
    invoke: vi.fn(async (command: string) => {
        if (command === "check_autostart_registered_cmd") {
            return { is_registered: false, is_same_path: false, registered_path: null };
        }
        return false;
    }),
}));

it("Altドラッグと中ドラッグは独立し、どちらでも追従の選択を使える", async () => {
    function Fixture() {
        const [config, setConfig] = useState(createConfig());
        return <SettingsPanel config={config} isAdmin onChange={setConfig}
            onResetConfig={() => {}} onShowDialog={async () => false} />;
    }

    render(<Fixture />);
    const move = screen.getByRole("switch", { name: "Alt＋左ドラッグでウィンドウを移動" });
    const middle = screen.getByRole("switch", { name: "中ボタンドラッグでウィンドウを移動" });
    const follow = screen.getByRole("switch", { name: "隣接ウィンドウを吸着して一緒に移動" });
    const snap = screen.getByRole("switch", { name: "ドラッグ時に作業領域の端へ吸着" });
    const resize = screen.getByRole("switch", { name: "解像度の異なるモニターでサイズを調整" });
    expect(follow).toBeDisabled();
    fireEvent.click(move);
    fireEvent.click(follow);
    expect(follow).toBeChecked();
    fireEvent.click(move);
    expect(follow).toBeDisabled();
    fireEvent.click(middle);
    await waitFor(() => expect(follow).toBeEnabled());
    expect(follow).toBeChecked();
    expect(move).not.toBeChecked();
    expect(snap).toBeChecked();
    expect(resize).toBeChecked();
    fireEvent.click(snap);
    expect(snap).not.toBeChecked();
    expect(resize).toBeChecked();
});
