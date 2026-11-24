//! Tauri commands exposed to the frontend: they wrap the Pocket TTS engine.
//!
//! The model is cached per (language/config, generation options, quantization) in
//! [`crate::state::AppState`]; `generate` reloads it automatically when the settings change,
//! so the explicit `load_model` command is only a preload.

use crate::encode::{AudioSink, Codec, pcm_to_wav};
use crate::model::{CpuBackend, LoadOptions};
use crate::state::{AppState, F32Backend, Q8Backend, ensure_bundle};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde::Serialize;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};

/// Information about the application.
#[derive(Serialize, Clone)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub description: String,
}

/// A selectable language.
#[derive(Serialize, Clone)]
pub struct Language {
    pub code: String,
    pub name: String,
}

/// A selectable voice.
#[derive(Serialize, Clone)]
pub struct Voice {
    pub name: String,
    pub language: String,
    pub gender: String,
    pub accent: String,
}

/// Hugging Face authentication and model cache status.
#[derive(Serialize, Clone)]
pub struct HfStatus {
    pub has_token: bool,
    pub token_preview: Option<String>,
    pub has_voice_cloning_weights: bool,
    pub token_path: String,
}

/// Result of audio generation.
#[derive(Serialize, Clone)]
pub struct GenerateResult {
    pub sample_rate: usize,
    pub duration_ms: f64,
    pub num_samples: usize,
    pub wav_base64: String,
}

/// Whether a newer release is published, for the update button.
#[derive(Serialize, Clone)]
pub struct UpdateInfo {
    pub available: bool,
    pub current_version: String,
    pub version: Option<String>,
    pub notes: Option<String>,
}

/// Result of rendering a text file to disk.
#[derive(Serialize, Clone)]
pub struct RenderResult {
    pub output_path: String,
    pub duration_ms: f64,
    pub num_samples: usize,
    pub format: String,
}

/// Progress of a text-file render, emitted once per rendered second of audio.
#[derive(Serialize, Clone)]
struct RenderProgress {
    seconds: f64,
    samples: usize,
}

/// The languages the shipped configs cover, with the model variants of the CLI.
fn supported_languages() -> Vec<Language> {
    let mut languages = vec![
        ("english", "English"),
        ("french", "French"),
        ("german", "German"),
        ("portuguese", "Portuguese"),
        ("italian", "Italian"),
        ("spanish", "Spanish"),
        ("dutch", "Dutch"),
    ];
    // The 24-layer variants and the English model variants, in the CLI's order.
    let mut extra: Vec<(&str, &str)> = crate::config::CONFIGS
        .iter()
        .filter_map(|(name, _)| {
            let known = [
                "english", "french", "german", "portuguese", "italian", "spanish", "dutch",
            ];
            (!known.contains(name)).then_some(match *name {
                n if n.starts_with("english_drifting") => (n, "English (drifting sampler)"),
                n if n.contains("refl") => (n, "English (reward fine-tuned)"),
                n if n.contains("18m") => (n, "English (18M)"),
                n if n.contains("7m") => (n, "English (7M)"),
                n if n.contains("2m") => (n, "English (2M)"),
                n if n.ends_with("24l") => (n, "24-layer variant"),
                n => (n, "English variant"),
            })
        })
        .collect();
    extra.sort();
    languages.extend(extra);
    languages
        .into_iter()
        .map(|(code, name)| Language {
            code: code.to_string(),
            name: name.to_string(),
        })
        .collect()
}

/// Returns basic app information.
#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        name: "Pocket TTS".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        description: "Kyutai Pocket TTS - Text-to-Speech with Liquid Glass UI".into(),
    }
}

/// Returns the list of supported languages.
#[tauri::command]
pub fn get_languages() -> Vec<Language> {
    supported_languages()
}

/// Returns the list of available voices, optionally filtered by language.
#[tauri::command]
pub fn get_voices(language: Option<String>) -> Vec<Voice> {
    let mut voices: Vec<Voice> = crate::defaults::ORIGINS_OF_PREDEFINED_VOICES
        .iter()
        .map(|(name, origin)| Voice {
            name: name.to_string(),
            language: voice_language(origin).to_string(),
            gender: voice_gender(name).to_string(),
            accent: voice_accent(name).to_string(),
        })
        .collect();
    if let Some(lang) = language {
        voices.retain(|v| v.language == lang);
    }
    voices
}

/// Categorize predefined voice gender.
fn voice_gender(name: &str) -> &'static str {
    match name {
        "alba" | "cosette" | "anna" | "vera" | "fantine" | "eponine" | "azelma" | "mary"
        | "jane" | "eve" | "caro_davy" | "lola" | "estelle" => "female",
        _ => "male",
    }
}

