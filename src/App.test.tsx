import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import App from "./App";
import { createConfig } from "./test/fixtures";
import type { MonitorInfo } from "./types";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
vi.mock("@/components/MonitorMap", () => ({
  MonitorMap: ({ monitors }: { monitors: MonitorInfo[] }) => (
    <output>{monitors.map((monitor) => monitor.name).join(", ")}</output>
  ),
}));

afterEach(cleanup);

it("ディスプレイ変更の通知でモニター一覧を再取得し、画面を閉じると購読を解除する", async () => {
  let monitors = [{ name: "検証モニター1" }];
  let notify = async () => {};
  const unlisten = vi.fn();
  vi.mocked(listen).mockImplementation(async (event, handler) => {
    expect(event).toBe("display-changed");
    notify = async () => { await handler({ event, id: 1, payload: undefined as never }); };
    return unlisten;
  });
  vi.mocked(invoke).mockImplementation(async (command) => {
    switch (command) {
      case "get_config": return createConfig();
      case "get_all_monitors_cmd": return monitors;
      case "is_user_an_admin": return false;
    }
  });

  const view = render(<App />);
  await screen.findByText("検証モニター1");
  monitors = [{ name: "検証モニター2" }, { name: "検証モニター3" }];
  await act(notify);
  await screen.findByText("検証モニター2, 検証モニター3");
  expect(screen.queryByText("検証モニター1")).not.toBeInTheDocument();
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === "get_all_monitors_cmd")).toHaveLength(2);
  view.unmount();
  await waitFor(() => expect(unlisten).toHaveBeenCalledOnce());
});
