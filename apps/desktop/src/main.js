const hasTauri = !!window.__TAURI__;
const invoke = hasTauri ? window.__TAURI__.core.invoke : null;

const $ = (id) => document.getElementById(id);
const forcedSetup = () =>
  location.search.includes("setup") || localStorage.getItem("setupPreview") === "1";
let state = null;
let saveTimer = null;

function toast(text, bad = false) {
  const el = $("toast");
  el.textContent = text;
  el.classList.toggle("bad", bad);
  el.classList.add("show");
  clearTimeout(toast._t);
  toast._t = setTimeout(() => el.classList.remove("show"), 3200);
}

/* Browser preview (`python3 -m http.server` in this folder): mock data so the
   UI can be styled without starting the engine. The desktop app always uses
   the real commands. */
const mockState = {
  version: "0.1.0",
  config_path: "~/.config/s2x3/config.toml",
  engine_id: "ps2-rust",
  engine_name: "SSX 3 (PlayStation 2, Rust port)",
  engine_root: "/home/player/Projects/ssx3/ssx3-decomp",
  iso: "/run/media/disc/SSX 3 (USA).iso",
  saves: "/home/player/Games/SSX3-ps2/saves",
  binary: { state: "ready", detail: "target/game/ssx3" },
  disc: { state: "ready", detail: "SLUS_207.72, 2.8 GB" },
  notes: [],
  capabilities: {
    platforms: ["linux", "windows", "macos"],
    needs_disc: true,
    plugins: true,
    netplay: false,
    splitscreen: true,
    max_tick_hz: 240,
    notes: [],
  },
  schema: [
    { key: "SSX3_TICK_HZ", label: "Updates per second", help: "60 is the original; 120/144/240 for fast displays.", kind: "int", min: 60, max: 240, default: "60", choices: [] },
    { key: "SSX3_RENDER_SCALE", label: "Internal resolution", help: "N times the game's resolution (1-8).", kind: "int", min: 1, max: 8, default: "1", choices: [] },
    { key: "SSX3_ASPECT", label: "Aspect", help: "Fixed aspect instead of following the window.", kind: "choice", choices: ["", "4:3", "16:9", "stretch"], default: "", min: null, max: null },
    { key: "SSX3_FOV", label: "Field of view", help: "Relative to the game's FOV (1.2 = 20% wider).", kind: "text", default: "1.0", choices: [], min: null, max: null },
    { key: "SSX3_ANISO", label: "Anisotropic filtering", help: "1 (off) to 16, for mipmapped textures.", kind: "int", min: 1, max: 16, default: "16", choices: [] },
    { key: "SSX3_SKIP_MOVIES", label: "Skip intro movies", help: "Skip the EA, THX and intro movies.", kind: "bool", default: "0", choices: [], min: null, max: null },
    { key: "SSX3_FULLSCREEN", label: "Fullscreen", help: "Start borderless (F11 toggles).", kind: "bool", default: "0", choices: [], min: null, max: null },
    { key: "SSX3_AA", label: "Anti-aliasing", help: "Modern renderer anti-aliasing.", kind: "bool", default: "0", choices: [], min: null, max: null },
    { key: "SSX3_AO", label: "Ambient occlusion", help: "Modern renderer ambient occlusion.", kind: "bool", default: "0", choices: [], min: null, max: null },
    { key: "SSX3_BLOOM", label: "Bloom", help: "Modern renderer bloom.", kind: "bool", default: "0", choices: [], min: null, max: null },
    { key: "SSX3_SHADOWS", label: "Real-time lighting", help: "Experimental: sun shadows from the renderer.", kind: "bool", default: "0", choices: [], min: null, max: null },
    { key: "SSX3_PAD2", label: "Second controller", help: "Plug in pad 2 for Multi Play.", kind: "bool", default: "0", choices: [], min: null, max: null },
    { key: "SSX3_STICK_TRICKS", label: "Trick Spin Controls", help: "Cross Pad is the original scheme. Left Stick or Right Stick also presses the d-pad.", kind: "choice", choices: ["", "left", "right"], choice_labels: ["Cross Pad (default)", "Left Stick", "Right Stick"], default: "", min: null, max: null },
  ],
  values: {
    SSX3_TICK_HZ: "60",
    SSX3_RENDER_SCALE: "2",
    SSX3_ASPECT: "16:9",
    SSX3_FOV: "1.0",
    SSX3_ANISO: "16",
    SSX3_SKIP_MOVIES: "1",
    SSX3_FULLSCREEN: "1",
    SSX3_AA: "1",
    SSX3_AO: "0",
    SSX3_BLOOM: "0",
    SSX3_SHADOWS: "0",
    SSX3_PAD2: "0",
  },
  buttons: "xbox",
  texreplace: "/home/player/Games/SSX3-ps2/texreplace",
  texreplace_present: true,
  log_path: "…/saves/launcher.log",
};

