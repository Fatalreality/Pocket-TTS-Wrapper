// The GUI owns its own window: never attach a console, not even on a debug build, so
// launching the app does not open a terminal beside it. Logs go to the in-app console.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

//! Pocket TTS desktop application with Liquid Glass UI.
//! A Tauri-based GUI wrapping the Pocket TTS Rust engine.

mod audio;
mod cli;
mod commands;
mod config;
mod defaults;
mod download;
mod encode;
mod logging;
mod model;
mod paths;
mod rng;
mod state;
mod text;
mod tokenizer;
mod weights;

use commands::{
    app_info, cancel_generation, check_update, clear_terminal_logs, export_voice, generate,
    get_hf_status, get_languages, get_terminal_logs, get_voices, install_update, load_model,
    render_text_file, save_audio, save_logs, set_hf_token,
};
use logging::init as logging_init;
use state::AppState;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

static CURRENT_THEME: Mutex<String> = Mutex::new(String::new());
/// The text the popup is currently offering. The popup is shown before its page may
/// have finished loading, so it can ask for this instead of losing the emit.
static POPUP_TEXT: Mutex<String> = Mutex::new(String::new());
static SELECTION_POPUP_ENABLED: AtomicBool = AtomicBool::new(true);
use tauri::Emitter;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

#[cfg(target_os = "windows")]
#[repr(C)]
struct Margins {
    left: i32,
    right: i32,
    top: i32,
    bottom: i32,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct AccentPolicy {
    state: i32,
    flags: i32,
    gradient_color: u32,
    animation_id: i32,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct WindowCompositionAttributeData {
    attribute: i32,
    data: *mut std::ffi::c_void,
    size: usize,
}

#[cfg(target_os = "windows")]
type SetWindowCompositionAttributeFn =
    unsafe extern "system" fn(isize, *mut WindowCompositionAttributeData) -> i32;

#[cfg(target_os = "windows")]
#[link(name = "dwmapi")]
extern "system" {
    fn DwmExtendFrameIntoClientArea(hwnd: isize, margins: *const Margins) -> i32;
    fn DwmSetWindowAttribute(
        hwnd: isize,
        attribute: u32,
        value: *const std::ffi::c_void,
        size: u32,
    ) -> i32;
}

#[cfg(target_os = "windows")]
#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryA(name: *const u8) -> isize;
    fn GetProcAddress(module: isize, name: *const u8) -> *const std::ffi::c_void;
}

#[cfg(target_os = "windows")]
#[link(name = "ntdll")]
extern "system" {
    fn RtlGetVersion(info: *mut RtlOsVersionInfoW) -> i32;
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct RtlOsVersionInfoW {
    size: u32,
    major: u32,
    minor: u32,
    build: u32,
    platform: u32,
    csd: [u16; 128],
}

/// Windows build number, e.g. 19045 for Windows 10 22H2 and 22631 for Windows 11 23H2.
#[cfg(target_os = "windows")]
fn windows_build() -> u32 {
    let mut info: RtlOsVersionInfoW = unsafe { std::mem::zeroed() };
    info.size = std::mem::size_of::<RtlOsVersionInfoW>() as u32;
    // SAFETY: `info` is a correctly sized and initialised RtlOsVersionInfoW.
    let status = unsafe { RtlGetVersion(&mut info) };
    if status == 0 {
        info.build
    } else {
        0
    }
}

/// Asks the compositor to blur whatever is behind the window. The undocumented
/// `SetWindowCompositionAttribute` accent policy is the only path that samples the windows
/// and wallpaper behind us. Windows 11 removed it: there the call succeeds but paints the
/// client area solid black, so it is only used on Windows 10.
///
/// `acrylic` selects the tinted acrylic variant (the Windows 10 acrylic look) instead of
/// the plain blur behind.
#[cfg(target_os = "windows")]
unsafe fn blur_behind(hwnd: isize, acrylic: bool) -> bool {
    const WCA_ACCENT_POLICY: i32 = 19;
    const ACCENT_ENABLE_BLURBEHIND: i32 = 3;
    const ACCENT_ENABLE_ACRYLICBLURBEHIND: i32 = 4;
    const ACCENT_FLAG_DRAW_ALL_BORDERS: i32 = 2;

    let module = LoadLibraryA(b"user32.dll\0".as_ptr());
    if module == 0 {
        return false;
    }
    let symbol = GetProcAddress(module, b"SetWindowCompositionAttribute\0".as_ptr());
    if symbol.is_null() {
        return false;
    }
    // SAFETY: the symbol is resolved from user32 and its signature is the documented one.
    let set_attribute: SetWindowCompositionAttributeFn = std::mem::transmute(symbol);

    let mut policy = AccentPolicy {
        state: if acrylic {
            ACCENT_ENABLE_ACRYLICBLURBEHIND
        } else {
            ACCENT_ENABLE_BLURBEHIND
        },
        flags: ACCENT_FLAG_DRAW_ALL_BORDERS,
        // No tint for a plain blur: the page supplies the glass colour. Acrylic requires
        // one, in AABBGGRR, with a near-transparent alpha.
        gradient_color: if acrylic { 0x01_00_00_00 } else { 0 },
        animation_id: 0,
    };
    let mut data = WindowCompositionAttributeData {
        attribute: WCA_ACCENT_POLICY,
        data: (&mut policy as *mut AccentPolicy).cast(),
        size: std::mem::size_of::<AccentPolicy>(),
    };
    set_attribute(hwnd, &mut data) != 0
}

/// Applies the window chrome for a theme: dark titles/materials for the dark and emerald
/// themes, light ones for the two light themes. The blur itself is theme independent.
///
/// `acrylic` asks Windows for its own acrylic material behind the client area, which is what
/// the surface toggle's third mode selects; the other modes clear it again.
#[cfg(target_os = "windows")]
fn apply_glass(window: &tauri::WebviewWindow, dark: bool, acrylic: bool) {
    const DWMWA_SYSTEMBACKDROP_TYPE: u32 = 38;
    const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
    const DWMSBT_NONE: i32 = 1; // 0 is DWMSBT_AUTO, which still lets DWM pick a material
    const DWMSBT_TRANSIENTWINDOW: i32 = 3; // acrylic

    let Ok(hwnd) = window.hwnd() else {
        log::warn!("no native window handle for the backdrop effect");
        return;
    };
    let hwnd = hwnd.0 as isize;
    let margins = Margins {
        left: -1,
        right: -1,
        top: -1,
        bottom: -1,
    };
    // SAFETY: the handle comes from the window we just created, and the pointers refer to
    // locals that outlive every call below.
    unsafe {
        let mode: i32 = i32::from(dark);
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            (&mode as *const i32).cast(),
            4,
        );

        // The frame has to cover the client area, otherwise Windows paints that area with
        // an opaque brush and nothing behind the window can show through.
        let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);

        // Windows 11 has no blur that samples the desktop for a normal window: the legacy
        // accent policy paints this window solid black, and the DWM system backdrop (acrylic)
        // is painted as a flat tint on many displays - it hid the desktop completely. So the
        // window stays fully transparent there and the page's own glass supplies the tint,
        // unless the acrylic surface mode explicitly asks the compositor for its material.
        if acrylic {
            if windows_build() >= 22_000 {
                let _ = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_SYSTEMBACKDROP_TYPE,
                    (&DWMSBT_TRANSIENTWINDOW as *const i32).cast(),
                    4,
                );
                log::info!("acrylic system backdrop active");
            } else if blur_behind(hwnd, true) {
                log::info!("acrylic accent backdrop active");
            } else {
                log::warn!("acrylic backdrop unavailable; keeping the transparent window");
            }
        } else if windows_build() < 22_000 && blur_behind(hwnd, false) {
            log::info!("desktop blur backdrop active");
        } else {
            let none = DWMSBT_NONE;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_SYSTEMBACKDROP_TYPE,
                (&none as *const i32).cast(),
                4,
            );
            log::info!("window transparent; the desktop shows through without a blur");
        }
    }
}

