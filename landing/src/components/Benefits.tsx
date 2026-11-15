const features = [
  {
    title: "Bar atas & Control Center",
    description:
      "Menu aplikasi aktif, kontrol audio dan kecerahan, pengalih perangkat output, serta titik indikator privasi mikrofon dan kamera di samping notch.",
    preview: "bar",
  },
  {
    title: "Dock & efek Genie",
    description:
      "Ikon membesar saat disorot dengan indikator aplikasi berjalan, tumpukan Downloads, Recycle Bin, serta animasi Genie dari jendela Windows ke Dock.",
    preview: "dock",
  },
  {
    title: "Kontrol jendela",
    description:
      "Minimize, zoom, dan tutup dari pojok kanan atas jendela aktif secara seragam, dengan perilaku bawaan tetap aman pada jendela sistem yang terlindungi.",
    preview: "window",
  },
  {
    title: "Dynamic Island, Pocket & Clip",
    description:
      "Timer fokus Pomodoro, pemutar musik dan lirik opsional, rak sementara Pocket untuk seret-lepas berkas, serta riwayat Clip yang hanya tinggal di RAM.",
    preview: "island",
  },
] as const;

function Preview({ kind }: { kind: (typeof features)[number]["preview"] }) {
  if (kind === "bar") {
    return (
      <div className="flex h-20 w-48 items-start rounded-xl border border-line bg-sky-mid p-3">
        <div className="flex w-full items-center gap-2 rounded-md bg-shell px-2.5 py-1.5 text-xs text-shell-ink">
          <span className="font-medium">Explorer</span>
          <span className="text-shell-muted">File</span>
          <span className="ml-auto tabular-nums">14:02</span>
        </div>
      </div>
    );
  }

  if (kind === "dock") {
    return (
      <div className="flex h-20 w-48 items-end justify-center rounded-xl border border-line bg-sky-mid pb-3">
        <div className="flex items-end gap-1.5 rounded-lg border border-white/25 bg-shell/90 px-2.5 py-1.5">
          <span className="size-5 rounded-md bg-sky-500" />
          <span className="relative size-6 rounded-md bg-accent after:absolute after:-bottom-2 after:left-1/2 after:size-1 after:-translate-x-1/2 after:rounded-full after:bg-white" />
          <span className="size-5 rounded-md bg-emerald-500" />
          <span className="size-5 rounded-md bg-zinc-500" />
        </div>
      </div>
    );
  }

  if (kind === "window") {
    return (
      <div className="flex h-20 w-48 flex-col overflow-hidden rounded-xl border border-line bg-surface">
        <div className="flex h-7 items-center justify-between border-b border-line bg-surface-subtle px-3">
          <span className="text-xs font-medium text-ink-soft">Dokumen</span>
          <div className="flex items-center gap-1.5">
            <span className="size-2.5 rounded-full bg-window-minimize" />
            <span className="size-2.5 rounded-full bg-window-zoom" />
            <span className="size-2.5 rounded-full bg-window-close" />
          </div>
        </div>
        <div className="grid gap-1.5 px-3 pt-2.5">
          <span className="block h-2 w-24 rounded-sm bg-line" />
          <span className="block h-2 w-16 rounded-sm bg-line" />
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-20 w-48 items-start justify-center rounded-xl border border-line bg-sky-mid">
      <div className="flex h-8 w-32 items-center justify-center gap-2 rounded-b-xl bg-black px-3 text-xs font-medium text-shell-ink">
        <span className="size-2 rounded-full bg-accent" />
        <span className="tabular-nums">24:12</span>
        <span className="flex h-3 items-end gap-0.5">
          <i className="h-1.5 w-0.5 bg-shell-ink" />
          <i className="h-2.5 w-0.5 bg-shell-ink" />
          <i className="h-2 w-0.5 bg-shell-ink" />
        </span>
      </div>
    </div>
  );
}

export default function Benefits() {
  return (
    <div className="grid w-full gap-8 lg:grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)] lg:gap-16">
      <div className="text-left lg:sticky lg:top-28 lg:self-start">
        <h2 className="max-w-md text-2xl leading-tight font-medium tracking-[-0.02em] text-ink sm:text-3xl">
          Empat bagian utama di desktop Windows
        </h2>
        <p className="mt-3 max-w-md text-sm leading-relaxed text-ink-soft sm:text-base">
          Status sistem, perpindahan aplikasi, kontrol jendela, hingga alat fokus tersusun di tepi layar tanpa menumpuk.
        </p>
      </div>
      <ul className="divide-y divide-line border-y border-line">
        {features.map((feature) => (
          <li
            key={feature.title}
            className="grid gap-4 py-6 text-left sm:grid-cols-[minmax(0,1fr)_192px] sm:items-center sm:gap-6"
          >
            <div>
              <h3 className="text-base font-medium text-ink">{feature.title}</h3>
              <p className="mt-1.5 max-w-md text-sm leading-relaxed text-ink-soft">
                {feature.description}
              </p>
            </div>
            <div aria-hidden="true">
              <Preview kind={feature.preview} />
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}
