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
use std::sync::Arc;
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
        ])
        .setup(|app| {
            // Register app handle for terminal log broadcast to the frontend
            logging::set_app_handle(app.handle().clone());
            logging::add_log_entry("INFO", "Pocket TTS engine initialized successfully.");

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
