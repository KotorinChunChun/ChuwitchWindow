import { useMemo, useState } from "react";
import { MonitorInfo, AppConfig } from "../types";

const GROUP_COLORS: Record<number, { fill: string, stroke: string }> = {
    1: { fill: "fill-red-900/40", stroke: "stroke-red-500" },
    2: { fill: "fill-orange-900/40", stroke: "stroke-orange-500" },
    3: { fill: "fill-green-900/40", stroke: "stroke-green-500" },
    4: { fill: "fill-purple-900/40", stroke: "stroke-purple-500" },
    5: { fill: "fill-pink-900/40", stroke: "stroke-pink-500" },
};

export function MonitorMap({ monitors, config, onChange }: { monitors: MonitorInfo[], config: AppConfig, onChange: (c: AppConfig) => void }) {
    const [selectedMonitor, setSelectedMonitor] = useState<string | null>(null);

    // Compute sorted monitors to determine rank
    const orderedMonitors = useMemo(() => {
        return [...monitors].sort((a, b) => {
            const posA = config.monitor_order.indexOf(a.name);
            const posB = config.monitor_order.indexOf(b.name);
            if (posA !== -1 && posB !== -1) return posA - posB;
            if (posA !== -1) return -1;
            if (posB !== -1) return 1;
            return a.monitor_area.left - b.monitor_area.left;
        });
    }, [monitors, config.monitor_order]);

    // Calculate bounding box
    const bbox = useMemo(() => {
        if (monitors.length === 0) return { minX: 0, minY: 0, maxX: 1920, maxY: 1080 };

        let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
        monitors.forEach(m => {
            if (m.monitor_area.left < minX) minX = m.monitor_area.left;
            if (m.monitor_area.top < minY) minY = m.monitor_area.top;
            if (m.monitor_area.right > maxX) maxX = m.monitor_area.right;
            if (m.monitor_area.bottom > maxY) maxY = m.monitor_area.bottom;
        });

        // Add padding (virtual)
        const pX = (maxX - minX) * 0.1;
        const pY = (maxY - minY) * 0.1;

        return {
            minX: minX - pX,
            minY: minY - pY,
            maxX: maxX + pX,
            maxY: maxY + pY
        };
    }, [monitors]);

    const avgH = useMemo(() => {
        if (monitors.length === 0) return 1080;
        return monitors.reduce((sum, m) => sum + (m.monitor_area.bottom - m.monitor_area.top), 0) / monitors.length;
    }, [monitors]);

    const fontTitle = avgH * 0.08;
    const fontSub = avgH * 0.06;
    const fontRes = avgH * 0.05;
    const fontSmall = avgH * 0.04;
    const badgeR = avgH * 0.08;
    const badgeText = avgH * 0.1;
    const strokeW = avgH * 0.005;

    const totalW = bbox.maxX - bbox.minX;
    const totalH = bbox.maxY - bbox.minY;

    return (
        <div className="relative flex-1 w-full h-full flex items-center justify-center p-4 min-h-0 overflow-hidden">
            <svg
                viewBox={`${bbox.minX} ${bbox.minY} ${totalW} ${totalH}`}
                className="w-full h-full max-h-full max-w-full"
                preserveAspectRatio="xMidYMid meet"
            >
                {monitors.map((m, i) => {
                    const w = m.monitor_area.right - m.monitor_area.left;
                    const h = m.monitor_area.bottom - m.monitor_area.top;
                    const isPrimary = config.primary_monitor_id ? m.name === config.primary_monitor_id : m.is_primary;
                    const groupId = config.monitor_groups[m.name] || 0;
                    const isSelected = selectedMonitor === m.name;
                    const rank = orderedMonitors.findIndex(om => om.name === m.name) + 1;

                    const handleMonitorClick = () => {
                        setSelectedMonitor(m.name);
                    };

                    let fillClass = isPrimary ? "fill-blue-900/30" : "fill-slate-800/40";
                    let strokeClass = isPrimary ? "stroke-blue-500" : "stroke-slate-600";
                    if (isSelected) {
                        strokeClass = "stroke-white";
                    }
                    let groupBadge = null;

                    if (groupId > 0 && GROUP_COLORS[groupId]) {
                        fillClass = GROUP_COLORS[groupId].fill;
                        strokeClass = GROUP_COLORS[groupId].stroke;
                        // Add a small styled badge for the group number
                        groupBadge = (
                            <g transform={`translate(${m.monitor_area.left + w - badgeR * 1.5}, ${m.monitor_area.top + badgeR * 1.5})`}>
                                <circle r={badgeR} className={fillClass.replace("fill-", "fill-").replace("/40", "") + " " + strokeClass} strokeWidth={strokeW} />
                                <text textAnchor="middle" alignmentBaseline="middle" className="fill-white font-bold" fontSize={badgeText}>G{groupId}</text>
                            </g>
                        );
                    }

                    return (
                        <g key={i} onClick={handleMonitorClick} className="cursor-pointer hover:opacity-90 transition-opacity">
                            {/* Screen background */}
                            <rect
                                x={m.monitor_area.left}
                                y={m.monitor_area.top}
                                width={w}
                                height={h}
                                rx={w * 0.02}
                                className={`transition-colors duration-300 ${fillClass} ${strokeClass}`}
                                strokeWidth={w * 0.01}
                            />

                            {/* Screen inner glare */}
                            <rect
                                x={m.monitor_area.left + w * 0.02}
                                y={m.monitor_area.top + w * 0.02}
                                width={w - w * 0.04}
                                height={h * 0.4}
                                fill="url(#glare)"
                                className="opacity-30 pointer-events-none"
                                rx={w * 0.01}
                            />

                            {/* Text centered */}
                            <text
                                x={m.monitor_area.left + w / 2}
                                y={m.monitor_area.top + h / 2 - fontTitle * 1.5}
                                textAnchor="middle"
                                alignmentBaseline="middle"
                                className={`font-semibold tracking-wider ${isPrimary ? "fill-blue-400" : "fill-slate-400"} pointer-events-none`}
                                fontSize={fontTitle}
                            >
                                {isPrimary ? `Display ${m.display_number} (Primary)` : `Display ${m.display_number}`}
                            </text>
                            <text
                                x={m.monitor_area.left + w / 2}
                                y={m.monitor_area.top + h / 2}
                                textAnchor="middle"
                                alignmentBaseline="middle"
                                className="fill-slate-300 font-medium pointer-events-none"
                                fontSize={fontSub}
                            >
                                {m.manufacturer}
                            </text>
                            <text
                                x={m.monitor_area.left + w / 2}
                                y={m.monitor_area.top + h / 2 + fontSmall * 2.5}
                                textAnchor="middle"
                                alignmentBaseline="middle"
                                className="fill-slate-400 font-mono pointer-events-none"
                                fontSize={fontSmall}
                            >
                                {m.serial_number}
                            </text>
                            <text
                                x={m.monitor_area.left + w / 2}
                                y={m.monitor_area.top + h / 2 + fontRes * 4}
                                textAnchor="middle"
                                alignmentBaseline="middle"
                                className="fill-slate-500 font-mono pointer-events-none"
                                fontSize={fontRes}
                            >
                                {w}x{h}
                            </text>

                            {/* Rank Badge */}
                            <g transform={`translate(${m.monitor_area.left + badgeR * 1.5}, ${m.monitor_area.top + badgeR * 1.5})`}>
                                <circle r={badgeR} className="fill-slate-700 stroke-slate-500" strokeWidth={strokeW} />
                                <text textAnchor="middle" alignmentBaseline="central" className="fill-slate-200 font-bold" fontSize={badgeText}>
                                    {rank}
                                </text>
                            </g>

                            {groupBadge}
                        </g>
                    );
                })}

                <defs>
                    <linearGradient id="glare" x1="0" x2="0" y1="0" y2="1">
                        <stop offset="0%" stopColor="white" stopOpacity="0.1" />
                        <stop offset="100%" stopColor="white" stopOpacity="0" />
                    </linearGradient>
                </defs>
            </svg>

            {/* Editing toolbar for selected monitor */}
            {selectedMonitor && (
                <div className="absolute bottom-4 left-1/2 -translate-x-1/2 bg-slate-800 border border-slate-700 p-4 rounded-xl shadow-xl flex flex-col items-center space-y-3 z-10 w-max">
                    <div className="text-white font-medium flex items-center space-x-2">
                        <span>選択中: {monitors.find(m => m.name === selectedMonitor)?.display_number || selectedMonitor}</span>
                        <div className="flex space-x-1 bg-slate-900 rounded p-1">
                            <button
                                onClick={() => {
                                    const currentOrder = orderedMonitors.map(om => om.name);
                                    const idx = currentOrder.indexOf(selectedMonitor);
                                    if (idx > 0) {
                                        const newOrder = [...currentOrder];
                                        [newOrder[idx - 1], newOrder[idx]] = [newOrder[idx], newOrder[idx - 1]];
                                        onChange({ ...config, monitor_order: newOrder });
                                    }
                                }}
                                disabled={orderedMonitors.findIndex(om => om.name === selectedMonitor) <= 0}
                                className="px-2 py-1 bg-slate-700 hover:bg-slate-600 disabled:opacity-50 disabled:cursor-not-allowed rounded text-xs text-white"
                            >
                                ▲ 順位を上げる
                            </button>
                            <button
                                onClick={() => {
                                    const currentOrder = orderedMonitors.map(om => om.name);
                                    const idx = currentOrder.indexOf(selectedMonitor);
                                    if (idx < currentOrder.length - 1) {
                                        const newOrder = [...currentOrder];
                                        [newOrder[idx + 1], newOrder[idx]] = [newOrder[idx], newOrder[idx + 1]];
                                        onChange({ ...config, monitor_order: newOrder });
                                    }
                                }}
                                disabled={orderedMonitors.findIndex(om => om.name === selectedMonitor) >= orderedMonitors.length - 1}
                                className="px-2 py-1 bg-slate-700 hover:bg-slate-600 disabled:opacity-50 disabled:cursor-not-allowed rounded text-xs text-white"
                            >
                                ▼ 順位を下げる
                            </button>
                        </div>
                    </div>
                    <div className="flex space-x-4">
                        <button
                            onClick={() => {
                                onChange({ ...config, primary_monitor_id: selectedMonitor });
                            }}
                            className="bg-blue-600 hover:bg-blue-500 text-white px-4 py-2 rounded-lg text-sm font-medium transition-colors"
                        >
                            プライマリにする
                        </button>
                        <div className="flex bg-slate-900 rounded-lg overflow-hidden border border-slate-700">
                            <button
                                onClick={() => {
                                    const newGroups = { ...config.monitor_groups };
                                    delete newGroups[selectedMonitor];
                                    onChange({ ...config, monitor_groups: newGroups });
                                }}
                                className={`px-3 py-2 text-sm font-medium transition-colors ${!config.monitor_groups[selectedMonitor] ? "bg-slate-600 text-white" : "text-slate-400 hover:bg-slate-800"}`}
                            >
                                グループ無
                            </button>
                            {[1, 2, 3, 4, 5].map(g => (
                                <button
                                    key={g}
                                    onClick={() => {
                                        const newGroups = { ...config.monitor_groups };
                                        newGroups[selectedMonitor] = g;
                                        onChange({ ...config, monitor_groups: newGroups });
                                    }}
                                    className={`px-3 py-2 text-sm font-medium transition-colors ${config.monitor_groups[selectedMonitor] === g ? "bg-slate-600 text-white" : "text-slate-400 hover:bg-slate-800"}`}
                                >
                                    G{g}
                                </button>
                            ))}
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
