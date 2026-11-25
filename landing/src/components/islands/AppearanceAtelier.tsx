import { Check, Eye, EyeOff } from "lucide-react";
import { useInView, useReducedMotion } from "motion/react";
import { useEffect, useRef, useState } from "react";
import { T } from "../lib/T";
import { ControlCenter, type SurfaceStyleId, type ThemeAppearance } from "./ControlCenter";
import { Switch } from "./Switch";

type ThemeChoice = "auto" | "light" | "dark";

const STYLES: { id: SurfaceStyleId; label: string; desc: [string, string] }[] = [
  { id: "liquid", label: "Liquid Glass", desc: ["Refraksi tepi & rim spekular", "Edge refraction & specular rim"] },
  { id: "frosted", label: "Frosted", desc: ["Kaca buram lembut netral", "Calm neutral frosted surface"] },
  { id: "tinted", label: "Tinted", desc: ["Kaca menyatu warna aksen", "Glass infused with accent hue"] },
  { id: "solid", label: "Solid", desc: ["Hemat daya, tanpa capture", "Zero capture, lower battery"] },
];

const ACCENTS = [
  { id: "system", label: "System", swatch: "#007aff", light: "#0064e1", dark: "#0a84ff" },
  { id: "blue", label: "Blue", swatch: "#0a84ff", light: "#0064e1", dark: "#0069eb" },
  { id: "purple", label: "Purple", swatch: "#af52de", light: "#7c34db", dark: "#8944f0" },
  { id: "rose", label: "Rose", swatch: "#ff375f", light: "#cc2058", dark: "#d92863" },
  { id: "amber", label: "Amber", swatch: "#ff9f0a", light: "#b05000", dark: "#be5a00" },
  { id: "emerald", label: "Emerald", swatch: "#30d158", light: "#0b7946", dark: "#0e8650" },
  { id: "graphite", label: "Graphite", swatch: "#8e8e93", light: "#4f5461", dark: "#5c616e" },
] as const;

const THEME_MODES: ThemeChoice[] = ["auto", "light", "dark"];

function swatchFill(id: SurfaceStyleId, dark: boolean, accent: string) {
  if (id === "solid") return dark ? "#2c2c30" : "#ffffff";
  if (id === "tinted") return `color-mix(in srgb, ${accent} 34%, rgb(255 255 255 / 0.55))`;
  if (id === "frosted") return "rgb(255 255 255 / 0.72)";
  return "rgb(255 255 255 / 0.42)";
}

