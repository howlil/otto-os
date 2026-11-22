import { useState, type CSSProperties } from "react";
import { BatteryMedium, Bluetooth, Moon, Pause, Play, Settings, SkipBack, SkipForward, Sun, Volume2, VolumeX, Wifi } from "lucide-react";
import type { LucideIcon } from "lucide-react";

const NATIVE_ACCENT = "#007aff";
const CARD = "bg-white/80 ring-1 ring-inset ring-black/[0.05]";
const FOCUS_RING = "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent";

function ConnectionRow({
  icon: Icon,
  title,
  subtitle,
  on,
  onToggle,
}: {
  icon: LucideIcon;
  title: string;
  subtitle: string;
  on: boolean;
  onToggle: () => void;
}) {
  return (
    <div className="flex flex-1 items-center gap-2.5 px-3">
      <button
        type="button"
        aria-label={title}
        aria-pressed={on}
        onClick={onToggle}
        className={`grid size-8 shrink-0 place-items-center rounded-full transition-colors ${FOCUS_RING} ${on ? "text-white" : "bg-black/[0.08] text-ink-soft"}`}
        style={on ? { background: NATIVE_ACCENT } : undefined}
      >
        <Icon size={15} strokeWidth={2} aria-hidden="true" />
      </button>
      <div className="min-w-0">
        <p className="text-[13px] font-semibold text-ink">{title}</p>
        <p className="text-[12px] text-ink-soft">{subtitle}</p>
      </div>
    </div>
  );
}

function Tile({
  icon: Icon,
  title,
  status,
  on,
  onToggle,
  gear,
}: {
  icon: LucideIcon;
  title: string;
  status: string;
  on: boolean;
  onToggle: () => void;
  gear: string;
}) {
  return (
    <div
      className={`relative flex flex-col justify-center gap-1 rounded-[14px] p-2.5 ${on ? "text-white" : `${CARD} text-ink`}`}
      style={on ? { background: NATIVE_ACCENT } : undefined}
    >
      <button type="button" aria-pressed={on} onClick={onToggle} className={`flex flex-col items-start gap-1 rounded-[8px] text-left ${FOCUS_RING}`}>
        <span className="flex items-center gap-2">
          <Icon size={15} strokeWidth={2} aria-hidden="true" />
          <span className="text-[13px] font-semibold">{title}</span>
        </span>
        <span className={`pl-[23px] text-[12px] ${on ? "text-white/85" : "text-ink-soft"}`}>{status}</span>
      </button>
      <button
        type="button"
        aria-label={gear}
        className={`absolute top-1.5 right-1.5 grid size-5 place-items-center rounded-full transition-colors ${FOCUS_RING} ${on ? "text-white/85 hover:bg-white/15" : "text-ink-soft hover:bg-black/5"}`}
      >
        <Settings size={12} strokeWidth={2} aria-hidden="true" />
      </button>
    </div>
  );
}

function Slider({
  label,
  value,
  onChange,
  disabled = false,
}: {
  label: string;
  value: number;
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
      style={{ "--range-value": `${value}%`, "--range-fill": NATIVE_ACCENT } as CSSProperties}
      className="range-quiet w-full disabled:opacity-40"
    />
  );
}

