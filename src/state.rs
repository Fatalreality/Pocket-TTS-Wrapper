//! Shared application state managed by Tauri: one cached model per backend (f32 and int8
//! quantized), keyed by the settings the model was loaded with. Generations on the same
//! backend are serialized behind the mutex, as the engine is not thread-safe.

use std::sync::{Arc, Mutex};

use crate::model::{CpuBackend, LoadOptions, TtsModel, VoiceState};
use xn::Unquantized;

pub type F32Backend = Unquantized<f32, xn::CpuDevice>;
pub type Q8Backend = xn::quantized::Q80F32;

/// A loaded model with its default voice, and the settings key it was loaded for.
pub struct Bundle<Q: CpuBackend> {
    pub key: String,
    pub model: Arc<TtsModel<Q>>,
    pub default_voice: Arc<VoiceState<Q>>,
}

/// Application state. Tauri manages one instance for the lifetime of the app.
#[derive(Default)]
pub struct AppState {
    f32: Mutex<Option<Bundle<F32Backend>>>,
    q8: Mutex<Option<Bundle<Q8Backend>>>,
}

impl AppState {
    pub fn f32_slot(&self) -> std::sync::MutexGuard<'_, Option<Bundle<F32Backend>>> {
        self.f32.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn q8_slot(&self) -> std::sync::MutexGuard<'_, Option<Bundle<Q8Backend>>> {
        self.q8.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Loads a model and its default voice. Blocks: run off the UI thread.
pub fn load_bundle<Q: CpuBackend>(
    key: &str,
    language: Option<&str>,
    config: Option<&str>,
    options: &LoadOptions,
) -> anyhow::Result<Bundle<Q>> {
    let model = crate::cli::load_model::<Q>(language, config, options)?;
    let voice_name =
        crate::defaults::get_default_voice_for_language(language, config.is_some());
    let voice = model.get_state_for_audio_prompt(&voice_name, false)?;
    Ok(Bundle {
        key: key.to_string(),
        model: Arc::new(model),
        default_voice: Arc::new(voice),
    })
}

/// Returns the cached bundle for `key`, loading it into the slot when missing or stale.
pub fn ensure_bundle<'a, Q: CpuBackend>(
    slot: &'a mut Option<Bundle<Q>>,
    key: &str,
    language: Option<&str>,
    config: Option<&str>,
    options: &LoadOptions,
) -> anyhow::Result<&'a Bundle<Q>> {
    if slot.as_ref().map_or(true, |b| b.key != key) {
        *slot = Some(load_bundle::<Q>(key, language, config, options)?);
    }
    Ok(slot.as_ref().expect("bundle was just loaded"))
}
