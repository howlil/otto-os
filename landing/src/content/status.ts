export type StatusState = "ready" | "pre-beta" | "next";

export type StatusItem = {
  label: string;
  state: StatusState;
};

export const statusItems: StatusItem[] = [
  { label: "Bar atas, menu aplikasi & indikator privasi", state: "ready" },
  { label: "Dock, pengelompokan aplikasi & efek Genie", state: "ready" },
  { label: "Dynamic Island (Pomodoro, Musik, Pocket, Clip)", state: "ready" },
  { label: "Optimasi CPU idle & penandatanganan rilis", state: "pre-beta" },
  { label: "Aksesibilitas keyboard penuh & paket Store (v1.1)", state: "next" },
];