/// Predefined voice accent and character tag.
fn voice_accent(name: &str) -> &'static str {
    match name {
        "alba" => "Scottish (Casual)",
        "cosette" => "Expressive",
        "anna" => "British (Southern)",
        "vera" => "British",
        "fantine" => "British",
        "eponine" => "Scottish (Edinburgh)",
        "azelma" => "British",
        "mary" => "British",
        "jane" => "British",
        "eve" => "American",
        "caro_davy" => "Warm Narrator",
        "lola" => "Spanish (Castilian)",
        "estelle" => "French (Natural)",
        "marius" => "Casual",
        "javert" => "Deep & Bold",
        "jean" => "French",
        "charles" => "British",
        "paul" => "British",
        "george" => "British",
        "michael" => "British",
        "bill_boerst" => "American",
        "peter_yearsley" => "British Narrator",
        "stuart_bell" => "British",
        "giovanni" => "Italian",
        "juergen" => "German",
        "rafael" => "Portuguese",
        "daan" => "Dutch",
        _ => "General",
    }
}

/// Determine the language of a voice from its origin URL.
fn voice_language(origin: &str) -> &'static str {
    if origin.contains("common_voice_it") {
        "italian"
    } else if origin.contains("common_voice_es") {
        "spanish"
    } else if origin.contains("de-DE") {
        "german"
    } else if origin.contains("g-Vi8PgmSY0") {
        "portuguese"
    } else if origin.contains("developpeuse") {
        "french"
    } else if origin.contains("cml_tts_nl") {
        "dutch"
    } else {
        "english"
    }
}

/// The options a model is loaded with, from the generation request.
fn load_options(
    temperature: Option<f32>,
    noise_clamp: Option<f32>,
    eos_threshold: Option<f32>,
    sampler_decode_steps: Option<usize>,
) -> LoadOptions {
    LoadOptions {
        temperature,
        sampler_decode_steps: sampler_decode_steps
            .unwrap_or(crate::defaults::DEFAULT_SAMPLER_DECODE_STEPS),
        noise_clamp,
        eos_threshold,
    }
}

/// The settings a loaded model depends on, as one cache key. `frames_after_eos` and
/// `max_tokens` are per-generation and not part of it.
fn cache_key(
    language: Option<&str>,
    config: Option<&str>,
    options: &LoadOptions,
    quantize: bool,
) -> String {
    let source = language
        .or(config)
        .unwrap_or(crate::defaults::DEFAULT_LANGUAGE);
    format!(
        "{source}|t{:?}|nc{:?}|eos{:?}|steps{}|q{}",
        options.temperature, options.noise_clamp, options.eos_threshold,
        options.sampler_decode_steps, quantize
    )
}

/// Preloads the model for the given settings. Slow on first use: weights are downloaded from
/// Hugging Face, so it runs off the UI thread.
#[tauri::command]
#[allow(non_snake_case)]
pub async fn load_model(
    language: Option<String>,
    config: Option<String>,
    temperature: Option<f32>,
    noise_clamp: Option<f32>,
    noiseClamp: Option<f32>,
    eos_threshold: Option<f32>,
    eosThreshold: Option<f32>,
    sampler_decode_steps: Option<usize>,
    samplerDecodeSteps: Option<usize>,
    quantize: Option<bool>,
    state: State<'_, Arc<AppState>>,
) -> Result<String, String> {
    let noise_clamp = noise_clamp.or(noiseClamp);
    let eos_threshold = eos_threshold.or(eosThreshold);
    let sampler_decode_steps = sampler_decode_steps.or(samplerDecodeSteps);
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let options = load_options(temperature, noise_clamp, eos_threshold, sampler_decode_steps);
        let quantize = quantize.unwrap_or(false);
        let key = cache_key(language.as_deref(), config.as_deref(), &options, quantize);
        if quantize {
            let mut slot = state.q8_slot();
            ensure_bundle::<Q8Backend>(&mut slot, &key, language.as_deref(), config.as_deref(), &options)
                .map_err(|e| e.to_string())?;
        } else {
            let mut slot = state.f32_slot();
            ensure_bundle::<F32Backend>(&mut slot, &key, language.as_deref(), config.as_deref(), &options)
                .map_err(|e| e.to_string())?;
        }
        crate::logging::add_log_entry("INFO", &format!("Model ready ({key})"));
        Ok(format!("Model ready ({key})"))
    })
    .await
    .map_err(|e| format!("task failed: {e}"))?
}

