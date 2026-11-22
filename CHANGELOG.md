# Changelog

All notable changes to Pocket TTS (the Windows wrapper) are documented here.

## [0.2.0] - 2026-11-22

### Added
- "Continue from previous chunk" option (teacher-forced chunk continuation) in the
  Advanced card. Each chunk after the first now continues the previous one — its
  text and generated latents are teacher-forced — instead of starting afresh
  from the voice prompt, keeping prosody smooth across long texts.
- Vendored the `ptts` inference crate (`vendor/ptts`, from xn-ptts rev
  `7efa269`) with two additions: `FlowLM::bos_embedding()` and
  `TTSModel::prompt_text_and_latents()`.
- Mimi decoder warmup for teacher-forced chunks: replays the previous chunk's
  last ~12 latents (audio dropped) so the decoder state continues, and skips the
  per-chunk fade-in.
- `MAX_TOKEN_PER_CHUNK_TEACHER_FORCED = 25` chunk cap when the option is on.

### Changed
- `ptts` is now a path dependency (`vendor/ptts`) instead of a git dependency.
- `generate` command accepts `teacher_force_previous_chunk` /
  `teacherForcePreviousChunk`.

## [0.1.0] - 2026-11-22

### Added
- Initial release of the Pocket TTS Windows wrapper with a glass Tauri UI.
- Voice, language, temperature, noise clamp, EOS threshold, sampler steps,
  max tokens and chunk controls.
- Per-user NSIS installer and portable executable.
- MIT license and CI workflow.
EOF