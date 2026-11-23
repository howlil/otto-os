import { useState, type CSSProperties } from "react";
import {
  BatteryMedium,
  Bluetooth,
  Check,
  ChevronRight,
  Headphones,
  Monitor,
  Moon,
  Pause,
  Play,
  Settings,
  SkipBack,
  SkipForward,
  Speaker,
  Sun,
  Volume2,
  VolumeX,
  Wifi,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";

export type SurfaceStyleId = "liquid" | "frosted" | "tinted" | "solid";
export type ThemeAppearance = "light" | "dark";

const FOCUS_RING = "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent";

const OUTPUT_DEVICES = [
  { id: "headphones", name: "WH-1000XM5", icon: Headphones },
  { id: "speakers", name: "Speakers (Realtek)", icon: Speaker },
  { id: "hdmi", name: "Studio Display Audio", icon: Monitor },
] as const;

function ConnectionRow({
  icon: Icon,
  title,
  subtitle,
  on,
  dark,
  accent,
  onToggle,
}: {
  icon: LucideIcon;
  title: string;
  subtitle: string;
  on: boolean;
  dark: boolean;
  accent: string;
  onToggle: () => void;
}) {
  return (
    <div className="flex flex-1 items-center gap-2.5 px-3">
      <button
        type="button"
        aria-label={title}
        aria-pressed={on}
        onClick={onToggle}
        className={`grid size-8 shrink-0 place-items-center rounded-full transition-colors ${FOCUS_RING} ${
          on ? "text-white" : dark ? "bg-white/12 text-white/70" : "bg-black/[0.08] text-ink-soft"
        }`}
        style={on ? { background: accent } : undefined}
      >
        <Icon size={15} strokeWidth={2} aria-hidden="true" />
      </button>
      <div className="min-w-0">
        <p className={`text-[13px] font-semibold ${dark ? "text-[#f5f5f7]" : "text-ink"}`}>{title}</p>
        <p className={`text-[12px] ${dark ? "text-[#a1a1a6]" : "text-ink-soft"}`}>{subtitle}</p>
      </div>
    </div>
  );
}

function Slider({
  label,
  value,
  accent,
  dark,
  onChange,
  disabled = false,
}: {
  label: string;
  value: number;
  accent: string;
  dark: boolean;
  onChange: (value: number) => void;
  disabled?: boolean;
}) {
  return (
    <input
      type="range"
      aria-label={label}
      min={0}
      max={100}
      value={value}
      disabled={disabled}
      onChange={(event) => onChange(Number(event.target.value))}
      style={
        {
          "--range-value": `${value}%`,
          "--range-fill": accent,
          "--range-track": dark ? "rgb(255 255 255 / 0.14)" : "rgb(27 27 29 / 0.11)",
        } as CSSProperties
      }
      className="range-quiet w-full disabled:opacity-40"
    />
  );
}

export function ControlCenter({
  surfaceStyle = "liquid",
  theme = "light",
  accent = "#007aff",
}: {
  surfaceStyle?: SurfaceStyleId;
  theme?: ThemeAppearance;
  accent?: string;
}) {
  const [wifi, setWifi] = useState(true);
  const [bluetooth, setBluetooth] = useState(true);
  const [focus, setFocus] = useState(false);
  const [display, setDisplay] = useState(74);
  const [sound, setSound] = useState(62);
  const [muted, setMuted] = useState(false);
  const [playing, setPlaying] = useState(true);
  const [audioMenu, setAudioMenu] = useState(false);
  const [device, setDevice] = useState<string>("headphones");

  const dark = theme === "dark";

  const shellStyle: CSSProperties = (() => {
    if (surfaceStyle === "solid") {
      return { background: dark ? "#222226" : "#f2f2f7" };
    }
    if (surfaceStyle === "tinted") {
      return {
        background: dark
          ? `color-mix(in srgb, ${accent} 24%, rgb(22 22 28 / 0.72))`
          : `color-mix(in srgb, ${accent} 18%, rgb(255 255 255 / 0.64))`,
        backdropFilter: "blur(28px) saturate(1.4)",
      };
    }
    if (surfaceStyle === "frosted") {
      return {
        background: dark ? "rgb(32 32 38 / 0.78)" : "rgb(255 255 255 / 0.76)",
        backdropFilter: "blur(32px) saturate(1.2)",
      };
    }
    return {
      background: dark ? "rgb(20 22 28 / 0.48)" : "rgb(255 255 255 / 0.44)",
      backdropFilter: "blur(22px) saturate(1.35)",
    };
  })();

  const cardBg =
    surfaceStyle === "solid"
      ? dark
        ? "bg-[#2c2c30] ring-1 ring-inset ring-white/[0.08]"
        : "bg-white ring-1 ring-inset ring-black/[0.06]"
      : dark
        ? "bg-[#26262c]/82 ring-1 ring-inset ring-white/[0.1]"
        : "bg-white/82 ring-1 ring-inset ring-black/[0.05]";

  const primaryInk = dark ? "text-[#f5f5f7]" : "text-ink";
  const secondaryInk = dark ? "text-[#a1a1a6]" : "text-ink-soft";

  return (
    <div className={`w-full max-w-[376px] rounded-[26px] border p-3 transition-all duration-300 ${dark ? "border-white/[0.12]" : "border-black/[0.08]"}`} style={shellStyle}>
      <div className="grid grid-cols-2 gap-2">
        <div className={`flex h-36 flex-col overflow-hidden rounded-[14px] ${cardBg}`}>
          <ConnectionRow
            icon={Wifi}
            title="Wi-Fi"
            subtitle={wifi ? "On" : "Off"}
            on={wifi}
            dark={dark}
            accent={accent}
            onToggle={() => setWifi((v) => !v)}
          />
          <span aria-hidden="true" className={`mx-3 h-px ${dark ? "bg-white/10" : "bg-black/[0.08]"}`} />
          <ConnectionRow
            icon={Bluetooth}
            title="Bluetooth"
            subtitle={bluetooth ? "On" : "Off"}
            on={bluetooth}
            dark={dark}
            accent={accent}
            onToggle={() => setBluetooth((v) => !v)}
          />
        </div>

        <div className="grid h-36 grid-rows-2 gap-2">
          <div
            className={`relative flex flex-col justify-center gap-1 rounded-[14px] p-2.5 transition-colors ${
              focus ? "text-white" : `${cardBg} ${primaryInk}`
            }`}
            style={focus ? { background: accent } : undefined}
          >
            <button
              type="button"
              aria-pressed={focus}
              onClick={() => setFocus((v) => !v)}
              className={`flex flex-col items-start gap-0.5 rounded-[8px] text-left ${FOCUS_RING}`}
            >
              <span className="flex items-center gap-2">
                <Moon size={15} strokeWidth={2} aria-hidden="true" />
                <span className="text-[13px] font-semibold">Focus</span>
              </span>
              <span className={`pl-[23px] text-[12px] ${focus ? "text-white/85" : secondaryInk}`}>
                {focus ? "Focus On" : "Off"}
              </span>
            </button>
            <a
              href="#fokus"
              aria-label="Focus settings"
              className={`absolute top-1.5 right-1.5 grid size-5 place-items-center rounded-full transition-colors ${
                focus ? "text-white/85 hover:bg-white/15" : `${secondaryInk} hover:bg-black/5`
              }`}
            >
              <Settings size={12} strokeWidth={2} aria-hidden="true" />
            </a>
          </div>

          <div className={`flex flex-col justify-center gap-0.5 rounded-[14px] p-2.5 ${cardBg} ${primaryInk}`}>
            <span className="flex items-center gap-2">
              <BatteryMedium size={15} strokeWidth={2} aria-hidden="true" />
              <span className="text-[13px] font-semibold">Battery</span>
            </span>
            <span className={`pl-[23px] text-[12px] ${secondaryInk}`}>Standard · 86%</span>
          </div>
        </div>
      </div>

      <div className={`mt-2 flex h-16 flex-col justify-center gap-1 rounded-[14px] px-3 ${cardBg}`}>
        <div className="flex items-center justify-between">
          <p className={`text-[12px] font-semibold ${secondaryInk}`}>Display · DDC/CI</p>
          <span className={`text-[11px] tabular-nums ${secondaryInk}`}>{display}%</span>
        </div>
        <div className="flex items-center gap-2.5">
          <Sun size={15} strokeWidth={2} className={`shrink-0 ${secondaryInk}`} aria-hidden="true" />
          <Slider label="Brightness" value={display} accent={accent} dark={dark} onChange={setDisplay} />
        </div>
      </div>

      <div className={`mt-2 flex flex-col justify-center gap-1.5 rounded-[14px] px-3 py-2.5 ${cardBg}`}>
        <div className="flex items-center justify-between">
          <p className={`text-[12px] font-semibold ${secondaryInk}`}>Sound</p>
          <button
            type="button"
            aria-expanded={audioMenu}
            onClick={() => setAudioMenu((v) => !v)}
            className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] font-medium transition-colors ${
              dark ? "bg-white/10 text-white/85 hover:bg-white/15" : "bg-black/[0.06] text-ink-soft hover:bg-black/10"
            }`}
          >
            {OUTPUT_DEVICES.find((d) => d.id === device)?.name}
            <ChevronRight size={11} className={`transition-transform ${audioMenu ? "rotate-90" : ""}`} aria-hidden="true" />
          </button>
        </div>
        <div className="flex items-center gap-2.5">
          <button
            type="button"
            aria-label={muted ? "Unmute" : "Mute"}
            aria-pressed={muted}
            onClick={() => setMuted((v) => !v)}
            className={`grid size-6 shrink-0 place-items-center rounded-full transition-colors ${secondaryInk} ${
              dark ? "hover:bg-white/10" : "hover:bg-black/5"
            } ${FOCUS_RING}`}
          >
            {muted ? <VolumeX size={15} strokeWidth={2} aria-hidden="true" /> : <Volume2 size={15} strokeWidth={2} aria-hidden="true" />}
          </button>
          <Slider label="Output volume" value={sound} accent={accent} dark={dark} onChange={setSound} disabled={muted} />
        </div>
        {audioMenu ? (
          <div className={`mt-1 flex flex-col gap-0.5 border-t pt-1.5 ${dark ? "border-white/10" : "border-black/[0.08]"}`}>
            {OUTPUT_DEVICES.map((item) => {
              const Icon = item.icon;
              const selected = item.id === device;
              return (
                <button
                  key={item.id}
                  type="button"
                  onClick={() => {
                    setDevice(item.id);
                    setAudioMenu(false);
                  }}
                  className={`flex items-center justify-between rounded-[8px] px-2 py-1.5 text-left text-[12px] transition-colors ${
                    selected
                      ? dark
                        ? "bg-white/12 font-semibold text-white"
                        : "bg-black/[0.06] font-semibold text-ink"
                      : `${secondaryInk} ${dark ? "hover:bg-white/[0.07]" : "hover:bg-black/[0.04]"}`
                  }`}
                >
                  <span className="flex items-center gap-2">
                    <Icon size={13} strokeWidth={2} aria-hidden="true" />
                    {item.name}
                  </span>
                  {selected ? <Check size={13} strokeWidth={2.4} style={{ color: accent }} aria-hidden="true" /> : null}
                </button>
              );
            })}
          </div>
        ) : null}
      </div>

      <div className={`mt-2 flex h-[74px] items-center gap-3 rounded-[14px] py-3 pr-2.5 pl-3 ${cardBg}`}>
        <span
          className="size-10 shrink-0 rounded-[10px]"
          style={{ background: "linear-gradient(140deg, #ffb0b8 0%, #fc3c54 48%, #4b1224 100%)" }}
          aria-hidden="true"
        />
        <div className="min-w-0 flex-1">
          <p className={`truncate text-[13px] font-semibold ${primaryInk}`}>Lampu Kota</p>
          <p className={`truncate text-[12px] ${secondaryInk}`}>Hujan Sore · Spotify</p>
        </div>
        <div className="flex items-center gap-1">
          <button
            type="button"
            aria-label="Previous"
            className={`grid size-7 place-items-center rounded-full transition-colors ${primaryInk} ${dark ? "hover:bg-white/10" : "hover:bg-black/5"} ${FOCUS_RING}`}
          >
            <SkipBack size={14} fill="currentColor" strokeWidth={1.6} aria-hidden="true" />
          </button>
          <button
            type="button"
            aria-label={playing ? "Pause" : "Play"}
            onClick={() => setPlaying((v) => !v)}
            className={`grid size-8 place-items-center rounded-full text-white transition-transform active:scale-95 ${FOCUS_RING}`}
            style={{ background: accent }}
          >
            {playing ? <Pause size={14} fill="currentColor" aria-hidden="true" /> : <Play size={14} fill="currentColor" aria-hidden="true" />}
          </button>
          <button
            type="button"
            aria-label="Next"
            className={`grid size-7 place-items-center rounded-full transition-colors ${primaryInk} ${dark ? "hover:bg-white/10" : "hover:bg-black/5"} ${FOCUS_RING}`}
          >
            <SkipForward size={14} fill="currentColor" strokeWidth={1.6} aria-hidden="true" />
          </button>
        </div>
      </div>
    </div>
  );
}