/// Generates speech from text. Runs off the UI thread; the model is loaded or reloaded
/// automatically when the settings changed since the last generation.
#[tauri::command]
#[allow(non_snake_case)]
pub async fn generate(
    text: String,
    language: Option<String>,
    config: Option<String>,
    voice: Option<String>,
    sampler_decode_steps: Option<usize>,
    samplerDecodeSteps: Option<usize>,
    temperature: Option<f32>,
    noise_clamp: Option<f32>,
    noiseClamp: Option<f32>,
    eos_threshold: Option<f32>,
    eosThreshold: Option<f32>,
    frames_after_eos: Option<usize>,
    framesAfterEos: Option<usize>,
    max_tokens: Option<usize>,
    maxTokens: Option<usize>,
    teacher_force_previous_chunk: Option<bool>,
    teacherForcePreviousChunk: Option<bool>,
    quantize: Option<bool>,
    state: State<'_, Arc<AppState>>,
) -> Result<GenerateResult, String> {
    let sampler_decode_steps = sampler_decode_steps.or(samplerDecodeSteps);
    let noise_clamp = noise_clamp.or(noiseClamp);
    let eos_threshold = eos_threshold.or(eosThreshold);
    let frames_after_eos = frames_after_eos.or(framesAfterEos);
    let max_tokens = max_tokens.or(maxTokens);
    let teacher_force_previous_chunk = teacher_force_previous_chunk
        .or(teacherForcePreviousChunk)
        .unwrap_or(false);
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let options = load_options(temperature, noise_clamp, eos_threshold, sampler_decode_steps);
        let quantize = quantize.unwrap_or(false);
        let key = cache_key(language.as_deref(), config.as_deref(), &options, quantize);
        let max_tokens = max_tokens.unwrap_or(crate::defaults::MAX_TOKEN_PER_CHUNK);

        // One generation at a time on the same backend: the lock is held for the whole
        // generation, as the engine is not thread-safe.
        if quantize {
            let mut slot = state.q8_slot();
            let bundle = ensure_bundle::<Q8Backend>(
                &mut slot, &key, language.as_deref(), config.as_deref(), &options,
            )
            .map_err(|e| e.to_string())?;
            generate_with_bundle(
                &bundle.model,
                &bundle.default_voice,
                &text,
                &voice,
                max_tokens,
                frames_after_eos,
                teacher_force_previous_chunk,
            )
        } else {
            let mut slot = state.f32_slot();
            let bundle = ensure_bundle::<F32Backend>(
                &mut slot, &key, language.as_deref(), config.as_deref(), &options,
            )
            .map_err(|e| e.to_string())?;
            generate_with_bundle(
                &bundle.model,
                &bundle.default_voice,
                &text,
                &voice,
                max_tokens,
                frames_after_eos,
                teacher_force_previous_chunk,
            )
        }
    })
    .await
    .map_err(|e| format!("task failed: {e}"))?
}

/// Generates from a loaded model, resolving the voice and collecting the audio as WAV.
fn generate_with_bundle<Q: CpuBackend>(
    model: &Arc<crate::model::TtsModel<Q>>,
    default_voice: &Arc<crate::model::VoiceState<Q>>,
    text: &str,
    voice: &Option<String>,
    max_tokens: usize,
    frames_after_eos: Option<usize>,
    teacher_force_previous_chunk: bool,
) -> Result<GenerateResult, String> {
    let sample_rate = model.sample_rate();
    let voice_state = resolve_voice(model, default_voice, voice)?;
    let mut pcm: Vec<f32> = Vec::new();
    {
        let mut on_chunk = |chunk: &[f32]| -> anyhow::Result<()> {
            pcm.extend_from_slice(chunk);
            Ok(())
        };
        model
            .generate_audio_stream(
                &voice_state,
                text,
                max_tokens,
                frames_after_eos,
                teacher_force_previous_chunk,
                &AtomicBool::new(false),
                &mut on_chunk,
            )
            .map_err(|e| e.to_string())?;
    }
    let wav_bytes = pcm_to_wav(&pcm, sample_rate as u32);
    let wav_base64 = BASE64.encode(wav_bytes);
    Ok(GenerateResult {
        sample_rate,
        duration_ms: (pcm.len() as f64 / sample_rate as f64) * 1000.0,
        num_samples: pcm.len(),
        wav_base64,
    })
}