function mockInvoke(cmd, args) {
  if (cmd === "state") {
    if (location.search.includes("setup")) {
      return Promise.resolve({
        ...structuredClone(mockState),
        binary: { state: "missing", detail: "target/game/ssx3" },
        disc: { state: "missing", detail: "no disc image" },
      });
    }
    return Promise.resolve(structuredClone(mockState));
  }
  if (cmd === "choose_iso" || cmd === "set_iso") {
    mockState.iso = "/run/media/disc/SSX 3 (USA).iso";
    mockState.disc = { state: "ready", detail: "SLUS_207.72, 2.8 GB" };
    return Promise.resolve(structuredClone(mockState));
  }
  if (cmd === "bootstrap_engine" || cmd === "update_engine" || cmd === "install")
    return Promise.resolve(777);
  if (cmd === "check_engine_update")
    return Promise.resolve({
      available: true,
      local: "321776e",
      remote: "9f31c20",
      detail: "a newer engine commit is available",
    });
  if (cmd === "install_log_tail")
    return Promise.resolve("[setup] cloning ssx3-decomp…\n[setup] cargo build --profile game -p ssx3\n");
  if (cmd === "launch") return Promise.resolve(4242);
  if (cmd === "install") return Promise.resolve(777);
  if (cmd === "saves_info")
    return Promise.resolve({
      path: "/home/player/Games/SSX3-ps2/saves/ssx3-mc0.ps2",
      exists: true,
      size: 8650752,
    });
  if (cmd === "export_saves") return Promise.resolve("/home/player/Desktop/ssx3-mc0.ps2");
  if (cmd === "import_saves") return Promise.resolve("/home/player/Games/SSX3-ps2/saves/ssx3-mc0.ps2");
  if (cmd === "log_tail") return Promise.resolve("[ssx3] 8082 hand-written functions installed\n[window] display 60 Hz, game 60 Hz");
  if (cmd === "set_buttons") {
    mockState.buttons = args.value;
    return Promise.resolve(structuredClone(mockState));
  }
  if (cmd === "save_settings") {
    mockState.values = args.values;
    return Promise.resolve(structuredClone(mockState));
  }
  return Promise.resolve(structuredClone(mockState));
}

async function call(cmd, args) {
  try {
    return await (invoke ? invoke(cmd, args) : mockInvoke(cmd, args));
  } catch (e) {
    toast(String(e), true);
    throw e;
  }
}

/* Xbox glyphs: the launcher's controller language. */
const XBOX_ICONS = {
  A: "assets/buttons/a.png",
  B: "assets/buttons/b.png",
  X: "assets/buttons/x.png",
  Y: "assets/buttons/y.png",
};

function glyph(button) {
  return `<img class="btn-glyph" src="${XBOX_ICONS[button]}" alt="${button}" />`;
}

function psGlyph(kind) {
  const shapes = {
    triangle: '<path d="M10 3.2 17 16H3z" fill="#3fbf5f" />',
    circle: '<circle cx="10" cy="10" r="6.6" fill="none" stroke="#e5484d" stroke-width="2.8" />',
    cross: '<path d="M4.2 4.2 15.8 15.8M15.8 4.2 4.2 15.8" stroke="#5b8fd6" stroke-width="3" />',
    square: '<rect x="3.6" y="3.6" width="12.8" height="12.8" rx="1.5" fill="none" stroke="#e88bb0" stroke-width="2.8" />',
  };
  return `<svg class="ps-glyph" viewBox="0 0 20 20" aria-hidden="true">${shapes[kind]}</svg>`;
}

