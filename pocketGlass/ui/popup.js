(() => {
  "use strict";
  const hasTauri = !!window.__TAURI__;
  const core = hasTauri ? (window.__TAURI__.core || {}) : {};
  const invoke = core.invoke || null;
  const eventApi = (hasTauri && (window.__TAURI__.event || core.event)) || {};
  const listen = eventApi.listen ? (...a) => eventApi.listen(...a) : null;
  const emit = eventApi.emit ? (...a) => eventApi.emit(...a) : null;
  const appWindow = hasTauri && window.__TAURI__.window ? window.__TAURI__.window.getCurrentWindow() : null;
  const actionButtons = [...document.querySelectorAll(".btns button")];
  const preview = document.getElementById("preview");
  let currentText = "";
  let hideTimer = null;
  let sending = false;

  function cancelHide() {
    if (hideTimer !== null) clearTimeout(hideTimer);
    hideTimer = null;
  }

  function setText(text) {
    currentText = text || "";
    const chars = Array.from(currentText);
    preview.textContent = chars.length > 60 ? chars.slice(0, 60).join("") + "…" : (currentText || "Selected text");
    preview.title = currentText;
  }

  async function hidePopup() {
    cancelHide();
    setText("");
    // Ask the backend as well: this is what guarantees the window actually goes away.
    if (invoke) {
      try { await invoke("hide_popup"); return; } catch (e) { console.warn("Could not hide selection popup from backend", e); }
    }
    if (appWindow) {
      try { await appWindow.hide(); } catch (e) { console.warn("Could not hide selection popup", e); }
    }
  }

  function scheduleHide() {
    cancelHide();
    hideTimer = setTimeout(hidePopup, 10000);
  }

  // The popup can be shown before this page has loaded, in which case its text event
  // was already sent. Ask the backend so the buttons never act on nothing.
  async function fetchText() {
    if (!invoke) return "";
    try { return String((await invoke("get_popup_text")) || ""); } catch (e) { return ""; }
  }

  async function send(action) {
    if (sending) return;
    let text = currentText;
    if (!text.trim()) {
      text = await fetchText();
      if (text.trim()) setText(text);
    }
    if (!text.trim()) return;
    sending = true;
    cancelHide();
    actionButtons.forEach(button => { button.disabled = true; });
    let delivered = false;
    try {
      if (emit) {
        await emit("popup-action", { action, text });
        delivered = true;
      }
    } catch (e) {
      console.error("Could not deliver selection popup action", e);
    } finally {
      sending = false;
      actionButtons.forEach(button => { button.disabled = false; });
    }
    // Keep the action available if delivery failed instead of silently discarding it.
    if (delivered) await hidePopup();
    else scheduleHide();
  }

  document.getElementById("gen-save").onclick = () => send("save");
  document.getElementById("play").onclick = () => send("play");
  document.getElementById("open").onclick = () => send("open");
  document.getElementById("close").onclick = hidePopup;
  document.addEventListener("keydown", event => {
    if (event.key === "Escape") {
      event.preventDefault();
      hidePopup();
    }
  });
  document.addEventListener("mouseenter", cancelHide);
  document.addEventListener("focusin", cancelHide);
  document.addEventListener("mouseleave", scheduleHide);
  document.addEventListener("focusout", () => {
    if (!document.hasFocus()) scheduleHide();
  });

  function applyTheme(theme) {
    if (theme) document.documentElement.setAttribute("data-theme", theme);
  }
  if (listen) {
    const onTheme = event => applyTheme(event && event.payload);
    listen("popup-theme", onTheme).catch(e => console.warn("Popup theme listener unavailable", e));
    listen("theme-changed", onTheme).catch(e => console.warn("Theme listener unavailable", e));
    // Rust positions and shows the window (without stealing focus from the app the
    // text was selected in); this only fills it in and arms the auto-hide timer.
    listen("popup-text", event => {
      setText((event && event.payload) || "");
      scheduleHide();
    }).catch(e => console.error("Popup text listener unavailable", e));
  }
  if (invoke) {
    invoke("get_app_theme").then(applyTheme).catch(e => console.warn("Could not load popup theme", e));
    fetchText().then(text => {
      if (text.trim() && !currentText) {
        setText(text);
        scheduleHide();
      }
    });
  }
})();
