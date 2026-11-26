# Changelog

All notable changes to Pocket TTS (the Windows wrapper) are documented here.

## [Unreleased]

## [1.2.0] - 2026-11-26

The hardening release: the token, the file handling and the release pipeline are locked down, and
selecting text anywhere now offers to speak it.

### Added
- **Selection popup**: selecting text in another app shows a small window with the selection and
  three actions — generate and save it, play it live, or open the words in the app. The `Show
  selection popup` checkbox in Settings turns it off.
- **Minimize to tray on close**: closing the main window keeps the app, and the `Ctrl+Alt+S`
  clipboard shortcut, alive instead of quitting; the Settings checkbox turns it off.

### Changed
- Voice previews play bundled recordings (`previews/*.ogg`, shipped in the installer), so
  auditioning a voice no longer waits on the engine.
- The content security policy no longer allows remote stylesheets, fonts or `ws:`/`wss:`
  connections; the UI loads only what ships with the app.

### Security
- The Hugging Face token is written to a temporary file, restricted to a single access-control
  entry, and only then moved into place, so other accounts cannot read it even briefly.
- Logs and error messages redact the token itself and any `hf_`-shaped value.
- File commands reject oversized selections and payloads, refuse downloads above 4 GiB, and
  validate output paths before writing.
- Settings are stored as plain text on purpose: the key the scanner wanted sat beside the
  ciphertext, and the settings hold nothing secret.
- Release builds are compiled unsigned, and the updater package is signed in a step that shares no
  environment with the compiler, the package manager or any downloaded code.

### Fixed
- The advanced sliders survive a restart again, and a resumed render sends the settings it was
  started with (temperature, EOS threshold, steps, quantize), so an interrupted render continues
  instead of being refused as a different job.

### Tooling
- Every workflow action runs on its Node.js 24 release, clearing the Node.js 20 deprecation
  warnings from the runs.

## [1.1.0] - 2026-11-24

The workflow release: generation and long renders can now be stopped and picked up again, voices
can be auditioned before use, and the app keeps its settings between launches.

### Added
- **Stop** for generation and long-form rendering: the engine ends at the next chunk and keeps
  the audio finished so far, instead of leaving a long render to run to completion.
- **Resume unfinished render**: an interrupted text-file render checkpoints every finished
  sentence (`<output>.resume.json` plus the raw samples in `<output>.resume.pcm`) and continues
  from there, appending to the same file, even after an app restart.
- **Voice previews**: picking a built-in voice plays the short sample "Hello, this is how I
  sound.", cached per voice for the session, with a checkbox to turn it off.
- **Recent clips**: every generation this session is listed with its voice and length; clicking
  one reloads it into the player so it can be exported again without re-synthesizing.
- **Speak the clipboard from anywhere**: a tray icon and the global shortcut `Ctrl+Alt+S` read the
  clipboard and send it to the app to be spoken.
- The advanced sliders, language, voice, gender filter, quantize switch and preview toggle are
  remembered across launches.

### Changed
- A missing WebView2 runtime now shows a dialog explaining the fix and offering the download page,
  instead of the app exiting silently.
- The bundled user guide (`README.txt`) documents export formats, long-form rendering and resume,
  recent clips, voice previews, the tray shortcut and saved settings.

### Tooling
- A scheduled `Upstream check` workflow compares the vendored `configs/` and the `vendor/ptts`
  revision with upstream and opens or updates an issue when they move, so wrapper fixes no longer
  depend on someone remembering to look.

### Fixed
- The Stop button now shares the Generate row instead of taking its full width, which had pushed
  it over the card edge and squeezed "Generate speech" onto two lines.
- Durations under ten seconds are shown to a tenth of a second, so a generation stopped part-way
  through its first chunk reads "0.8s" instead of the misleading "0s".

## [1.0.1] - 2026-11-24

### Fixed
- **Capitalize the first word after a leading quote or bracket**: text like
  `"hello there," she said` now becomes `"Hello there," she said` instead of keeping the
  lowercase first letter, matching upstream Pocket TTS. Sentences split from such a prompt
  inherit the fix, so `(quietly.) Okay then.` becomes `(Quietly.) Okay then.`

## [1.0.0] - 2026-11-24

The first non-0.x release: everything below lands together with self-updating, compressed
export and long-form rendering, so the desktop app is feature-complete for everyday use.

### Added
- **In-app updates**: the header's download button checks the published release and installs a
  signed update, restarting into it (`tauri-plugin-updater`).
- **MP3 and Ogg Vorbis export** beside WAV, chosen by the file name in the save dialog; LAME and
  libvorbis do the encoding, so a saved clip is a fraction of the WAV's size.
- **Long-form rendering**: "Render a text file…" turns a whole `.txt` into one WAV, MP3 or Ogg
  file, streaming to disk with a progress readout per rendered second.
- **Save log** in the console drawer, writing the transcript with a header naming the build,
  platform and CPU thread count for bug reports.
- `SHA256SUMS.txt` and the updater's `latest.json` on every release.

### Changed
- Releases are built and published by `.github/workflows/release.yml` on a `v*` tag, instead of
  by hand; `scripts/release.mjs` owns the version bump, the notes and the asset names, and
  [`RELEASING.md`](RELEASING.md) documents the process.
- `save_audio` picks its codec from the target extension and rejects unknown ones instead of
  writing WAV bytes under any name.

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