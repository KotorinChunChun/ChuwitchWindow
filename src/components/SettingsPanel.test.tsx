import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { createConfig } from "../test/fixtures";
import { SettingsPanel } from "./SettingsPanel";

vi.mock("@tauri-apps/api/core", () => ({
    invoke: vi.fn(),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: vi.fn() }));
afterEach(cleanup);

beforeEach(() => {
    vi.resetAllMocks();
    vi.mocked(invoke).mockImplementation(async (command: string) => {
        if (command === "check_autostart_registered_cmd") {
            return { is_registered: false, is_same_path: false, registered_path: null };
        }
        return false;
    });
});

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

it("設定画面から隔離した自動起動とPATHを登録・解除して表示を更新する", async () => {
    let autostart = false;
    let path = false;
    vi.mocked(invoke).mockImplementation(async (command, args) => {
        switch (command) {
            case "sync_admin_startup": autostart = Boolean((args as { enable: boolean }).enable); break;
            case "check_autostart_registered_cmd": return { is_registered: autostart, is_same_path: autostart, registered_path: null };
            case "register_to_path_cmd": path = true; break;
            case "unregister_from_path_cmd": path = false; break;
            case "check_path_registered_cmd": return path;
        }
    });
    const dialog = vi.fn(async () => true);
    function Fixture() {
        const [config, setConfig] = useState(createConfig());
        return <SettingsPanel config={config} isAdmin onChange={setConfig}
            onResetConfig={() => {}} onShowDialog={dialog} />;
    }
    render(<Fixture />);
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("check_autostart_registered_cmd"));
    const startup = screen.getByRole("switch", { name: /PC起動時に自動実行/ });
    fireEvent.click(startup);
    await waitFor(() => expect(startup).toBeChecked());
    expect(autostart).toBe(true);
    fireEvent.click(startup);
    await waitFor(() => expect(startup).not.toBeChecked());
    expect(autostart).toBe(false);

    fireEvent.click(screen.getByRole("button", { name: "登録を追加" }));
    const remove = await screen.findByRole("button", { name: "削除" });
    expect(path).toBe(true);
    fireEvent.click(remove);
    await screen.findByRole("button", { name: "登録を追加" });
    expect(path).toBe(false);
});

it("設定画面から昇格・入出力・初期化を適切なコマンドへ渡す", async () => {
    const onChange = vi.fn();
    const onReset = vi.fn();
    const dialog = vi.fn(async () => true);
    const saved = createConfig({ middle_drag_enabled: true });
    vi.mocked(save).mockResolvedValue("fixture-export.json");
    vi.mocked(open).mockResolvedValue("fixture-import.json");
    vi.mocked(invoke).mockImplementation(async (command) => {
        if (command === "import_config_cmd") return saved;
        if (command === "check_autostart_registered_cmd") return { is_registered: false, is_same_path: false, registered_path: null };
        return false;
    });
    render(<SettingsPanel config={createConfig()} isAdmin={false} onChange={onChange}
        onResetConfig={onReset} onShowDialog={dialog} />);
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("check_autostart_registered_cmd"));
    expect(screen.getByRole("switch", { name: /PC起動時に自動実行/ })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: /管理者として再起動/ }));
    expect(invoke).toHaveBeenCalledWith("restart_as_admin");
    fireEvent.click(screen.getByRole("button", { name: /設定をファイルに保存/ }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("export_config_cmd", { path: "fixture-export.json" }));
    fireEvent.click(screen.getByRole("button", { name: /ファイルから設定を復元/ }));
    await waitFor(() => expect(onChange).toHaveBeenCalledWith(saved));
    expect(invoke).toHaveBeenCalledWith("import_config_cmd", { path: "fixture-import.json" });
    fireEvent.click(screen.getByRole("button", { name: /設定を完全に消去して初期化/ }));
    expect(onReset).toHaveBeenCalledOnce();
});
