// Pocket TTS — Translucent Glassmorphism Frontend
// Talks to the Rust backend through Tauri v2 commands.

(() => {
  "use strict";

  // ---------- Tauri API shims ----------
  const hasTauri = !!window.__TAURI__;
  let invoke = null;
  let openDialog = null;
  let saveDialog = null;

  if (hasTauri) {
    const core = window.__TAURI__.core || {};
    invoke = core.invoke || null;

    // Dialog API may be at window.__TAURI_PLUGIN_DIALOG__, __TAURI__.dialog, or __TAURI__.core.dialog
    const pluginDialog = window.__TAURI_PLUGIN_DIALOG__ || {};
    const dialog = pluginDialog.open || pluginDialog.save ? pluginDialog : (window.__TAURI__.dialog || core.dialog || {});
    openDialog = dialog.open ? (...a) => dialog.open(...a) : null;
    saveDialog = dialog.save ? (...a) => dialog.save(...a) : null;
  }

  // ---------- Theme variant persistence ----------
  const THEME_VARIANTS = ["light", "dark", "green", "green-light"];
  const DEFAULT_THEME = "light";
  // New key: the shipped look is the light frosted glass, so a saved tint from the
  // previous build must not override it on first launch.
  const THEME_KEY = "pocket-glass-theme";

  function getSavedTheme() {
    try {
      const saved = localStorage.getItem(THEME_KEY);
      if (saved && THEME_VARIANTS.includes(saved)) return saved;
    } catch {}
    return DEFAULT_THEME;
  }

  function setThemeVariant(variant) {
    if (!THEME_VARIANTS.includes(variant)) variant = DEFAULT_THEME;
    document.documentElement.setAttribute("data-theme", variant);
    try { localStorage.setItem(THEME_KEY, variant); } catch {}
    syncWindowMaterial();
    updateThemeSwitcherUI(variant);
    if (typeof drawWaveform === "function") {
      drawWaveform(lastSamples);
    }
    // Keep the selection popup in step with the theme.
    try {
      if (invoke) invoke("set_app_theme", { theme: variant }).catch(() => {});
      if (hasTauri && window.__TAURI__ && window.__TAURI__.event && window.__TAURI__.event.emitTo) {
        window.__TAURI__.event.emitTo("popup", "theme-changed", variant).catch(() => {});
      }
    } catch {}
  }

  // ---------- Surface mode: transparent glass, high-visibility, Windows acrylic ----------
  const SURFACE_MODES = ["glass", "solid", "acrylic"];
  const DEFAULT_SURFACE = "glass";
  const SURFACE_KEY = "pocket-glass-surface";

  function getSavedSurface() {
    try {
      const saved = localStorage.getItem(SURFACE_KEY);
      if (saved && SURFACE_MODES.includes(saved)) return saved;
    } catch {}
    return DEFAULT_SURFACE;
  }

  function currentTheme() {
    return document.documentElement.getAttribute("data-theme") || DEFAULT_THEME;
  }

  function currentSurface() {
    const mode = document.documentElement.dataset.surface;
    return SURFACE_MODES.includes(mode) ? mode : DEFAULT_SURFACE;
  }

  // `glass` keeps the desktop showing through, `solid` weights the panels and pills
  // up so text stays readable over a busy wallpaper, and `acrylic` hands the blur to
  // the Windows compositor instead of the page's own backdrop filter.
  function setSurface(mode) {
    if (!SURFACE_MODES.includes(mode)) mode = DEFAULT_SURFACE;
    document.documentElement.dataset.surface = mode;
    try { localStorage.setItem(SURFACE_KEY, mode); } catch {}
    syncWindowMaterial();
    updateSurfaceUI(mode);
  }

  function updateSurfaceUI(mode) {
    document.querySelectorAll(".surface-pill").forEach((btn) => {
      const active = btn.dataset.surface === mode;
      btn.classList.toggle("active", active);
      btn.setAttribute("aria-pressed", String(active));
    });
  }

  // Keep the native window chrome in step with the tint and surface mode, so the dark
  // and emerald themes no longer sit under a light title bar, and the acrylic backdrop
  // is applied or cleared along with the toggle.
  function syncWindowMaterial() {
    if (!invoke) return;
    try {
      invoke("set_window_style", { theme: currentTheme(), surface: currentSurface() }).catch(() => {});
    } catch {}
  }

  function updateThemeSwitcherUI(variant) {
    const btn = document.getElementById("bt-theme-variant");
    if (!btn) return;
    const icons = {
      "dark": "🌙",
      "light": "☀️",
      "green": "🐸",
      "green-light": "🌱"
    };
    btn.querySelector(".theme-opt").textContent = icons[variant] || "✨";
    btn.title = `Theme: ${variant} (click to cycle)`;
  }

  function cycleThemeVariant() {
    const current = document.documentElement.getAttribute("data-theme") || DEFAULT_THEME;
    const idx = THEME_VARIANTS.indexOf(current);
    const next = THEME_VARIANTS[(idx + 1) % THEME_VARIANTS.length];
    setThemeVariant(next);
  }

  // ---------- DOM refs ----------
  const els = {
    text: document.getElementById("text"),
    chars: document.getElementById("chars"),
    language: document.getElementById("language"),
    quantize: document.getElementById("quantize"),
    btLoad: document.getElementById("bt-load"),
    modelStatus: document.getElementById("model-status"),
    modelProgress: document.getElementById("model-progress"),
    btGenerate: document.getElementById("bt-generate"),
    genSpinner: document.getElementById("gen-spinner"),
    genStatus: document.getElementById("gen-status"),
    audio: document.getElementById("audio"),
    btPlay: document.getElementById("bt-play"),
    btPause: document.getElementById("bt-pause"),
    audioScrub: document.getElementById("audio-scrub"),
    waveform: document.getElementById("waveform"),
    playerinfo: document.getElementById("playerinfo"),
    durVal: document.getElementById("dur-val"),
    rateVal: document.getElementById("rate-val"),
    btSave: document.getElementById("bt-save"),
    voicegrid: document.getElementById("voicegrid"),
    btVoicePick: document.getElementById("bt-voice-pick"),
    voiceChip: document.getElementById("voice-chip"),
    temp: document.getElementById("temp"), tempVal: document.getElementById("temp-val"),
    eos: document.getElementById("eos"), eosVal: document.getElementById("eos-val"),
    steps: document.getElementById("steps"), stepsVal: document.getElementById("steps-val"),
    maxtok: document.getElementById("maxtok"), maxtokVal: document.getElementById("maxtok-val"),
    nc: document.getElementById("nc"), ncVal: document.getElementById("nc-val"),
    tforce: document.getElementById("tforce"),
    btRecord: document.getElementById("bt-record"),
    recIcon: document.getElementById("rec-icon"),
    recLabel: document.getElementById("rec-label"),
    recChip: document.getElementById("rec-chip"),
    btCloneImport: document.getElementById("bt-clone-import"),
    cloneImportChip: document.getElementById("clone-import-chip"),
    btCloneSafetensors: document.getElementById("bt-clone-safetensors"),
    cloneSafetensorsChip: document.getElementById("clone-safetensors-chip"),
    btCloneSave: document.getElementById("bt-clone-save"),
    cloneChip: document.getElementById("clone-chip"),
    btClone: document.getElementById("bt-clone"),
    cloneStatus: document.getElementById("clone-status"),
    btTheme: document.getElementById("bt-theme"),
    btThemeVariant: document.getElementById("bt-theme-variant"),
    btSurfaceGlass: document.getElementById("bt-surface-glass"),
    btSurfaceSolid: document.getElementById("bt-surface-solid"),
    btSurfaceAcrylic: document.getElementById("bt-surface-acrylic"),
    genderFilterBtns: document.querySelectorAll(".filter-pill[data-gender]"),
    hfAuthBox: document.getElementById("hf-auth-box"),
    hfBadge: document.getElementById("hf-badge"),
    hfTokenInput: document.getElementById("hf-token-input"),
    btSaveHfToken: document.getElementById("bt-save-hf-token"),
    terminalDrawer: document.getElementById("terminal-drawer"),
    terminalHeader: document.getElementById("terminal-header"),
    terminalBody: document.getElementById("terminal-body"),
    terminalCount: document.getElementById("terminal-count"),
    terminalPreview: document.getElementById("terminal-preview"),
    btTermCopy: document.getElementById("bt-term-copy"),
    btTermClear: document.getElementById("bt-term-clear"),
    btTermSave: document.getElementById("bt-term-save"),
    btTermToggle: document.getElementById("bt-term-toggle"),
    btUpdate: document.getElementById("bt-update"),
    btRenderFile: document.getElementById("bt-render-file"),
    btStop: document.getElementById("bt-stop"),
    btResumeRender: document.getElementById("bt-resume-render"),
    historyList: document.getElementById("history-list"),
    previewVoices: document.getElementById("preview-voices"),
    minimizeToTray: document.getElementById("minimize-to-tray"),
    selectionPopup: document.getElementById("selection-popup"),
  };

  // ---------- state ----------
  let selectedVoice = null;       // custom voice file path or null
  let selectedVoiceName = "alba"; // built-in voice name
  let wavBase64 = null;           // last generated WAV (base64)
  let recordedPath = null;        // saved recording path
  let cloneImportPath = null;     // imported audio file path for cloning
  let cloneAudioPath = null;      // active audio path for voice clone export
  let mediaRecorder = null;
  let isRecording = false;
  let audioChunks = [];
  let lastSamples = null;
  let activeGenderFilter = "all";
  let voicesCache = [];
  let terminalLogs = [];
  let modelReady = false;
  let busyKind = null;            // "generate" or "render" while a job runs
  let history = [];               // clips generated since launch
  let activeHistoryId = null;
  const previewCache = new Map(); // voice key -> object URL of its sample
  const previewPending = new Set();
  const previewAudio = new Audio();
  const PREVIEW_TEXT = "Hello, this is how I sound.";

  // ---------- helpers ----------
  function toast(msg, ms = 4000) {
    const t = document.getElementById("toast");
    t.textContent = msg;
    t.classList.add("show");
    setTimeout(() => t.classList.remove("show"), ms);
  }

  function setEnabled(btn, on) {
    if (!btn) return;
    btn.disabled = !on;
  }

  // Durations are shown to a tenth of a second while they are short, so a generation stopped
  // part-way through its first chunk reads "0.8s kept" instead of a bare "0s".
  function fmtMs(ms) {
    if (!(ms > 0)) return "0s";
    if (ms < 10000) return `${(ms / 1000).toFixed(1)}s`;
    const s = Math.round(ms / 1000);
    const m = Math.floor(s / 60);
    const r = s % 60;
    return m ? `${m}:${String(r).padStart(2, "0")}` : `${s}s`;
  }

  // A running job swaps Generate for Stop, so a long render can always be abandoned.
  function setBusy(kind) {
    busyKind = kind;
    if (els.btStop) {
      els.btStop.hidden = !kind;
      if (!kind) els.btStop.disabled = false;
    }
    if (els.btGenerate) els.btGenerate.disabled = !!kind || !modelReady;
    if (els.btRenderFile) els.btRenderFile.disabled = !!kind;
    if (els.btResumeRender) els.btResumeRender.disabled = !!kind;
  }

  // ---------- settings that survive a restart ----------
  const SETTINGS_KEY = "pocket-tts-settings";
  const RESUME_KEY = "pocket-tts-resume";

  function readSettings() {
    try {
      return JSON.parse(localStorage.getItem(SETTINGS_KEY) || "{}") || {};
    } catch {
      return {};
    }
  }

  function saveSettings() {
    if (!els.temp) return;
    try {
      localStorage.setItem(
        SETTINGS_KEY,
        JSON.stringify({
          language: els.language.value,
          voice: selectedVoiceName,
          gender: activeGenderFilter,
          temp: els.temp.value,
          eos: els.eos.value,
          steps: els.steps.value,
          maxtok: els.maxtok.value,
          nc: els.nc.value,
          ncUnset: els.nc.dataset.unset === "true",
          tforce: els.tforce.checked,
          quantize: els.quantize.checked,
          preview: !!(els.previewVoices && els.previewVoices.checked),
          minimizeToTray: !!(els.minimizeToTray && els.minimizeToTray.checked),
          selectionPopup: !!(els.selectionPopup && els.selectionPopup.checked),
        })
      );
    } catch {}
  }

  // Restores the sliders, voice and language picked last time, before the model loads, so the
  // first generation already uses them.
  function applySettings(s) {
    const setRange = (input, label, value) => {
      if (!input || value === undefined || value === null) return;
      input.value = value;
      if (label) label.textContent = value;
      updateRangeFill(input);
    };
    setRange(els.temp, els.tempVal, s.temp);
    setRange(els.eos, els.eosVal, s.eos);
    setRange(els.steps, els.stepsVal, s.steps);
    setRange(els.maxtok, els.maxtokVal, s.maxtok);
    setRange(els.nc, els.ncVal, s.nc);
    if (s.ncUnset === false) els.nc.dataset.unset = "false";
    if (typeof s.tforce === "boolean") els.tforce.checked = s.tforce;
    if (typeof s.quantize === "boolean") els.quantize.checked = s.quantize;
    if (typeof s.preview === "boolean" && els.previewVoices) {
      els.previewVoices.checked = s.preview;
    }
    if (typeof s.minimizeToTray === "boolean" && els.minimizeToTray) {
      els.minimizeToTray.checked = s.minimizeToTray;
    }
    if (typeof s.selectionPopup === "boolean" && els.selectionPopup) {
      els.selectionPopup.checked = s.selectionPopup;
    }
    if (s.gender) {
      activeGenderFilter = s.gender;
      els.genderFilterBtns.forEach(b =>
        b.classList.toggle("active", b.dataset.gender === s.gender)
      );
    }
    if (s.voice) selectedVoiceName = s.voice;
  }

  // ---------- unfinished renders ----------
  function readResumeJob() {
    try {
      const raw = JSON.parse(localStorage.getItem(RESUME_KEY) || "null");
      if (!raw || typeof raw !== "object") return null;
      if (typeof raw.input !== "string" || typeof raw.output !== "string") return null;
      return {
        input: raw.input,
        output: raw.output,
        at: typeof raw.at === "number" ? raw.at : Date.now(),
      };
    } catch {
      return null;
    }
  }

  function writeResumeJob(job) {
    try {
      if (job) {
        const stored = {
          input: String(job.input || ""),
          output: String(job.output || ""),
          at: typeof job.at === "number" ? job.at : Date.now(),
        };
        localStorage.setItem(RESUME_KEY, JSON.stringify(stored));
      } else {
        localStorage.removeItem(RESUME_KEY);
      }
    } catch {}
    if (els.btResumeRender) els.btResumeRender.hidden = !job;
  }

  // ---------- clips generated this session ----------
  function addHistory(entry) {
    history.unshift({ ...entry, id: `clip-${Date.now()}-${history.length}`, at: new Date() });
    while (history.length > 20) {
      const dropped = history.pop();
      if (dropped && dropped.url) URL.revokeObjectURL(dropped.url);
    }
    renderHistory();
  }

  function renderHistory() {
    const list = els.historyList;
    if (!list) return;
    if (!history.length) {
      list.innerHTML = '<div class="history-empty">Generated clips appear here.</div>';
      return;
    }
    list.innerHTML = "";
    for (const clip of history) {
      const item = document.createElement("button");
      item.type = "button";
      item.className = "history-item" + (clip.id === activeHistoryId ? " active" : "");
      item.title = clip.text;
      const when = clip.at.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
      item.innerHTML = `
        <span class="history-snippet">${escapeHtml(clip.snippet)}</span>
        <span class="history-meta">${escapeHtml(clip.voice)} · ${fmtMs(clip.duration)} · ${when}</span>
      `;
      item.onclick = () => {
        activeHistoryId = clip.id;
        wavBase64 = clip.wav;
        showAudio(clip.wav, clip.duration, clip.sampleRate);
        renderHistory();
      };
      list.appendChild(item);
    }
  }

  // ---------- voice samples ----------
  async function previewVoice(name, pill) {
    // A running job owns the engine; a preview would queue behind it and reset its stop flag.
    if (!invoke || !modelReady || busyKind || !name) return;
    const key = `${els.language.value}|${name}`;
    const cached = previewCache.get(key);
    if (cached) {
      previewAudio.src = cached;
      previewAudio.play().catch(() => {});
      return;
    }
    if (previewPending.has(key)) return;
    previewPending.add(key);
    // Try a pre-shipped cached preview first to avoid realtime generation.
    try {
      if (invoke) {
        const b64 = await invoke("get_cached_preview", { language: els.language.value, voice: name });
        if (b64) {
          const bytes = atobToBytes(b64);
          const url = URL.createObjectURL(new Blob([bytes], { type: "audio/ogg" }));
          if (previewCache.size >= 16) {
            const [oldKey, oldUrl] = previewCache.entries().next().value;
            URL.revokeObjectURL(oldUrl);
            previewCache.delete(oldKey);
          }
          previewCache.set(key, url);
          previewAudio.src = url;
          previewAudio.play().catch(() => {});
          previewPending.delete(key);
          if (pill) pill.classList.remove("previewing");
          addClientLog("TTS", `Voice preview (cached): ${name}`);
          return;
        }
      }
    } catch (e) {}

    if (pill) pill.classList.add("previewing");
    try {
      const ncValue = els.nc.dataset.unset === "true" ? undefined : parseFloat(els.nc.value);
      const steps = parseInt(els.steps.value, 10) || undefined;
      const res = await api("generate", {
        text: PREVIEW_TEXT,
        language: els.language.value,
        config: null,
        voice: name,
        sampler_decode_steps: steps,
        samplerDecodeSteps: steps,
        temperature: parseFloat(els.temp.value) || undefined,
        noise_clamp: ncValue,
        noiseClamp: ncValue,
        eos_threshold: parseFloat(els.eos.value) || undefined,
        eosThreshold: parseFloat(els.eos.value) || undefined,
        quantize: els.quantize.checked,
      });
      const bytes = atobToBytes(res.wav_base64);
      const url = URL.createObjectURL(new Blob([bytes], { type: "audio/wav" }));
      if (previewCache.size >= 16) {
        const [oldKey, oldUrl] = previewCache.entries().next().value;
        URL.revokeObjectURL(oldUrl);
        previewCache.delete(oldKey);
      }
      previewCache.set(key, url);
      previewAudio.src = url;
      previewAudio.play().catch(() => {});
      addClientLog("TTS", `Voice preview: ${name}`);
    } catch (e) {
      addClientLog("WARN", `Voice preview failed: ${e.message || e}`);
    } finally {
      previewPending.delete(key);
      if (pill) pill.classList.remove("previewing");
    }
  }

  function toggleLightDark() {
    const current = document.documentElement.getAttribute("data-theme") || DEFAULT_THEME;
    let next;
    switch (current) {
      case "dark":
        next = "light";
        break;
      case "light":
        next = "dark";
        break;
      case "green":
        next = "green-light";
        break;
      case "green-light":
        next = "green";
        break;
      default:
        next = "dark";
    }
    setThemeVariant(next);
  }

  function updateRangeFill(slider) {
    if (!slider) return;
    const min = parseFloat(slider.min) || 0;
    const max = parseFloat(slider.max) || 100;
    const val = parseFloat(slider.value) || 0;
    const pct = ((val - min) / (max - min)) * 100;
    slider.style.setProperty("--range-fill", `${pct}%`);
    slider.style.setProperty("--fill", `${pct}%`);
  }

  function escapeHtml(str) {
    return String(str)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  // ---------- status capsule (header chips) ----------
  function setChipModel(text, state) {
    const chip = document.getElementById("chip-model");
    const led = document.getElementById("led-model");
    if (chip) chip.textContent = text;
    if (led) led.className = "led" + (state ? " " + state : "");
  }

  function setChipOutput(text) {
    const chip = document.getElementById("chip-output");
    if (chip) chip.textContent = text;
  }

  // ---------- in-app terminal console ----------
  function appendTerminalLine(entry) {
    if (!entry) return;
    terminalLogs.push(entry);
    if (terminalLogs.length > 500) terminalLogs.shift();

    if (els.terminalCount) {
      els.terminalCount.textContent = `${terminalLogs.length} entries`;
    }
    if (els.terminalPreview) {
      els.terminalPreview.textContent = `[${entry.level}] ${entry.message}`;
    }

    if (els.terminalBody) {
      const empty = els.terminalBody.querySelector(".terminal-empty");
      if (empty) empty.remove();

      const line = document.createElement("div");
      line.className = "term-line";
      const lvl = entry.level || "INFO";
      const lvlClass = `lvl-${lvl}`;
      line.innerHTML = `
        <span class="log-time">${entry.timestamp || ""}</span>
        <span class="log-lvl ${lvlClass}">${lvl}</span>
        <span class="log-msg">${escapeHtml(entry.message)}</span>
      `;
      els.terminalBody.appendChild(line);
      els.terminalBody.scrollTop = els.terminalBody.scrollHeight;
    }
  }

  function addClientLog(level, message) {
    const now = new Date();
    const ts = [
      String(now.getHours()).padStart(2, "0"),
      String(now.getMinutes()).padStart(2, "0"),
      String(now.getSeconds()).padStart(2, "0"),
    ].join(":");
    appendTerminalLine({ timestamp: ts, level, message });
  }

  function toggleTerminal() {
    if (!els.terminalDrawer) return;
    const isCollapsed = els.terminalDrawer.classList.toggle("collapsed");
    if (els.btTermToggle) {
      els.btTermToggle.textContent = isCollapsed ? "▲ Expand" : "▼ Collapse";
    }
    if (!isCollapsed && els.terminalBody) {
      els.terminalBody.scrollTop = els.terminalBody.scrollHeight;
    }
  }

  function copyTerminalLogs() {
    const text = terminalLogs
      .map(l => `[${l.timestamp}] [${l.level}] ${l.message}`)
      .join("\n");
    if (!text) {
      toast("No logs to copy");
      return;
    }
    navigator.clipboard.writeText(text).then(
      () => toast("Terminal logs copied to clipboard!"),
      () => toast("Failed to copy logs to clipboard")
    );
  }

  async function clearTerminal() {
    terminalLogs = [];
    if (els.terminalBody) {
      els.terminalBody.innerHTML = '<div class="terminal-empty">Terminal cleared.</div>';
    }
    if (els.terminalCount) {
      els.terminalCount.textContent = "0 entries";
    }
    if (els.terminalPreview) {
      els.terminalPreview.textContent = "Console cleared.";
    }
    try {
      await api("clear_terminal_logs");
    } catch {}
  }

  // Hook browser console to in-app terminal
  const origLog = console.log;
  const origWarn = console.warn;
  const origError = console.error;
  console.log = (...args) => {
    origLog.apply(console, args);
    addClientLog("INFO", args.map(String).join(" "));
  };
  console.warn = (...args) => {
    origWarn.apply(console, args);
    addClientLog("WARN", args.map(String).join(" "));
  };
  console.error = (...args) => {
    origError.apply(console, args);
    addClientLog("ERROR", args.map(String).join(" "));
  };

  // ---------- voices list rendering with gender filtering ----------
  function renderVoices(voices, filterLang, filterGender = activeGenderFilter) {
    voicesCache = voices;
    const grid = els.voicegrid;
    grid.innerHTML = "";
    let shown = filterLang
      ? voices.filter(v => v.language === filterLang)
      : voices;

    if (filterGender && filterGender !== "all") {
      shown = shown.filter(v => (v.gender || "male") === filterGender);
    }

    const chipVoices = document.getElementById("chip-voices");
    if (chipVoices) chipVoices.textContent = `${shown.length} / ${voices.length} voices`;

    if (shown.length === 0) {
      grid.innerHTML = '<div style="grid-column:1/-1;color:var(--text-dim);font-size:12px;padding:12px 0;text-align:center">No voices match filter</div>';
      return;
    }

    for (const v of shown) {
      const pill = document.createElement("button");
      pill.type = "button";
      const isActive = !selectedVoice && !recordedPath && selectedVoiceName === v.name;
      pill.className = "pill" + (isActive ? " active" : "");
      const icon = v.gender === "female" ? "♀ " : "♂ ";
      pill.innerHTML = `<span class="voice-gender-icon">${icon}</span>${v.name}`;
      pill.title = `${v.name} (${v.gender || "voice"} · ${v.accent || v.language})`;
      pill.onclick = () => {
        selectedVoice = null;
        selectedVoiceName = v.name;
        recordedPath = null;
        els.recChip.textContent = "none";
        els.voiceChip.textContent = v.name;
        renderVoices(voices, filterLang, activeGenderFilter);
        if (!els.previewVoices || els.previewVoices.checked) previewVoice(v.name, pill);
      };
      grid.appendChild(pill);
    }

    if (selectedVoice || recordedPath) {
      const pill = document.createElement("button");
      pill.type = "button";
      pill.className = "pill selected";
      const label = recordedPath
        ? "rec: " + recordedPath.split(/[\\/]/).pop()
        : "file: " + selectedVoice.split(/[\\/]/).pop();
      pill.textContent = label;
      pill.title = recordedPath || selectedVoice;
      pill.onclick = () => {
        selectedVoice = null;
        recordedPath = null;
        selectedVoiceName = "alba";
        els.recChip.textContent = "none";
        els.voiceChip.textContent = "alba";
        renderVoices(voices, filterLang, activeGenderFilter);
      };
      grid.appendChild(pill);
    }
    saveSettings();
  }

  // ---------- Mock data for graceful degradation ----------
  const MOCK_LANGUAGES = [
    { code: "english", name: "English" },
    { code: "french", name: "French" },
    { code: "german", name: "German" },
    { code: "spanish", name: "Spanish" },
    { code: "italian", name: "Italian" },
    { code: "portuguese", name: "Portuguese" },
    { code: "dutch", name: "Dutch" },
  ];

  const MOCK_VOICES = [
    { name: "alba", language: "english", gender: "female", accent: "Scottish" },
    { name: "cosette", language: "english", gender: "female", accent: "Expressive" },
    { name: "anna", language: "english", gender: "female", accent: "British" },
    { name: "vera", language: "english", gender: "female", accent: "British" },
    { name: "fantine", language: "english", gender: "female", accent: "British" },
    { name: "eponine", language: "english", gender: "female", accent: "Scottish" },
    { name: "azelma", language: "english", gender: "female", accent: "British" },
    { name: "mary", language: "english", gender: "female", accent: "British" },
    { name: "jane", language: "english", gender: "female", accent: "British" },
    { name: "eve", language: "english", gender: "female", accent: "American" },
    { name: "marius", language: "english", gender: "male", accent: "Casual" },
    { name: "javert", language: "english", gender: "male", accent: "Deep" },
    { name: "jean", language: "french", gender: "male", accent: "French" },
    { name: "lola", language: "spanish", gender: "female", accent: "Spanish" },
    { name: "giovanni", language: "italian", gender: "male", accent: "Italian" },
    { name: "rafael", language: "portuguese", gender: "male", accent: "Portuguese" },
    { name: "daan", language: "dutch", gender: "male", accent: "Dutch" },
    { name: "estelle", language: "french", gender: "female", accent: "French" },
  ];

  // ---------- API ----------
  async function api(cmd, args = {}) {
    if (invoke) {
      return invoke(cmd, args);
    }
    // Graceful degradation when Tauri runtime is not available
    if (cmd === "get_languages") {
      return MOCK_LANGUAGES;
    }
    if (cmd === "get_voices") {
      const lang = args.language;
      return lang ? MOCK_VOICES.filter(v => v.language === lang) : MOCK_VOICES;
    }
    if (cmd === "app_info") {
      return {
        name: "Pocket TTS",
        version: "preview (no backend)",
        description: "Kyutai Pocket TTS desktop application with Liquid Glass UI",
      };
    }
    if (cmd === "check_update") {
      return { available: false, current_version: "preview", version: null, notes: null };
    }
    if (cmd === "save_logs") {
      return args.file_path || args.filePath || "pocket-tts-log.txt";
    }
    if (cmd === "render_text_file") {
      throw new Error("Rendering a text file needs the desktop app");
    }
    if (cmd === "cancel_generation") {
      return true;
    }
    if (cmd === "load_model") {
      return "Model ready (browser preview)";
    }
    if (cmd === "export_voice") {
      return `Voice exported to ${args.export_path || args.exportPath || "clone.safetensors"} (preview)`;
    }
    if (cmd === "save_audio") {
      return args.file_path || args.filePath || "saved.wav";
    }
    if (cmd === "get_hf_status") {
      return { has_token: false, token_preview: null, has_voice_cloning_weights: false, token_path: "~/.cache/huggingface/token" };
    }
    if (cmd === "set_hf_token") {
      return "Token saved (browser preview)";
    }
    if (cmd === "get_terminal_logs") {
      return terminalLogs;
    }
    if (cmd === "clear_terminal_logs") {
      return true;
    }
    throw new Error(`Command "${cmd}" unavailable: Tauri API not loaded`);
  }

  async function loadLanguages() {
    try {
      const langs = await api("get_languages");
      els.language.innerHTML = "";
      const saved = readSettings();
      for (const l of langs) {
        const opt = document.createElement("option");
        opt.value = l.code;
        opt.textContent = l.name;
        if (l.code === (saved.language || "english")) opt.selected = true;
        els.language.appendChild(opt);
      }
      await onLanguageChange();
    } catch (e) {
      toast("Failed to load languages: " + e);
    }
  }

  async function onLanguageChange() {
    const lang = els.language.value;
    try {
      const voices = await api("get_voices", { language: lang });
      renderVoices(voices, lang, activeGenderFilter);
      loadModel();
    } catch (e) {
      toast("Failed to load voices: " + e);
    }
  }

  // ---------- model loading ----------
  async function loadModel() {
    const lang = els.language.value;
    const q = els.quantize.checked;
    const ncValue = els.nc.dataset.unset === "true" ? undefined : parseFloat(els.nc.value);

    setEnabled(els.btLoad, false);
    els.modelStatus.textContent = "Loading model…";
    els.modelProgress.style.width = "40%";
    setChipModel("Loading model…", "busy");

    try {
      const msg = await api("load_model", {
        language: lang,
        config: null,
        temperature: parseFloat(els.temp.value) || undefined,
        noise_clamp: ncValue,
        noiseClamp: ncValue,
        eos_threshold: parseFloat(els.eos.value) || undefined,
        eosThreshold: parseFloat(els.eos.value) || undefined,
        sampler_decode_steps: parseInt(els.steps.value, 10) || undefined,
        samplerDecodeSteps: parseInt(els.steps.value, 10) || undefined,
        quantize: q,
      });
      els.modelStatus.textContent = msg;
      els.modelProgress.style.width = "100%";
      setChipModel("Model ready", "on");
      modelReady = true;
      setBusy(busyKind);
      addClientLog("INFO", `Model loaded: ${msg}`);
      await checkHfStatus();
    } catch (e) {
      els.modelStatus.textContent = "Failed: " + e;
      els.modelProgress.style.width = "0%";
      setChipModel("Model failed", "err");
      modelReady = false;
      setBusy(null);
      toast("Model load failed: " + e);
    } finally {
      setEnabled(els.btLoad, true);
    }
  }

  // ---------- Hugging Face token checking and saving ----------
  async function checkHfStatus() {
    try {
      const status = await api("get_hf_status");
      if (!els.hfBadge) return;
      if (status.has_token) {
        if (status.has_voice_cloning_weights) {
          els.hfBadge.textContent = "Cloning Ready";
          els.hfBadge.className = "hf-badge active";
        } else {
          els.hfBadge.textContent = "Token Saved";
          els.hfBadge.className = "hf-badge active";
        }
        if (status.token_preview && els.hfTokenInput) {
          els.hfTokenInput.placeholder = `Current token: ${status.token_preview}`;
        }
      } else {
        els.hfBadge.textContent = "No Token";
        els.hfBadge.className = "hf-badge missing";
      }
    } catch {}
  }

  async function saveHfToken() {
    if (!els.hfTokenInput) return;
    const val = els.hfTokenInput.value.trim();
    if (!val) {
      toast("Please paste your Hugging Face access token (hf_...)");
      return;
    }
    setEnabled(els.btSaveHfToken, false);
    try {
      const res = await api("set_hf_token", { token: val });
      toast(res);
      els.hfTokenInput.value = "";
      await checkHfStatus();
      loadModel();
    } catch (e) {
      toast("Failed to save HF token: " + (e.message || e));
    } finally {
      setEnabled(els.btSaveHfToken, true);
    }
  }

  // ---------- waveform renderer ----------
  function drawWaveform(samples) {
    lastSamples = samples;
    const canvas = els.waveform;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    canvas.width = (rect.width || 300) * dpr;
    canvas.height = (rect.height || 64) * dpr;
    const ctx = canvas.getContext("2d");
    ctx.scale(dpr, dpr);

    const w = rect.width || 300;
    const h = rect.height || 64;
    ctx.clearRect(0, 0, w, h);

    const mid = h / 2;
    const theme = document.documentElement.getAttribute("data-theme") || DEFAULT_THEME;
    const glowColor = (theme === "green" || theme === "green-light")
      ? "rgba(110, 231, 183, 0.95)"
      : (theme === "light" ? "rgba(180, 83, 9, 0.95)" : "rgba(251, 191, 36, 0.95)");

    if (!samples || samples.length === 0) {
      ctx.strokeStyle = "rgba(255, 255, 255, 0.18)";
      if (theme === "light") ctx.strokeStyle = "rgba(18, 22, 30, 0.20)";
      if (theme === "green-light") ctx.strokeStyle = "rgba(22, 163, 74, 0.25)";
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      ctx.moveTo(0, mid);
      ctx.lineTo(w, mid);
      ctx.stroke();
      return;
    }

    const bars = Math.min(100, Math.floor(w / 3.5));
    const step = Math.floor(samples.length / bars);
    const barW = (w / bars) * 0.6;

    ctx.fillStyle = glowColor;
    for (let i = 0; i < bars; i++) {
      let max = 0;
      const start = i * step;
      for (let j = 0; j < step; j++) {
        const v = Math.abs(samples[start + j] || 0);
        if (v > max) max = v;
      }
      const barH = Math.max(3, max * (h * 0.88));
      const x = i * (w / bars) + (w / bars - barW) / 2;
      ctx.beginPath();
      ctx.roundRect(x, mid - barH / 2, barW, barH, 2);
      ctx.fill();
    }
  }

  // ---------- custom audio transport ----------
  els.btPlay.onclick = () => {
    if (els.audio.src) els.audio.play().catch(() => {});
  };
  els.btPause.onclick = () => {
    els.audio.pause();
  };
  els.audio.ontimeupdate = () => {
    if (!els.audio.duration) return;
    const pct = (els.audio.currentTime / els.audio.duration) * 100;
    els.audioScrub.value = pct;
    updateRangeFill(els.audioScrub);
  };
  els.audio.onended = () => {
    els.audioScrub.value = 0;
    updateRangeFill(els.audioScrub);
  };
  els.audioScrub.oninput = () => {
    updateRangeFill(els.audioScrub);
    if (els.audio.duration) {
      els.audio.currentTime = (parseFloat(els.audioScrub.value) / 100) * els.audio.duration;
    }
  };

  // ---------- generation ----------
  async function generate() {
    const text = (els.text.value || "").trim();
    if (!text) {
      toast("Enter some text first");
      return;
    }
    const lang = els.language.value;
    const q = els.quantize.checked;
    const ncValue = els.nc.dataset.unset === "true" ? undefined : parseFloat(els.nc.value);

    setBusy("generate");
    els.genSpinner.style.display = "flex";
    els.audio.src = "";
    els.audioScrub.value = 0;
    updateRangeFill(els.audioScrub);
    els.genStatus.textContent = "Generating…";

    try {
      // A chosen built-in voice must be sent by name: without it the backend falls back
      // to the language's default speaker, so every catalog voice sounded identical.
      const activeVoice = recordedPath || selectedVoice || selectedVoiceName || undefined;
      const res = await api("generate", {
        text,
        language: lang,
        config: null,
        voice: activeVoice,
        sampler_decode_steps: parseInt(els.steps.value, 10) || undefined,
        samplerDecodeSteps: parseInt(els.steps.value, 10) || undefined,
        temperature: parseFloat(els.temp.value) || undefined,
        noise_clamp: ncValue,
        noiseClamp: ncValue,
        eos_threshold: parseFloat(els.eos.value) || undefined,
        eosThreshold: parseFloat(els.eos.value) || undefined,
        max_tokens: parseInt(els.maxtok.value, 10) || undefined,
        maxTokens: parseInt(els.maxtok.value, 10) || undefined,
        teacher_force_previous_chunk: els.tforce.checked,
        teacherForcePreviousChunk: els.tforce.checked,
        quantize: q,
      });
      wavBase64 = res.wav_base64;
      const url = await showAudio(wavBase64, res.duration_ms, res.sample_rate);
      if (res.cancelled) {
        els.genStatus.textContent = `Stopped — kept ${fmtMs(res.duration_ms)}`;
        addClientLog("TTS", `Generation stopped after ${fmtMs(res.duration_ms)}`);
        toast("Generation stopped — the audio finished so far is kept");
      } else {
        els.genStatus.textContent = `Done — ${fmtMs(res.duration_ms)}`;
        addClientLog("TTS", `Generated speech: ${res.num_samples} samples, ${fmtMs(res.duration_ms)}`);
      }
      if (res.num_samples > 0) {
        const source = recordedPath || selectedVoice;
        const voiceLabel = source ? source.split(/[\\/]/).pop() : selectedVoiceName;
        addHistory({
          text,
          snippet: text.length > 60 ? text.slice(0, 60) + "…" : text,
          voice: voiceLabel,
          duration: res.duration_ms,
          sampleRate: res.sample_rate,
          wav: wavBase64,
          url,
        });
      }
    } catch (e) {
      const errStr = String(e.message || e);
      els.genStatus.textContent = "Error: " + errStr;
      if (errStr.includes("voice cloning") || errStr.includes("kyutai/pocket-tts")) {
        toast("Voice cloning requires Hugging Face terms acceptance — see access box in Voice Clone card", 6000);
        if (els.hfAuthBox) {
          els.hfAuthBox.scrollIntoView({ behavior: "smooth" });
          if (els.hfTokenInput) els.hfTokenInput.focus();
        }
      } else {
        toast("Generation failed: " + errStr);
      }
      addClientLog("ERROR", `Generation failed: ${errStr}`);
    } finally {
      els.genSpinner.style.display = "none";
      setBusy(null);
    }
  }

  // Puts a clip in the player and draws its waveform, returning its blob URL.
  async function showAudio(base64, durationMs, sampleRate) {
    const bytes = atobToBytes(base64);
    const blob = new Blob([bytes], { type: "audio/wav" });
    const url = URL.createObjectURL(blob);
    els.audio.src = url;
    els.audio.play().catch(() => {});
    els.durVal.textContent = fmtMs(durationMs);
    els.rateVal.textContent = `${sampleRate} Hz`;
    setChipOutput(`Audio ${fmtMs(durationMs)}`);
    try {
      const ac = new (window.AudioContext || window.webkitAudioContext)();
      const ab = await ac.decodeAudioData(bytes.buffer.slice(0));
      drawWaveform(ab.getChannelData(0));
      await ac.close().catch(() => {});
    } catch {
      drawWaveform(null);
    }
    return url;
  }

  function atobToBytes(b64) {
    const bin = atob(b64);
    const len = bin.length;
    const bytes = new Uint8Array(len);
    for (let i = 0; i < len; i++) bytes[i] = bin.charCodeAt(i);
    return bytes;
  }

  // ---------- save audio ----------
  // The backend picks the codec from the chosen extension, so the dialog is the format menu.
  const AUDIO_FILTERS = [
    { name: "WAV Audio", extensions: ["wav"] },
    { name: "MP3 Audio", extensions: ["mp3"] },
    { name: "Ogg Vorbis", extensions: ["ogg"] },
  ];

  async function saveAudio() {
    if (!wavBase64) {
      toast("No audio to save");
      return;
    }
    if (!saveDialog) {
      // Graceful degradation: HTML5 download
      try {
        const bytes = atobToBytes(wavBase64);
        const blob = new Blob([bytes], { type: "audio/wav" });
        const url = URL.createObjectURL(blob);
        const a = document.createElement("a");
        a.href = url;
        a.download = "pocket-tts-output.wav";
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        URL.revokeObjectURL(url);
        toast("Downloaded pocket-tts-output.wav");
      } catch (e) {
        toast("Download failed: " + (e.message || e));
      }
      return;
    }
    let path;
    try {
      path = await saveDialog({
        defaultPath: "pocket-tts-output.wav",
        filters: AUDIO_FILTERS,
      });
    } catch (e) {
      return;
    }
    if (!path) return;
    if (!/\.(wav|mp3|ogg)$/i.test(path)) path += ".wav";
    try {
      const saved = await api("save_audio", {
        wav_base64: wavBase64,
        wavBase64: wavBase64,
        file_path: path,
        filePath: path,
      });
      toast("Saved to " + saved);
      addClientLog("INFO", `Saved generated audio to ${saved}`);
    } catch (e) {
      toast("Save failed: " + (e.message || e));
      addClientLog("ERROR", `Save failed: ${e.message || e}`);
    }
  }

  // ---------- long form: render a whole text file ----------
  async function renderTextFile(job) {
    let input;
    let output;
    let args;
    let resume = false;

    if (job) {
      // Resuming uses the settings the render started with, not whatever the sliders say now.
      input = job.input;
      output = job.output;
      args = job.args;
      resume = true;
    } else {
      input = await pickOpen([
        { name: "Text files (*.txt, *.md)", extensions: ["txt", "md"] },
      ]);
      if (!input) return;
      const stem = input.split(/[\\/]/).pop().replace(/\.[^.]+$/, "") || "text";
      output = await pickSave(AUDIO_FILTERS, `pocket-tts-${stem}.wav`);
      if (!output) return;
      if (!/\.(wav|mp3|ogg)$/i.test(output)) output += ".wav";

      const ncValue = els.nc.dataset.unset === "true" ? undefined : parseFloat(els.nc.value);
      const steps = parseInt(els.steps.value, 10) || undefined;
      args = {
        language: els.language.value,
        config: null,
        voice: recordedPath || selectedVoice || selectedVoiceName || undefined,
        sampler_decode_steps: steps,
        samplerDecodeSteps: steps,
        temperature: parseFloat(els.temp.value) || undefined,
        noise_clamp: ncValue,
        noiseClamp: ncValue,
        eos_threshold: parseFloat(els.eos.value) || undefined,
        eosThreshold: parseFloat(els.eos.value) || undefined,
        max_tokens: parseInt(els.maxtok.value, 10) || undefined,
        maxTokens: parseInt(els.maxtok.value, 10) || undefined,
        teacher_force_previous_chunk: els.tforce.checked,
        teacherForcePreviousChunk: els.tforce.checked,
        quantize: els.quantize.checked,
      };
    }

    const jobRecord = { input, output, args, at: Date.now() };
    setBusy("render");
    els.genSpinner.style.display = "flex";
    els.genStatus.textContent = resume ? "Resuming render…" : "Rendering text file…";
    addClientLog("INFO", `${resume ? "Resuming" : "Rendering"} ${input} to ${output}`);

    try {
      const res = await api("render_text_file", {
        ...args,
        input_path: input,
        inputPath: input,
        output_path: output,
        outputPath: output,
        resume,
      });
      if (res.cancelled) {
        writeResumeJob(jobRecord);
        els.genStatus.textContent = `Stopped — ${fmtMs(res.duration_ms)} saved; resume to finish`;
        toast("Render stopped — use Resume unfinished render to continue", 7000);
        addClientLog("TTS", `Render stopped at ${fmtMs(res.duration_ms)}; progress kept for resuming`);
      } else {
        writeResumeJob(null);
        els.genStatus.textContent = `Saved ${fmtMs(res.duration_ms)} to ${res.output_path}`;
        setChipOutput(`Audio ${fmtMs(res.duration_ms)}`);
        toast("Rendered to " + res.output_path, 7000);
        addClientLog("TTS", `Rendered ${res.num_samples} samples to ${res.output_path} (${res.format})`);
      }
    } catch (e) {
      const errStr = String(e.message || e);
      els.genStatus.textContent = "Error: " + errStr;
      if (errStr.includes("different text")) {
        writeResumeJob(null);
        toast("Render failed: " + errStr, 7000);
      } else {
        writeResumeJob(jobRecord);
        toast("Render failed: " + errStr + " — progress kept, resume to retry", 7000);
      }
      addClientLog("ERROR", `Render failed: ${errStr}`);
    } finally {
      els.genSpinner.style.display = "none";
      setBusy(null);
    }
  }

  async function resumeRender() {
    const job = readResumeJob();
    if (!job) {
      writeResumeJob(null);
      toast("There is no interrupted render to resume");
      return;
    }
    await renderTextFile(job);
  }

  // Stop ends the job at its next chunk boundary and keeps whatever was finished.
  async function stopGeneration() {
    if (!busyKind) return;
    if (els.btStop) els.btStop.disabled = true;
    try {
      els.genStatus.textContent = "Stopping…";
      await api("cancel_generation");
    } catch (e) {
      toast("Could not stop: " + (e.message || e));
    } finally {
      setTimeout(() => {
        if (els.btStop) els.btStop.disabled = false;
      }, 2000);
    }
  }

  // ---------- updates ----------
  async function checkForUpdates() {
    try {
      const info = await api("check_update");
      if (!info || !info.available) {
        toast(`Pocket TTS ${info?.current_version || ""} is up to date`);
        addClientLog("INFO", "No update available.");
        return;
      }
      const notes = (info.notes || "").split("\n").slice(0, 8).join("\n");
      const proceed = window.confirm(
        `Pocket TTS ${info.version} is available (you have ${info.current_version}).\n\n` +
          (notes ? notes + "\n\n" : "") +
          "Download and install it now? The app will restart."
      );
      if (!proceed) return;
      els.genStatus.textContent = `Downloading ${info.version}…`;
      addClientLog("INFO", `Installing update ${info.version}`);
      await api("install_update");
    } catch (e) {
      const errStr = String(e.message || e);
      toast("Update failed: " + errStr, 6000);
      addClientLog("ERROR", `Update failed: ${errStr}`);
    }
  }

  // ---------- save the log ----------
  async function saveLogs() {
    const text = terminalLogs
      .map(l => `[${l.timestamp}] [${l.level}] ${l.message}`)
      .join("\n");
    const stamp = new Date()
      .toISOString()
      .replace(/[-:]/g, "")
      .replace(/\..*$/, "")
      .replace("T", "-");
    const path = await pickSave(
      [{ name: "Text file", extensions: ["txt"] }, { name: "Log file", extensions: ["log"] }],
      `pocket-tts-log-${stamp}.txt`
    );
    if (!path) return;
    try {
      const saved = await api("save_logs", {
        content: text || "No log entries.",
        file_path: path,
        filePath: path,
      });
      toast("Log saved to " + saved);
    } catch (e) {
      toast("Could not save the log: " + (e.message || e));
    }
  }

  // ---------- file picker helper ----------
  function normalizeFilters(filter) {
    if (!filter) return [];
    const list = Array.isArray(filter) ? filter : [filter];
    return list.map(f => {
      let extList = [];
      if (typeof f.extensions === "string") {
        extList = [f.extensions];
      } else if (Array.isArray(f.extensions)) {
        extList = f.extensions.map(String);
      }
      return {
        name: f.name || "Files",
        extensions: extList,
      };
    });
  }

  async function pickOpen(filter) {
    const filters = normalizeFilters(filter);
    if (!openDialog) {
      toast("File dialog unavailable in browser environment");
      return null;
    }
    try {
      return await openDialog({ multiple: false, directory: false, filters });
    } catch (e) {
      return null;
    }
  }

  async function pickSave(filter, defaultName) {
    const filters = normalizeFilters(filter);
    if (!saveDialog) {
      toast("File save unavailable in browser environment");
      return defaultName || null;
    }
    try {
      return await saveDialog({ defaultPath: defaultName, filters });
    } catch (e) {
      return null;
    }
  }

  // ---------- 16-bit PCM WAV encoding helper ----------
  function audioBufferToWav(audioBuffer) {
    const numChannels = audioBuffer.numberOfChannels;
    const sampleRate = audioBuffer.sampleRate;
    const format = 1; // PCM
    const bitDepth = 16;

    // Mix down to mono if multi-channel
    let samples;
    if (numChannels === 1) {
      samples = audioBuffer.getChannelData(0);
    } else {
      const ch0 = audioBuffer.getChannelData(0);
      const ch1 = audioBuffer.getChannelData(1);
      samples = new Float32Array(audioBuffer.length);
      for (let i = 0; i < audioBuffer.length; i++) {
        samples[i] = (ch0[i] + ch1[i]) * 0.5;
      }
    }

    const bytesPerSample = bitDepth / 8;
    const blockAlign = 1 * bytesPerSample;
    const byteRate = sampleRate * blockAlign;
    const dataSize = samples.length * bytesPerSample;
    const buffer = new ArrayBuffer(44 + dataSize);
    const view = new DataView(buffer);

    function writeStr(offset, str) {
      for (let i = 0; i < str.length; i++) {
        view.setUint8(offset + i, str.charCodeAt(i));
      }
    }

    // RIFF header
    writeStr(0, "RIFF");
    view.setUint32(4, 36 + dataSize, true);
    writeStr(8, "WAVE");

    // fmt subchunk
    writeStr(12, "fmt ");
    view.setUint32(16, 16, true);
    view.setUint16(20, format, true);
    view.setUint16(22, 1, true); // 1 channel (mono)
    view.setUint32(24, sampleRate, true);
    view.setUint32(28, byteRate, true);
    view.setUint16(32, blockAlign, true);
    view.setUint16(34, bitDepth, true);

    // data subchunk
    writeStr(36, "data");
    view.setUint32(40, dataSize, true);

    let offset = 44;
    for (let i = 0; i < samples.length; i++, offset += 2) {
      const s = Math.max(-1, Math.min(1, samples[i]));
      view.setInt16(offset, s < 0 ? s * 0x8000 : s * 0x7FFF, true);
    }

    return buffer;
  }

  // ---------- voice recording (mic) ----------
  async function toggleRecord() {
    if (isRecording) {
      stopRecording();
    } else {
      startRecording();
    }
  }

  async function startRecording() {
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      mediaRecorder = new MediaRecorder(stream);
      audioChunks = [];
      mediaRecorder.ondataavailable = e => {
        if (e.data.size) audioChunks.push(e.data);
      };
      mediaRecorder.onstop = async () => {
        try {
          const rawBlob = new Blob(audioChunks, { type: mediaRecorder.mimeType || "audio/webm" });
          if (rawBlob.size === 0) {
            toast("No audio recorded");
            stream.getTracks().forEach(t => t.stop());
            return;
          }

          // Decode recorded audio into PCM samples, then encode into standard 16-bit PCM WAV
          const arrayBuf = await rawBlob.arrayBuffer();
          let wavBuffer;
          try {
            const ac = new (window.AudioContext || window.webkitAudioContext)();
            const audioBuf = await ac.decodeAudioData(arrayBuf);
            wavBuffer = audioBufferToWav(audioBuf);
            await ac.close().catch(() => {});
          } catch (decErr) {
            toast("Audio conversion to WAV failed: " + decErr.message);
            stream.getTracks().forEach(t => t.stop());
            return;
          }

          // Open save dialog for WAV audio
          let path;
          if (saveDialog) {
            try {
              path = await saveDialog({
                defaultPath: "pocket-tts-voice.wav",
                filters: [{ name: "WAV Audio", extensions: ["wav"] }],
              });
            } catch (e) {
              toast("Save dialog failed: " + e.message);
              stream.getTracks().forEach(t => t.stop());
              return;
            }
          } else {
            // HTML5 download fallback
            const blob = new Blob([wavBuffer], { type: "audio/wav" });
            const url = URL.createObjectURL(blob);
            const a = document.createElement("a");
            a.href = url;
            a.download = "pocket-tts-voice.wav";
            document.body.appendChild(a);
            a.click();
            document.body.removeChild(a);
            URL.revokeObjectURL(url);
            path = "pocket-tts-voice.wav";
          }

          if (!path) {
            // User cancelled the save dialog
            stream.getTracks().forEach(t => t.stop());
            return;
          }

          // Save the recorded WAV audio via backend
          try {
            const base64 = arrayBufferToBase64(wavBuffer);
            const saved = await api("save_audio", {
              wav_base64: base64,
              wavBase64: base64,
              file_path: path,
              filePath: path,
            });

            recordedPath = saved;
            cloneAudioPath = saved;
            selectedVoice = null;
            els.recChip.textContent = "recorded";
            els.voiceChip.textContent = path.split(/[\\/]/).pop();
            toast("Recording saved as WAV: " + path.split(/[\\/]/).pop());
            addClientLog("INFO", `Microphone recording saved: ${path}`);
          } catch (e) {
            toast("Failed to save recording: " + (e.message || e));
            addClientLog("ERROR", `Failed to save recording: ${e.message || e}`);
          }
        } catch (e) {
          toast("Recording error: " + (e.message || e));
        } finally {
          stream.getTracks().forEach(t => t.stop());
        }
      };
      mediaRecorder.onerror = e => {
        toast("MediaRecorder error: " + (e.error ? e.error.message : "unknown"));
        stream.getTracks().forEach(t => t.stop());
      };
      mediaRecorder.start();
      isRecording = true;
      els.btRecord.classList.add("recording");
      els.recIcon.textContent = "■";
      els.recLabel.textContent = "Stop recording";
      els.recChip.textContent = "recording…";
      toast("Recording from microphone — click Stop to save WAV");
      addClientLog("INFO", "Microphone recording started");
    } catch (e) {
      toast("Mic access denied: " + (e.message || e));
      addClientLog("ERROR", `Mic access denied: ${e.message || e}`);
    }
  }

  function stopRecording() {
    if (mediaRecorder && isRecording) {
      mediaRecorder.stop();
      isRecording = false;
      els.btRecord.classList.remove("recording");
      els.recIcon.textContent = "●";
      els.recLabel.textContent = "Record voice";
    }
  }

  function arrayBufferToBase64(buf) {
    let binary = "";
    const bytes = new Uint8Array(buf);
    for (let i = 0; i < bytes.byteLength; i++) binary += String.fromCharCode(bytes[i]);
    return btoa(binary);
  }

  // ---------- voice export (clone) ----------
  async function exportVoice() {
    const sourceAudio = cloneAudioPath || cloneImportPath || recordedPath;
    if (!sourceAudio) {
      toast("Record a voice or import an audio file first");
      return;
    }
    const exportPath = await pickSave(
      { name: "Safetensors", extensions: ["safetensors"] },
      "my-voice-clone.safetensors"
    );
    if (!exportPath) return;
    setEnabled(els.btClone, false);
    if (els.btCloneSave) setEnabled(els.btCloneSave, false);
    els.cloneStatus.textContent = "Exporting voice clone…";
    try {
      const msg = await api("export_voice", {
        audio_path: sourceAudio,
        audioPath: sourceAudio,
        export_path: exportPath,
        exportPath: exportPath,
        language: els.language.value || null,
        config: null,
      });
      els.cloneStatus.textContent = msg;
      els.cloneChip.textContent = exportPath.split(/[\\/]/).pop();
      toast("Voice clone exported: " + exportPath.split(/[\\/]/).pop());
      addClientLog("INFO", `Voice clone exported: ${exportPath}`);
    } catch (e) {
      els.cloneStatus.textContent = "Error: " + e;
      toast("Export failed: " + e);
      addClientLog("ERROR", `Voice clone export failed: ${e}`);
    } finally {
      setEnabled(els.btClone, true);
      if (els.btCloneSave) setEnabled(els.btCloneSave, true);
    }
  }

  // ---------- wiring ----------
  els.text.oninput = () => {
    els.chars.textContent = (els.text.value || "").length;
  };
  els.language.onchange = onLanguageChange;
  els.btLoad.onclick = loadModel;
  els.btGenerate.onclick = generate;
  els.btSave.onclick = saveAudio;
  if (els.btRenderFile) els.btRenderFile.onclick = () => renderTextFile();
  if (els.btResumeRender) els.btResumeRender.onclick = resumeRender;
  if (els.btStop) els.btStop.onclick = stopGeneration;
  if (els.btUpdate) els.btUpdate.onclick = checkForUpdates;
  els.btRecord.onclick = toggleRecord;

  // Clone actions: import audio file, import safetensors, record, export, and Hugging Face auth
  if (els.btCloneImport) {
    els.btCloneImport.onclick = async () => {
      const p = await pickOpen([
        {
          name: "Audio Files (*.wav, *.mp3, *.flac, *.ogg, *.m4a)",
          extensions: ["wav", "mp3", "flac", "ogg", "m4a", "webm"],
        },
        {
          name: "All Supported (*.safetensors, audio)",
          extensions: ["safetensors", "wav", "mp3", "flac", "ogg", "m4a", "webm"],
        },
      ]);
      if (!p) return;
      cloneImportPath = p;
      cloneAudioPath = p;
      selectedVoice = p;
      recordedPath = null;
      const filename = p.split(/[\\/]/).pop();
      if (els.cloneImportChip) {
        els.cloneImportChip.textContent = filename;
      }
      els.voiceChip.textContent = filename;
      els.cloneStatus.textContent = "Imported: " + filename;
      toast("Audio imported for voice clone: " + filename);
      renderVoices(voicesCache, els.language.value, activeGenderFilter);
      addClientLog("INFO", `Imported audio for clone: ${p}`);
    };
  }

  if (els.btCloneSafetensors) {
    els.btCloneSafetensors.onclick = async () => {
      const p = await pickOpen([
        {
          name: "Voice Clone Model (*.safetensors)",
          extensions: ["safetensors"],
        },
        {
          name: "All Supported (*.safetensors, audio)",
          extensions: ["safetensors", "wav", "mp3", "flac", "ogg", "m4a", "webm"],
        },
      ]);
      if (!p) return;
      selectedVoice = p;
      cloneAudioPath = p;
      cloneImportPath = p;
      recordedPath = null;
      const filename = p.split(/[\\/]/).pop();
      if (els.cloneSafetensorsChip) {
        els.cloneSafetensorsChip.textContent = filename;
      }
      els.voiceChip.textContent = filename;
      els.cloneStatus.textContent = "Voice model loaded: " + filename;
      toast("Voice clone (.safetensors) loaded: " + filename);
      renderVoices(voicesCache, els.language.value, activeGenderFilter);
      addClientLog("INFO", `Imported voice clone (.safetensors): ${p}`);
    };
  }
  els.btClone.onclick = exportVoice;
  if (els.btCloneSave) els.btCloneSave.onclick = exportVoice;
  if (els.btSaveHfToken) els.btSaveHfToken.onclick = saveHfToken;

  // Gender filter pills
  if (els.genderFilterBtns) {
    els.genderFilterBtns.forEach(btn => {
      btn.onclick = () => {
        els.genderFilterBtns.forEach(b => b.classList.remove("active"));
        btn.classList.add("active");
        activeGenderFilter = btn.dataset.gender || "all";
        renderVoices(voicesCache, els.language ? els.language.value : null, activeGenderFilter);
      };
    });
  }

  // Terminal actions
  if (els.terminalHeader) els.terminalHeader.onclick = (e) => {
    // Only toggle if click didn't land on action buttons
    if (!e.target.closest(".terminal-actions")) toggleTerminal();
  };
  if (els.btTermToggle) els.btTermToggle.onclick = toggleTerminal;
  if (els.btTermCopy) els.btTermCopy.onclick = copyTerminalLogs;
  if (els.btTermClear) els.btTermClear.onclick = clearTerminal;
  if (els.btTermSave) els.btTermSave.onclick = saveLogs;

  els.btTheme.onclick = toggleLightDark;
  els.btThemeVariant.onclick = cycleThemeVariant;
  els.btSurfaceGlass.onclick = () => setSurface("glass");
  els.btSurfaceSolid.onclick = () => setSurface("solid");
  els.btSurfaceAcrylic.onclick = () => setSurface("acrylic");

  const bindSlider = (input, label) => {
    updateRangeFill(input);
    input.oninput = () => {
      input.dataset.unset = "false";
      label.textContent = input.value;
      updateRangeFill(input);
    };
  };
  bindSlider(els.temp, els.tempVal);
  bindSlider(els.eos, els.eosVal);
  bindSlider(els.steps, els.stepsVal);
  bindSlider(els.maxtok, els.maxtokVal);
  bindSlider(els.nc, els.ncVal);
  updateRangeFill(els.audioScrub);

  // Every control that changes how speech sounds is remembered for the next launch.
  [els.temp, els.eos, els.steps, els.maxtok, els.nc, els.language].forEach(input => {
    if (input) input.addEventListener("input", saveSettings);
    if (input) input.addEventListener("change", saveSettings);
  });
  [els.tforce, els.quantize, els.previewVoices].forEach(input => {
    if (input) input.addEventListener("change", saveSettings);
  });

  els.btVoicePick.onclick = async () => {
    const p = await pickOpen([
      {
        name: "All Supported Voices (*.safetensors, audio)",
        extensions: ["safetensors", "wav", "mp3", "flac", "ogg", "m4a", "webm"],
      },
      {
        name: "Voice Clone Model (*.safetensors)",
        extensions: ["safetensors"],
      },
      {
        name: "Audio Files (*.wav, *.mp3, *.flac, *.ogg)",
        extensions: ["wav", "mp3", "flac", "ogg", "m4a", "webm"],
      },
    ]);
    if (!p) return;
    selectedVoice = p;
    recordedPath = null;
    els.recChip.textContent = "none";
    const filename = p.split(/[\\/]/).pop();
    els.voiceChip.textContent = filename;
    toast("Selected voice: " + filename);
    renderVoices(voicesCache, els.language.value, activeGenderFilter);
    addClientLog("INFO", `Selected custom voice: ${p}`);
  };

  // Keyboard shortcuts: Ctrl+Enter (generate) and Space (play/pause)
  window.addEventListener("keydown", (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
      e.preventDefault();
      if (!els.btGenerate.disabled) {
        generate();
      }
      return;
    }

    if (e.code === "Space" || e.key === " ") {
      const active = document.activeElement;
      const isEditing = active && (
        active.tagName === "INPUT" ||
        active.tagName === "TEXTAREA" ||
        active.tagName === "SELECT" ||
        active.isContentEditable
      );
      if (!isEditing) {
        e.preventDefault();
        if (els.audio.src) {
          if (els.audio.paused) {
            els.audio.play().catch(() => {});
          } else {
            els.audio.pause();
          }
        }
      }
    }
  });

  window.addEventListener("resize", () => drawWaveform(lastSamples));

  // ---------- boot ----------
  (async () => {
    const savedTheme = getSavedTheme();
    setSurface(getSavedSurface());
    setThemeVariant(savedTheme);

    drawWaveform(null);
    applySettings(readSettings());
    await loadLanguages();
    els.voiceChip.textContent = selectedVoiceName;
    setBusy(null);
    // An interrupted render stays offered until it is finished or its progress is discarded.
    writeResumeJob(readResumeJob());
    await checkHfStatus();

    // Fetch initial logs from backend
    try {
      const logs = await api("get_terminal_logs");
      if (Array.isArray(logs)) {
        logs.forEach(appendTerminalLine);
      }
    } catch {}

    // Listen to live backend logs if Tauri event system is available
    if (hasTauri && window.__TAURI__ && window.__TAURI__.event) {
      window.__TAURI__.event.listen("terminal_log", e => {
        if (e.payload) appendTerminalLine(e.payload);
      });
      window.__TAURI__.event.listen("render_progress", e => {
        const seconds = e.payload && e.payload.seconds;
        if (typeof seconds === "number") {
          els.genStatus.textContent = `Rendering… ${Math.round(seconds)}s of audio so far`;
        }
      });
      // Minimize to tray on close when the setting is on.
    if (hasTauri && window.__TAURI__ && window.__TAURI__.window) {
      const aw = window.__TAURI__.window.getCurrentWindow();
      aw.onCloseRequested(async (e) => {
        const minimize = els.minimizeToTray ? els.minimizeToTray.checked : false;
        e.preventDefault();
        if (minimize) {
          try { await aw.hide(); } catch {}
        } else {
          // Exit the whole app (the hidden selection-popup window would otherwise keep it alive).
          try { if (invoke) await invoke("quit_app"); } catch {}
          try { await aw.destroy(); } catch {}
        }
      }).catch(() => {});
    }

    // Keep the backend selection-popup watcher in step with the toggle.
    if (els.selectionPopup) {
      const syncPopup = () => {
        if (invoke) invoke("set_selection_popup_enabled", { enabled: !!els.selectionPopup.checked }).catch(() => {});
      };
      els.selectionPopup.addEventListener("change", syncPopup);
      syncPopup();
    }
    if (els.minimizeToTray) {
      els.minimizeToTray.addEventListener("change", saveSettings);
    }
    if (els.selectionPopup) {
      els.selectionPopup.addEventListener("change", saveSettings);
    }

    // Actions coming from the floating selection popup.
    if (hasTauri && window.__TAURI__ && window.__TAURI__.event) {
      window.__TAURI__.event.listen("popup-action", async (e) => {
        const p = (e && e.payload) || {};
        const text = String(p.text || "");
        if (!text.trim()) return;
        const action = p.action;
        // "Open" has to bring this window to the front, not just un-hide it: the
        // popup takes the focus while it is clicked, and hiding it hands focus back
        // to the app the text came from. Un-minimize, show, then focus.
        const raise = async () => {
          try {
            const win = window.__TAURI__.window.getCurrentWindow();
            await win.unminimize().catch(() => {});
            await win.show().catch(() => {});
            await win.setFocus().catch(() => {});
          } catch {}
        };
        await raise();
        els.text.value = text;
        els.text.dispatchEvent(new Event("input"));
        if (action === "open") {
          // The popup hides itself as soon as its emit resolves, which can take the
          // focus back; claim it again once that has settled.
          setTimeout(async () => {
            try { els.text.focus(); els.text.scrollIntoView({ block: "nearest" }); } catch {}
            await raise();
          }, 120);
          return;
        }
        if (busyKind) {
          toast("Pocket TTS is busy — the text is waiting in the box");
          return;
        }
        await generate();
        if (action === "save") {
          await saveAudio();
        }
      });
    }

    // The global shortcut (Ctrl+Alt+S) and the tray menu hand text over this way.
      window.__TAURI__.event.listen("speak-text", e => {
        const text = String((e.payload && e.payload) || "");
        if (!text.trim()) return;
        els.text.value = text;
        els.text.dispatchEvent(new Event("input"));
        addClientLog("INFO", "Clipboard text loaded from the global shortcut.");
        if (busyKind) {
          toast("Pocket TTS is busy — the text is waiting in the box");
          return;
        }
        generate();
      });
    }

    addClientLog("INFO", "Pocket TTS Liquid Glass UI initialized.");
  })();
})();
