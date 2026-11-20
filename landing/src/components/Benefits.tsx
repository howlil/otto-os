import {
  BatteryFull,
  Bell,
  Bluetooth,
  ChevronDown,
  ChevronRight,
  Code2,
  Copy,
  Download,
  FileText,
  Folder,
  Image as ImageIcon,
  Link2,
  Music2,
  Pause,
  Play,
  Search,
  SlidersHorizontal,
  Sun,
  Trash2,
  Volume2,
  Wifi,
} from "lucide-react";

const features = [
  {
    kind: "bar",
    name: "Bar atas & Control Center",
    headline: "Semua yang penting, tepat di atas",
    description:
      "Nama aplikasi aktif, menu, dan status sistem hadir dalam satu bar yang tenang. Control Center membuka pengaturan seperti Wi-Fi, suara, dan kecerahan saat Anda membutuhkannya.",
    details: ["Menu aplikasi aktif", "Status & notifikasi", "Kontrol cepat"],
  },
  {
    kind: "island",
    name: "Dynamic Island",
    headline: "Yang berlangsung, tetap terlihat",
    description:
      "Timer fokus dan musik tampil ringkas di notch. Saat keduanya aktif, satu menjadi aktivitas utama dan satu lagi tetap terlihat di lingkaran samping; klik untuk bertukar.",
    details: ["Pomodoro", "Musik & lirik opsional", "Dua aktivitas sekaligus"],
  },
  {
    kind: "dock",
    name: "Dock & efek Genie",
    headline: "Perpindahan aplikasi punya arah",
    description:
      "Dock menandai aplikasi yang berjalan, membesarkan ikon saat dituju, dan menampung Downloads serta Recycle Bin. Efek Genie mengikuti jendela yang diperkecil menuju ikon Dock.",
    details: ["Aplikasi berjalan", "Downloads & Recycle Bin", "Genie minimize"],
  },
  {
    kind: "window",
    name: "Kontrol jendela",
    headline: "Gerakan kecil yang terasa konsisten",
    description:
      "Minimize, zoom, dan tutup hadir di pojok kanan atas jendela yang didukung. Jendela sistem atau aplikasi yang terlindungi tetap menggunakan kontrol bawaan Windows.",
    details: ["Minimize", "Zoom / fullscreen", "Tutup"],
  },
  {
    kind: "tools",
    name: "Pocket & Clip",
    headline: "Yang sementara, tetap dekat",
    description:
      "Pocket menahan berkas, tautan, dan teks yang Anda seret ke notch. Clip menyimpan hingga 12 salinan terakhir sementara di RAM, dengan pilihan untuk menjeda atau menghapus riwayat.",
    details: ["Pocket: seret & lepas", "Clip: hanya di RAM", "Format sensitif dilewati"],
  },
] as const;

type PreviewKind = (typeof features)[number]["kind"];