/// Re-applies the window chrome when the frontend switches theme, so the dark and emerald
/// themes stop sitting on the title bar and material of the light one, and applies or
/// clears the acrylic backdrop when the surface toggle changes.
#[tauri::command]
fn set_window_style(window: tauri::WebviewWindow, theme: String, surface: String) {
    #[cfg(target_os = "windows")]
    {
        let dark = matches!(theme.as_str(), "dark" | "green");
        let acrylic = surface == "acrylic";
        apply_glass(&window, dark, acrylic);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (window, theme, surface);
    }
}

/// The global shortcut that speaks whatever is on the clipboard: Ctrl+Alt+S.
fn speak_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyS)
}

/// Brings the window back from the tray, un-minimized and focused.
fn show_main(app: &tauri::AppHandle) {
    use tauri::Manager;
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Reads the clipboard and asks the frontend to speak it. Used by the tray menu and the global
/// shortcut, so text from any application can be read aloud without copying it into the app.
fn speak_clipboard(app: &tauri::AppHandle) {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    let text = match app.clipboard().read_text() {
        Ok(text) if !text.trim().is_empty() => text,
        Ok(_) => {
            log::warn!("Speak clipboard: the clipboard has no text");
            return;
        }
        Err(e) => {
            log::warn!("Speak clipboard: cannot read the clipboard: {e}");
            return;
        }
    };
    show_main(app);
    if let Err(e) = app.emit("speak-text", text) {
        log::warn!("Speak clipboard: cannot reach the window: {e}");
    }
}

/// Tray icon: show the window, speak the clipboard, quit. Closing the window keeps the app in
/// the tray, so the shortcut keeps working.
fn build_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::TrayIconBuilder;

    let show = MenuItem::with_id(app, "show", "Show Pocket TTS", true, None::<&str>)?;
    let speak = MenuItem::with_id(
        app,
        "speak",
        "Speak clipboard (Ctrl+Alt+S)",
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &speak, &quit])?;

    let mut tray = TrayIconBuilder::with_id("pocket-tts")
        .tooltip("Pocket TTS — Ctrl+Alt+S speaks the clipboard")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main(app),
            "speak" => speak_clipboard(app),
            "quit" => app.exit(0),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