/// Validates that byte buffer contains standard RIFF/WAVE audio data.
pub fn validate_wav_bytes(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() < 44 {
        return Err("WAV audio data too short (minimum 44 bytes)".into());
    }
    if &bytes[0..4] != b"RIFF" {
        return Err("Invalid WAV file: missing RIFF header".into());
    }
    if &bytes[8..12] != b"WAVE" {
        return Err("Invalid WAV file: missing WAVE format identifier".into());
    }

    let mut offset = 12;
    let mut found_fmt = false;
    let mut found_data = false;

    while offset + 8 <= bytes.len() {
        let chunk_id = &bytes[offset..offset + 4];
        let chunk_size = u32::from_le_bytes(
            bytes[offset + 4..offset + 8]
                .try_into()
                .map_err(|_| "Failed to parse chunk size")?,
        ) as usize;

        offset += 8;

        if chunk_id == b"fmt " {
            if chunk_size < 16 {
                return Err("Invalid WAV fmt chunk: size less than 16 bytes".into());
            }
            if offset + chunk_size > bytes.len() {
                return Err("Invalid WAV file: truncated fmt chunk".into());
            }

            let format_tag = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
            let channels = u16::from_le_bytes([bytes[offset + 2], bytes[offset + 3]]);
            let sample_rate = u32::from_le_bytes([
                bytes[offset + 4],
                bytes[offset + 5],
                bytes[offset + 6],
                bytes[offset + 7],
            ]);

            if format_tag != 1 && format_tag != 3 {
                return Err(format!("Unsupported WAV audio format tag: {format_tag} (must be 1 for PCM or 3 for IEEE Float)"));
            }
            if channels == 0 {
                return Err("Invalid WAV audio: channel count cannot be 0".into());
            }
            if sample_rate == 0 {
                return Err("Invalid WAV audio: sample rate cannot be 0".into());
            }

            found_fmt = true;
        } else if chunk_id == b"data" {
            if offset + chunk_size > bytes.len() {
                return Err("Invalid WAV file: truncated data chunk".into());
            }
            found_data = true;
        }

        offset += chunk_size;
        // RIFF chunks are padded to 2-byte boundary
        if chunk_size % 2 != 0 {
            offset += 1;
        }
    }

    if !found_fmt {
        return Err("Invalid WAV file: missing fmt chunk".into());
    }
    if !found_data {
        return Err("Invalid WAV file: missing data chunk".into());
    }

    Ok(())
}

/// Saves audio data to a file path provided by the frontend, in the format the file name asks
/// for: WAV is written as-is, MP3 and Ogg are encoded from its samples.
/// Accepts both snake_case and camelCase parameters for resilient IPC binding.
#[tauri::command]
#[allow(non_snake_case)]
pub fn save_audio(
    wav_base64: Option<String>,
    wavBase64: Option<String>,
    file_path: Option<String>,
    filePath: Option<String>,
) -> Result<String, String> {
    let b64 = wav_base64
        .or(wavBase64)
        .ok_or_else(|| "Missing audio base64 data".to_string())?;
    let path = file_path
        .or(filePath)
        .ok_or_else(|| "Missing target file path".to_string())?;

    let wav_bytes = BASE64.decode(&b64).map_err(|e| format!("Base64 decode error: {e}"))?;
    validate_wav_bytes(&wav_bytes)?;

    let codec = Codec::for_path(Path::new(&path)).map_err(|e| e.to_string())?;
    let bytes = if codec == Codec::Wav {
        wav_bytes
    } else {
        let (pcm, sample_rate) =
            crate::encode::wav_to_pcm(&wav_bytes).map_err(|e| e.to_string())?;
        crate::encode::encode_in_memory(codec, &pcm, sample_rate).map_err(|e| e.to_string())?
    };

    if let Some(parent) = Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&path, &bytes).map_err(|e| format!("File write error: {e}"))?;
    crate::logging::add_log_entry(
        "INFO",
        &format!(
            "Saved {} audio to {path} ({} bytes)",
            codec.name(),
            bytes.len()
        ),
    );
    Ok(path)
}

/// Exports a voice from an audio file to a .safetensors state file. Slow: loads the model and
/// encodes the audio, so it runs off the UI thread.
/// Accepts both snake_case and camelCase parameters.
#[tauri::command]
#[allow(non_snake_case)]
pub async fn export_voice(
    audio_path: Option<String>,
    audioPath: Option<String>,
    export_path: Option<String>,
    exportPath: Option<String>,
    language: Option<String>,
    config: Option<String>,
) -> Result<String, String> {
    let a_path = audio_path
        .or(audioPath)
        .ok_or_else(|| "Missing audio_path".to_string())?;
    let e_path = export_path
        .or(exportPath)
        .ok_or_else(|| "Missing export_path".to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        crate::cli::export_voice_with(
            language.as_deref(),
            config.as_deref(),
            &a_path,
            &e_path,
        )
        .map_err(|e| e.to_string())?;
        crate::logging::add_log_entry("INFO", &format!("Voice exported to {e_path}"));
        Ok(format!("Voice exported to {e_path}"))
    })
    .await
    .map_err(|e| format!("task failed: {e}"))?
}

