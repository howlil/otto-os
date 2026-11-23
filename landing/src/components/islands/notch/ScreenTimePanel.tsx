import { useState } from "react";
import { ChevronRight } from "lucide-react";
import { PanelFrame } from "./Panels";
import { PALETTE, TOP_SCREEN_APPS, WEEKLY_SCREEN_TIME, formatMinutesBrief, type NotchTabKey } from "./spec";

const PEAK_MINUTES = Math.max(...WEEKLY_SCREEN_TIME.map((d) => d.totalMinutes));
const AVG_MINUTES = Math.round(WEEKLY_SCREEN_TIME.reduce((sum, d) => sum + d.totalMinutes, 0) / WEEKLY_SCREEN_TIME.length);
const TOTAL_WEEK_MINUTES = WEEKLY_SCREEN_TIME.reduce((sum, d) => sum + d.totalMinutes, 0);
const TOTAL_WEEK_FOCUS = WEEKLY_SCREEN_TIME.reduce((sum, d) => sum + d.focusMinutes, 0);
const WEEK_FOCUS_PCT = Math.round((TOTAL_WEEK_FOCUS / TOTAL_WEEK_MINUTES) * 100);
const AVG_PCT = Math.round((AVG_MINUTES / PEAK_MINUTES) * 100);

export function ScreenTimePanel({ onSelectTab }: { onSelectTab?: (tab: NotchTabKey) => void }) {
  const [range, setRange] = useState<"today" | "week">("today");
  const [selectedDay, setSelectedDay] = useState<number | null>(null);
  const [hoveredDay, setHoveredDay] = useState<number | null>(null);

  const activeDayIndex = hoveredDay ?? selectedDay;
  const activeDay = activeDayIndex !== null ? WEEKLY_SCREEN_TIME[activeDayIndex] : null;
  const readout = activeDay
    ? `${activeDay.today ? `Today (${activeDay.day})` : activeDay.day} · ${formatMinutesBrief(activeDay.totalMinutes)} · ${formatMinutesBrief(activeDay.focusMinutes)} focus`
    : `Avg ${formatMinutesBrief(AVG_MINUTES)}/day · ${WEEK_FOCUS_PCT}% focus`;

  const focusPctToday = 68;
  const ringR = 15;
  const ringCirc = 2 * Math.PI * ringR;

  return (
    <PanelFrame
      tab="time"
      onSelectTab={onSelectTab}
      actions={
        <div className="flex items-center gap-1.5">
          <div className="flex items-center rounded-full bg-white/[0.08] p-[2px]" role="group" aria-label="Screen Time range">
            <button
              type="button"
              aria-pressed={range === "today"}
              onClick={() => setRange("today")}
              className={`h-[18px] rounded-full px-2 text-[10.5px] font-semibold transition-colors focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-[#75b0ff] ${
                range === "today" ? "bg-white/[0.18] text-[#f5f5f7]" : "text-[#a8abb2] hover:text-[#f5f5f7]"
              }`}
            >
              Today
            </button>
            <button
              type="button"
              aria-pressed={range === "week"}
              onClick={() => setRange("week")}
              className={`h-[18px] rounded-full px-2 text-[10.5px] font-semibold transition-colors focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-[#75b0ff] ${
                range === "week" ? "bg-white/[0.18] text-[#f5f5f7]" : "text-[#a8abb2] hover:text-[#f5f5f7]"
              }`}
            >
              7 Days
            </button>
          </div>
          <a
            href="#fokus"
            className="inline-flex h-[20px] items-center gap-0.5 rounded-full bg-white/[0.1] px-2 text-[10.5px] font-semibold text-[#f5f5f7] transition-colors hover:bg-white/[0.16] focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-[#75b0ff]"
          >
            Details
            <ChevronRight size={10} strokeWidth={2.4} aria-hidden="true" />
          </a>
        </div>
      }
    >
      <div className="flex h-[138px] w-full flex-col overflow-hidden rounded-[12px] bg-white/[0.055] p-3 ring-1 ring-inset ring-white/10">
        {range === "today" ? (
          <div className="grid h-full grid-cols-[136px_1px_minmax(0,1fr)] items-center gap-3">
            <div className="flex h-full flex-col justify-between">
              <div>
                <div className="flex items-center justify-between gap-1">
                  <span className="text-[10.5px] font-medium text-[#a8abb2]">Screen Time</span>
                  <span className="text-[10px] font-semibold tabular-nums" style={{ color: PALETTE.positive }}>
                    4 blocked
                  </span>
                </div>
                <p className="mt-0.5 text-[22px] leading-tight font-semibold tracking-[-0.02em] text-[#f5f5f7] tabular-nums">
                  3h 48m
                </p>
              </div>
              <div className="flex items-center gap-2.5">
                <div className="relative grid size-[38px] shrink-0 place-items-center">
                  <svg width={38} height={38} viewBox="0 0 38 38" className="-rotate-90" aria-hidden="true">
                    <circle cx="19" cy="19" r={ringR} fill="none" stroke="rgb(255 255 255 / 0.14)" strokeWidth={3.2} />
                    <circle
                      cx="19"
                      cy="19"
                      r={ringR}
                      fill="none"
                      stroke={PALETTE.focus}
                      strokeWidth={3.2}
                      strokeLinecap="round"
                      strokeDasharray={ringCirc}
                      strokeDashoffset={ringCirc * (1 - focusPctToday / 100)}
                    />
                  </svg>
                  <span className="absolute text-[9.5px] font-semibold text-[#f5f5f7] tabular-nums">{focusPctToday}%</span>
                </div>
                <div className="min-w-0">
                  <p className="text-[10px] text-[#a8abb2]">Focus</p>
                  <p className="text-[12.5px] font-semibold text-[#f5f5f7] tabular-nums">2h 35m</p>
                </div>
              </div>
            </div>

            <span className="h-full w-px bg-white/10" aria-hidden="true" />

            <div className="flex h-full min-w-0 flex-col justify-between">
              <span className="text-[10.5px] font-medium text-[#a8abb2]">Most Used</span>
              <div className="flex flex-col gap-2">
                {TOP_SCREEN_APPS.map((app, idx) => (
                  <div key={app.key} className="flex items-center gap-2">
                    <span
                      className="grid size-5 shrink-0 place-items-center rounded-[5px] text-[9px] font-bold text-white"
                      style={{ background: app.color }}
                      aria-hidden="true"
                    >
                      {app.badge}
                    </span>
                    <div className="min-w-0 flex-1">
                      <div className="flex items-baseline justify-between gap-2">
                        <span className="truncate text-[11px] font-medium text-[#f5f5f7]">{app.name}</span>
                        <span className="shrink-0 text-[10.5px] text-[#a8abb2] tabular-nums">{app.duration}</span>
                      </div>
                      <div className="mt-0.5 h-[3px] w-full overflow-hidden rounded-full bg-white/[0.12]">
                        <div
                          className="h-full rounded-full"
                          style={{
                            width: `${app.share}%`,
                            background: idx === 0 ? PALETTE.screenBlue : idx === 1 ? "#3da0ff" : "#6bb7ff",
                          }}
                        />
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          </div>
        ) : (
          <div className="flex h-full flex-col justify-between">
            <div className="flex items-center justify-between gap-2">
              <p className="truncate text-[11px] font-medium text-[#f5f5f7] tabular-nums">{readout}</p>
              <div className="flex shrink-0 items-center gap-2.5 text-[10px] text-[#a8abb2]">
                <span className="flex items-center gap-1">
                  <span className="size-1.5 rounded-full bg-[#0a84ff]" aria-hidden="true" />
                  Screen
                </span>
                <span className="flex items-center gap-1">
                  <span className="size-1.5 rounded-full bg-[#ff9230]" aria-hidden="true" />
                  Focus
                </span>
              </div>
            </div>

            <div className="relative mt-2 flex flex-1 items-end gap-2 pt-3">
              <div
                className="pointer-events-none absolute inset-x-0 flex items-center"
                style={{ bottom: `calc(${AVG_PCT}% * 0.72 + 18px)` }}
                aria-hidden="true"
              >
                <span className="w-full border-t border-dashed border-[#30d158]/65" />
                <span className="ml-1.5 shrink-0 text-[9px] font-semibold text-[#30d158]">avg</span>
              </div>

              {WEEKLY_SCREEN_TIME.map((day, idx) => {
                const totalH = Math.max(12, Math.round((day.totalMinutes / PEAK_MINUTES) * 66));
                const focusH = Math.max(4, Math.round((day.focusMinutes / day.totalMinutes) * totalH));
                const active = activeDayIndex === idx;
                return (
                  <button
                    key={day.day}
                    type="button"
                    aria-pressed={selectedDay === idx}
                    onMouseEnter={() => setHoveredDay(idx)}
                    onMouseLeave={() => setHoveredDay(null)}
                    onFocus={() => setHoveredDay(idx)}
                    onBlur={() => setHoveredDay(null)}
                    onClick={() => setSelectedDay((curr) => (curr === idx ? null : idx))}
                    className="group flex flex-1 flex-col items-center gap-1 rounded-md py-0.5 focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-[#75b0ff]"
                  >
                    <div className="flex h-[68px] w-full items-end justify-center">
                      <div
                        className={`relative w-[18px] overflow-hidden rounded-[4px] transition-opacity ${
                          active ? "opacity-100 ring-1 ring-white/50" : "opacity-85 group-hover:opacity-100"
                        }`}
                        style={{ height: totalH, background: PALETTE.screenBlue }}
                      >
                        <span
                          className="absolute inset-x-0 bottom-0 rounded-t-[2px]"
                          style={{ height: focusH, background: PALETTE.focus }}
                        />
                      </div>
                    </div>
                    <span className="flex items-center gap-0.5 text-[9.5px] font-medium text-[#a8abb2]">
                      {day.day}
                      {day.today ? <span className="size-1 rounded-full bg-[#ff9230]" aria-hidden="true" /> : null}
                    </span>
                  </button>
                );
              })}
            </div>
          </div>
        )}
      </div>
    </PanelFrame>
  );
}
