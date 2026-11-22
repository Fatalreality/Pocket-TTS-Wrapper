//! Model loading helpers shared by the Tauri commands.

use crate::config::Config;
use crate::model::{CpuBackend, LoadOptions, TtsModel};
use anyhow::Result;

/// Loads the TTS model for a language or a custom config, as the CLI's `load_model` does.
pub fn load_model<Q: CpuBackend>(
    language: Option<&str>,
    config: Option<&str>,
    options: &LoadOptions,
) -> Result<TtsModel<Q>> {
    let (config, origin) = Config::load(language, config)?;
    TtsModel::load(config, origin, options)
}

/// Exports a voice from an audio file to a `.safetensors` state file, always on the
/// unquantized backend, as the CLI's `export-voice` does.
pub fn export_voice_with(
    language: Option<&str>,
    config: Option<&str>,
    audio_path: &str,
    export_path: &str,
) -> Result<()> {
    let model: TtsModel<UnquantizedBackend> =
        load_model(language, config, &LoadOptions::default())?;
    let voice = model.get_state_for_audio_prompt(audio_path, true)?;
    voice.export(std::path::Path::new(export_path))?;
    Ok(())
}

/// The backend `export_voice_with` runs on.
pub type UnquantizedBackend = crate::state::F32Backend;