export function ControlCenter() {
  const [wifi, setWifi] = useState(true);
  const [bluetooth, setBluetooth] = useState(true);
  const [focus, setFocus] = useState(false);
  const [display, setDisplay] = useState(72);
  const [sound, setSound] = useState(58);
  const [muted, setMuted] = useState(false);
  const [playing, setPlaying] = useState(true);

  return (
    <div className="w-full max-w-[376px] rounded-[26px] bg-white/45 p-3 shadow-[0_24px_48px_-24px_rgb(20_40_80/0.45)] ring-1 ring-inset ring-white/70 backdrop-blur-2xl">
      <div className="grid grid-cols-2 gap-2">
        <div className={`flex h-36 flex-col overflow-hidden rounded-[14px] ${CARD}`}>
          <ConnectionRow
            icon={Wifi}
            title="Wi-Fi"
            subtitle={wifi ? "On" : "Off"}
            on={wifi}
            onToggle={() => setWifi((value) => !value)}
          />
          <span aria-hidden="true" className="mx-3 h-px bg-black/[0.08]" />
          <ConnectionRow
            icon={Bluetooth}
            title="Bluetooth"
            subtitle={bluetooth ? "On" : "Off"}
            on={bluetooth}
            onToggle={() => setBluetooth((value) => !value)}
          />
        </div>
        <div className="grid h-36 grid-rows-2 gap-2">
          <Tile
            icon={Moon}
            title="Focus"
            status={focus ? "Focus On" : "Off"}
            on={focus}
            onToggle={() => setFocus((value) => !value)}
            gear="Focus settings"
          />
          <div className={`flex flex-col justify-center gap-1 rounded-[14px] p-2.5 text-ink ${CARD}`}>
            <span className="flex items-center gap-2">
              <BatteryMedium size={15} strokeWidth={2} aria-hidden="true" />
              <span className="text-[13px] font-semibold">Battery</span>
            </span>
            <span className="pl-[23px] text-[12px] text-ink-soft">Standard</span>
          </div>
        </div>
      </div>

      <div className={`mt-2 flex h-16 flex-col justify-center gap-1 rounded-[14px] px-3 ${CARD}`}>
        <p className="text-[12px] font-semibold text-ink-soft">Display</p>
        <div className="flex items-center gap-2.5">
          <Sun size={15} strokeWidth={2} className="shrink-0 text-ink-soft" aria-hidden="true" />
          <Slider label="Brightness" value={display} onChange={setDisplay} />
        </div>
      </div>

      <div className={`mt-2 flex h-16 flex-col justify-center gap-1 rounded-[14px] px-3 ${CARD}`}>
        <p className="text-[12px] font-semibold text-ink-soft">Sound</p>
        <div className="flex items-center gap-2.5">
          <button
            type="button"
            aria-label={muted ? "Unmute" : "Mute"}
            aria-pressed={muted}
            onClick={() => setMuted((value) => !value)}
            className={`grid size-6 shrink-0 place-items-center rounded-full text-ink-soft transition-colors hover:bg-black/5 ${FOCUS_RING}`}
          >
            {muted ? <VolumeX size={15} strokeWidth={2} aria-hidden="true" /> : <Volume2 size={15} strokeWidth={2} aria-hidden="true" />}
          </button>
          <Slider label="Output volume" value={sound} onChange={setSound} disabled={muted} />
        </div>
      </div>

      <div className={`mt-2 flex h-[74px] items-center gap-3 rounded-[14px] py-3 pr-2.5 pl-3 ${CARD}`}>
        <div className="min-w-0 flex-1">
          <p className="truncate text-[13px] font-semibold text-ink">Lampu Kota</p>
          <p className="truncate text-[12px] text-ink-soft">Demo data</p>
        </div>
        <div className="flex items-center gap-1.5">
          <button
            type="button"
            aria-label="Previous"
            className={`grid size-7 place-items-center rounded-full text-ink transition-colors hover:bg-black/5 ${FOCUS_RING}`}
          >
            <SkipBack size={14} fill="currentColor" strokeWidth={1.6} aria-hidden="true" />
          </button>
          <button
            type="button"
            aria-label={playing ? "Pause" : "Play"}
            onClick={() => setPlaying((value) => !value)}
            className={`grid size-7 place-items-center rounded-full text-ink transition-colors hover:bg-black/5 ${FOCUS_RING}`}
          >
            {playing ? <Pause size={14} fill="currentColor" aria-hidden="true" /> : <Play size={14} fill="currentColor" aria-hidden="true" />}
          </button>
          <button
            type="button"
            aria-label="Next"
            className={`grid size-7 place-items-center rounded-full text-ink transition-colors hover:bg-black/5 ${FOCUS_RING}`}
          >
            <SkipForward size={14} fill="currentColor" strokeWidth={1.6} aria-hidden="true" />
          </button>
        </div>
      </div>
    </div>
  );
}
