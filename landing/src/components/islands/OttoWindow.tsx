import type { LucideIcon } from "lucide-react";
import { Minus, Square, X } from "lucide-react";
import type { CSSProperties, ReactNode } from "react";

export type Theme = "light" | "dark";

export const COLORS = {
  light: {
    body: "#f2f2f7",
    sidebar: "#e6e6eb",
    card: "#ffffff",
    text: "#1d1d1f",
    secondary: "#6e6e73",
    hairline: "rgb(0 0 0 / 0.10)",
    selected: "#ffffff",
    ring: "rgb(0 0 0 / 0.12)",
    captionHover: "hover:bg-black/5",
    good: "#1f9d4c",
    warning: "#c77400",
    neutral: "#6e6e73",
  },
  dark: {
    body: "#1e1e20",
    sidebar: "#28282b",
    card: "#2c2c2f",
    text: "#f5f5f7",
    secondary: "#a1a1a6",
    hairline: "rgb(255 255 255 / 0.09)",
    selected: "rgb(255 255 255 / 0.12)",
    ring: "rgb(255 255 255 / 0.14)",
    captionHover: "hover:bg-white/10",
    good: "#30d158",
    warning: "#ff9f0a",
    neutral: "#a1a1a6",
  },
} as const;

const CAPTION_BUTTONS: { id: string; icon: LucideIcon; close: boolean }[] = [
  { id: "minimize", icon: Minus, close: false },
  { id: "maximize", icon: Square, close: false },
  { id: "close", icon: X, close: true },
];

export function WindowFrame({
  theme,
  title,
  sidebar,
  bodyClassName = "",
  children,
}: {
  theme: Theme;
  title: string;
  sidebar?: ReactNode;
  bodyClassName?: string;
  children: ReactNode;
}) {
  const c = COLORS[theme];
  return (
    <div
      className="flex w-full min-w-0 flex-col overflow-hidden rounded-[12px] border border-line"
      style={{ background: c.body, color: c.text }}
    >
      <div
        className="flex h-9 shrink-0 items-center justify-between border-b pl-3.5"
        style={{ borderColor: c.hairline }}
      >
        <div className="flex min-w-0 items-center gap-2 text-[12px] font-medium">
          <img src="/favicon.svg" alt="" width={14} height={14} />
          <span className="truncate">{title}</span>
        </div>
        <div className="flex h-full" aria-hidden="true">
          {CAPTION_BUTTONS.map(({ id, icon: Icon, close }) => (
            <span
              key={id}
              className={`grid h-full w-[44px] place-items-center ${close ? "hover:bg-[#c42b1c] hover:text-white" : c.captionHover}`}
            >
              <Icon size={10} strokeWidth={1.3} />
            </span>
          ))}
        </div>
      </div>
      <div className={`flex min-h-0 ${bodyClassName}`}>
        {sidebar}
        {children}
      </div>
    </div>
  );
}

export function Sidebar({ theme, children }: { theme: Theme; children: ReactNode }) {
  return (
    <div
      className="flex w-[56px] shrink-0 flex-col overflow-hidden border-r p-2 sm:w-[216px] sm:p-2.5"
      style={{ background: COLORS[theme].sidebar, borderColor: COLORS[theme].hairline }}
    >
      {children}
    </div>
  );
}

export function SidebarGroup({ theme, title, children }: { theme: Theme; title?: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-0.5">
      {title ? (
        <p
          className="hidden px-2 pt-2.5 pb-1 text-[11px] font-semibold sm:block"
          style={{ color: COLORS[theme].secondary }}
        >
          {title}
        </p>
      ) : null}
      {children}
    </div>
  );
}

export function SidebarItem({
  theme,
  icon: Icon,
  tint,
  label,
  selected = false,
  onClick,
  interactive = false,
}: {
  theme: Theme;
  icon: LucideIcon;
  tint: string;
  label: string;
  selected?: boolean;
  onClick?: () => void;
  interactive?: boolean;
}) {
  const c = COLORS[theme];
  const inner = (
    <>
      <span
        className="grid size-6 shrink-0 place-items-center rounded-[7px] text-white"
        style={{ background: tint }}
        aria-hidden="true"
      >
        <Icon size={13} strokeWidth={2} />
      </span>
      <span className="hidden truncate text-[13px] font-medium sm:block">{label}</span>
    </>
  );

  if (onClick) {
    return (
      <button
        type="button"
        aria-current={selected ? "page" : undefined}
        onClick={onClick}
        className={`flex h-8 w-full items-center gap-2.5 rounded-[8px] px-1.5 text-left transition-colors focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent ${
          selected ? "" : theme === "dark" ? "hover:bg-white/[0.06]" : "hover:bg-black/[0.04]"
        }`}
        style={{ background: selected ? c.selected : "transparent", color: c.text }}
      >
        {inner}
      </button>
    );
  }

  return (
    <div
      className={`flex h-8 items-center gap-2.5 rounded-[8px] px-1.5 ${interactive ? "" : "opacity-80"}`}
      style={{ background: selected ? c.selected : "transparent", color: c.text }}
    >
      {inner}
    </div>
  );
}

export function Section({
  theme,
  title,
  footer,
  children,
}: {
  theme: Theme;
  title?: string;
  footer?: string;
  children: ReactNode;
}) {
  const c = COLORS[theme];
  return (
    <section className="flex flex-col gap-1.5">
      {title ? (
        <h3 className="px-1 text-[12px] font-medium" style={{ color: c.secondary }}>
          {title}
        </h3>
      ) : null}
      <div
        className="overflow-hidden rounded-[10px] [&>*+*]:[border-top:1px_solid_var(--hair)]"
        style={{ background: c.card, "--hair": c.hairline } as CSSProperties}
      >
        {children}
      </div>
      {footer ? (
        <p className="px-1 text-[11px] leading-[1.5]" style={{ color: c.secondary }}>
          {footer}
        </p>
      ) : null}
    </section>
  );
}

export function Row({
  theme,
  icon: Icon,
  tint,
  title,
  subtitle,
  muted = false,
  trailing,
}: {
  theme: Theme;
  icon?: LucideIcon;
  tint?: string;
  title: string;
  subtitle?: string;
  muted?: boolean;
  trailing?: ReactNode;
}) {
  const c = COLORS[theme];
  return (
    <div className={`flex min-h-[48px] items-center gap-3 px-4 py-2.5 ${muted ? "opacity-60" : ""}`}>
      {Icon ? (
        <span
          className="grid size-7 shrink-0 place-items-center rounded-[8px] text-white"
          style={{ background: tint }}
          aria-hidden="true"
        >
          <Icon size={15} strokeWidth={2} />
        </span>
      ) : null}
      <div className="min-w-0 flex-1">
        <p className="truncate text-[13px] font-medium" style={{ color: c.text }}>
          {title}
        </p>
        {subtitle ? (
          <p className="text-[11.5px] leading-[1.4]" style={{ color: c.secondary }}>
            {subtitle}
          </p>
        ) : null}
      </div>
      {trailing}
    </div>
  );
}

export function Badge({
  theme,
  tone,
  children,
}: {
  theme: Theme;
  tone: "good" | "warning" | "neutral";
  children: ReactNode;
}) {
  const color = COLORS[theme][tone];
  return (
    <span
      className="shrink-0 rounded-full px-2.5 py-0.5 text-center text-[11px] font-semibold"
      style={{ color, background: `color-mix(in srgb, ${color} 15%, transparent)` }}
    >
      {children}
    </span>
  );
}