/// Checks Hugging Face authentication status and whether voice cloning weights exist locally.
#[tauri::command]
pub fn get_hf_status() -> Result<HfStatus, String> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".into());
    let token_path = std::path::PathBuf::from(&home).join(".cache").join("huggingface").join("token");
    let env_token = std::env::var("HF_TOKEN").ok();

    let (has_token, token_str) = if let Ok(content) = std::fs::read_to_string(&token_path) {
        let t = content.trim().to_string();
        (!t.is_empty(), Some(t))
    } else if let Some(t) = env_token {
        let trimmed = t.trim().to_string();
        (!trimmed.is_empty(), Some(trimmed))
    } else {
        (false, None)
    };

    let token_preview = token_str.map(|t| {
        if t.len() > 8 {
            format!("{}...{}", &t[..4], &t[t.len() - 4..])
        } else {
            "****".into()
        }
    });

    let hub_dir = std::path::PathBuf::from(&home).join(".cache").join("huggingface").join("hub");
    let vc_model_dir = hub_dir.join("models--kyutai--pocket-tts");
    let has_voice_cloning_weights = vc_model_dir.exists();

    Ok(HfStatus {
        has_token,
        token_preview,
        has_voice_cloning_weights,
        token_path: token_path.to_string_lossy().to_string(),
    })
}

/// Sets Hugging Face access token by writing to `~/.cache/huggingface/token` and `HF_TOKEN`.
#[tauri::command]
pub fn set_hf_token(token: String) -> Result<String, String> {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return Err("Hugging Face token cannot be empty".into());
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map_err(|e| format!("Cannot locate home directory: {e}"))?;
    let hf_dir = std::path::PathBuf::from(&home).join(".cache").join("huggingface");
    std::fs::create_dir_all(&hf_dir)
        .map_err(|e| format!("Failed to create directory {}: {e}", hf_dir.display()))?;
    let token_path = hf_dir.join("token");
    std::fs::write(&token_path, trimmed)
        .map_err(|e| format!("Failed to write token file {}: {e}", token_path.display()))?;
    std::env::set_var("HF_TOKEN", trimmed);
    crate::logging::add_log_entry("INFO", "Hugging Face access token updated successfully.");
    Ok("Token saved successfully. Voice cloning weights can now be downloaded.".into())
}

/// Retrieves buffered terminal logs for the in-app collapsible console.
#[tauri::command]
pub fn get_terminal_logs() -> Vec<crate::logging::LogEntry> {
    crate::logging::get_recent_logs()
}

/// Clears the buffered terminal logs.
#[tauri::command]
pub fn clear_terminal_logs() -> Result<(), String> {
    crate::logging::clear_logs();
    Ok(())
}

/// Resolves the voice a generation should use: an explicit source, or the language default.
fn resolve_voice<Q: CpuBackend>(
    model: &Arc<crate::model::TtsModel<Q>>,
    default_voice: &Arc<crate::model::VoiceState<Q>>,
    voice: &Option<String>,
) -> Result<Arc<crate::model::VoiceState<Q>>, String> {
    match voice {
        Some(source) => Ok(Arc::new(
            model
                .get_state_for_audio_prompt(source, false)
                .map_err(|e| e.to_string())?,
        )),
        None => Ok(default_voice.clone()),
    }
}

