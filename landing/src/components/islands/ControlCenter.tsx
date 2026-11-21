import { useState, type CSSProperties, type ReactNode } from "react";
import { motion } from "motion/react";
import { BatteryMedium, Bluetooth, Disc3, Moon, Pause, Play, Sun, Volume2, Wifi } from "lucide-react";
import { T } from "../lib/T";

const SPRING = { type: "spring" as const, stiffness: 500, damping: 34, mass: 0.7 };

function Switch({
  checked,
  onChange,
  label,
}: {
  checked: boolean;
  onChange: (next: boolean) => void;
  label: [string, string];
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      onClick={() => onChange(!checked)}
      className={`relative inline-flex h-[22px] w-[38px] shrink-0 items-center rounded-full p-[2px] transition-colors duration-200 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent ${
        checked ? "bg-accent" : "bg-[#c9ccd3]"
      }`}
    >
      <motion.span
        className="block size-[18px] rounded-full bg-white shadow-[0_1px_2px_rgb(0_0_0/0.25)]"
        animate={{ x: checked ? 16 : 0 }}
        transition={SPRING}
      />
      <span className="sr-only">
        <T id={label[0]} en={label[1]} />
      </span>
    </button>
  );
}

function Slider({
  value,
  onChange,
  icon,
  label,
}: {
  value: number;
  onChange: (value: number) => void;
  icon: ReactNode;
  label: [string, string];
}) {
  const style = {
    "--range-value": `${value}%`,
    "--range-fill": "#075bef",
  } as CSSProperties;
  return (
    <label className="flex items-center gap-3 rounded-[14px] bg-[#f3f4f6] px-3 py-3 text-ink">
      <span className="shrink-0 text-ink-soft" aria-hidden="true">
        {icon}
      </span>
      <input
        type="range"
        min={0}
        max={100}
        value={value}
        onChange={(event) => onChange(Number(event.target.value))}
        style={style}
        className="range-quiet w-full"
      />
      <span className="sr-only">
        <T id={label[0]} en={label[1]} />
      </span>
      <span className="w-9 text-right text-[13px] font-medium tabular-nums text-ink-soft" aria-hidden="true">
        {value}
      </span>
    </label>
  );
}

export function ControlCenter() {
  const [wifi, setWifi] = useState(true);
  const [bluetooth, setBluetooth] = useState(true);
  const [focus, setFocus] = useState(false);
  const [saver, setSaver] = useState(false);
  const [display, setDisplay] = useState(72);
  const [sound, setSound] = useState(58);
  const [playing, setPlaying] = useState(true);

  return (
    <div className="mx-auto w-full max-w-[380px] rounded-[26px] bg-paper p-3 shadow-[0_30px_60px_-30px_rgb(20_40_80/0.35)] ring-1 ring-black/[0.06]">
      <div className="grid grid-cols-2 gap-2">
        <div className="flex flex-col justify-between gap-4 rounded-[14px] bg-[#f3f4f6] p-3">
          <div className="flex items-center justify-between">
            <span className="grid size-8 place-items-center rounded-full bg-accent text-white" aria-hidden="true">
              <Wifi size={16} strokeWidth={2} />
            </span>
            <Switch checked={wifi} onChange={setWifi} label={["Wi-Fi", "Wi-Fi"]} />
          </div>
          <div className="flex flex-col">
            <span className="text-[13px] font-semibold text-ink">Wi-Fi</span>
            <span className="text-[12px] text-ink-soft">
              {wifi ? <T id="Terhubung" en="Connected" /> : <T id="Mati" en="Off" />}
            </span>
          </div>
        </div>
        <div className="flex flex-col gap-2">
          <div className="flex items-center justify-between rounded-[14px] bg-[#f3f4f6] px-3 py-2.5">
            <span className="flex items-center gap-2 text-[13px] font-medium text-ink">
              <Bluetooth size={15} strokeWidth={2} aria-hidden="true" />
              Bluetooth
            </span>
            <Switch checked={bluetooth} onChange={setBluetooth} label={["Bluetooth", "Bluetooth"]} />
          </div>
          <div className="flex items-center justify-between rounded-[14px] bg-[#f3f4f6] px-3 py-2.5">
            <span className="flex items-center gap-2 text-[13px] font-medium text-ink">
              <Moon size={15} strokeWidth={2} aria-hidden="true" />
              <T id="Fokus" en="Focus" />
            </span>
            <Switch checked={focus} onChange={setFocus} label={["Fokus", "Focus"]} />
          </div>
        </div>
      </div>

      <div className="mt-2 flex items-center justify-between rounded-[14px] bg-[#f3f4f6] px-3 py-2.5">
        <span className="flex items-center gap-2 text-[13px] font-medium text-ink">
          <BatteryMedium size={15} strokeWidth={2} aria-hidden="true" />
          <T id="Baterai" en="Battery" />
        </span>
        <button
          type="button"
          onClick={() => setSaver((value) => !value)}
          className="rounded-full bg-paper px-3 py-1 text-[12px] font-medium text-ink ring-1 ring-black/10 transition-colors hover:bg-white focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
        >
          {saver ? <T id="Hemat" en="Saver" /> : <T id="Seimbang" en="Balanced" />}
        </button>
      </div>

      <div className="mt-2 flex flex-col gap-2">
        <Slider value={display} onChange={setDisplay} icon={<Sun size={16} strokeWidth={2} />} label={["Kecerahan", "Brightness"]} />
        <Slider value={sound} onChange={setSound} icon={<Volume2 size={16} strokeWidth={2} />} label={["Suara", "Sound"]} />
      </div>

      <div className="mt-2 flex items-center gap-3 rounded-[14px] bg-[#f3f4f6] px-3 py-2.5">
        <span className="grid size-9 shrink-0 place-items-center rounded-xl bg-music-activity text-white" aria-hidden="true">
          <Disc3 size={18} strokeWidth={1.8} />
        </span>
        <div className="min-w-0 flex-1">
          <p className="text-[13px] font-semibold text-ink">
            <T id="Demo lagu" en="Demo track" />
          </p>
          <p className="text-[12px] text-ink-soft">
            <T id="Sumber demo" en="Demo source" />
          </p>
        </div>
        <button
          type="button"
          onClick={() => setPlaying((value) => !value)}
          className="grid size-9 place-items-center rounded-full bg-accent text-white transition-colors hover:bg-accent-hover focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
        >
          {playing ? <Pause size={15} fill="currentColor" aria-hidden="true" /> : <Play size={15} fill="currentColor" aria-hidden="true" />}
          <span className="sr-only">
            {playing ? <T id="Jeda" en="Pause" /> : <T id="Putar" en="Play" />}
          </span>
        </button>
      </div>
    </div>
  );
}
