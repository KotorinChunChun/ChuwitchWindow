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

it("中ドラッグを有効にすると追従とスナップを個別に設定できる", async () => {
    function Fixture() {
        const [config, setConfig] = useState(createConfig());
        return <SettingsPanel config={config} isAdmin onChange={setConfig}
            onResetConfig={() => {}} onShowDialog={async () => false} />;
    }

    render(<Fixture />);
    const middle = screen.getByRole("switch", { name: "中ボタンドラッグでウィンドウを移動" });
    const follow = screen.getByRole("switch", { name: "隣接ウィンドウを吸着して一緒に移動" });
    const snap = screen.getByRole("switch", { name: "ドラッグ時に作業領域の端へ吸着" });
    const resize = screen.getByRole("switch", { name: "解像度の異なるモニターでサイズを調整" });
    expect(follow).toBeDisabled();
    expect(screen.queryByRole("switch", { name: /Alt＋左ドラッグ/ })).not.toBeInTheDocument();
    fireEvent.click(middle);
    fireEvent.click(follow);
    expect(follow).toBeChecked();
    fireEvent.click(middle);
    expect(follow).toBeDisabled();
    fireEvent.click(middle);
    await waitFor(() => expect(follow).toBeEnabled());
    expect(follow).toBeChecked();
    expect(snap).toBeChecked();
    expect(resize).toBeChecked();
    fireEvent.click(snap);
    expect(snap).not.toBeChecked();
    expect(resize).toBeChecked();
});