/// A UTF-16, NUL-terminated copy of `text`, for the Win32 calls below.
#[cfg(target_os = "windows")]
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(target_os = "windows")]
#[link(name = "user32")]
extern "system" {
    fn MessageBoxW(hwnd: isize, text: *const u16, caption: *const u16, flags: u32) -> i32;
}

#[cfg(target_os = "windows")]
#[link(name = "shell32")]
extern "system" {
    fn ShellExecuteW(
        hwnd: isize,
        operation: *const u16,
        file: *const u16,
        parameters: *const u16,
        directory: *const u16,
        show: i32,
    ) -> isize;
}

/// Why the app could not start, in a dialog instead of a silent exit: the usual cause is a
/// missing WebView2 runtime, and the message offers the download page so it can be fixed.
#[cfg(target_os = "windows")]
fn report_startup_failure(error: &str) -> ! {
    const MB_ICONERROR: u32 = 0x10;
    const MB_YESNO: u32 = 0x04;
    const IDYES: i32 = 6;
    const SW_SHOWNORMAL: i32 = 1;
    const WEBVIEW2_DOWNLOAD: &str = "https://go.microsoft.com/fwlink/p/?LinkId=2124703";

    let message = format!(
        "Pocket TTS could not start.\n\nThe Microsoft Edge WebView2 runtime is usually the \
         missing piece on Windows 10; installing it and starting Pocket TTS again fixes this.\n\n\
         Open the WebView2 download page now?\n\nDetails: {error}"
    );
    let text = wide(&message);
    let caption = wide("Pocket TTS");
    // SAFETY: both strings are correctly sized NUL-terminated UTF-16 buffers that outlive the
    // call, and the handle is 0 for "no owner window", which is what a pre-window failure has.
    let answer = unsafe { MessageBoxW(0, text.as_ptr(), caption.as_ptr(), MB_ICONERROR | MB_YESNO) };
    if answer == IDYES {
        let operation = wide("open");
        let url = wide(WEBVIEW2_DOWNLOAD);
        // SAFETY: same as above; ShellExecuteW only opens the URL in the default browser.
        unsafe {
            ShellExecuteW(
                0,
                operation.as_ptr(),
                url.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            );
        }
    }
    std::process::exit(1)
}

