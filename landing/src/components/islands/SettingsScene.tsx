import { ControlCenter } from "./ControlCenter";
import { OttoSettingsWindow } from "./OttoSettings";

export function SettingsScene() {
  return (
    <div className="rounded-[24px] bg-[linear-gradient(160deg,var(--color-wallpaper),var(--color-wallpaper-edge))] p-4 ring-1 ring-black/10 sm:p-8 lg:p-10">
      <div className="flex flex-col gap-6 lg:flex-row lg:items-start lg:gap-8">
        <div className="min-w-0 lg:flex-1">
          <OttoSettingsWindow />
        </div>
        <div className="w-full lg:w-[376px] lg:shrink-0">
          <ControlCenter />
        </div>
      </div>
    </div>
  );
}
