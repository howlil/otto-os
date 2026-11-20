const scenes = document.querySelectorAll<HTMLElement>("[data-showcase-scene]");

function setDemoIcon(button: HTMLElement, playing: boolean) {
  button.querySelector<HTMLElement>("[data-demo-pause]")?.toggleAttribute("hidden", !playing);
  button.querySelector<HTMLElement>("[data-demo-play]")?.toggleAttribute("hidden", playing);
  const scene = button.closest<HTMLElement>("[data-showcase-scene]");
  const label = scene?.dataset.activity === "focus" ? "timer" : "musik";
  button.setAttribute("aria-label", (playing ? "Jeda " : "Putar ") + label + " demo");
}

function setSceneStatus(scene: HTMLElement, message: string) {
  const status = scene.querySelector<HTMLElement>("[data-showcase-status]");
  if (status) status.textContent = message;
}

function setIslandTab(scene: HTMLElement, tab: string) {
  scene.dataset.activity = tab;
  scene.querySelectorAll<HTMLElement>("[data-showcase-tab]").forEach((button) => {
    button.setAttribute("aria-selected", String(button.dataset.showcaseTab === tab));
  });
  scene.querySelectorAll<HTMLElement>("[data-showcase-panel-body]").forEach((panel) => {
    panel.hidden = panel.dataset.showcasePanelBody !== tab;
  });
  const label = scene.querySelector<HTMLElement>("[data-showcase-activity-label]");
  if (label) label.textContent = tab === "focus" ? "Session 1 of 4" : "Demo media";
  const secondary = scene.querySelector<HTMLElement>('[data-showcase-action="notch-secondary"]');
  if (secondary) secondary.setAttribute("aria-label", "Tukar aktivitas utama ke " + (tab === "focus" ? "Music" : "Focus"));
  const toggle = scene.querySelector<HTMLElement>('[data-showcase-action="notch-toggle"]');
  if (toggle) toggle.setAttribute("aria-label", (tab === "focus" ? "Focus" : "Music") + " aktif. Buka detail aktivitas");
}

function setIslandExpanded(scene: HTMLElement, expanded: boolean) {
  const notch = scene.querySelector<HTMLElement>("[data-showcase-notch]");
  const panel = scene.querySelector<HTMLElement>("[data-showcase-panel]");
  const toggle = scene.querySelector<HTMLElement>('[data-showcase-action="notch-toggle"]');
  const secondary = scene.querySelector<HTMLElement>('[data-showcase-action="notch-secondary"]');
  if (!notch || !panel || !toggle) return;
  notch.classList.toggle("is-expanded", expanded);
  panel.hidden = !expanded;
  toggle.hidden = expanded;
  toggle.setAttribute("aria-expanded", String(expanded));
  if (secondary) secondary.hidden = expanded;
}

function setItemsHidden(scene: HTMLElement, type: string, hidden: boolean) {
  const items = scene.querySelectorAll<HTMLElement>('[data-showcase-items="' + type + '"] article');
  items.forEach((item) => {
    if (!item.hasAttribute("data-showcase-extra")) item.hidden = hidden;
  });
  const empty = scene.querySelector<HTMLElement>('[data-showcase-empty="' + type + '"]');
  if (empty) empty.hidden = !hidden;
}