/// Renders a whole text file to an audio file of its own, streaming samples to disk as the
/// model produces them. Long texts therefore cost no more memory than a short one, and the
/// frontend gets a progress event per rendered second.
#[tauri::command]
#[allow(non_snake_case)]
pub async fn render_text_file(
    app: AppHandle,
    input_path: Option<String>,
    inputPath: Option<String>,
    output_path: Option<String>,
    outputPath: Option<String>,
    voice: Option<String>,
    language: Option<String>,
    config: Option<String>,
    temperature: Option<f32>,
    noise_clamp: Option<f32>,
    noiseClamp: Option<f32>,
    eos_threshold: Option<f32>,
    eosThreshold: Option<f32>,
    sampler_decode_steps: Option<usize>,
    samplerDecodeSteps: Option<usize>,
    frames_after_eos: Option<usize>,
    framesAfterEos: Option<usize>,
    max_tokens: Option<usize>,
    maxTokens: Option<usize>,
    teacher_force_previous_chunk: Option<bool>,
    teacherForcePreviousChunk: Option<bool>,
    quantize: Option<bool>,
    state: State<'_, Arc<AppState>>,
) -> Result<RenderResult, String> {
    let input = input_path
        .or(inputPath)
        .ok_or_else(|| "Missing input path".to_string())?;
    let output = output_path
        .or(outputPath)
        .ok_or_else(|| "Missing output path".to_string())?;
    let noise_clamp = noise_clamp.or(noiseClamp);
    let eos_threshold = eos_threshold.or(eosThreshold);
    let sampler_decode_steps = sampler_decode_steps.or(samplerDecodeSteps);
    let frames_after_eos = frames_after_eos.or(framesAfterEos);
    let max_tokens = max_tokens.or(maxTokens);
    let teacher_force_previous_chunk = teacher_force_previous_chunk
        .or(teacherForcePreviousChunk)
        .unwrap_or(false);
    let state = state.inner().clone();

    tauri::async_runtime::spawn_blocking(move || {
        let text = std::fs::read_to_string(&input)
            .map_err(|e| format!("cannot read {input}: {e}"))?;
        let text = text.trim_start_matches('\u{feff}');
        if text.trim().is_empty() {
            return Err(format!("{input} has no text to speak"));
        }
        let codec = Codec::for_path(Path::new(&output)).map_err(|e| e.to_string())?;
        let options = load_options(temperature, noise_clamp, eos_threshold, sampler_decode_steps);
        let quantize = quantize.unwrap_or(false);
        let key = cache_key(language.as_deref(), config.as_deref(), &options, quantize);
        let max_tokens = max_tokens.unwrap_or(crate::defaults::MAX_TOKEN_PER_CHUNK);

        let (samples, sample_rate) = if quantize {
            let mut slot = state.q8_slot();
            let bundle = ensure_bundle::<Q8Backend>(
                &mut slot, &key, language.as_deref(), config.as_deref(), &options,
            )
            .map_err(|e| e.to_string())?;
            render_to_file::<Q8Backend>(
                &app,
                &bundle.model,
                &bundle.default_voice,
                &voice,
                text,
                codec,
                Path::new(&output),
                max_tokens,
                frames_after_eos,
                teacher_force_previous_chunk,
            )?
        } else {
            let mut slot = state.f32_slot();
            let bundle = ensure_bundle::<F32Backend>(
                &mut slot, &key, language.as_deref(), config.as_deref(), &options,
            )
            .map_err(|e| e.to_string())?;
            render_to_file::<F32Backend>(
                &app,
                &bundle.model,
                &bundle.default_voice,
                &voice,
                text,
                codec,
                Path::new(&output),
                max_tokens,
                frames_after_eos,
                teacher_force_previous_chunk,
            )?
        };

        let duration_ms = (samples as f64 / sample_rate as f64) * 1000.0;
        crate::logging::add_log_entry(
            "INFO",
            &format!(
                "Rendered {input} to {output} ({:.1}s, {})",
                duration_ms / 1000.0,
                codec.name()
            ),
        );
        Ok(RenderResult {
            output_path: output,
            duration_ms,
            num_samples: samples,
            format: codec.name().to_string(),
        })
    })
    .await
    .map_err(|e| format!("task failed: {e}"))?
}

/// Streams one generation into `output`, writing from a dedicated thread so the encoder never
/// has to be handed across threads. Returns the samples written and the model's sample rate.
#[allow(clippy::too_many_arguments)]
fn render_to_file<Q: CpuBackend>(
    app: &AppHandle,
    model: &Arc<crate::model::TtsModel<Q>>,
    default_voice: &Arc<crate::model::VoiceState<Q>>,
    voice: &Option<String>,
    text: &str,
    codec: Codec,
    output: &Path,
    max_tokens: usize,
    frames_after_eos: Option<usize>,
    teacher_force_previous_chunk: bool,
) -> Result<(usize, usize), String> {
    let sample_rate = model.sample_rate();
    let voice_state = resolve_voice(model, default_voice, voice)?;
    let path = output.to_path_buf();
    let rate = sample_rate as u32;
    let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<f32>>(8);

    let writer = std::thread::spawn(move || -> anyhow::Result<usize> {
        let mut sink = AudioSink::create(codec, &path, rate)?;
        let mut total = 0usize;
        for block in rx {
            sink.write(&block)?;
            total += block.len();
        }
        sink.finish()?;
        Ok(total)
    });

    let generation = (|| -> Result<(), String> {
        let mut sent = 0usize;
        let mut next_report = sample_rate;
        let mut on_chunk = move |chunk: &[f32]| -> anyhow::Result<()> {
            sent += chunk.len();
            if sent >= next_report {
                next_report += sample_rate;
                let seconds = sent as f64 / sample_rate as f64;
                let _ = app.emit(
                    "render_progress",
                    RenderProgress {
                        seconds,
                        samples: sent,
                    },
                );
                crate::logging::add_log_entry(
                    "INFO",
                    &format!("Rendered {seconds:.0}s of audio to {}", output.display()),
                );
            }
            tx.send(chunk.to_vec())
                .map_err(|_| anyhow::anyhow!("the audio writer stopped"))?;
            Ok(())
        };
        model
            .generate_audio_stream(
                &voice_state,
                text,
                max_tokens,
                frames_after_eos,
                teacher_force_previous_chunk,
                &AtomicBool::new(false),
                &mut on_chunk,
            )
            .map_err(|e| e.to_string())
    })();

    // The sender is dropped with the closure above, which is what ends the writer's loop.
    let written = writer
        .join()
        .map_err(|_| "the audio writer thread panicked".to_string())?
        .map_err(|e| e.to_string())?;
    generation?;
    Ok((written, sample_rate))
}