function dpadGlyph() {
  return `<svg width="18" height="18" viewBox="0 0 20 20" aria-hidden="true">
    <rect x="7" y="1" width="6" height="6" rx="1" fill="#7c8ea0"/>
    <rect x="7" y="13" width="6" height="6" rx="1" fill="#7c8ea0"/>
    <rect x="1" y="7" width="6" height="6" rx="1" fill="#7c8ea0"/>
    <rect x="13" y="7" width="6" height="6" rx="1" fill="#7c8ea0"/>
  </svg>`;
}

function hints() {
  const ps = state?.buttons === "ps2";
  const play = ps ? psGlyph("cross") : glyph("A");
  const back = ps ? psGlyph("circle") : glyph("B");
  $("hints").innerHTML = `
    <span class="hint">${play} Play</span>
    <span class="hint">${dpadGlyph()} Navigate</span>
    <span class="hint">${back} Back (in game)</span>
    <span class="hint" style="color:#5f7d92">F11 fullscreen &middot; F10 settings in game</span>
  `;
}

function promptSet() {
  const ps2 = state.buttons === "ps2";
  const wrap = document.createElement("div");
  wrap.className = "prompt-set";
  wrap.innerHTML = `
    <div class="prompt-head">
      <b>Button prompts</b>
      <small>${
        state.texreplace_present
          ? "Xbox uses the texture patch in the texreplace folder."
          : "No texture patch found. In game prompts stay PlayStation 2."
      }</small>
    </div>
    <div class="prompt-cards">
      <button class="prompt-card ${ps2 ? "active" : ""}" data-set="ps2">
        <span class="glyphs">${psGlyph("triangle")}${psGlyph("circle")}${psGlyph("cross")}${psGlyph("square")}</span>
        <b>PlayStation 2</b>
        <small>original glyphs</small>
      </button>
      <button class="prompt-card ${ps2 ? "" : "active"}" data-set="xbox">
        <span class="glyphs">
          <img src="assets/buttons/x.png" alt="" />
          <img src="assets/buttons/b.png" alt="" />
          <img src="assets/buttons/a.png" alt="" />
          <img src="assets/buttons/y.png" alt="" />
        </span>
        <b>Xbox</b>
        <small>texture patch</small>
      </button>
    </div>`;
  wrap.querySelectorAll("[data-set]").forEach((btn) => {
    btn.addEventListener("click", async () => {
      const next = await call("set_buttons", { value: btn.dataset.set });
      render(next);
      toast(`Button prompts: ${btn.dataset.set.toUpperCase()}`);
    });
  });
  return wrap;
}

const TABS = [
  {
    id: "display",
    label: "display",
    keys: ["SSX3_TICK_HZ", "SSX3_ASPECT", "SSX3_FULLSCREEN", "SSX3_SKIP_MOVIES", "SSX3_REAL_CLOCK"],
  },
  {
    id: "graphics",
    label: "graphics",
    keys: ["SSX3_RENDER_SCALE", "SSX3_RENDER_SIZE", "SSX3_RESOLUTION", "SSX3_FOV", "SSX3_ANISO", "SSX3_LOD_BIAS", "SSX3_AA", "SSX3_AO", "SSX3_BLOOM", "SSX3_SHADOWS", "SSX3_SNOW"],
  },
  { id: "controls", label: "controls", keys: ["SSX3_STICK_TRICKS", "SSX3_PAD2"] },
  {
    id: "advanced",
    label: "advanced",
    keys: ["SSX3_CATCHUP", "SSX3_FAST_MATH", "SSX3_RENDER", "SSX3_SND", "SSX3_IOP_SIO2", "SSX3_NATIVE_FILES"],
  },
];
let activeTab = localStorage.getItem("settingsTab") || "display";