#[cfg(not(target_os = "windows"))]
fn report_startup_failure(error: &str) -> ! {
    eprintln!("Pocket TTS could not start: {error}");
    std::process::exit(1)
}


fn popup_axis_position(cursor: i32, extent: u32, origin: i32, available: u32, gap: i32, margin: i32) -> i32 {
    let min = origin.saturating_add(margin);
    let max = origin
        .saturating_add(available as i32)
        .saturating_sub(extent as i32)
        .saturating_sub(margin)
        .max(min);
    let preferred = cursor.saturating_add(gap);
    let flipped = cursor.saturating_sub(extent as i32).saturating_sub(gap);
    (if preferred > max { flipped } else { preferred }).clamp(min, max)
}

/// Position the popup beside the cursor, flipping at screen edges and respecting the taskbar.
fn popup_position(
    window: &tauri::WebviewWindow,
    cursor_x: i32,
    cursor_y: i32,
) -> tauri::PhysicalPosition<i32> {
    let (width, height) = window
        .outer_size()
        .map(|size| (size.width, size.height))
        .unwrap_or((280, 184));
    let monitor = window
        .monitor_from_point(cursor_x as f64, cursor_y as f64)
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten());
    if let Some(monitor) = monitor {
        let area = monitor.work_area();
        let gap = (16.0 * monitor.scale_factor()).round() as i32;
        let margin = (8.0 * monitor.scale_factor()).round() as i32;
        return tauri::PhysicalPosition::new(
            popup_axis_position(cursor_x, width, area.position.x, area.size.width, gap, margin),
            popup_axis_position(cursor_y, height, area.position.y, area.size.height, gap, margin),
        );
    }
    tauri::PhysicalPosition::new(cursor_x.saturating_add(16), cursor_y.saturating_add(16))
}

/// True when the foreground window belongs to this process (the main window or the
/// selection popup). Focus moving into our own windows must not be mistaken for the
/// user having cleared their selection, or the popup hides and immediately reappears.
#[cfg(target_os = "windows")]
fn foreground_is_own() -> bool {
    use windows::Win32::System::Threading::GetCurrentProcessId;
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    unsafe {
        let fg = GetForegroundWindow();
        if fg.0.is_null() {
            return false;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(fg, Some(&mut pid));
        pid != 0 && pid == GetCurrentProcessId()
    }
}

/// Shows the popup. This must go through Tauri's own show: a bare Win32 show leaves
/// the WebView2 controller hidden, so the window appears but ignores every click.
fn show_popup(window: &tauri::WebviewWindow) {
    let _ = window.show();
}

/// Reads selected text using Windows UI Automation, with its cursor position.
#[cfg(target_os = "windows")]
fn selected_text_and_cursor() -> Option<(String, i32, i32)> {
    use windows::core::Interface;
    use windows::Win32::Foundation::POINT;
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
    use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation, IUIAutomationTextPattern, UIA_TextPatternId};
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
    unsafe {
        if foreground_is_own() {
            return None;
        }
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let automation: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
        let element = automation.GetFocusedElement().ok()?;
        if element.CurrentIsPassword().ok().unwrap_or_default().as_bool() { return None; }
        let pattern = element.GetCurrentPattern(UIA_TextPatternId).ok()?;
        let text_pattern: IUIAutomationTextPattern = pattern.cast::<IUIAutomationTextPattern>().ok()?;
        let selection = text_pattern.GetSelection().ok()?;
        if selection.Length().ok()? == 0 { return None; }
        let text = selection.GetElement(0).ok()?.GetText(-1).ok()?.to_string();
        if text.trim().is_empty() { return None; }
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        Some((text, pt.x, pt.y))
    }
}

#[cfg(not(target_os = "windows"))]
fn selected_text_and_cursor() -> Option<(String, i32, i32)> { None }

#[tauri::command]
fn set_selection_popup_enabled(enabled: bool) {
    SELECTION_POPUP_ENABLED.store(enabled, Ordering::Relaxed);
}

#[tauri::command]
fn set_app_theme(theme: String) {
    if let Ok(mut guard) = CURRENT_THEME.lock() { *guard = theme; }
}