function Preview({ kind }: { kind: PreviewKind }) {
  if (kind === "bar") {
    return (
      <div className="showcase-scene showcase-scene--bar" data-showcase-scene="bar" aria-label="Preview interaktif Bar atas dan Control Center">
        <img className="showcase-wallpaper" src="/hero-bg.webp" alt="" aria-hidden="true" loading="lazy" decoding="async" />
        <div className="showcase-menubar">
          <div className="showcase-menubar__start">
            <img src="/favicon.svg" alt="" aria-hidden="true" />
            <strong>Explorer</strong>
            <span>File</span><span>Edit</span><span className="showcase-hide-mobile">View</span><span className="showcase-hide-mobile">Window</span><span className="showcase-hide-mobile">Help</span>
          </div>
          <div className="showcase-menubar__end">
            <span className="showcase-system-slot" aria-hidden="true"><ChevronDown /></span>
            <span className="showcase-system-slot" aria-hidden="true"><Wifi /></span>
            <span className="showcase-system-slot showcase-hide-mobile" aria-hidden="true"><Volume2 /></span>
            <span className="showcase-system-slot showcase-hide-mobile" aria-hidden="true"><BatteryFull /></span>
            <button type="button" aria-label="Search preview" data-showcase-action="search"><Search /></button>
            <button type="button" aria-label="Tutup Control Center" aria-expanded="true" data-showcase-action="cc-toggle"><SlidersHorizontal /></button>
            <button type="button" aria-label="Notifications preview" data-showcase-action="notifications"><Bell /></button>
            <span className="showcase-clock">14:02</span>
          </div>
        </div>
        <div className="showcase-desktop-window" aria-hidden="true">
          <div className="showcase-desktop-window__top"><Folder /> Home <Search /></div>
          <div className="showcase-desktop-window__body">
            <strong>Quick access</strong>
            <span><Folder /> Projects</span><span><FileText /> Documents</span><span><Download /> Downloads</span>
          </div>
        </div>
        <section className="showcase-control-center" aria-label="Control Center" data-showcase-cc>
          <div className="showcase-control-center__heading">
            <strong>Control Center</strong>
            <button type="button" aria-label="Tutup Control Center" data-showcase-action="cc-close"><SlidersHorizontal /></button>
          </div>
          <div className="showcase-control-center__layout">
            <div className="showcase-control-center__connectivity">
              <button type="button" className="showcase-cc-toggle is-on" aria-pressed="true" data-showcase-action="wifi">
                <Wifi /><span><strong>Wi-Fi</strong><small data-showcase-label="wifi">Connected</small></span>
              </button>
              <button type="button" className="showcase-cc-toggle is-on" aria-pressed="true" data-showcase-action="bluetooth">
                <Bluetooth /><span><strong>Bluetooth</strong><small data-showcase-label="bluetooth">On</small></span>
              </button>
            </div>
            <div className="showcase-control-center__quick">
              <button type="button" className="showcase-cc-quick is-on" aria-pressed="true" data-showcase-action="focus"><span className="showcase-focus-dot" /><span><strong>Focus</strong><small data-showcase-label="focus">On</small></span></button>
              <button type="button" className="showcase-cc-quick is-on" aria-pressed="true" data-showcase-action="battery"><BatteryFull /><span><strong>Battery</strong><small data-showcase-label="battery">Balanced</small></span></button>
            </div>
            <label className="showcase-control-center__slider"><span><Sun /> Display <strong data-showcase-value="brightness">72%</strong></span><input type="range" min="0" max="100" value="72" aria-label="Kecerahan layar" data-showcase-range="brightness" /></label>
            <label className="showcase-control-center__slider"><span><Volume2 /> Sound <strong data-showcase-value="volume">58%</strong></span><input type="range" min="0" max="100" value="58" aria-label="Volume suara" data-showcase-range="volume" /></label>
            <div className="showcase-control-center__media">
              <Music2 /><span><strong>Slow Morning</strong><small>Otto Sessions</small></span>
              <button type="button" aria-label="Jeda musik demo" data-showcase-action="media-toggle"><span data-demo-pause><Pause /></span><span data-demo-play hidden><Play /></span></button>
            </div>
            <button type="button" className="showcase-control-center__settings" data-showcase-action="settings"><SlidersHorizontal /> Otto Modules &amp; Settings <ChevronRight /></button>
          </div>
          <p className="showcase-interaction-status" role="status" aria-live="polite" data-showcase-status>Kontrol demo hanya mengubah preview ini.</p>
        </section>
      </div>
    );
  }

  if (kind === "island") {
    return (
      <div className="showcase-scene showcase-scene--island" data-showcase-scene="island" data-activity="focus" aria-label="Preview interaktif Dynamic Island">
        <img className="showcase-wallpaper" src="/hero-bg.webp" alt="" aria-hidden="true" loading="lazy" decoding="async" />
        <div className="showcase-island-bezel" />
        <div className="showcase-notch" data-showcase-notch>
          <button type="button" className="showcase-notch-compact" aria-expanded="false" aria-label="Buka detail aktivitas Focus" data-showcase-action="notch-toggle">
            <span className="showcase-compact-focus"><i /><strong>24:12</strong><small>Focus</small></span>
            <span className="showcase-compact-music"><span className="showcase-mini-album"><Music2 /></span><strong>Slow Morning</strong><i aria-hidden="true"><b /><b /><b /><b /></i></span>
          </button>
          <section className="showcase-notch-panel" aria-label="Panel aktivitas" data-showcase-panel hidden>
            <div className="showcase-notch-panel__header" role="tablist" aria-label="Aktivitas">
              <button type="button" role="tab" aria-selected="true" data-showcase-tab="focus">Focus</button>
              <button type="button" role="tab" aria-selected="false" data-showcase-tab="music">Music</button>
              <small data-showcase-activity-label>Session 1 of 4</small>
              <button type="button" className="showcase-notch-collapse" aria-label="Tutup detail aktivitas" data-showcase-action="notch-toggle">×</button>
            </div>
            <div className="showcase-notch-panel__body" data-showcase-panel-body="focus">
              <span className="showcase-notch-ring"><i>25</i></span>
              <span className="showcase-notch-copy"><strong>24:12</strong><small>Deep focus · Stay in flow</small></span>
              <span className="showcase-notch-transport">
                <button type="button" aria-label="Jeda timer demo" data-showcase-action="notch-play"><span data-demo-pause><Pause /></span><span data-demo-play hidden><Play /></span></button>
                <button type="button" aria-label="Lewati sesi demo" data-showcase-action="notch-skip"><ChevronRight /></button>
              </span>
            </div>
            <div className="showcase-notch-panel__body" data-showcase-panel-body="music" hidden>
              <span className="showcase-album"><Music2 /></span>
              <span className="showcase-notch-copy"><strong>Slow Morning</strong><small>Otto Sessions · Demo audio</small></span>
              <span className="showcase-notch-transport">
                <button type="button" aria-label="Jeda musik demo" data-showcase-action="notch-play"><span data-demo-pause><Pause /></span><span data-demo-play hidden><Play /></span></button>
                <button type="button" aria-label="Lagu demo berikutnya" data-showcase-action="notch-skip"><ChevronRight /></button>
              </span>
            </div>
            <div className="showcase-notch-progress"><i /><i /><i /><i /></div>
          </section>
        </div>
        <button type="button" className="showcase-island-secondary" aria-label="Tukar aktivitas utama ke Music" data-showcase-action="notch-secondary"><Music2 /></button>
        <p className="showcase-island-caption">Klik notch untuk membuka detail. Lingkaran samping menukar aktivitas.</p>
        <p className="showcase-interaction-status" role="status" aria-live="polite" data-showcase-status>Focus aktif; Music menunggu di samping.</p>
      </div>
    );
  }

  if (kind === "dock") {
    return (
      <div className="showcase-scene showcase-scene--dock" data-showcase-scene="dock" aria-label="Preview interaktif Dock dan Genie">
        <img className="showcase-wallpaper" src="/hero-bg.webp" alt="" aria-hidden="true" loading="lazy" decoding="async" />
        <div className="showcase-dock-window" data-showcase-window>
          <div className="showcase-dock-window__top">
            <span><Code2 /> <strong data-showcase-window-title>Otto OS</strong></span>
            <span className="showcase-window-controls">
              <button type="button" className="is-minimize" aria-label="Minimalkan jendela demo ke Dock" data-showcase-action="dock-minimize" />
              <button type="button" className="is-zoom" aria-label="Perbesar jendela demo" data-showcase-action="dock-zoom" />
              <button type="button" className="is-close" aria-label="Tutup jendela demo" data-showcase-action="dock-close" />
            </span>
          </div>
          <div className="showcase-dock-window__body"><span>crates</span><span>landing</span><span>README.md</span></div>
        </div>
        <button type="button" className="showcase-dock-restore" data-showcase-action="dock-restore" hidden>Buka kembali jendela demo</button>
        <div className="showcase-dockbar" aria-label="Dock demo">
          <button type="button" className="showcase-dockbar__item showcase-dockbar__item--folder is-running" aria-label="Buka Explorer demo" aria-pressed="false" data-showcase-app="Explorer"><Folder /></button>
          <button type="button" className="showcase-dockbar__item showcase-dockbar__item--browser is-running" aria-label="Buka Browser demo" aria-pressed="false" data-showcase-app="Browser"><Search /></button>
          <button type="button" className="showcase-dockbar__item showcase-dockbar__item--code is-running is-selected" aria-label="Buka Code demo" aria-pressed="true" data-showcase-app="Code"><Code2 /></button>
          <button type="button" className="showcase-dockbar__item showcase-dockbar__item--music" aria-label="Buka Music demo" aria-pressed="false" data-showcase-app="Music"><Music2 /></button>
          <span className="showcase-dockbar__divider" aria-hidden="true" />
          <button type="button" className="showcase-dockbar__item showcase-dockbar__item--neutral" aria-label="Lihat Downloads demo" data-showcase-action="dock-stack"><Download /></button>
          <button type="button" className="showcase-dockbar__item showcase-dockbar__item--neutral" aria-label="Lihat Recycle Bin demo" data-showcase-action="dock-stack"><Trash2 /></button>
        </div>
        <p className="showcase-interaction-status" role="status" aria-live="polite" data-showcase-status>Hover atau fokuskan ikon untuk magnifikasi Dock.</p>
      </div>
    );
  }

  if (kind === "window") {
    return (
      <div className="showcase-scene showcase-scene--window" data-showcase-scene="window" aria-label="Preview interaktif kontrol jendela">
        <section className="showcase-window-detail" data-showcase-window>
          <div className="showcase-window-detail__top">
            <span><Folder /> <strong>File Explorer</strong></span>
            <span className="showcase-window-controls">
              <button type="button" className="is-minimize" aria-label="Minimalkan jendela demo" data-showcase-action="window-minimize" />
              <button type="button" className="is-zoom" aria-label="Perbesar jendela demo" data-showcase-action="window-zoom" />
              <button type="button" className="is-close" aria-label="Tutup jendela demo" data-showcase-action="window-close" />
            </span>
          </div>
          <div className="showcase-window-detail__toolbar"><span>Home</span><Search /> Search Home</div>
          <div className="showcase-window-detail__content">
            <strong>Recent files</strong>
            <div><FileText /> Proposal Otto OS <span>Today</span></div>
            <div><ImageIcon /> Product preview <span>Yesterday</span></div>
            <div><Folder /> Design assets <span>Monday</span></div>
          </div>
        </section>
        <button type="button" className="showcase-window-restore" data-showcase-action="window-restore" hidden>Buka kembali jendela demo</button>
        <p className="showcase-interaction-status" role="status" aria-live="polite" data-showcase-status>Pilih kontrol jendela untuk melihat state-nya.</p>
      </div>
    );
  }

  return (
    <div className="showcase-scene showcase-scene--tools" data-showcase-scene="tools" aria-label="Preview interaktif Pocket dan Clip">
      <section className="showcase-tool showcase-tool--pocket">
        <div className="showcase-tool__heading">
          <span><strong>Pocket</strong><small>File Drop Shelf · Contoh lokal</small></span>
          <button type="button" data-showcase-action="pocket-add">Tambah contoh</button>
        </div>
        <div className="showcase-pocket-items" data-showcase-items="pocket">
          <article><FileText /><span><strong>Rancangan.pdf</strong><small>File</small></span></article>
          <article><Link2 /><span><strong>Referensi</strong><small>Link</small></span></article>
          <article data-showcase-extra hidden><ImageIcon /><span><strong>Contoh baru</strong><small>Demo lokal</small></span></article>
        </div>
        <div className="showcase-tool__footer">
          <small data-showcase-empty="pocket" hidden>Pocket demo kosong.</small>
          <button type="button" data-showcase-action="pocket-clear">Bersihkan demo</button>
        </div>
      </section>
      <section className="showcase-tool showcase-tool--clip">
        <div className="showcase-tool__heading">
          <span><strong>Clip</strong><small>Riwayat sementara · RAM</small></span>
          <button type="button" aria-pressed="true" data-showcase-action="clip-toggle">Live</button>
        </div>
        <div className="showcase-clip-items" data-showcase-items="clip">
          <article><Copy /><strong>Ringkasan konsep Otto OS</strong></article>
          <article><Link2 /><strong>github.com/howlil/otto-os</strong></article>
          <article><FileText /><strong>Catatan untuk rilis beta</strong></article>
        </div>
        <div className="showcase-tool__footer">
          <small data-showcase-empty="clip" hidden>Clip demo kosong.</small>
          <button type="button" data-showcase-action="clip-restore" hidden>Kembalikan contoh</button>
          <button type="button" data-showcase-action="clip-clear">Bersihkan demo</button>
        </div>
      </section>
      <p className="showcase-tools-status" role="status" aria-live="polite" data-showcase-status>Interaksi hanya mengubah contoh di halaman ini; tidak membaca file atau clipboard.</p>
    </div>
  );
}

export default function Benefits() {
  return (
    <div className="showcase">
      <header className="showcase-intro">
        <h2>Dari bar atas hingga Dock, setiap gerak punya tempat</h2>
        <p>Otto OS menyusun menu, aktivitas, jendela, dan hal-hal sementara di tepi desktop Windows 11. Jelajahi bagian yang membuatnya terasa berbeda.</p>
      </header>
      <div className="showcase-stories">
        {features.map((feature) => (
          <article className={"showcase-story showcase-story--" + feature.kind} key={feature.kind}>
            <div className="showcase-story__copy">
              <h3>{feature.headline}</h3>
              <p className="showcase-story__name">{feature.name}</p>
              <p className="showcase-story__description">{feature.description}</p>
              <ul className="showcase-story__details">
                {feature.details.map((detail) => <li key={detail}>{detail}</li>)}
              </ul>
            </div>
            <Preview kind={feature.kind} />
          </article>
        ))}
      </div>
      <p className="showcase-footnote">Preview interaktif; data di dalamnya hanya contoh.</p>
    </div>
  );
}