function field(f) {
  const row = document.createElement("div");
  row.className = "field";

  const label = document.createElement("div");
  label.className = "label";
  label.innerHTML = `<b>${escapeHtml(f.label)}</b><small>${escapeHtml(f.help)}</small>`;

  const control = document.createElement("div");
  const value = state.values[f.key] ?? f.default ?? "";

  if (f.kind === "bool") {
    control.innerHTML = `<label class="switch">
      <input type="checkbox" data-key="${f.key}" ${value && value !== "0" ? "checked" : ""}>
      <span></span></label>`;
  } else if (f.kind === "choice") {
    const options = f.choices
      .map((c, i) => {
        const label = f.choice_labels?.[i] ?? (c === "" ? "Auto" : c);
        return `<option value="${c}" ${c === value ? "selected" : ""}>${label}</option>`;
      })
      .join("");
    control.innerHTML = `<select data-key="${f.key}">${options}</select>`;
  } else if (f.kind === "int") {
    control.innerHTML = `<input type="number" data-key="${f.key}"
      min="${f.min ?? ""}" max="${f.max ?? ""}" value="${escapeAttr(value ?? "")}">`;
  } else {
    control.innerHTML = `<input type="text" data-key="${f.key}" spellcheck="false"
      value="${escapeAttr(value ?? "")}">`;
  }

  row.append(label, control);
  return row;
}

function escapeHtml(s) {
  return String(s ?? "").replace(/[&<>"']/g, (c) => ({
    "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;",
  })[c]);
}

function escapeAttr(s) {
  return escapeHtml(s).replace(/"/g, "&quot;");
}

function collect() {
  const values = {};
  document.querySelectorAll("[data-key]").forEach((el) => {
    const key = el.dataset.key;
    if (el.type === "checkbox") {
      values[key] = el.checked ? "1" : "0";
    } else {
      values[key] = el.value.trim();
    }
  });
  return values;
}

function render(next) {
  state = next;
  document.querySelectorAll(".prompt-set").forEach((el) => el.remove());

  const preview = forcedSetup();
  const engineReady = !preview && state.binary.state === "ready";
  const discReady = !preview && state.disc.state === "ready";
  const ready = engineReady && discReady;
  const chip = (id, ok, text) => {
    const el = $(id);
    el.classList.toggle("ok", ok);
    el.classList.toggle("bad", !ok);
    el.querySelector(".v").textContent = text;
  };
  chip("chip-engine", engineReady, engineReady ? "ready" : "missing");
  chip("chip-disc", discReady, discReady ? "disc ok" : "no disc");


  $("rig-engine").textContent = state.engine_root;
  $("rig-disc").textContent = state.disc.detail;
  $("rig-saves").textContent = state.saves;
  $("path-engine").value = state.engine_root;
  $("path-iso").value = state.iso;
  $("path-saves").value = state.saves;

  $("caps").innerHTML = [
    state.capabilities.plugins ? "Plugins" : null,
    state.capabilities.splitscreen ? "Splitscreen" : null,
    state.capabilities.netplay ? "Netplay" : null,
    state.capabilities.max_tick_hz > 60 ? `${state.capabilities.max_tick_hz} Hz` : null,
  ]
    .filter(Boolean)
    .map((c) => `<span>${c}</span>`)
    .join("");

  const tabs = $("tabs");
  tabs.innerHTML = "";
  TABS.forEach((t) => {
    const b = document.createElement("button");
    b.className = "tab" + (t.id === activeTab ? " active" : "");
    b.textContent = t.label;
    b.addEventListener("click", () => {
      activeTab = t.id;
      localStorage.setItem("settingsTab", t.id);
      render(state);
    });
    tabs.append(b);
  });

  const known = new Set(TABS.flatMap((t) => t.keys));
  const keys = new Set(TABS.find((t) => t.id === activeTab)?.keys ?? []);
  const fields = $("fields");
  if (activeTab === "controls") fields.before(promptSet());
  fields.innerHTML = "";
  state.schema
    .filter((f) => keys.has(f.key) || !known.has(f.key))
    .forEach((f) => fields.append(field(f)));

  fields.querySelectorAll("[data-key]").forEach((el) => {
    el.addEventListener("change", scheduleSave);
    if (el.tagName === "INPUT" && el.type === "text") {
      el.addEventListener("input", scheduleSave);
    }
  });

  $("setup").hidden = ready;
  $("settings-card").hidden = !ready;
  $("install-engine").hidden = engineReady;
  $("engine-note").hidden = engineReady;
  $("grid").classList.toggle("setup-mode", !ready);
  if (!engineReady) {
    $("update-engine").hidden = true;
    $("update-note").hidden = true;
  }
  $("drop-note").textContent = discReady
    ? "disc image selected"
    : "or drag the file anywhere into this window";
  $("drop").classList.toggle("has-disc", discReady);

  hints();
  $("play").disabled = !ready;
  if (engineReady && !checkEngineUpdate.done) {
    checkEngineUpdate.done = true;
    setTimeout(checkEngineUpdate, 400);
  }
  $("play-sub").textContent = ready
    ? "start the game"
    : state.binary.state !== "ready"
      ? "engine not built"
      : "choose a disc image";
}

function scheduleSave() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(saveSettings, 350);
}