/// Writes the in-app console transcript to a file, with a header describing the build and the
/// machine, so a bug report carries something diagnosable.
#[tauri::command]
#[allow(non_snake_case)]
pub fn save_logs(
    content: String,
    file_path: Option<String>,
    filePath: Option<String>,
) -> Result<String, String> {
    let path = file_path
        .or(filePath)
        .ok_or_else(|| "Missing target file path".to_string())?;
    let header = format!(
        "Pocket TTS {} diagnostic log\nGenerated: {}\nPlatform: {} {}\nCPU threads: {}\n\n",
        env!("CARGO_PKG_VERSION"),
        utc_stamp(),
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::thread::available_parallelism().map_or(1, |n| n.get()),
    );
    if let Some(parent) = Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&path, format!("{header}{content}"))
        .map_err(|e| format!("File write error: {e}"))?;
    crate::logging::add_log_entry("INFO", &format!("Saved logs to {path}"));
    Ok(path)
}

/// Asks the release endpoint whether a newer version exists.
#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<UpdateInfo, String> {
    use tauri_plugin_updater::UpdaterExt;
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let updater = app.updater().map_err(|e| e.to_string())?;
    let found = updater.check().await.map_err(|e| e.to_string())?;
    Ok(match found {
        Some(update) => {
            crate::logging::add_log_entry(
                "INFO",
                &format!("Update available: {} (running {current_version})", update.version),
            );
            UpdateInfo {
                available: true,
                current_version,
                version: Some(update.version.clone()),
                notes: update.body.clone(),
            }
        }
        None => UpdateInfo {
            available: false,
            current_version,
            version: None,
            notes: None,
        },
    })
}

/// Downloads, verifies and installs the announced update, then restarts into it.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_updater::UpdaterExt;
    let updater = app.updater().map_err(|e| e.to_string())?;
    let Some(update) = updater.check().await.map_err(|e| e.to_string())? else {
        return Err("No update is available".into());
    };
    crate::logging::add_log_entry("INFO", &format!("Downloading update {}", update.version));
    let mut downloaded: u64 = 0;
    let mut last_logged: u64 = 0;
    update
        .download_and_install(
            |chunk, total| {
                downloaded += chunk as u64;
                if downloaded - last_logged >= 4 * 1024 * 1024 {
                    last_logged = downloaded;
                    match total {
                        Some(total) => log::info!(
                            "update download: {:.1} of {:.1} MiB",
                            downloaded as f64 / 1_048_576.0,
                            total as f64 / 1_048_576.0
                        ),
                        None => log::info!(
                            "update download: {:.1} MiB",
                            downloaded as f64 / 1_048_576.0
                        ),
                    }
                }
            },
            || log::info!("update downloaded; installing"),
        )
        .await
        .map_err(|e| e.to_string())?;
    crate::logging::add_log_entry(
        "INFO",
        &format!("Update {} installed; restarting", update.version),
    );
    app.restart();
}

/// The current UTC time as `YYYY-MM-DD HH:MM:SS UTC`, for log headers.
fn utc_stamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Days since the Unix epoch to a civil date, after Howard Hinnant's algorithm.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC",
        rem / 3_600,
        (rem / 60) % 60,
        rem % 60
    )
}

