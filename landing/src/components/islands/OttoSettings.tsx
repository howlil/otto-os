import { useState } from "react";
import { CircleDot, ClipboardList, Gauge, Inbox, Info, Keyboard, LayoutGrid, ListTodo, Music, Palette, PanelBottom, PanelTop, Pill, Puzzle, Search, Settings2, SlidersHorizontal, Timer, Wind } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { Switch } from "./Switch";
import { Row, Section, Sidebar, SidebarGroup, SidebarItem, WindowFrame, type Theme } from "./OttoWindow";

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

type ModuleRow = {
  label: string;
  subtitle: string;
  group: Group;
  icon: LucideIcon;
  tint: string;
};

const MODULES: ModuleRow[] = [
  { label: "Top Bar", subtitle: "Menus, tray, status, clock", group: "Surfaces", icon: PanelTop, tint: TINT.cyan },
  { label: "Notch", subtitle: "Focus, music, HUD, notifications", group: "Surfaces", icon: Pill, tint: TINT.graphite },
  { label: "Dock", subtitle: "Apps, Downloads, Trash", group: "Surfaces", icon: PanelBottom, tint: TINT.violet },
  { label: "Window Controls", subtitle: "Close, minimize, zoom circles", group: "Surfaces", icon: CircleDot, tint: TINT.amber },
  { label: "Genie", subtitle: "Minimize into the Dock", group: "Surfaces", icon: Wind, tint: TINT.amber },
  { label: "Spotlight", subtitle: "Local app search", group: "Surfaces", icon: Search, tint: TINT.gray },
  { label: "Control Center", subtitle: "Sound, display, now playing", group: "Surfaces", icon: SlidersHorizontal, tint: TINT.slate },
  { label: "Now Playing", subtitle: "Playback in the Notch", group: "Notch tools", icon: Music, tint: TINT.graphite },
  { label: "Focus", subtitle: "Pomodoro, timer, stopwatch", group: "Notch tools", icon: Timer, tint: TINT.indigo },
  { label: "Clipboard", subtitle: "Recent copies in the Notch", group: "Notch tools", icon: ClipboardList, tint: TINT.graphite },
  { label: "Pocket", subtitle: "File drop shelf in the Notch", group: "Notch tools", icon: Inbox, tint: TINT.graphite },
  { label: "Quick Capture", subtitle: "Tasks and Ctrl+Alt+Q capture", group: "Notch tools", icon: ListTodo, tint: TINT.graphite },
  { label: "Background Cleanup", subtitle: "Throttles CPU hogs, never kills", group: "System", icon: Gauge, tint: TINT.gray },
];

const OFF_BY_DEFAULT = new Set(["Quick Capture", "Background Cleanup"]);

const SECTIONS: { group: Group; title: string; footer?: string }[] = [
  {
    group: "Surfaces",
    title: "Surfaces",
    footer: "A module you turn off stops right away. Its settings are kept.",
  },
  {
    group: "Notch tools",
    title: "Notch tools",
    footer: "Notch tools pause while the Notch is off. Their own switches stay as you set them.",
  },
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

export function SettingsSidebar({ theme, selected }: { theme: Theme; selected: string }) {
  return (
    <Sidebar theme={theme}>
      {PAGES.map((group) => (
        <SidebarGroup key={group.title ?? "home"} theme={theme} title={group.title}>
          {group.items.map((item) => (
            <SidebarItem
              key={item.label}
              theme={theme}
              icon={item.icon}
              tint={item.tint}
              label={item.label}
              selected={item.label === selected}
            />
          ))}
        </SidebarGroup>
      ))}
    </Sidebar>
  );
}

export function OttoSettingsWindow() {
  const [enabled, setEnabled] = useState<Record<string, boolean>>(() =>
    Object.fromEntries(MODULES.map((module) => [module.label, !OFF_BY_DEFAULT.has(module.label)])),
  );

  function setModule(label: string, next: boolean) {
    setEnabled((current) => ({ ...current, [label]: next }));
  }

  return (
    <WindowFrame
      theme="light"
      title="Otto Settings"
      bodyClassName="h-[460px] lg:h-[536px]"
      sidebar={<SettingsSidebar theme="light" selected="Modules" />}
    >
      <div className="flex min-w-0 flex-1 flex-col gap-5 overflow-y-auto p-4 [scrollbar-color:rgb(0_0_0/0.2)_transparent] [scrollbar-width:thin]">
        {SECTIONS.map((section) => (
          <Section key={section.group} theme="light" title={section.title} footer={section.footer}>
            {MODULES.filter((module) => module.group === section.group).map((module) => (
              <Row
                key={module.label}
                theme="light"
                icon={module.icon}
                tint={module.tint}
                title={module.label}
                subtitle={module.subtitle}
                trailing={
                  <Switch
                    tone="system-light"
                    checked={enabled[module.label] ?? false}
                    onChange={(next) => setModule(module.label, next)}
                    label={[module.label, module.label]}
                  />
                }
              />
            ))}
          </Section>
        ))}
      </div>
    </WindowFrame>
  );
}
