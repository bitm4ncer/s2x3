const hasTauri = !!window.__TAURI__;
const invoke = hasTauri ? window.__TAURI__.core.invoke : null;

const $ = (id) => document.getElementById(id);
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
  config_path: "~/.config/slope3/config.toml",
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
  log_path: "…/saves/launcher.log",
};

function mockInvoke(cmd, args) {
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
const XBOX_COLORS = {
  A: "#3fbf5f",
  B: "#e5484d",
  X: "#3b82f6",
  Y: "#f2c744",
};

function glyph(button) {
  const color = XBOX_COLORS[button] || "#7c8ea0";
  return `<svg width="18" height="18" viewBox="0 0 20 20" aria-hidden="true">
    <circle cx="10" cy="10" r="9" fill="${color}" />
    <text x="10" y="14" text-anchor="middle" font-family="Arial, sans-serif"
      font-size="11" font-weight="700" fill="#04121f">${button}</text>
  </svg>`;
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
  $("hints").innerHTML = `
    <span class="hint">${glyph("A")} Play</span>
    <span class="hint">${dpadGlyph()} Navigate</span>
    <span class="hint">${glyph("B")} Back (in game)</span>
    <span class="hint" style="color:#5f7d92">F11 fullscreen &middot; F10 settings in game</span>
  `;
}

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
      .map((c) => {
        const label = c === "" ? "Auto" : c;
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

  const chip = (id, ok, text) => {
    const el = $(id);
    el.classList.toggle("ok", ok);
    el.classList.toggle("bad", !ok);
    el.querySelector(".v").textContent = text;
  };
  chip("chip-engine", state.binary.state === "ready", state.binary.state === "ready" ? "ready" : "missing");
  chip("chip-disc", state.disc.state === "ready", state.disc.state === "ready" ? "disc ok" : "no disc");

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

  const fields = $("fields");
  fields.innerHTML = "";
  state.schema.forEach((f) => fields.append(field(f)));

  fields.querySelectorAll("[data-key]").forEach((el) => {
    el.addEventListener("change", scheduleSave);
    if (el.tagName === "INPUT" && el.type === "text") {
      el.addEventListener("input", scheduleSave);
    }
  });

  const ready = state.binary.state === "ready" && state.disc.state === "ready";
  $("play").disabled = !ready;
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

function bind() {
  hints();
  refreshSaves();

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
refreshState();
setInterval(() => {
  const tag = document.activeElement?.tagName;
  if (!["INPUT", "SELECT", "TEXTAREA"].includes(tag)) refreshState();
}, 5000);