#[tauri::command]
fn get_app_theme() -> String {
    CURRENT_THEME.lock().map(|g| if g.is_empty() { "dark".to_string() } else { g.clone() }).unwrap_or_else(|_| "dark".to_string())
}

#[tauri::command]
fn quit_app(app: tauri::AppHandle) { app.exit(0); }

/// Hides the selection popup from the backend. The popup hides itself too, but doing it
/// here keeps the window away for certain when the webview's own hide is a no-op.
#[tauri::command]
fn get_popup_text() -> String {
    POPUP_TEXT.lock().map(|g| g.clone()).unwrap_or_default()
}

#[tauri::command]
fn hide_popup(app: tauri::AppHandle) {
    use tauri::Manager;
    if let Ok(mut guard) = POPUP_TEXT.lock() { guard.clear(); }
    if let Some(window) = app.get_webview_window("popup") {
        let _ = window.hide();
        #[cfg(target_os = "windows")]
        if window.is_visible().unwrap_or(false) {
            if let Ok(hwnd) = window.hwnd() {
                use windows::Win32::Foundation::HWND;
                use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};
                unsafe { let _ = ShowWindow(HWND(hwnd.0), SW_HIDE); }
            }
        }
    }
}

#[tauri::command]
fn get_cached_preview(language: String, voice: String, app: tauri::AppHandle) -> Option<String> {
    use tauri::Manager;
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    // Both parts become a file name, so anything but a plain id is refused rather than
    // allowed to walk out of the previews folder.
    let filename = format!(
        "{}_{}.ogg",
        crate::paths::safe_name(&language)?,
        crate::paths::safe_name(&voice)?
    );
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(dir) = app.path().resource_dir() { candidates.push(dir.join("previews").join(&filename)); }
    candidates.push(std::path::PathBuf::from("previews").join(&filename));
    if let Ok(cwd) = std::env::current_dir() { candidates.push(cwd.join("previews").join(&filename)); }
    for candidate in candidates {
        if let Ok(bytes) = std::fs::read(&candidate) { return Some(STANDARD.encode(&bytes)); }
    }
    None
}