/// Serializes generations across commands, as the Python server does with its generation lock.
#[allow(dead_code)]
type GenerationLock = Mutex<()>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_wav_bytes_valid_pcm() {
        let pcm = vec![0.0f32, 0.25, -0.25, 0.5, -0.5, 0.0];
        let wav = pcm_to_wav(&pcm, 24000);
        assert!(validate_wav_bytes(&wav).is_ok());
    }

    #[test]
    fn test_validate_wav_bytes_too_short() {
        let short = vec![0u8; 43];
        let err = validate_wav_bytes(&short).unwrap_err();
        assert!(err.contains("too short"));
    }

    #[test]
    fn test_validate_wav_bytes_empty() {
        let err = validate_wav_bytes(&[]).unwrap_err();
        assert!(err.contains("too short"));
    }

    #[test]
    fn test_validate_wav_bytes_missing_riff() {
        let mut wav = pcm_to_wav(&[0.1f32], 24000);
        wav[0..4].copy_from_slice(b"NOPE");
        let err = validate_wav_bytes(&wav).unwrap_err();
        assert!(err.contains("missing RIFF header"));
    }

    #[test]
    fn test_validate_wav_bytes_missing_wave() {
        let mut wav = pcm_to_wav(&[0.1f32], 24000);
        wav[8..12].copy_from_slice(b"NOPE");
        let err = validate_wav_bytes(&wav).unwrap_err();
        assert!(err.contains("missing WAVE format identifier"));
    }

    #[test]
    fn test_validate_wav_bytes_webm_magic() {
        // Matroska / WebM EBML header magic: 1A 45 DF A3
        let webm = vec![0x1a, 0x45, 0xdf, 0xa3, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let err = validate_wav_bytes(&webm).unwrap_err();
        assert!(err.contains("missing RIFF header"));
    }

    #[test]
    fn test_validate_wav_bytes_truncated_data() {
        let pcm = vec![0.0f32; 100];
        let mut wav = pcm_to_wav(&pcm, 24000);
        // Truncate some samples from the end
        wav.truncate(wav.len() - 20);
        let err = validate_wav_bytes(&wav).unwrap_err();
        assert!(err.contains("truncated data chunk"));
    }

    #[test]
    fn test_validate_wav_bytes_zero_sample_rate() {
        let pcm = vec![0.0f32; 10];
        let mut wav = pcm_to_wav(&pcm, 24000);
        // Offset 24 is sample_rate
        wav[24..28].copy_from_slice(&0u32.to_le_bytes());
        let err = validate_wav_bytes(&wav).unwrap_err();
        assert!(err.contains("sample rate cannot be 0"));
    }

    #[test]
    fn test_validate_wav_bytes_zero_channels() {
        let pcm = vec![0.0f32; 10];
        let mut wav = pcm_to_wav(&pcm, 24000);
        // Offset 22 is channels
        wav[22..24].copy_from_slice(&0u16.to_le_bytes());
        let err = validate_wav_bytes(&wav).unwrap_err();
        assert!(err.contains("channel count cannot be 0"));
    }

    #[test]
    fn test_save_audio_rejects_invalid_wav() {
        let temp_dir = tempfile::tempdir().unwrap();
        let target_file = temp_dir.path().join("test.wav").to_str().unwrap().to_string();
        let bad_base64 = BASE64.encode(b"not a wav file at all");
        let result = save_audio(Some(bad_base64), None, Some(target_file), None);
        assert!(result.is_err());
    }

    #[test]
    fn test_save_audio_encodes_mp3_by_extension() {
        let temp_dir = tempfile::tempdir().unwrap();
        let target_file = temp_dir.path().join("clip.mp3").to_str().unwrap().to_string();
        let pcm: Vec<f32> = (0..4800).map(|i| (i as f32 * 0.05).sin() * 0.3).collect();
        let base64 = BASE64.encode(pcm_to_wav(&pcm, 24000));
        let result = save_audio(Some(base64), None, Some(target_file.clone()), None);
        assert!(result.is_ok(), "save_audio failed: {result:?}");
        let bytes = std::fs::read(&target_file).unwrap();
        assert_eq!(bytes[0], 0xFF, "saved file is not an MPEG stream");
    }

    #[test]
    fn test_save_audio_rejects_unknown_extension() {
        let temp_dir = tempfile::tempdir().unwrap();
        let target_file = temp_dir.path().join("clip.flac").to_str().unwrap().to_string();
        let base64 = BASE64.encode(pcm_to_wav(&[0.0f32; 100], 24000));
        let err = save_audio(Some(base64), None, Some(target_file), None).unwrap_err();
        assert!(err.contains("unsupported audio format"), "unexpected error: {err}");
    }

    #[test]
    fn test_save_audio_accepts_valid_wav() {
        let temp_dir = tempfile::tempdir().unwrap();
        let target_file = temp_dir.path().join("valid.wav").to_str().unwrap().to_string();
        let wav = pcm_to_wav(&[0.1f32, -0.1f32], 24000);
        let good_base64 = BASE64.encode(&wav);
        let result = save_audio(Some(good_base64), None, Some(target_file.clone()), None);
        assert!(result.is_ok());
        let read_back = std::fs::read(&target_file).unwrap();
        assert_eq!(read_back, wav);
    }
}