scenes.forEach((scene) => {
  scene.addEventListener("click", (event) => {
    const target = event.target;
    if (!(target instanceof Element)) return;
    const control = target.closest<HTMLElement>("[data-showcase-action], [data-showcase-tab], [data-showcase-app]");
    if (!control || !scene.contains(control)) return;

    const action = control.dataset.showcaseAction;
    const sceneName = scene.dataset.showcaseScene;

    if (control.dataset.showcaseTab) {
      setIslandTab(scene, control.dataset.showcaseTab);
      setSceneStatus(scene, control.dataset.showcaseTab === "focus" ? "Focus demo aktif." : "Music demo aktif.");
      return;
    }

    if (control.dataset.showcaseApp) {
      const windowElement = scene.querySelector<HTMLElement>("[data-showcase-window]");
      const title = scene.querySelector<HTMLElement>("[data-showcase-window-title]");
      const restore = scene.querySelector<HTMLElement>('[data-showcase-action="dock-restore"]');
      if (windowElement) {
        windowElement.hidden = false;
        windowElement.classList.remove("is-genie-minimizing", "is-maximized");
      }
      if (title) title.textContent = control.dataset.showcaseApp;
      if (restore) restore.hidden = true;
      scene.querySelectorAll<HTMLElement>("[data-showcase-app]").forEach((app) => {
        app.classList.toggle("is-selected", app === control);
        app.setAttribute("aria-pressed", String(app === control));
      });
      setSceneStatus(scene, control.dataset.showcaseApp + " demo dibuka kembali dari Dock.");
      return;
    }

    if (action === "cc-toggle" || action === "cc-close") {
      const panel = scene.querySelector<HTMLElement>("[data-showcase-cc]");
      const trigger = scene.querySelector<HTMLElement>('[data-showcase-action="cc-toggle"]');
      if (panel && trigger) {
        panel.hidden = action === "cc-close" || !panel.hidden;
        trigger.setAttribute("aria-expanded", String(!panel.hidden));
        trigger.setAttribute("aria-label", panel.hidden ? "Buka Control Center" : "Tutup Control Center");
      }
      setSceneStatus(scene, panel?.hidden ? "Control Center ditutup." : "Control Center demo terbuka.");
      return;
    }

    if (action === "wifi" || action === "bluetooth" || action === "focus" || action === "battery") {
      const enabled = control.getAttribute("aria-pressed") !== "true";
      control.setAttribute("aria-pressed", String(enabled));
      control.classList.toggle("is-on", enabled);
      const label = scene.querySelector<HTMLElement>('[data-showcase-label="' + action + '"]');
      if (label) {
        const names: Record<string, [string, string]> = {
          wifi: ["Connected", "Off"],
          bluetooth: ["On", "Off"],
          focus: ["On", "Off"],
          battery: ["Balanced", "Saver"],
        };
        label.textContent = enabled ? names[action][0] : names[action][1];
      }
      setSceneStatus(scene, (action === "wifi" ? "Wi-Fi" : action === "bluetooth" ? "Bluetooth" : action === "focus" ? "Focus" : "Battery") + (enabled ? " demo aktif." : " demo berubah."));
      return;
    }

    if (action === "media-toggle" || action === "notch-play") {
      const pauseIcon = control.querySelector<HTMLElement>("[data-demo-pause]");
      const playing = Boolean(pauseIcon && !pauseIcon.hasAttribute("hidden"));
      setDemoIcon(control, !playing);
      setSceneStatus(scene, !playing ? "Demo media diputar." : "Demo media dijeda.");
      return;
    }

    if (action === "notch-toggle") {
      const expanded = scene.querySelector<HTMLElement>("[data-showcase-panel]")?.hidden ?? true;
      setIslandExpanded(scene, expanded);
      if (expanded) setSceneStatus(scene, "Panel aktivitas demo dibuka.");
      return;
    }

    if (action === "notch-secondary") {
      const next = scene.dataset.activity === "focus" ? "music" : "focus";
      setIslandTab(scene, next);
      setSceneStatus(scene, "Aktivitas utama ditukar ke " + (next === "focus" ? "Focus" : "Music") + ".");
      return;
    }

    if (action === "notch-skip") {
      setSceneStatus(scene, scene.dataset.activity === "focus" ? "Sesi istirahat demo dipilih." : "Track demo berikutnya dipilih.");
      return;
    }

    if (action === "dock-minimize" || action === "window-minimize") {
      const windowElement = scene.querySelector<HTMLElement>("[data-showcase-window]");
      if (windowElement) windowElement.classList.add("is-genie-minimizing");
      const restore = scene.querySelector<HTMLElement>('[data-showcase-action="' + (sceneName === "dock" ? "dock-restore" : "window-restore") + '"]');
      if (restore) restore.hidden = false;
      setSceneStatus(scene, "Jendela demo diperkecil ke Dock.");
      return;
    }

    if (action === "dock-zoom" || action === "window-zoom") {
      const windowElement = scene.querySelector<HTMLElement>("[data-showcase-window]");
      const maximized = windowElement?.classList.toggle("is-maximized") ?? false;
      control.setAttribute("aria-label", maximized ? "Pulihkan ukuran jendela demo" : "Perbesar jendela demo");
      setSceneStatus(scene, maximized ? "Jendela demo diperbesar." : "Ukuran jendela demo dipulihkan.");
      return;
    }

    if (action === "dock-close" || action === "window-close") {
      const windowElement = scene.querySelector<HTMLElement>("[data-showcase-window]");
      const restore = scene.querySelector<HTMLElement>('[data-showcase-action="' + (sceneName === "dock" ? "dock-restore" : "window-restore") + '"]');
      if (windowElement) windowElement.hidden = true;
      if (restore) restore.hidden = false;
      setSceneStatus(scene, "Jendela demo ditutup. Buka kembali lewat tombol di preview.");
      return;
    }

    if (action === "dock-restore" || action === "window-restore") {
      const windowElement = scene.querySelector<HTMLElement>("[data-showcase-window]");
      const restore = scene.querySelector<HTMLElement>('[data-showcase-action="' + action + '"]');
      if (windowElement) {
        windowElement.hidden = false;
        windowElement.classList.remove("is-genie-minimizing", "is-maximized");
      }
      if (restore) restore.hidden = true;
      setSceneStatus(scene, "Jendela demo dibuka kembali.");
      return;
    }

    if (action === "dock-stack") {
      setSceneStatus(scene, "Downloads dan Recycle Bin ditampilkan sebagai contoh Dock.");
      return;
    }

    if (action === "settings") {
      setSceneStatus(scene, "Otto Modules & Settings ditampilkan sebagai contoh.");
      return;
    }

    if (action === "search") {
      setSceneStatus(scene, "Search ditampilkan sebagai contoh bar atas.");
      return;
    }

    if (action === "notifications") {
      setSceneStatus(scene, "Notifications ditampilkan sebagai contoh bar atas.");
      return;
    }

    if (action === "pocket-add") {
      setItemsHidden(scene, "pocket", false);
      const extra = scene.querySelector<HTMLElement>("[data-showcase-extra]");
      if (extra) extra.hidden = false;
      const empty = scene.querySelector<HTMLElement>('[data-showcase-empty="pocket"]');
      if (empty) empty.hidden = true;
      setSceneStatus(scene, "Satu contoh lokal ditambahkan ke Pocket preview.");
      return;
    }

    if (action === "pocket-clear" || action === "clip-clear") {
      const type = action === "pocket-clear" ? "pocket" : "clip";
      setItemsHidden(scene, type, true);
      if (type === "pocket") {
        const extra = scene.querySelector<HTMLElement>("[data-showcase-extra]");
        if (extra) extra.hidden = true;
      } else {
        const restore = scene.querySelector<HTMLElement>('[data-showcase-action="clip-restore"]');
        const clear = scene.querySelector<HTMLElement>('[data-showcase-action="clip-clear"]');
        if (restore) restore.hidden = false;
        if (clear) clear.hidden = true;
      }
      setSceneStatus(scene, (type === "pocket" ? "Pocket" : "Clip") + " demo dibersihkan.");
      return;
    }

    if (action === "clip-restore") {
      setItemsHidden(scene, "clip", false);
      const restore = scene.querySelector<HTMLElement>('[data-showcase-action="clip-restore"]');
      const clear = scene.querySelector<HTMLElement>('[data-showcase-action="clip-clear"]');
      if (restore) restore.hidden = true;
      if (clear) clear.hidden = false;
      setSceneStatus(scene, "Contoh Clip dipulihkan.");
      return;
    }

    if (action === "clip-toggle") {
      const live = control.getAttribute("aria-pressed") === "true";
      control.setAttribute("aria-pressed", String(!live));
      control.textContent = live ? "Paused" : "Live";
      setSceneStatus(scene, live ? "Clip demo dijeda." : "Clip demo dilanjutkan.");
    }
  });

  scene.addEventListener("input", (event) => {
    const target = event.target;
    if (!(target instanceof HTMLInputElement) || !target.dataset.showcaseRange) return;
    const output = scene.querySelector<HTMLElement>('[data-showcase-value="' + target.dataset.showcaseRange + '"]');
    if (output) output.textContent = target.value + "%";
    target.style.setProperty("--range-value", target.value + "%");
  });

  scene.querySelectorAll<HTMLInputElement>("[data-showcase-range]").forEach((input) => {
    input.style.setProperty("--range-value", input.value + "%");
  });

  scene.querySelectorAll<HTMLElement>("[data-showcase-window]").forEach((windowElement) => {
    windowElement.addEventListener("animationend", (event) => {
      if (event.target === windowElement && windowElement.classList.contains("is-genie-minimizing")) {
        windowElement.hidden = true;
        windowElement.classList.remove("is-genie-minimizing");
      }
    });
  });
});
