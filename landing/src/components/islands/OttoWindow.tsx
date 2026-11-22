import type { CSSProperties, ReactNode } from "react";
import { Minus, Square, X } from "lucide-react";
import type { LucideIcon } from "lucide-react";

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
    good: "#34c759",
    warning: "#ff9500",
  },
  dark: {
    body: "#1e1e20",
    sidebar: "#28282b",
    card: "#2c2c2f",
    text: "#f5f5f7",
    secondary: "#a1a1a6",
    hairline: "rgb(255 255 255 / 0.09)",
    selected: "rgb(255 255 255 / 0.10)",
    ring: "rgb(255 255 255 / 0.14)",
    captionHover: "hover:bg-white/10",
    good: "#30d158",
    warning: "#ff9f0a",
  },
} as const;

const CAPTION_BUTTONS: { icon: LucideIcon; close: boolean }[] = [
  { icon: Minus, close: false },
  { icon: Square, close: false },
  { icon: X, close: true },
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
      className="flex w-full min-w-0 flex-col overflow-hidden rounded-[10px]"
      style={{ background: c.body, color: c.text, boxShadow: `inset 0 0 0 1px ${c.ring}` }}
    >
      <div className="flex h-8 shrink-0 items-center justify-between pl-3">
        <div className="flex min-w-0 items-center gap-2 text-[12px] font-medium">
          <img src="/favicon.svg" alt="" width={14} height={14} />
          <span className="truncate">{title}</span>
        </div>
        <div className="flex h-full" aria-hidden="true">
          {CAPTION_BUTTONS.map(({ icon: Icon, close }, index) => (
            <span
              key={index}
              className={`grid h-full w-[46px] place-items-center ${close ? "hover:bg-[#c42b1c] hover:text-white" : c.captionHover}`}
            >
              <Icon size={10} strokeWidth={1.2} />
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
      className="flex w-[60px] shrink-0 flex-col overflow-hidden p-2.5 lg:w-[240px]"
      style={{ background: COLORS[theme].sidebar }}
    >
      {children}
    </div>
  );
}

export function SidebarGroup({ theme, title, children }: { theme: Theme; title?: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-0.5">
      {title ? (
        <p className="hidden px-2 pt-3 pb-1 text-[11px] font-semibold lg:block" style={{ color: COLORS[theme].secondary }}>
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
}: {
  theme: Theme;
  icon: LucideIcon;
  tint: string;
  label: string;
  selected?: boolean;
}) {
  const c = COLORS[theme];
  return (
    <div
      className="flex h-8 items-center gap-2.5 rounded-[8px] px-1.5"
      style={{ background: selected ? c.selected : "transparent", color: c.text }}
    >
      <span className="grid size-6 shrink-0 place-items-center rounded-[7px] text-white" style={{ background: tint }} aria-hidden="true">
        <Icon size={13} strokeWidth={2} />
      </span>
      <span className="hidden truncate text-[13px] lg:block">{label}</span>
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
  trailing,
}: {
  theme: Theme;
  icon?: LucideIcon;
  tint?: string;
  title: string;
  subtitle?: string;
  trailing?: ReactNode;
}) {
  const c = COLORS[theme];
  return (
    <div className="flex min-h-[48px] items-center gap-3 px-4 py-2.5">
      {Icon ? (
        <span className="grid size-7 shrink-0 place-items-center rounded-[8px] text-white" style={{ background: tint }} aria-hidden="true">
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

export function Badge({ theme, tone, children }: { theme: Theme; tone: "good" | "warning"; children: ReactNode }) {
  const color = COLORS[theme][tone];
  return (
    <span
      className="w-[84px] shrink-0 rounded-full py-0.5 text-center text-[11px] font-semibold"
      style={{ color, background: `color-mix(in srgb, ${color} 16%, transparent)` }}
    >
      {children}
    </span>
  );
}