async function saveSettings() {
  const values = collect();
  await call("save_settings", { values });
  state.values = values;
  $("settings-state").textContent = "saved";
  setTimeout(() => ($("settings-state").textContent = "stored in the engine's cfg"), 1800);
}

async function play() {
  const btn = $("play");
  btn.disabled = true;
  try {
    const pid = await call("launch", { overrides: null });
    toast(`Engine started (pid ${pid})`);
  } finally {
    setTimeout(() => (btn.disabled = false), 1200);
  }
}

async function refreshState() {
  render(await call("state"));
}

function human(bytes) {
  const u = ["B", "KB", "MB", "GB"];
  let v = bytes, i = 0;
  while (v >= 1024 && i < u.length - 1) { v /= 1024; i++; }
  return `${v.toFixed(i ? 1 : 0)} ${u[i]}`;
}

async function refreshSaves() {
  try {
    const info = await call("saves_info");
    $("saves-info").textContent = info.exists
      ? `${human(info.size)} · ${info.path}`
      : `no memory card yet · ${info.path}`;
  } catch (e) {
    $("saves-info").textContent = "save info unavailable";
  }
}

async function setIso(path) {
  try {
    const next = await call("set_iso", { iso: path });
    render(next);
    toast("Disc image set");
  } catch (e) {
    /* toast already shown */
  }
}

async function pollBuild() {
  $("setup-log").hidden = false;
  const timer = setInterval(async () => {
    try {
      const text = await call("install_log_tail", { lines: 40 });
      $("setup-log").textContent = text || "starting…";
      $("setup-log").scrollTop = $("setup-log").scrollHeight;
    } catch (e) {}
  }, 2000);
  setTimeout(async () => {
    clearInterval(timer);
    await refreshState();
    if ($("setup").hidden) toast("Engine ready");
  }, 30000);
}

async function checkEngineUpdate() {
  if (!state || state.binary.state !== "ready" || forcedSetup()) return;
  try {
    const u = await call("check_engine_update");
    const btn = $("update-engine");
    btn.hidden = !u.available;
    $("update-note").hidden = !u.available;
    if (u.available) {
      $("update-note").textContent = `Engine ${u.local} → ${u.remote}: ${u.detail}`;
    }
  } catch (e) {}
}

