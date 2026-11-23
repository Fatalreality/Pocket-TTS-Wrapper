# Changelog

All notable changes to Pocket TTS (the Windows wrapper) are documented here.

## [0.2.1] - 2026-11-23

### Changed
- Refreshed `Cargo.lock` to current patch releases. Notable bumps: `tauri`
  2.12.6 → 2.12.7, `tauri-runtime-wry` 2.12.4 → 2.12.5, `wry` 0.57.2 → 0.57.3,
  `tao` 0.37.2 → 0.37.3, `tauri-runtime` 2.12.3 → 2.12.4, `tauri-build` and its
  codegen/macros 2.7.5 → 2.7.6, `xn` 0.2.14 → 0.2.15, plus transitive patch
  bumps (`serde_with` 3.24.0 → 3.25.0, `regex` 1.13.1 → 1.13.2,
  `pin-project` 1.1.13 → 1.2.0, `url` 2.5.8 → 2.5.9, `walkdir` 2.5.0 → 2.5.1
  and others). No API or behaviour changes.
- Rebuilt the installer and portable executable so the shipped binaries match
  the refreshed lockfile.

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