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
mod logging;
mod model;
mod rng;
mod state;
mod text;
mod tokenizer;
mod weights;

use commands::{
    app_info, clear_terminal_logs, export_voice, generate, get_hf_status, get_languages,
    get_terminal_logs, get_voices, load_model, save_audio, set_hf_token,
};
use logging::init as logging_init;
use state::AppState;
use std::sync::Arc;

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

fn main() {
    logging_init(false);
    if std::env::var_os("RAYON_NUM_THREADS").is_none() {
        let cpus = std::thread::available_parallelism().map_or(1, |n| n.get());
        xn::set_num_threads(4.min(cpus));
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Arc::new(AppState::default()))
        .invoke_handler(tauri::generate_handler![
            load_model,
            get_voices,
            get_languages,
            generate,
            save_audio,
            export_voice,
            app_info,
            get_hf_status,
            set_hf_token,
            get_terminal_logs,
            clear_terminal_logs,
            set_window_style,
        ])
        .setup(|app| {
            // Register app handle for terminal log broadcast to the frontend
            logging::set_app_handle(app.handle().clone());
            logging::add_log_entry("INFO", "Pocket TTS engine initialized successfully.");

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
        .expect("error while running tauri application");
}