function bind() {
  hints();
  refreshSaves();
  setTimeout(checkEngineUpdate, 800);

  const search = "https://duckduckgo.com/?q=SSX+3+USA+PS2+iso";
  ["iso-search", "iso-search-2"].forEach((id) => {
    const el = $(id);
    if (!el) return;
    el.addEventListener("click", async (e) => {
      e.preventDefault();
      try {
        await call("open_url", { url: search });
      } catch (err) {
        toast(String(err), true);
      }
    });
  });

  $("browse-iso").addEventListener("click", async () => {
    try {
      render(await call("choose_iso"));
      toast("Disc image set");
    } catch (e) {
      if (!String(e).includes("cancelled")) toast(String(e), true);
    }
  });

  $("install-engine").addEventListener("click", async () => {
    const pid = await call("bootstrap_engine");
    toast(`Engine download started (pid ${pid})`);
    pollBuild();
  });

  $("update-engine").addEventListener("click", async () => {
    const pid = await call("update_engine");
    toast(`Engine update started (pid ${pid})`);
    pollBuild();
  });

  const drop = $("drop");
  const over = (on) => drop.classList.toggle("over", on);
  ["dragenter", "dragover"].forEach((name) =>
    document.addEventListener(name, (e) => {
      e.preventDefault();
      over(true);
    }),
  );
  ["dragleave", "drop"].forEach((name) =>
    document.addEventListener(name, () => over(false)),
  );
  document.addEventListener("drop", (e) => {
    e.preventDefault();
    const file = e.dataTransfer?.files?.[0];
    if (file) {
      const path = file.path || file.name;
      if (path) setIso(path);
    }
  });

  if (hasTauri) {
    const ev = window.__TAURI__.event;
    ev.listen("tauri://drag-enter", () => over(true));
    ev.listen("tauri://drag-leave", () => over(false));
    ev.listen("tauri://drag-drop", (e) => {
      over(false);
      const path = e.payload?.paths?.[0];
      if (path) setIso(path);
    });
  }

  $("export-saves").addEventListener("click", async () => {
    try {
      const path = await call("export_saves", { dest: null });
      toast(`Save exported to ${path}`);
    } catch (e) {
      if (!String(e).includes("cancelled")) toast(String(e), true);
    }
  });

  $("import-saves").addEventListener("click", async () => {
    try {
      const path = await call("import_saves", { src: null });
      toast(`Save imported into ${path}`);
      refreshSaves();
    } catch (e) {
      if (!String(e).includes("cancelled")) toast(String(e), true);
    }
  });
  $("play").addEventListener("click", play);

  $("save-paths").addEventListener("click", async () => {
    const next = await call("set_paths", {
      engineRoot: $("path-engine").value.trim(),
      iso: $("path-iso").value.trim(),
      saves: $("path-saves").value.trim(),
    });
    render(next);
    toast("Paths saved");
  });

  $("refresh-log").addEventListener("click", async () => {
    const text = await call("log_tail", { lines: 80 });
    $("log").textContent = text || "no log yet";
    $("log").scrollTop = $("log").scrollHeight;
  });

  $("build-engine").addEventListener("click", async () => {
    const pid = await call("install");
    toast(`Build started (pid ${pid}); this can take a while`);
  });

  document.addEventListener("keydown", (e) => {
    if (e.ctrlKey && e.shiftKey && (e.key === "D" || e.key === "d")) {
      const on = localStorage.getItem("setupPreview") === "1";
      localStorage.setItem("setupPreview", on ? "0" : "1");
      refreshState();
      return;
    }
    if (e.key === "Enter" && !e.repeat && document.activeElement.tagName !== "INPUT") {
      if (!$("play").disabled) play();
    }
  });

  // Xbox pad: A starts the game, like the footer says.
  const pad = { prevA: false };
  const poll = () => {
    const gp = navigator.getGamepads?.()[0];
    if (gp) {
      const a = !!(gp.buttons[0] && gp.buttons[0].pressed);
      if (a && !pad.prevA && !$("play").disabled) play();
      pad.prevA = a;
    }
    requestAnimationFrame(poll);
  };
  requestAnimationFrame(poll);
}

bind();
history.scrollRestoration = "manual";
document.querySelector("main").scrollTop = 0;
refreshState();
function active() {
  return !document.hidden && document.hasFocus();
}

setInterval(() => {
  const tag = document.activeElement?.tagName;
  if (active() && !["INPUT", "SELECT", "TEXTAREA"].includes(tag)) refreshState();
}, 5000);

document.addEventListener("visibilitychange", () => {
  if (active()) refreshState();
});
window.addEventListener("focus", refreshState);