export function AppearanceAtelier() {
  const [style, setStyle] = useState<SurfaceStyleId>("liquid");
  const [themeChoice, setThemeChoice] = useState<ThemeChoice>("light");
  const [accentId, setAccentId] = useState<string>("blue");
  const [captureVisible, setCaptureVisible] = useState(true);

  const reduced = useReducedMotion() ?? false;
  const stageRef = useRef<HTMLDivElement>(null);
  const inView = useInView(stageRef, { amount: 0.35 });
  const [auto, setAuto] = useState(true);

  useEffect(() => {
    if (!auto || !inView || reduced) return;
    const id = window.setInterval(() => {
      setStyle((curr) => {
        const index = STYLES.findIndex((item) => item.id === curr);
        return STYLES[(index + 1) % STYLES.length].id;
      });
    }, 2800);
    return () => window.clearInterval(id);
  }, [auto, inView, reduced]);

  const resolvedTheme: ThemeAppearance = themeChoice === "dark" ? "dark" : "light";
  const dark = resolvedTheme === "dark";
  const accentObj = ACCENTS.find((a) => a.id === accentId) ?? ACCENTS[1];
  const resolvedAccent = dark ? accentObj.dark : accentObj.light;

  return (
    <div className="grid items-center gap-12 lg:grid-cols-[minmax(0,0.92fr)_minmax(0,1.08fr)] lg:gap-16">
      <div
        ref={stageRef}
        className="relative flex items-center justify-center overflow-hidden rounded-[28px] p-6 transition-colors duration-500 sm:p-12 lg:order-2"
        style={{ background: dark ? "#0c0e14" : `color-mix(in srgb, ${resolvedAccent} 12%, #ffffff)` }}
      >
        <ControlCenter surfaceStyle={style} theme={resolvedTheme} accent={resolvedAccent} />
      </div>

      <div className="flex min-w-0 flex-col gap-9">
        <div>
          <p className="text-[13px] font-medium text-muted">
            <T id="Gaya Permukaan" en="Surface Style" />
          </p>
          <div className="mt-3 border-t border-line" role="group" aria-label="Surface style">
            {STYLES.map((item) => {
              const selected = style === item.id;
              return (
                <button
                  key={item.id}
                  type="button"
                  aria-pressed={selected}
                  onClick={() => {
                    setAuto(false);
                    setStyle(item.id);
                  }}
                  className="flex w-full items-center gap-4 border-b border-line py-3.5 text-left focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
                >
                  <span
                    className={`relative h-10 w-14 shrink-0 overflow-hidden rounded-[10px] border border-line ${dark ? "bg-[#0c0e14]" : "bg-page"}`}
                    aria-hidden="true"
                  >
                    <span
                      className="absolute -top-2.5 -right-2 size-7 rounded-full"
                      style={{ background: resolvedAccent }}
                    />
                    <span
                      className={`absolute inset-x-2 top-2 bottom-2 rounded-[6px] border ${item.id === "liquid" ? "border-white/70" : "border-black/[0.08]"}`}
                      style={{ background: swatchFill(item.id, dark, resolvedAccent) }}
                    />
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className={`block text-[15px] font-medium ${selected ? "text-ink" : "text-ink-soft"}`}>
                      {item.label}
                    </span>
                    <span className="block text-[12.5px] text-muted">
                      <T id={item.desc[0]} en={item.desc[1]} />
                    </span>
                  </span>
                  <Check
                    size={16}
                    strokeWidth={2.4}
                    className={`shrink-0 text-accent ${selected ? "opacity-100" : "opacity-0"}`}
                    aria-hidden="true"
                  />
                </button>
              );
            })}
          </div>
        </div>

        <div className="grid gap-7 sm:grid-cols-2">
          <div className="flex flex-col gap-3">
            <p className="text-[13px] font-medium text-muted">
              <T id="Mode Tema" en="Theme Mode" />
            </p>
            <div className="grid grid-cols-3 rounded-full bg-black/[0.05] p-1" role="group" aria-label="Theme mode">
              {THEME_MODES.map((mode) => (
                <button
                  key={mode}
                  type="button"
                  aria-pressed={themeChoice === mode}
                  onClick={() => setThemeChoice(mode)}
                  className={`h-8 rounded-full text-[12.5px] font-medium capitalize transition-colors ${
                    themeChoice === mode ? "bg-paper text-ink" : "text-muted hover:text-ink"
                  }`}
                >
                  {mode}
                </button>
              ))}
            </div>
          </div>

          <div className="flex flex-col gap-3">
            <p className="text-[13px] font-medium text-muted">
              <T id="Warna Aksen" en="Accent Color" /> ·{" "}
              <strong className="font-semibold text-ink">{accentObj.label}</strong>
            </p>
            <div className="flex flex-wrap items-center gap-2.5" role="group" aria-label="Accent color">
              {ACCENTS.map((item) => {
                const selected = accentId === item.id;
                return (
                  <button
                    key={item.id}
                    type="button"
                    aria-pressed={selected}
                    onClick={() => setAccentId(item.id)}
                    className="grid size-7 shrink-0 place-items-center rounded-full transition-transform hover:scale-110 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
                    style={{
                      background: item.swatch,
                      outline: selected ? "2px solid var(--color-ink)" : undefined,
                      outlineOffset: selected ? 2 : undefined,
                    }}
                  >
                    {selected ? <Check size={12} strokeWidth={3} className="text-white" aria-hidden="true" /> : null}
                    <span className="sr-only">{item.label}</span>
                  </button>
                );
              })}
            </div>
          </div>
        </div>

        <div className="flex items-center justify-between gap-4 border-t border-line pt-6">
          <div className="flex items-center gap-3">
            {captureVisible ? (
              <Eye size={16} strokeWidth={1.9} className="shrink-0 text-ink-soft" aria-hidden="true" />
            ) : (
              <EyeOff size={16} strokeWidth={1.9} className="shrink-0 text-muted" aria-hidden="true" />
            )}
            <div>
              <p className="text-[14px] font-semibold text-ink">
                <T id="Tampil di tangkapan layar & rekaman" en="Visible in screenshots & screen share" />
              </p>
              <p className="text-[12.5px] text-muted">
                <T
                  id="OBS, Discord, dan Windows Graphics Capture tanpa kedip."
                  en="Clean glass compositing for OBS, Discord and screen capture."
                />
              </p>
            </div>
          </div>
          <Switch
            checked={captureVisible}
            onChange={setCaptureVisible}
            label={["Tampil di tangkapan layar", "Visible in screenshots"]}
          />
        </div>
      </div>
    </div>
  );
}
