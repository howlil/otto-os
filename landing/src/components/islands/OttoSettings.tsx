import { useState } from "react";
import {
  CheckCircle2,
  CircleDot,
  ClipboardList,
  Cpu,
  Gauge,
  Inbox,
  Info,
  Keyboard,
  LayoutGrid,
  ListTodo,
  Lock,
  Music,
  Palette,
  PanelBottom,
  PanelTop,
  Pill,
  Puzzle,
  RefreshCw,
  Search,
  Settings2,
  ShieldCheck,
  SlidersHorizontal,
  Timer,
  Wind,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { T } from "../lib/T";
import { Switch } from "./Switch";
import { Badge, COLORS, Row, Section, Sidebar, SidebarGroup, SidebarItem, WindowFrame, type Theme } from "./OttoWindow";

const TINT = {
  blue: "#0a84ff",
  purple: "#af52de",
  cyan: "#32ade6",
  graphite: "#3a3a3c",
  indigo: "#5856d6",
  violet: "#5e5ce6",
  amber: "#ff9f0a",
  gray: "#8e8e93",
  slate: "#636366",
  teal: "#30b0c7",
  rose: "#ff375f",
};

type Group = "Surfaces" | "Notch tools" | "System";
type InteractivePage = "Modules" | "General" | "Shortcuts" | "About";

type ModuleRowDef = {
  label: string;
  subtitle: string;
  group: Group;
  icon: LucideIcon;
  tint: string;
  required?: boolean;
};

const MODULES: ModuleRowDef[] = [
  { label: "Top Bar", subtitle: "Menus, tray, status, calendar, clock", group: "Surfaces", icon: PanelTop, tint: TINT.cyan },
  { label: "Notch", subtitle: "Focus, Time, music, HUD, notifications", group: "Surfaces", icon: Pill, tint: TINT.graphite },
  { label: "Dock", subtitle: "Apps, Downloads stack, Recycle Bin", group: "Surfaces", icon: PanelBottom, tint: TINT.violet },
  { label: "Window Controls", subtitle: "Top-right circles & Tiling menu", group: "Surfaces", icon: CircleDot, tint: TINT.amber },
  { label: "Genie", subtitle: "Hardware mesh minimize into the Dock", group: "Surfaces", icon: Wind, tint: TINT.amber },
  { label: "Spotlight", subtitle: "Local app search (always ready)", group: "Surfaces", icon: Search, tint: TINT.gray, required: true },
  { label: "Control Center", subtitle: "Sound, DDC/CI display, now playing", group: "Surfaces", icon: SlidersHorizontal, tint: TINT.slate, required: true },
  { label: "Now Playing", subtitle: "Playback & synced lyrics in the Notch", group: "Notch tools", icon: Music, tint: TINT.graphite },
  { label: "Focus", subtitle: "Pomodoro, Shield & Screen Time", group: "Notch tools", icon: Timer, tint: TINT.indigo },
  { label: "Clipboard", subtitle: "12 ephemeral RAM-only copies", group: "Notch tools", icon: ClipboardList, tint: TINT.graphite },
  { label: "Pocket", subtitle: "12-item file & link drop shelf", group: "Notch tools", icon: Inbox, tint: TINT.graphite },
  { label: "Quick Capture", subtitle: "Inline Tasks & Ctrl+Alt+Q capture", group: "Notch tools", icon: ListTodo, tint: TINT.graphite },
  { label: "Background Cleanup", subtitle: "EcoQoS throttling & process audit", group: "System", icon: Gauge, tint: TINT.gray },
];

const OFF_BY_DEFAULT = new Set(["Quick Capture", "Background Cleanup"]);

const SECTIONS: { group: Group; title: string; footer?: string }[] = [
  { group: "Surfaces", title: "Surfaces", footer: "A module you turn off stops right away. Its settings are kept." },
  { group: "Notch tools", title: "Notch tools", footer: "Notch tools pause while the Notch is off. Their switches stay as set." },
  { group: "System", title: "System" },
];

const PAGES: { title?: string; items: { label: string; icon: LucideIcon; tint: string }[] }[] = [
  {
    items: [
      { label: "Overview", icon: LayoutGrid, tint: TINT.blue },
      { label: "Modules", icon: Puzzle, tint: TINT.purple },
    ],
  },
  {
    title: "Surfaces",
    items: [
      { label: "Top Bar", icon: PanelTop, tint: TINT.cyan },
      { label: "Notch", icon: Pill, tint: TINT.graphite },
      { label: "Focus", icon: Timer, tint: TINT.indigo },
      { label: "Dock", icon: PanelBottom, tint: TINT.violet },
      { label: "Window Controls", icon: CircleDot, tint: TINT.amber },
      { label: "Spotlight", icon: Search, tint: TINT.gray },
      { label: "Control Center", icon: SlidersHorizontal, tint: TINT.slate },
    ],
  },
  {
    title: "Otto",
    items: [
      { label: "Appearance", icon: Palette, tint: TINT.teal },
      { label: "Shortcuts", icon: Keyboard, tint: TINT.rose },
      { label: "General", icon: Settings2, tint: TINT.gray },
      { label: "About", icon: Info, tint: TINT.blue },
    ],
  },
];

type ProcessItem = {
  id: string;
  name: string;
  count: number;
  ramMb: number;
  cpu: string;
  safety: "background" | "protected";
  reason: string;
  throttled: boolean;
  safeToEnd: boolean;
};

const INITIAL_PROCESSES: ProcessItem[] = [
  { id: "chrome-bg", name: "Chrome Helper", count: 6, ramMb: 412, cpu: "8.4%", safety: "background", reason: "Hidden memory buffer", throttled: true, safeToEnd: true },
  { id: "updater", name: "Electron Renderer", count: 3, ramMb: 268, cpu: "6.1%", safety: "background", reason: "Background CPU hog", throttled: false, safeToEnd: true },
  { id: "discord-bg", name: "Discord", count: 4, ramMb: 194, cpu: "2.3%", safety: "background", reason: "Headless background", throttled: false, safeToEnd: true },
  { id: "dwm", name: "dwm.exe", count: 1, ramMb: 112, cpu: "1.4%", safety: "protected", reason: "Desktop Window Manager", throttled: false, safeToEnd: false },
  { id: "explorer", name: "explorer.exe", count: 1, ramMb: 96, cpu: "0.6%", safety: "protected", reason: "Windows Shell", throttled: false, safeToEnd: false },
];

const SHORTCUTS: [string, string][] = [
  ["Open Otto Settings", "Ctrl+Alt+,"],
  ["Open Control Center", "Ctrl+Alt+C"],
  ["Open Spotlight", "Ctrl+Alt+Space"],
  ["Toggle Top Bar", "Ctrl+Alt+B"],
  ["Toggle Dock", "Ctrl+Alt+D"],
  ["Quick Capture", "Ctrl+Alt+Q"],
  ["Expand Notch", "Alt+N"],
  ["Start or Pause Focus", "Alt+P"],
];

const INTERACTIVE_SET = new Set<string>(["Modules", "General", "Shortcuts", "About"]);

export function SettingsSidebar({
  theme,
  selected,
  onSelect,
}: {
  theme: Theme;
  selected: string;
  onSelect?: (page: InteractivePage) => void;
}) {
  return (
    <Sidebar theme={theme}>
      {PAGES.map((group) => (
        <SidebarGroup key={group.title ?? "home"} theme={theme} title={group.title}>
          {group.items.map((item) => {
            const clickable = Boolean(onSelect && INTERACTIVE_SET.has(item.label));
            return (
              <SidebarItem
                key={item.label}
                theme={theme}
                icon={item.icon}
                tint={item.tint}
                label={item.label}
                selected={item.label === selected}
                interactive={clickable}
                onClick={clickable ? () => onSelect?.(item.label as InteractivePage) : undefined}
              />
            );
          })}
        </SidebarGroup>
      ))}
    </Sidebar>
  );
}

type AuditFilter = "background" | "throttled" | "protected" | "all";
type UpdatePhase = "idle" | "checking" | "verifying" | "uptodate";

export function OttoSettingsWindow() {
  const [page, setPage] = useState<InteractivePage>("Modules");
  const [enabled, setEnabled] = useState<Record<string, boolean>>(() =>
    Object.fromEntries(MODULES.map((m) => [m.label, !OFF_BY_DEFAULT.has(m.label)])),
  );
  const [cleanupOn, setCleanupOn] = useState(true);
  const [auditFilter, setAuditFilter] = useState<AuditFilter>("background");
  const [processes, setProcesses] = useState<ProcessItem[]>(INITIAL_PROCESSES);
  const [updatePhase, setUpdatePhase] = useState<UpdatePhase>("idle");

  const notchOn = Boolean(enabled.Notch);
  const runningCount = MODULES.filter((m) => {
    if (!enabled[m.label]) return false;
    if (m.group === "Notch tools" && !notchOn) return false;
    return true;
  }).length;

  const bgRam = processes.filter((p) => p.safety === "background").reduce((s, p) => s + p.ramMb, 0);
  const throttledCount = processes.filter((p) => p.throttled).length;
  const protectedCount = processes.filter((p) => p.safety === "protected").length;

  const filteredProcesses = processes.filter((p) => {
    if (auditFilter === "background") return p.safety === "background";
    if (auditFilter === "throttled") return p.throttled;
    if (auditFilter === "protected") return p.safety === "protected";
    return true;
  });

  function runUpdateCheck() {
    if (updatePhase === "checking" || updatePhase === "verifying") return;
    setUpdatePhase("checking");
    window.setTimeout(() => setUpdatePhase("verifying"), 650);
    window.setTimeout(() => setUpdatePhase("uptodate"), 1450);
  }

  const c = COLORS.light;

  return (
    <div className="flex flex-col gap-6">
      <div className="flex flex-wrap items-center justify-between gap-4">
        <div className="inline-flex flex-wrap items-center gap-1.5 rounded-full bg-black/[0.05] p-1 ring-1 ring-inset ring-line" role="tablist" aria-label="Otto Settings pages">
          {(
            [
              ["Modules", "13 Modul", "13 Modules"],
              ["General", "Audit Proses & RAM", "Process & RAM Audit"],
              ["Shortcuts", "Pintasan", "Shortcuts"],
              ["About", "Pembaruan SHA-256", "SHA-256 Updates"],
            ] as const
          ).map(([id, idLabel, enLabel]) => (
            <button
              key={id}
              type="button"
              role="tab"
              aria-selected={page === id}
              onClick={() => setPage(id)}
              className={`h-8 rounded-full px-3.5 text-[13px] font-medium transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent ${
                page === id ? "bg-paper text-ink" : "text-muted hover:text-ink"
              }`}
            >
              <T id={idLabel} en={enLabel} />
            </button>
          ))}
        </div>

        <span className="text-[13px] font-medium text-muted tabular-nums">
          {runningCount} / {MODULES.length} <T id="modul aktif" en="modules active" />
        </span>
      </div>

      <WindowFrame
        theme="light"
        title="Otto Settings"
        bodyClassName="h-[520px] sm:h-[552px]"
        sidebar={<SettingsSidebar theme="light" selected={page} onSelect={setPage} />}
      >
        <div className="quiet-scroll flex min-w-0 flex-1 flex-col gap-5 overflow-y-auto p-4 sm:p-5">
          {page === "Modules" ? (
            SECTIONS.map((section) => (
              <Section key={section.group} theme="light" title={section.title} footer={section.footer}>
                {MODULES.filter((module) => module.group === section.group).map((module) => {
                  const paused = module.group === "Notch tools" && !notchOn;
                  return (
                    <Row
                      key={module.label}
                      theme="light"
                      icon={module.icon}
                      tint={module.tint}
                      title={module.label}
                      subtitle={paused ? "Needs Notch" : module.subtitle}
                      muted={paused}
                      trailing={
                        <div className="flex shrink-0 items-center gap-2.5">
                          <Badge theme="light" tone={paused ? "neutral" : enabled[module.label] ? "good" : "neutral"}>
                            {paused ? "Paused" : enabled[module.label] ? "Running" : "Off"}
                          </Badge>
                          {module.required ? (
                            <span className="w-[38px] text-center text-[11px] font-medium" style={{ color: c.secondary }}>
                              Core
                            </span>
                          ) : (
                            <Switch
                              tone="system-light"
                              checked={enabled[module.label] ?? false}
                              onChange={(next) => setEnabled((curr) => ({ ...curr, [module.label]: next }))}
                              label={[module.label, module.label]}
                            />
                          )}
                        </div>
                      }
                    />
                  );
                })}
              </Section>
            ))
          ) : null}

          {page === "General" ? (
            <>
              <Section
                theme="light"
                title="Background Cleanup"
                footer="EcoQoS lowers power & CPU priority without terminating apps. Protected OS services are locked."
              >
                <Row
                  theme="light"
                  icon={Gauge}
                  tint={TINT.gray}
                  title={cleanupOn ? "Background Cleanup: On" : "Background Cleanup: Off"}
                  subtitle="Auto-throttling sustained background CPU hogs with EcoQoS · Never auto-kills"
                  trailing={
                    <Switch
                      tone="system-light"
                      checked={cleanupOn}
                      onChange={setCleanupOn}
                      label={["Background Cleanup", "Background Cleanup"]}
                    />
                  }
                />
                <div className="grid grid-cols-3 divide-x divide-black/[0.08] px-2 py-3 text-center sm:text-left">
                  <div className="px-2.5">
                    <p className="text-[11px]" style={{ color: c.secondary }}>Background RAM</p>
                    <p className="mt-0.5 text-[17px] font-semibold tabular-nums">{bgRam} MB</p>
                  </div>
                  <div className="px-2.5">
                    <p className="text-[11px]" style={{ color: c.secondary }}>EcoQoS Throttled</p>
                    <p className="mt-0.5 text-[17px] font-semibold tabular-nums">{throttledCount}</p>
                  </div>
                  <div className="px-2.5">
                    <p className="text-[11px]" style={{ color: c.secondary }}>System Protected</p>
                    <p className="mt-0.5 text-[17px] font-semibold tabular-nums">{protectedCount}</p>
                  </div>
                </div>
              </Section>

              <Section
                theme="light"
                title="Process & Memory Buffer Audit"
                footer="3-tier safety classification protects Session 0 services, dwm.exe, explorer.exe and elevated processes."
              >
                <div className="flex flex-wrap items-center justify-between gap-2 px-4 py-2.5">
                  <div className="flex flex-wrap gap-1.5" role="group" aria-label="Process audit filter">
                    {(
                      [
                        ["background", "Background"],
                        ["throttled", "Throttled"],
                        ["protected", "System Protected"],
                        ["all", "All"],
                      ] as const
                    ).map(([id, label]) => (
                      <button
                        key={id}
                        type="button"
                        aria-pressed={auditFilter === id}
                        onClick={() => setAuditFilter(id)}
                        className={`h-7 rounded-full px-2.5 text-[11.5px] font-medium transition-colors ${
                          auditFilter === id ? "bg-[#007aff] text-white" : "bg-black/[0.06] text-ink-soft hover:text-ink"
                        }`}
                      >
                        {label}
                      </button>
                    ))}
                  </div>
                  <button
                    type="button"
                    onClick={() => setProcesses(INITIAL_PROCESSES)}
                    className="inline-flex items-center gap-1 rounded-full px-2.5 py-1 text-[11.5px] font-medium text-[#007aff] hover:bg-black/[0.04]"
                  >
                    <RefreshCw size={11} aria-hidden="true" />
                    Rescan
                  </button>
                </div>

                {filteredProcesses.map((proc) => (
                  <Row
                    key={proc.id}
                    theme="light"
                    icon={proc.safety === "protected" ? Lock : Cpu}
                    tint={proc.safety === "protected" ? TINT.amber : TINT.slate}
                    title={proc.count > 1 ? `${proc.name} (${proc.count})` : proc.name}
                    subtitle={`${proc.ramMb} MB · ${proc.cpu} · ${proc.reason}`}
                    trailing={
                      proc.safety === "protected" ? (
                        <Badge theme="light" tone="warning">Protected</Badge>
                      ) : (
                        <div className="flex shrink-0 items-center gap-1.5">
                          <button
                            type="button"
                            onClick={() =>
                              setProcesses((curr) =>
                                curr.map((item) => (item.id === proc.id ? { ...item, throttled: !item.throttled } : item)),
                              )
                            }
                            className={`h-7 rounded-full px-2.5 text-[11.5px] font-semibold transition-colors ${
                              proc.throttled
                                ? "bg-[#34c759]/18 text-[#15803d]"
                                : "bg-black/[0.07] text-ink hover:bg-black/12"
                            }`}
                          >
                            {proc.throttled ? "Restore" : "EcoQoS"}
                          </button>
                          {proc.safeToEnd ? (
                            <button
                              type="button"
                              onClick={() => setProcesses((curr) => curr.filter((item) => item.id !== proc.id))}
                              className="h-7 rounded-full bg-[#ff3b30]/12 px-2.5 text-[11.5px] font-semibold text-[#c42b1c] transition-colors hover:bg-[#ff3b30]/20"
                            >
                              End Task
                            </button>
                          ) : null}
                        </div>
                      )
                    }
                  />
                ))}
              </Section>
            </>
          ) : null}

          {page === "Shortcuts" ? (
            <Section
              theme="light"
              title="Global Keys"
              footer="Ctrl+Alt+, Ctrl+Alt+B and Ctrl+Alt+D stay active even if Global Hotkeys are turned off, so you can always recover."
            >
              {SHORTCUTS.map(([label, keys]) => (
                <Row
                  key={label}
                  theme="light"
                  title={label}
                  trailing={
                    <kbd className="rounded-md border border-black/15 bg-[#f2f2f7] px-2.5 py-1 font-sans text-[12px] font-medium text-ink">
                      {keys}
                    </kbd>
                  }
                />
              ))}
            </Section>
          ) : null}

          {page === "About" ? (
            <>
              <div className="flex items-center gap-4 px-1 py-2">
                <img src="/favicon.svg" alt="" width={52} height={52} className="shrink-0" />
                <div>
                  <p className="text-[20px] font-semibold tracking-[-0.02em]">Otto OS</p>
                  <p className="text-[12.5px]" style={{ color: c.secondary }}>
                    Version 1.3.1 · Windows 11 22H2+ (x64)
                  </p>
                </div>
              </div>

              <Section
                theme="light"
                title="Software Update"
                footer="Updates are fetched only when you click Check and verified against SHA256SUMS.txt before silent /RELAUNCH."
              >
                <Row
                  theme="light"
                  icon={updatePhase === "uptodate" ? CheckCircle2 : ShieldCheck}
                  tint={updatePhase === "uptodate" ? "#34c759" : TINT.blue}
                  title={
                    updatePhase === "checking"
                      ? "Checking GitHub Releases..."
                      : updatePhase === "verifying"
                        ? "Verifying SHA-256 Integrity..."
                        : updatePhase === "uptodate"
                          ? "Otto OS is Up to Date"
                          : "Check for Updates"
                  }
                  subtitle={
                    updatePhase === "uptodate"
                      ? "Version 1.3.1 matches latest verified checksum"
                      : "On-demand check · Zero background polling"
                  }
                  trailing={
                    <button
                      type="button"
                      onClick={runUpdateCheck}
                      className="h-8 rounded-full bg-[#007aff] px-3.5 text-[12px] font-semibold text-white transition-opacity hover:opacity-90"
                    >
                      {updatePhase === "uptodate" ? "Check Again" : updatePhase === "idle" ? "Check" : "Working..."}
                    </button>
                  }
                />
              </Section>

              <Section
                theme="light"
                title="Recovery & Diagnostics"
                footer="Run otto-os.exe --restore-shell if you ever need to restore the native Windows taskbar immediately."
              >
                <Row theme="light" title="Settings folder" trailing={<span className="font-mono text-[11.5px]" style={{ color: c.secondary }}>%LOCALAPPDATA%\otto-os</span>} />
                <Row theme="light" title="Telemetry" trailing={<Badge theme="light" tone="good">None</Badge>} />
              </Section>
            </>
          ) : null}
        </div>
      </WindowFrame>
    </div>
  );
}