fn main() {
    logging_init(false);
    if std::env::var_os("RAYON_NUM_THREADS").is_none() {
        let cpus = std::thread::available_parallelism().map_or(1, |n| n.get());
        xn::set_num_threads(4.min(cpus));
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() == ShortcutState::Pressed && *shortcut == speak_shortcut() {
                        speak_clipboard(app);
                    }
                })
                .build(),
        )
        .manage(Arc::new(AppState::default()))
        .invoke_handler(tauri::generate_handler![
            load_model,
            get_voices,
            get_languages,
            generate,
            cancel_generation,
            save_audio,
            export_voice,
            app_info,
            get_hf_status,
            set_hf_token,
            get_terminal_logs,
            clear_terminal_logs,
            save_logs,
            render_text_file,
            check_update,
            install_update,
            set_window_style,
            set_selection_popup_enabled,
            set_app_theme,
            get_app_theme,
            get_cached_preview,
            get_popup_text,
            hide_popup,
            quit_app,
        ])
        .setup(|app| {
            // Register app handle for terminal log broadcast to the frontend
            logging::set_app_handle(app.handle().clone());
            logging::add_log_entry("INFO", "Pocket TTS engine initialized successfully.");


            // Floating popup for text selected in any application.
            if let Err(e) = tauri::WebviewWindowBuilder::new(
                app,
                "popup",
                tauri::WebviewUrl::App("popup.html".into()),
            )
            .title("Selection")
            .inner_size(280.0, 184.0)
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .visible(false)
            .build() {
                log::warn!("no selection popup window: {e}");
            }

            let handle = app.handle().clone();
            std::thread::spawn(move || {
                use tauri::Manager;
                let mut last_shown = String::new();
                let mut missing = 0u32;
                let mut pending: Option<(String, i32, i32, std::time::Instant)> = None;
                loop {
                    let enabled = SELECTION_POPUP_ENABLED.load(Ordering::Relaxed);
                    std::thread::sleep(std::time::Duration::from_millis(
                        if !enabled { 1000 } else if last_shown.is_empty() && pending.is_none() { 800 } else { 300 }
                    ));
                    if !enabled {
                        if !last_shown.is_empty() {
                            last_shown.clear();
                            pending = None;
                            if let Some(w) = handle.get_webview_window("popup") { let _ = w.hide(); }
                        }
                        continue;
                    }
                    // Our own windows taking focus (the popup was clicked, or it is
                    // closing) says nothing about the user's selection: leave the
                    // popup and last_shown untouched so they cannot flicker.
                    #[cfg(target_os = "windows")]
                    if foreground_is_own() {
                        continue;
                    }
                    #[cfg(target_os = "windows")]
                    match selected_text_and_cursor() {
                        Some((text, x, y)) => {
                            missing = 0;
                            if text == last_shown { pending = None; }
                            else if let Some((ref old, _, _, started)) = pending {
                                if *old != text { pending = Some((text, x, y, std::time::Instant::now())); }
                                else if started.elapsed() >= std::time::Duration::from_millis(350) {
                                    if let Some(w) = handle.get_webview_window("popup") {
                                        let _ = w.set_position(popup_position(&w, x, y));
                                        show_popup(&w);
                                    }
                                    if let Ok(mut guard) = POPUP_TEXT.lock() { *guard = text.clone(); }
                                    let _ = handle.emit_to("popup", "popup-text", text.clone());
                                    let theme = CURRENT_THEME.lock().map(|g| if g.is_empty() { "dark".to_string() } else { g.clone() }).unwrap_or_else(|_| "dark".to_string());
                                    let _ = handle.emit_to("popup", "popup-theme", theme);
                                    last_shown = text;
                                    pending = None;
                                }
                            } else { pending = Some((text, x, y, std::time::Instant::now())); }
                        }
                        None => {
                            pending = None;
                            // A single failed read is not the user clearing the text:
                            // focus shifting (including our own popup opening or its
                            // timeout closing it) makes the selection briefly
                            // unreadable. Treating that as "selection gone" cleared
                            // last_shown, so the still-selected text re-opened the
                            // popup in a loop. Require several misses in a row, and
                            // more of them while the popup is on screen.
                            missing += 1;
                            let popup_visible = handle
                                .get_webview_window("popup")
                                .is_some_and(|w| w.is_visible().unwrap_or(false));
                            let needed = if popup_visible { 12 } else { 3 };
                            if !last_shown.is_empty() && missing >= needed {
                                missing = 0;
                                last_shown.clear();
                                if let Some(w) = handle.get_webview_window("popup") { let _ = w.hide(); }
                            }
                        }
                    }
                }
            });

            // The tray and the global shortcut are how the app is used from another window:
            // select text anywhere, press Ctrl+Alt+S (or use the tray), and it is spoken here.
            if let Err(e) = build_tray(app.handle()) {
                log::warn!("no tray icon: {e}");
            }
            match app.global_shortcut().register(speak_shortcut()) {
                Ok(()) => logging::add_log_entry(
                    "INFO",
                    "Global shortcut Ctrl+Alt+S speaks the clipboard.",
                ),
                Err(e) => {
                    log::warn!("Ctrl+Alt+S is unavailable: {e}");
                    logging::add_log_entry(
                        "WARN",
                        &format!("Global shortcut Ctrl+Alt+S is unavailable: {e}"),
                    );
                }
            }

            // Blur whatever is behind the window without painting a background: the
            // frontend supplies the glass colour, the desktop shows through it.
            #[cfg(target_os = "windows")]
            {
                use tauri::Manager;
                if let Some(window) = app.get_webview_window("main") {
                    // `transparent: true` alone leaves the WebView2 controller painting an
                    // opaque, theme-derived base on recent runtimes. Setting the background
                    // to a fully transparent colour is what actually lets the desktop show
                    // through the page.
                    use tauri::utils::config::Color;
                    if let Err(e) = window.set_background_color(Some(Color(0, 0, 0, 0))) {
                        log::warn!("could not clear the webview background: {e}");
                    }
                    apply_glass(&window, false, false);
                    logging::add_log_entry(
                        "INFO",
                        "Desktop blur backdrop requested (accent blur behind).",
                    );
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| report_startup_failure(&e.to_string()));
}
